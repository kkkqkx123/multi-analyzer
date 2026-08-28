# 已有构建日志分析功能设计方案

## 1. 概述

### 1.1 背景与问题

multi-analyzer 当前的工作方式是**"先执行、后解析"**:用户指定 `<tech-stack> <command>`,插件
内部构造 `CommandBuilder`,执行真实命令并实时流式收集输出,再交给 `OutputParser` 解析。

这种模式无法处理以下场景:

- 构建已经完成,日志被保存到文件(例如 `cmake --build build > /tmp/zlm_build_warn.log 2>&1`),
  重新执行成本高、不可复现(依赖网络、环境、构建缓存),甚至可能因环境差异产生不同结果。
- 日志来自 CI / 远端机器 / 同事,本地根本没有对应的构建环境。
- 用户只想对一份历史日志反复试验不同的过滤参数,而不是反复跑构建。

当前项目对 `/tmp/zlm_build_warn.log` 这类输入**完全无法处理**:没有入口接收日志文件,
`run_analyzer` 强制要求 `CommandBuilder` 执行子进程,拿到的是 `CommandOutput`,而不是文件内容。

### 1.2 目标

为项目新增**"分析已有构建日志"**能力,调用方需要传递:

1. **技术栈**(决定用哪个插件的解析器,如 `cmake`);
2. **完整命令**(决定过滤规则查表、报告元数据,如 `--build build`);
3. **构建日志文件位置**(如 `/tmp/zlm_build_warn.log`)。

工具读取日志文件,**不执行任何外部命令**,复用现有"过滤 → 解析 → 统计 → 报告"管线,产出与
执行模式一致的 `AnalysisResult` 与报告。

### 1.3 核心原则

- 完全复用现有管线:输出后处理(`OutputPostProcessor`)、解析器(`OutputParser`)、
  结果过滤(`filter_by_options`)、报告(`ReporterFactory`)全部复用,不重写。
- 不破坏现有 `analyzer <tech-stack> <command>` 用法,日志模式是**增量入口**。
- 解析器本身只消费纯文本,天然与"日志文件"兼容,插件侧**零改动**或极小改动。
- 日志模式与执行模式在报告层面保持一致(同一种 markdown/json/html/raw 输出)。

---

## 2. 现状分析

### 2.1 现有管线

```
User Input → CLI 解析 (parse_arguments)
    → PluginRegistry.get(tech_stack) → analyzer.analyze(options)
        → create_command_builder()   (如 "cmake --build build")
        → core::stream::run_analyzer(&builder, &parser, options)
            ├── resolve_processor()  (options + TOML filter 合并成 OutputPostProcessor)
            ├── builder.execute_streaming()  ← 执行子进程,逐行喂 PostProcessLineFilter
            ├── parse_and_analyze(parser, filtered, options)
            │     ├── parser.parse(filtered)         ← OutputParser 吃纯文本
            │     └── AnalysisResult::filter_by_options(options)
            └── TrackingGuard 记录执行统计
    → ReporterFactory 生成报告
```

### 2.2 为什么无法分析已有日志

| 环节 | 现状 | 对日志模式的阻碍 |
|------|------|------------------|
| CLI 入口 | 只接受 `<tech-stack> <command>`,无文件参数 | 无法传日志路径 |
| `run_analyzer` | 必须 `CommandBuilder.execute_streaming()` | 无命令可执行,子进程必然失败 |
| 过滤配置查表 | `filter_registry().find_filter(&builder.command_string())` | 没有 builder,查表入口缺失 |
| `exit_code` | 来自真实进程退出码 | 日志里没有退出码 |

关键洞察:**管线中从 `parser.parse()` 往后的所有环节都只依赖"处理后的文本 + options"**,
与"文本来自子进程还是文件"无关。因此只需要把"执行子进程"这一环替换为"读取日志文件",
后半段管线即可原样复用。

### 2.3 与现有插件的关系

以 CMake 场景为例,`CMakeParser`(`src/plugins/cpp/cmake/parser.rs`)的 `parse()` 已经是:

