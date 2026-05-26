# Infrastructure Module Usage Analysis

> 分析 `src/core/` 下的基础设施模块应如何正确使用，以替代各 plugin 中的重复实现。
> 同时识别哪些部分完全多余、不应参考。

## 1. 现状：各 Plugin 中的"旧实现"重复模式

当前 **10 个 plugin** 存在高度重复的代码模式：

### 模式 A：`create_command_builder()`（每个 plugin 一份）

```rust
// cargo/analyzer.rs
fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
    let command_str = options.subcommand.as_ref().map(|s| s.as_str()).unwrap_or("check");
    let mut builder = CommandBuilder::new("cargo");
    for arg in command_str.split_whitespace() {
        builder = builder.arg(arg);
    }
    // 只有 Cargo 和 C++ 有额外 options→args 的非标映射
    if options.workspace { builder = builder.arg("--workspace"); }
    // ...
    builder
}
```

除程序名和默认参数外，**所有 plugin 的该函数逻辑完全相同**。

### 模式 B：`filter_issues()`（10 个 plugin，每个 30+ 行，完全一模一样的代码）

```rust
fn filter_issues(&self, result: AnalysisResult, options: &AnalyzeOptions) -> AnalysisResult {
    if !options.filter_warnings && options.filter_paths.is_empty() {
        return result;
    }
    let mut filtered = AnalysisResult::new();
    for (file_path, issues) in result.issues_by_file {
        if !options.filter_paths.is_empty() {
            let matches = options.filter_paths.iter().any(|filter| file_path.contains(filter));
            if !matches { continue; }
        }
        for issue in issues {
            if options.filter_warnings && matches!(issue.level, IssueLevel::Warning) { continue; }
            filtered.add_issue(issue);
        }
    }
    filtered
}
```

全部 10 份完全一致，无任何定制逻辑。

### 模式 C：`analyze()` 方法结构（每个 plugin）

```rust
fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
    let builder = self.create_command_builder(options);  // 不同
    let output = builder.execute()?;                     // 相同
    let issues = self.parser.parse(&output).data_or_default_owned();  // 相同（仅 parser 不同）
    let result = AnalysisResult::from_issues(issues);     // 相同
    Ok(self.filter_issues(result, options))               // 相同
}
```

---

## 2. 基础设施模块对比分析

### 2.1 `command.rs` — ✅ 已在正确使用

| 组件                                 | 现状                    | 评价                                   |
| ------------------------------------ | ----------------------- | -------------------------------------- |
| `CommandBuilder`                     | 所有 plugin 已使用      | 核心组件，保留                         |
| `RunOptions`                         | 已在 `core/mod.rs` 导出 | 尚未被 plugin 用于超时/环境变量配置    |
| `resolve_command()`                  | 已在内部使用            | 正确                                   |
| `CommandOutput`                      | 已导出但未被使用        | 备用的完整输出结构                     |
| `CommandBuilder::from_exec_string()` | 完全未被使用            | 可简化 `create_command_builder`，见 §3 |

**建议**：`RunOptions` 应当通过 CLI 参数注入；`from_exec_string` 可替代 `split_whitespace` 手动循环。

### 2.2 `parser.rs` — ✅ 已在正确使用

| 组件                 | 现状                       | 评价                 |
| -------------------- | -------------------------- | -------------------- |
| `OutputParser` trait | 所有 parser 已实现         | 核心组件，保留       |
| `ParseResult<T>`     | 已用于 `parse()` 返回值    | 保留，退化策略有价值 |
| `BaseParser`         | 被多数 parser 用作内部组件 | 保留                 |

**建议**：维持现状。

### 2.3 `stream.rs` — 🔄 可替代模式 B（`filter_issues`）和模式 C

```rust
// stream.rs 提供了 ProcessingPipeline，可替代每个 plugin 的 analyze() 模板
pub fn run<P: OutputParser>(
    &mut self,
    parser: P,
    output: &str,
    filter_level: Option<IssueLevel>,
) -> StageResult<AnalysisResult>
```

#### 可替代的部分

| 当前代码                                        | stream 替代方案                                         |
| ----------------------------------------------- | ------------------------------------------------------- |
| `parser.parse(&output).data_or_default_owned()` | `ParseStage` 封装                                       |
| `filter_issues(result, options)`                | `IncludePathsFilter` + `LevelFilter` 两个 PipelineStage |
| `AnalysisResult::from_issues(issues)`           | `AnalyzeStage`                                          |
| 手动组装上述三步                                | `ProcessingPipeline::run()`                             |

