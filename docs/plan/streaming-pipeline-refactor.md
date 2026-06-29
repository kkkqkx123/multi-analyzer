# Streaming 与输出净化管线重构方案

## 1. 现状分析

### 1.1 当前代码分布

| 文件 | 行数 | 内容 | 实际状态 |
|------|------|------|---------|
| `core/stream.rs` | 699 | PipelineStage/ProcessingPipeline/LineFilter/PostProcessLineFilter/FilterMode | 大量死代码 |
| `core/command.rs` | 476 | execute() / execute_streaming() / CommandBuilder | 两种执行路径并存 |
| `core/utils.rs` | 506 | OutputPostProcessor 9阶段管线 | 批量模式正确，但逻辑被重复 |
| `plugins/cargo/analyzer.rs` | 214 | Streaming 路径 | 使用 execute_streaming |
| `plugins/npm/analyzer.rs` | 157 | Streaming 路径 | 使用 execute_streaming |
| 其余 10 个插件 | ~50-100/个 | Buffered 路径 | 使用 execute() |

### 1.2 核心问题

**三个关注点被错误地耦合**：

| 关注点 | 当前实现 | 判定 |
|--------|---------|------|
| 管道抽象 (`ProcessingPipeline`, `PipelineStage`, 9 个 struct/trait) | `stream.rs:19-284` | 死代码，仅测试使用 |
| 输出净化 (`OutputPostProcessor` 9阶段) | `utils.rs:160-346` 批量 + `stream.rs:518-589` 逐行重复 | 逻辑重复，两处维护 |
| 执行模式 (streaming vs buffered) | `command.rs:314-429` 三种 FilterMode | 有价值但 Buggy |

**具体缺陷：**

1. `PostProcessLineFilter::feed_line()` 重写了与 `OutputPostProcessor::process()` 完全相同的管线，但跳过了 short-circuit 阶段（stage 3），导致 Turbo 的成功短路规则在 streaming 模式下永不触发。

2. `execute_streaming()` 中 stderr 行只记录原始值，不传给 LineFilter。很多构建工具（clang、gcc、go）把错误输出到 stderr。

3. 12 个插件中有 3 种不同的 `analyze()` 实现模式，其中 10 个未经 OutputPostProcessor 预处理。

4. `FilterMode::Buffered`、`SimpleLineFilter`、`PostProcessStage`、`ProcessingPipeline` 整条链均为死代码。

### 1.3 设计文档的历史预期

`tui-output-handling-design.md` 将 streaming 列为 Phase 3（预留），并明确指出"当前缓冲模式满足需求"。实际开发中 Phase 1-2 已基本完成（OutputPostProcessor 升级、FilterRegistry），但 Phase 3 的实现引入了上述问题。

---

## 2. 目标架构

```
               所有插件统一的入口
                       │
            CommandBuilder::run_and_process()
                       │
            ┌──────────┴──────────┐
            │  内部决定执行模式     │
            │  buffered/streaming  │
            └──────────┬──────────┘
                       │
            OutputPostProcessor (唯一管线实现)
            ├─ 1. strip_ansi
            ├─ 2. replace
            ├─ 3. short_circuit  ← streaming 下通过 on_complete() 回调实现
            ├─ 4. strip_tui_frames
            ├─ 5. noise_lines
            ├─ 6. keep_lines
            ├─ 7. truncate_lines
            ├─ 8. max_lines
            └─ 9. on_empty
                       │
                  Parser::parse()
                       │
                AnalysisResult
```

**核心原则**：
- `OutputPostProcessor` 是输出净化的唯一实现，消除 `utils.rs` 与 `stream.rs` 之间的管线重复。
- 执行层（buffered/streaming）透明决定何时调用净化管线，插件不感知差异。
- 所有插件走同一条 `analyze()` 路径。

---

## 3. 具体改动

### 3.1 删除死代码（约 300 行）

从 `core/stream.rs` 删除：

| 删除项 | 行号范围 | 原因 |
|--------|---------|------|
| `PipelineError` | 26-50 | 仅 Pipeline 链使用 |
| `StageResult<T>` | 52-76 | 仅 Pipeline 链使用 |
| `PipelineStage<I,O>` trait | 78-85 | 无生产代码实现 |
| `ParseStage<P>` | 88-122 | 无生产代码使用 |
| `FilterStage` | 124-141 | 无生产代码使用 |
| `IncludePathsFilter` | 143-167 | 无生产代码使用 |
| `LevelFilter` | 169-189 | 无生产代码使用 |
| `AnalyzeStage` | 191-203 | 无生产代码使用 |
| `ProcessingPipeline` | 205-276 | 仅测试中使用 |
| `process_stage()` | 278-284 | 无调用方 |
| `SimpleLineFilter` | 286-336 | 仅测试中使用 |
| `PostProcessStage` | 338-365 | 无生产代码使用 |
| `FilterMode::Buffered` | 444-445 | 从未使用 |