1. 用 `BlockCollector` 收集 `CMake Error at ...` / `CMake Warning at ...` 块;
2. `detect_compiler_type()` 自动识别 gcc/clang/msvc,再叠加 `CppParser` 解析编译器警告/错误。

也就是说 `cmake --build build` 产出的日志(编译器警告为主)**当前解析器已经完全能解析**,
缺的只是"把文件文本送进 `parser.parse()`"的入口。

---

## 3. CLI 设计方案

### 3.1 方案 A(推荐):`--log-file` 标志

在现有调用上追加一个值参数,复用全部参数解析逻辑:

```bash
# CMake 构建日志(zlm 场景)
analyzer cmake "--build build" --log-file /tmp/zlm_build_warn.log

# 其他技术栈同样适用
analyzer mypy "--strict ." --log-file /tmp/mypy.log
analyzer gcc "compile" --log-file /tmp/gcc_build.log
analyzer cargo "check --all-targets" --log-file /tmp/cargo_check.log

# 组合现有选项
analyzer cmake "--build build" --log-file /tmp/zlm_build_warn.log --format json --stdout
analyzer cmake "--build build" --log-file /tmp/zlm_build_warn.log --filter-warnings
```

**优点:**

- 改动最小:`tech_stack` / `command` / 过滤选项 / 报告选项的解析全部复用;
- `command` 语义自然——它就是"产生这份日志的完整命令",直接用于 filter 查表与报告元数据;
- 与现有用法并列展示在 `--help` 中,用户心智负担小。

**缺点:** 一个入口同时承担"执行"与"日志"两种模式,`run_analysis` 内部需要分支。

### 3.2 方案 B(备选):`analyzer log` 子命令

参照 `docs/plan/intercept-command-design.md` 中 `run`/`rewrite` 的入口风格:

```bash
analyzer log cmake "--build build" --file /tmp/zlm_build_warn.log
```

**优点:** 模式语义清晰;天然可扩展(未来支持 `--file -` 读 stdin、`--file a.log --file b.log`
多文件合并等)。

**缺点:** 需要新增一级子命令分发;两套 CLI 语义并存;`log` 与 `run` 子命令的定位需要长期
维护区分。

### 3.3 推荐结论

**采用方案 A,`--log-file <path>` 作为通用值参数**;方案 B 留作后续演进方向
(需求稳定后再抽象 `analyzer log` 子命令)。文档其余部分按方案 A 展开。

---

## 4. 核心设计

### 4.1 管线拆分:新增 `core/log_analyzer.rs`

将"执行"与"解析"解耦,新增两个函数:

```rust
//! Analyze existing build logs without executing any command.

use crate::core::analyzer::AnalyzerError;
use crate::core::parser::OutputParser;
use crate::core::types::{AnalysisResult, AnalyzeOptions};

/// Analyze log text captured in memory.
/// Mirrors the post-execution half of `stream::run_analyzer`.
pub fn analyze_log_text(
    raw: &str,
    command_str: &str,
    parser: &dyn OutputParser,
    options: &AnalyzeOptions,
) -> Result<AnalysisResult, AnalyzerError> {
    // 1. Resolve output post-processor (options + TOML filter, keyed by command_str)
    let processor = resolve_processor(command_str, options);

    // 2. Batch-process the full text (ANSI strip → replace → TUI frame →
    //    noise → keep → truncate → short-circuit), same semantics as streaming
    let processed = processor.process(raw);

    // 3. Parse + filter (reuse existing stage)
    let result = parse_and_analyze(parser, &processed, options)?;

    // 4. Log mode has no real exit code; never marks the command as failed
    Ok(result)
}

/// Read a log file and analyze it.
pub fn analyze_log_file(
    path: &std::path::Path,
    command_str: &str,
    parser: &dyn OutputParser,
    options: &AnalyzeOptions,
) -> Result<AnalysisResult, AnalyzerError> {
    let raw = read_log_file(path)?; // UTF-8 with lossy fallback + size guard
    analyze_log_text(&raw, command_str, parser, options)
}
```