#### 不应直接替代的部分

- `cargo/analyzer.rs` 中的特殊验证逻辑：
  ```rust
  if output.contains("error: could not compile") && issues.is_empty() {
      return Err(AnalyzerError::ParseError(...));
  }
  ```
  这种 plugin-specific 的后处理需要保留。

#### 改造后效果示例

```rust
// 当前 cargo/analyzer.rs
fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
    let builder = self.create_command_builder(options);
    let output = builder.execute()?;
    let issues = self.parser.parse(&output).data_or_default_owned();
    if output.contains("error: could not compile") && issues.is_empty() {
        return Err(AnalyzerError::ParseError(...));
    }
    let result = AnalysisResult::from_issues(issues);
    Ok(self.filter_issues(result, options))
}

// 改造后
fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
    let builder = self.create_command_builder(options);
    let output = builder.execute()?;
    let mut pipeline = ProcessingPipeline::new();
    let filter_level = if options.filter_warnings { Some(IssueLevel::Error) } else { None };
    match pipeline.run(&self.parser, &output, filter_level) {
        StageResult::Complete(result) | StageResult::Degraded(result, _) => {
            if output.contains("error: could not compile") && result.total_issues == 0 {
                return Err(AnalyzerError::ParseError(...));
            }
            Ok(result)
        }
        StageResult::Failed(warnings) => Err(AnalyzerError::ParseError(warnings.join("; "))),
    }
}
```

**结论**：`ProcessingPipeline` 值得引入来消除 10 份 `filter_issues` 副本，但需要保留 plugin-specific 后处理的钩子。

### 2.4 `utils.rs` — 🔄 可替代 Reporter 中的零散处理

| 功能                                | 使用场景                    | 替代目标                     |
| ----------------------------------- | --------------------------- | ---------------------------- |
| `strip_ansi()`                      | Reporter 中多个地方手动实现 | reporter 中统一使用          |
| `OutputPostProcessor`               | 无（完全未使用）            | 用于 CLI 的 output 预处理链  |
| `filter_noise_lines()`              | 无（完全未使用）            | 取代 reporter 中的手动行过滤 |
| `compact_path()`                    | markdown reporter 已使用    | 已正确，继续使用             |
| `truncate_output()`                 | reporter 已使用             | 已正确，继续使用             |
| `truncate()` / `summarize_output()` | reporter 已使用             | 已正确，继续使用             |

**建议**：`OutputPostProcessor` 可作为 CLI 的 `--quiet`/`--filter-paths` 等选项的统一后处理入口。

### 2.5 `config.rs` — ⏳ 未来功能，当前无需引用

| 组件            | 现状       | 评价                             |
| --------------- | ---------- | -------------------------------- |
| `Config` 加载   | 未接入 CLI | 需要 CLI 的 `--config` 参数      |
| `ReportConfig`  | 未使用     | 可替代 reporter 中的硬编码格式   |
| `CommandConfig` | 未使用     | 可替代 plugin 中的默认命令字符串 |
| `FilterConfig`  | 未使用     | 可提供更丰富的过滤配置           |

**建议**：保持 `#[allow(dead_code)]` 状态不动。这是一个完整的特性，但需要 CLI 参数解析（如 `--config analyzer.toml`）来驱动。**当前不要移除，也不要引入到 plugin 中**。

### 2.6 `tracking.rs` — ⏳ 未来功能，当前无需引用

| 组件             | 现状       | 评价                                |
| ---------------- | ---------- | ----------------------------------- |
| `History`        | 未接入 CLI | 完整的功能模块                      |
| `TimedExecution` | 未使用     | 可在 CLI 的 `run_analysis()` 中插入 |

**建议**：保持 `#[allow(dead_code)]`。这是一个独立的功能层，不替代任何现有代码。

### 2.7 `tee.rs` — ⏳ 未来功能，当前无需引用

| 组件            | 现状   | 评价       |
| --------------- | ------ | ---------- |
| `save_output()` | 未使用 | 调试用功能 |
| `load_output()` | 未使用 | 回放用功能 |