**保留**：
- `LineFilter` trait — streaming 执行的核心抽象
- `FilterMode` enum（简化为 `Buffered` / `Streaming` 两变体）
- `PostProcessLineFilter` — 改造后作为 OutputPostProcessor 的逐行适配器
- `StreamResult` — streaming 执行结果
- `run_analysis_pipeline()` — 改造为统一入口
- `parse_and_analyze()` — 保持不变

### 3.2 OutputPostProcessor 作为唯一管线实现

将 `OutputPostProcessor::process()` 的每个阶段抽取为独立方法，`PostProcessLineFilter` 调用这些方法而非重新实现：

```rust
impl OutputPostProcessor {
    // 各阶段作为公开方法暴露，供 PostProcessLineFilter 逐行调用
    pub fn process_line_ansi(&self, line: &str) -> String { /* ... */ }
    pub fn process_line_replace(&self, line: &str) -> String { /* ... */ }
    pub fn is_noise_line(&self, line: &str) -> bool { /* ... */ }
    pub fn is_keep_line(&self, line: &str) -> bool { /* ... */ }
    pub fn process_line_truncate(&self, line: &str) -> String { /* ... */ }

    // 完整的批量处理方法（不变）
    pub fn process(&self, output: &str) -> String { /* ... */ }
}
```

`PostProcessLineFilter` 改为委托：

```rust
impl LineFilter for PostProcessLineFilter {
    fn feed_line(&mut self, line: &str) -> Option<String> {
        if self.capped { return None; }

        let mut result = self.processor.process_line_ansi(line);
        result = self.processor.process_line_replace(&result);

        if self.processor.strip_tui_frames {
            if is_tui_border_line(result.trim()) { return None; }
            result = strip_tui_prefix(&result);
            if result.is_empty() { return None; }
        }

        if self.processor.is_noise_line(&result) { return None; }
        if !self.processor.is_keep_line(&result) { return None; }

        result = self.processor.process_line_truncate(&result);

        if let Some(max) = self.processor.max_lines {
            if self.line_count >= max { self.capped = true; return None; }
        }

        self.line_count += 1;
        self.accumulated.push(result.clone()); // 累积用于 on_complete
        Some(result)
    }

    fn on_complete(&mut self) -> Vec<String> {
        // 对累积输出执行批量阶段：short_circuit + on_empty
        let accumulated = self.accumulated.join("\n");
        if let Some(msg) = self.processor.check_short_circuit(&accumulated) {
            return vec![msg];
        }
        if accumulated.trim().is_empty() {
            if let Some(ref msg) = self.processor.on_empty_message {
                return vec![msg.clone()];
            }
        }
        Vec::new()
    }
}
```

### 3.3 LineFilter trait 增加 on_complete 回调

```rust
pub trait LineFilter: Send {
    fn feed_line(&mut self, line: &str) -> Option<String>;

    fn on_complete(&mut self) -> Vec<String> {
        Vec::new()
    }

    // 废弃 flush()，用 on_complete() 替代
    #[deprecated]
    fn flush(&mut self) -> Vec<String> {
        self.on_complete()
    }
}
```

`on_complete()` 在所有行处理完毕后调用，解决 short-circuit 和 on_empty 在 per-line 模式下无法工作的问题。

### 3.4 修复 stderr 过滤

`execute_streaming()` 中 stderr 行同样传给 LineFilter，并在返回结果中与 stdout 的过滤输出合并：

```rust
StreamLine::Stderr(line) => {
    raw_stderr.push_str(&line);
    raw_stderr.push('\n');
    if let Some(f) = filter.feed_line(&line) {
        filtered.push_str(&f);
        filtered.push('\n');
    }
}
```

### 3.5 统一插件入口

提供通用函数消除每个插件 `analyze()` 中的样板代码：

```rust
/// Run analysis with automatic execution mode selection.
/// Chooses streaming or buffered based on whether a post-processor is provided.
pub fn run_analyzer(
    builder: CommandBuilder,
    parser: &dyn OutputParser,
    options: &AnalyzeOptions,
    processor: Option<OutputPostProcessor>,
) -> Result<AnalysisResult, AnalyzerError> {
    let processed = match processor {
        Some(proc) => {
            let line_filter = PostProcessLineFilter::new(&proc);
            let result = builder.execute_streaming(
                FilterMode::Streaming(Box::new(line_filter)),
            )?;
            result.filtered
        }
        None => {
            let raw = builder.execute()?;
            OutputPostProcessor::default().process(&raw)
        }
    };

    match parse_and_analyze(parser, &processed, options) {
        StageResult::Complete(r) | StageResult::Degraded(r, _) => Ok(r),
        StageResult::Failed(w) => Err(AnalyzerError::ParseError(w.join("; "))),
    }
}
```