### 4.2 小重构:`resolve_processor` 与 `parse_and_analyze` 去 `CommandBuilder` 依赖

`src/core/stream.rs` 中两处私有函数与 `CommandBuilder` 耦合,需要改为接收命令字符串:

```rust
// 改动前
fn resolve_processor(builder: &CommandBuilder, options: &AnalyzeOptions) -> OutputPostProcessor {
    let command_str = builder.command_string();
    // ...filter_registry().find_filter(&command_str)
}

// 改动后:签名改为 command_str,内部逻辑不变
fn resolve_processor(command_str: &str, options: &AnalyzeOptions) -> OutputPostProcessor;

// 改动后:parse_and_analyze 由私有的 StageResult 改为可返回 Result<AnalysisResult, AnalyzerError>
// (或保留内部类型,log_analyzer 复用同一文件内的私有函数即可)
fn parse_and_analyze(
    parser: &dyn OutputParser,
    processed_output: &str,
    options: &AnalyzeOptions,
) -> Result<AnalysisResult, AnalyzerError>;
```

调用方同步更新:

```rust
// stream.rs::run_analyzer 内
let processor = resolve_processor(&builder.command_string(), options);

// log_analyzer.rs::analyze_log_text 内
let processor = resolve_processor(command_str, options);
```

> 实现取舍:若不想改 `parse_and_analyze` 的 `StageResult` 内部类型,可将 `log_analyzer.rs`
> 作为 `stream.rs` 的兄弟模块并复用其私有函数(二者同属 `core`);更干净的做法是把
> `parse_and_analyze` 提升为 `pub(crate)` 并返回 `Result`。推荐后者,减少重复。

### 4.3 `AnalyzeOptions` 新增字段

```rust
pub struct AnalyzeOptions {
    // ... existing fields

    /// --log-file <path>: analyze an existing build log file instead of
    /// executing the command. When set, no external command is run.
    pub log_file: Option<String>,
}
```

初始化:`AnalyzeOptions::from_config` 中置 `log_file: None`。

### 4.4 CLI 解析变更(`src/main.rs`)

1. `VALUE_FLAGS` 增加 `"--log-file"`;
2. `parse_options_from_args` 增加分支:

```rust
"--log-file" => {
    if let Some(v) = value {
        options.log_file = Some(v.clone());
    }
}
```

3. `run_analysis` 增加日志分支(见下),并把"报告生成 + 输出"抽成独立函数 `emit_report`
   供两个分支复用:

```rust
fn run_analysis(analyzer: &dyn core::BuildAnalyzer, options: &AnalyzeOptions) {
    let result = if let Some(log_path) = &options.log_file {
        // 合成命令字符串:与执行模式下 builder.command_string() 对齐,
        // 保证 filter 查表(如 turbo.toml)对日志模式同样生效
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| format!("{} {}", analyzer.name(), s.as_str()))
            .unwrap_or_else(|| analyzer.name().to_string());

        println!("Analyzing log file {} ({} {})...", log_path, analyzer.name(), command_str);

        core::analyze_log_file(
            Path::new(log_path),
            &command_str,
            analyzer.parser(),
            options,
        )
    } else {
        analyzer.analyze(options)
    };

    match result {
        Ok(analysis) => emit_report(analysis, analyzer, options),
        Err(e) => {
            eprintln!("Analysis failed: {}", e);
            std::process::exit(1);
        }
    }
}
```

### 4.5 过滤配置查表的关键细节

执行模式下 `filter_registry().find_filter(&builder.command_string())` 以 `"cmake --build build"`
这样的字符串查 TOML 过滤表(`src/filters/turbo.toml` 等)。日志模式必须合成出**一致的**
命令字符串,否则 `cmake`/`turbo` 等命令级过滤规则会丢失。

合成规则(与 4.4 一致):

```
command_str = "<tech-stack> <subcommand>"   // 例: "cmake --build build"
```