**建议**：保持 `#[allow(dead_code)]`。独立功能，不替代现有代码。

---

## 3. 完全多余、不应参考的部分

以下代码**完全不应被引用**，因为它们要么被更好的版本替代，要么是纯元余：

### 3.1 `utils::tool_exists()` — ❌ 被 `command::resolve_command()` 替代

```rust
// utils.rs — 不应使用
pub fn tool_exists(tool: &str) -> bool {
    std::process::Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(tool).output().ok()
        .map_or(false, |o| o.status.success())
}
```

`command::resolve_command()` 提供了更完整的实现（返回路径、Windows 扩展名优先级）且已在 `CommandBuilder::build()` 中被使用。`tool_exists()` 返回 `bool` 信息量不足，不应在任何新代码中使用。

### 3.2 `utils::count_lines()` — ❌ 纯元余

```rust
pub fn count_lines(s: &str) -> usize {
    if s.is_empty() { 0 } else { s.lines().count() }
}
```

等同于 `s.lines().count()`，一个直接的函数调用即可完成。增加抽象层无意义。

### 3.3 `utils::estimate_tokens()` — ❌ 过度耦合

```rust
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 4.0).ceil() as usize
}
```

这是一个 LLM token 估算启发式函数，与工具链错误分析无关。它假设了 4 字符/token 的比例，对中英文混合输出误差极大。不应在任何分析流程中使用。

### 3.4 `stream::PipelineStage` trait + 各 Stage struct — ❌ 过度工程化

当前项目中，每个 plugin 的 `analyze()` 方法只有三步：

```
parse → (filter) → collect into AnalysisResult
```

将其抽象为 `PipelineStage<Input, Output>` + `ParseStage` + `IncludePathsFilter` + `LevelFilter` + `AnalyzeStage` + `ProcessingPipeline` 带来了：

- 6 个额外的 `struct` 定义
- 4 个 trait 实现
- 3 个 `StageResult` 变体

**而实际运行时只有一个路径：** `Full → Complete`。`Degraded`/`Passthrough`/`Failed` 变体从未被触发——所有 parser 都返回 `ParseResult::Full`，两个 filter stage 不会失败。

**结论**：`ProcessingPipeline::run()` 作为消除 `filter_issues` 副本的实用函数可以保留，但 `PipelineStage` trait 和各个独立的 stage struct 是过度设计，新代码不应继续以此模式扩展。

### 3.5 `config.rs` / `tracking.rs` / `tee.rs` 中的未用内容 — ❌ 非冗余，但不当前引用

这三个模块被标记为 `#[allow(dead_code)]` 是**正确**的。它们不是冗余——它们提供了完整的功能，只是尚未接入 CLI。**不引用**的意思是：在接入 CLI 之前，不要在 plugin 或 reporter 中直接调用它们。

---

## 4. 总结：改造路线图

| 优先级 | 改造内容                                          | 影响文件                         | 收益                            |
| ------ | ------------------------------------------------- | -------------------------------- | ------------------------------- |
| **P0** | 消除 10 份 `filter_issues()` 副本                 | 10 个 `*/analyzer.rs`            | 删除 ~300 行重复代码            |
| **P0** | 引入 `ProcessingPipeline` 统一 `analyze()` 模板   | 10 个 `*/analyzer.rs`            | 减少 50% 的 `analyze()` 体量    |
| **P1** | Reporter 统一使用 `OutputPostProcessor`           | `reporter/*.rs`                  | 消除散落的 ANSI/truncation 逻辑 |
| **P1** | `CommandBuilder::from_exec_string()` 替代手动循环 | 10 个 `create_command_builder()` | 减少样板代码                    |
| **P2** | CLI 接入 `config.rs`                              | `main.rs`                        | 外部配置能力                    |
| **P2** | CLI 接入 `tracking.rs`                            | `main.rs`                        | 执行历史记录                    |
| **P3** | CLI 接入 `tee.rs`                                 | `main.rs`                        | 调试输出保存                    |

### 不动的内容

- `parser.rs` 的 `OutputParser` / `ParseResult` / `BaseParser` — 已在正确位置
- `command.rs` 的 `CommandBuilder` — 已在正确位置
- `config.rs` / `tracking.rs` / `tee.rs` 的 `#[allow(dead_code)]` — 正确，等待接入