插件 `analyze()` 简化为：

```rust
fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
    let builder = self.create_command_builder(options);
    let processor = build_cargo_post_processor(options);
    run_analyzer(builder, &self.parser, options, Some(processor))
}
```

### 3.6 FilterMode 简化

```rust
pub enum FilterMode<'a> {
    Streaming(Box<dyn LineFilter + 'a>),
    Buffered,
}
```

移除 `Passthrough`（当前项目无使用场景）和旧的 `Buffered(Box<dyn Fn>)`。

---

## 4. 实施计划

### Phase 1：清理死代码（0.5 天）

| # | 任务 | 文件 | 验证 |
|---|------|------|------|
| 1.1 | 删除 PipelineStage 系列 struct/trait | `core/stream.rs` | `cargo build` |
| 1.2 | 删除 SimpleLineFilter、FilterMode::Buffered | `core/stream.rs` | `cargo build` |
| 1.3 | 删除 PostProcessStage | `core/stream.rs` | `cargo test` |

### Phase 2：OutputPostProcessor 方法化（1 天）

| # | 任务 | 文件 | 验证 |
|---|------|------|------|
| 2.1 | 各阶段抽取为独立公开方法 | `core/utils.rs` | 单元测试 |
| 2.2 | PostProcessLineFilter 改为委托模式 | `core/stream.rs` | 单元测试 |
| 2.3 | LineFilter 增加 on_complete() | `core/stream.rs` | 集成测试 |
| 2.4 | 修复 stderr 过滤 | `core/command.rs` | `cargo test` |

### Phase 3：统一插件入口（1 天）

| # | 任务 | 文件 | 验证 |
|---|------|------|------|
| 3.1 | 实现 `run_analyzer()` 通用函数 | `core/stream.rs` | 单元测试 |
| 3.2 | Cargo 插件接入 `run_analyzer()` | `plugins/cargo/analyzer.rs` | 集成测试 |
| 3.3 | NPM 插件接入 `run_analyzer()` | `plugins/npm/analyzer.rs` | 集成测试 |
| 3.4 | 其余 10 个插件接入 `run_analyzer()` + OutputPostProcessor | `plugins/*/analyzer.rs` | `cargo test --all` |

### Phase 4：回归验证（0.5 天）

| # | 任务 |
|---|------|
| 4.1 | `cargo fmt --all && cargo clippy --all-targets && cargo test --all` |
| 4.2 | 验证 Cargo/NPM streaming 路径与重构前行为一致 |
| 4.3 | 验证 short-circuit 在 streaming 模式下生效 |
| 4.4 | 验证 stderr 内容正确进入解析器 |

---

## 5. 关键设计决策

1. **Streaming 保留但降级为执行层优化**：Streaming 是"怎么执行"的问题，不是"怎么处理"的问题。核心管线只有 `OutputPostProcessor` 一个实现。

2. **LineFilter 的 on_complete() 解决 per-line 局限性**：short-circuit 和 on_empty 需要全局上下文，不能在单行级别判断，延迟到所有行处理完毕后统一检查。

3. **Buffered 作为 Fallback**：当未提供 processor 时，使用默认的 `OutputPostProcessor` + buffered 执行。所有插件必须经过净化管线。

4. **FilterRegistry 保持独立**：TOML 配置系统（`config/filter_registry.rs`）不变，它产出 `OutputPostProcessor`，不感知执行模式。

5. **不引入 RTK 的 BlockFilter/LineHandler 模式**：RTK 的 `BlockStreamFilter` 和 `LineHandler` 是为通用 CLI 代理设计的，multi-analyzer 的解析器已足够处理结构化输出，不需要额外的状态机层。

---

## 6. 文件变更总结

| 文件 | 变更类型 | 预计行数变化 |
|------|---------|-------------|
| `core/stream.rs` | 重写 | 699 -> ~350 |
| `core/utils.rs` | 方法化 | 506 -> ~550 |
| `core/command.rs` | 修复 stderr + 简化 | 476 -> ~400 |
| `plugins/cargo/analyzer.rs` | 简化 | 214 -> ~60 |
| `plugins/npm/analyzer.rs` | 简化 | 157 -> ~50 |
| `plugins/*/analyzer.rs` (10个) | 接入 run_analyzer | 各减少 ~20 行 |