### 4.6 报告输出复用

`run_analysis` 中现有的报告生成/写文件/打印摘要代码(~60 行)抽为:

```rust
fn emit_report(result: AnalysisResult, analyzer: &dyn core::BuildAnalyzer, options: &AnalyzeOptions)
```

两个分支共用;`to_report_options` 不变(tech_stack 名已含 subcommand)。

### 4.7 `--format raw` 交互

执行模式下 `raw` 格式走 passthrough(不解析,直接透传原始流)。日志模式保持一致:
`--format raw` 时输出日志文件的处理结果文本(或原文),不进入解析。

```rust
// run_analysis 日志分支内,最先判断
if options.report_format.is_raw() {
    // 读取文件并输出(可经过 OutputPostProcessor::process 处理)
    // 不调用 parser.parse()
}
```

---

## 5. 关键实现细节

### 5.1 日志读取(`read_log_file`)

- 文件不存在 / 无权限 → `AnalyzerError::IoError`,CLI 打印错误并退出码 1;
- 编码:优先 UTF-8;非法字节用 `String::from_utf8_lossy` 兜底(构建日志常混有 ANSI 与
  非 UTF-8 输出,不因个别乱码字节放弃整份日志);
- 大小保护:超过上限(建议默认 64 MiB,可用 `max_output_lines` 反推更保守的阈值)时
  截断读取并给出提示,防止大日志(如全量 CI 日志数百 MB)打爆内存。

### 5.2 `exit_code` 与 `command_failed`

- 日志模式无真实退出码:`AnalysisResult.exit_code = None`(类型注释 "None if not executed"
  已支持),`command_failed = false`;
- 执行模式中"命令失败但未解析出 issue → 注入原始输出作为错误"的 fallback(`stream.rs`
  L89-113)是**执行模式专属**,日志模式不适用,不移植;
- 可选增强:为 `AnalysisResult` 增加 `source: Option<String>`(如 `Some("log:/tmp/xxx.log")`),
  在报告元数据中体现"分析对象是日志文件"。列为可选项,不影响核心功能。

### 5.3 Tracking 统计

日志模式不执行命令,建议**跳过 `TrackingGuard`**(或记录一条 `exit_code=None` 的条目)。
`analyze_log_file` 内部不调用 `tracking` 相关代码即可,保持简单。

### 5.4 命令与技术栈一致性

技术栈决定解析器,命令决定过滤查表。若用户组合不当(如 `analyzer gcc "compile"` 但日志实际
是 msbuild 输出),解析器会解析出 0 个 issue——与执行模式行为一致,不做额外校验,仅在
`--verbose` 下打印解析器类型提示(现有 `run_analysis` 已有类似输出)。

### 5.5 测试命令

`is_test_subcommand` 判断含 "test" 的子命令会走 `run_test_analysis`(测试执行)。日志模式下
第一阶段**不支持测试日志分析**(测试运行需要真实执行),`--log-file` 遇到 test 子命令时给出
明确提示并退出。列为后续扩展项。

---

## 6. 边界与限制

| 场景 | 行为 |
|------|------|
| 日志包含 ANSI 颜色 | 沿用 `options.strip_ansi`(默认开启),`OutputPostProcessor` 统一剥离 |
| 日志含 turbo/TUI 帧 | `strip_tui_frames` / TOML filter 照常生效(命令查表已对齐) |
| 超大日志(>64 MiB) | 截断读取 + 提示;`--max-issues`/`max_output_lines` 生效在解析后 |
| 文件不存在 | 报 `IO error`,退出码 1 |
| 日志为空 | 走 `on_empty_message` 短路,与执行模式一致 |
| 多段日志(configure + build 合并) | `CMakeParser` 已同时处理 CMake 块与编译器输出,无需特殊处理 |
| 非 UTF-8 日志 | lossy 转换,不中断分析 |
| `--format raw` | 输出文件处理结果,不解析 |
| test 子命令 + `--log-file` | 明确报错,暂不支持(后续扩展) |

---

## 7. 测试计划

### 7.1 单元测试

- `log_analyzer.rs`:
  - `analyze_log_text` 对 `tests/data/raw_output/cmake_build_msvc.txt` / `gcc_warnings.txt`
    等现有 fixture 解析,断言 issue 数量与现有解析器测试一致(证明"文本源替换"无损);
  - 空文本、纯噪声文本、仅 ANSI 文本;
  - `resolve_processor` 命令查表:合成命令字符串能命中 `turbo.toml` 过滤规则;
  - 文件不存在 → `IoError`;超大文件 → 截断。

### 7.2 集成测试(`tests/log_analysis_integration_tests.rs`)

- 新建 fixture `tests/data/raw_output/cmake_build_warn.log`:模拟 `cmake --build build`
  的真实输出(进度行 + gcc 警告 + 链接行),对应 zlm 场景;
- 通过 CLI 调用:

```bash
analyzer cmake "--build build" --log-file tests/data/raw_output/cmake_build_warn.log --format json --stdout
```

- 断言:退出码 0;JSON 中 issues 数量正确、level/code/file 字段正确;
- 断言 `--filter-warnings` 后 warning 被过滤;
- 断言 `--log-file` 指向不存在路径时退出码非 0 且报错信息含文件路径。

### 7.3 回归

- 现有 `tests/command_execution_tests.rs` / `cmake_parser_tests.rs` 全量通过(证明重构
  `resolve_processor`/`parse_and_analyze` 未破坏执行模式)。

---

## 8. 实施阶段与模块变更总结

### 8.1 阶段划分

| 阶段 | 内容 |
|------|------|
| Phase 1 | 重构 `core/stream.rs`:`resolve_processor` 改收 `command_str`;`parse_and_analyze` 提升为可复用并返回 `Result` |
| Phase 2 | 新增 `core/log_analyzer.rs`:`analyze_log_text` + `analyze_log_file` + `read_log_file` |
| Phase 3 | `AnalyzeOptions` 增加 `log_file` 字段;`main.rs` 增加 `--log-file` 解析与 `run_analysis` 日志分支;抽取 `emit_report` |
| Phase 4 | 测试(fixture + 单元 + 集成)与 README 更新 |

### 8.2 模块变更总结

**新增文件:**

| 路径 | 说明 |
|------|------|
| `src/core/log_analyzer.rs` | 日志读取 + 解析入口 |
| `tests/log_analysis_integration_tests.rs` | 日志模式集成测试 |
| `tests/data/raw_output/cmake_build_warn.log` | zlm 场景真实日志 fixture |
| `docs/plan/analyze-log-design.md` | 本文档 |

**修改文件:**

| 路径 | 变更 |
|------|------|
| `src/core/stream.rs` | `resolve_processor` 签名改为 `command_str`;`parse_and_analyze` 改为可复用 |
| `src/core/mod.rs` | 导出 `log_analyzer` 模块 |
| `src/core/types.rs` | `AnalyzeOptions` 新增 `log_file: Option<String>` |
| `src/main.rs` | `VALUE_FLAGS` 增加 `--log-file`;参数解析分支;`run_analysis` 日志分支;抽取 `emit_report` |

**不修改:**

- `src/plugins/**`(解析器零改动——CMakeParser 等已能消费纯文本);
- 现有 `analyzer <tech-stack> <command>` 执行模式;
- 配置加载、reporter、tracking 机制。

### 8.3 验收标准

```bash
# zlm 场景验收
cmake --build build > /tmp/zlm_build_warn.log 2>&1   # 用户已完成的构建
analyzer cmake "--build build" --log-file /tmp/zlm_build_warn.log

# 期望:
# 1. 不执行任何 cmake 子进程(可通过 strace / 断网验证);
# 2. 输出 analysis_report.md,内容与"重新执行构建"得到的报告一致(解析器同一套);
# 3. --format json 下 issues 字段完整;--filter-warnings 生效;
# 4. 现有执行模式回归测试全绿。
```
