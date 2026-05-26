## RTK 设计思路分析报告

以下是对 `ref/rtk-develop` (RTK) 与当前项目的对比分析，重点列出可借鉴的设计思路。

---

### 1. 架构总览对比

| 维度     | RTK                                     | 当前项目                                |
| -------- | --------------------------------------- | --------------------------------------- |
| 定位     | CLI 输出过滤/压缩代理                   | 构建错误分析器                          |
| 核心流程 | 执行命令 → 流式过滤 → 压缩输出 → 跟踪   | 执行命令 → 缓冲输出 → 解析 → 生成报告   |
| 模块组织 | 按生态系统（git/js/python/go/rust/...） | 按技术栈（cargo/npm/python/go/cpp/...） |
| 扩展方式 | 在 `src/cmds/` 下添加新模块             | 在 `src/plugins/` 下实现 Trait          |

---

### 2. 可借鉴的设计思路（按优先级排序）

#### 2.1 流式输出处理 (Streaming Filter Pipeline) ★★★★★

**RTK 的做法**：通过 `StreamFilter` Trait + `BlockHandler`/`LineHandler` 模式实现流式过滤。命令执行与输出过滤并行进行，无需等待整个命令完成。

[stream.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/stream.rs) 中的核心设计：

```rust
pub trait StreamFilter {
    fn feed_line(&mut self, line: &str) -> Option<String>;  // 逐行处理
    fn flush(&mut self) -> String;                           // 刷新缓冲区
    fn on_exit(&mut self, exit_code: i32, raw: &str) -> Option<String>; // 汇总摘要
}

pub trait BlockHandler {
    fn should_skip(&mut self, line: &str) -> bool;           // 跳过噪声行
    fn is_block_start(&mut self, line: &str) -> bool;        // 识别错误块开始
    fn is_block_continuation(&mut self, line: &str, block: &[String]) -> bool; // 块延续
    fn format_summary(&self, exit_code: i32, raw: &str) -> Option<String>; // 块摘要
}
```

**对当前项目的价值**：

- 当前项目使用 `CommandBuilder.execute()` 等待命令完全结束后再解析，大项目输出可能很大，内存占用高
- 采用流式处理可以边执行边解析，实现**实时进度反馈**和**早期错误检测**
- `BlockHandler` 模式非常适合编译错误（按 error/warning 块组织）

**实现建议**：在 `core/` 下引入 `stream.rs`，提供 `StreamRunner` 代替 `CommandBuilder`：

```rust
// 伪代码示意
pub struct StreamRunner {
    handler: Box<dyn BlockHandler>,
}

impl StreamRunner {
    pub fn run(&mut self, cmd: &mut Command) -> Result<AnalysisResult> {
        // 边读取 stdout/stderr 边调用 handler
        // 命令结束后调用 handler.format_summary()
    }
}
```

---

#### 2.2 解析降级策略 (Three-Tier Parsing Degradation) ★★★★★

**RTK 的做法**：在 [parser/mod.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/parser/mod.rs) 中定义了三级降级策略：

```rust
pub enum ParseResult<T> {
    Full(T),                    // 完整解析
    Degraded(T, Vec<String>),  // 部分解析 + 警告
    Passthrough(String),        // 透传原始输出
}
```

**对当前项目的价值**：

- 当前项目在解析失败时返回 `AnalyzerError::ParseError`，这会导致整个分析失败
- 采用三级降级后，即使特定版本的工具输出格式变了，也能降级到部分解析或直接展示原始输出
- 符合 RTK 的"永不阻塞"（Never Block）原则

**实现建议**：在 `parser.rs` 中引入 `ParseResult`，并修改 `OutputParser::parse()` 签名：

```rust
fn parse(&self, output: &str) -> ParseResult<Vec<Issue>>;
```

---

#### 2.3 配置系统 (Configuration System) ★★★★☆

**RTK 的做法**：在 [config.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/config.rs) 中实现了一个完整的 TOML 配置系统：

- 类型化配置段（tracking, display, filters, tee, telemetry, hooks, limits）
- 所有字段都有 `Default` 实现
- 配置不存在时自动返回默认值（不会报错）
- 通过 `Config::load()` 统一加载，支持环境变量覆盖

**对当前项目的价值**：

- 当前项目完全没有配置系统，所有行为硬编码
- 可引入配置来控制：警告是否过滤、最大报告行数、默认输出格式、超时时间等

**可借鉴的关键设计**：

```rust
// RTK 的配置加载模式
pub fn limits() -> LimitsConfig {
    Config::load().map(|c| c.limits).unwrap_or_default()
}
```

---

#### 2.4 Runner 构建器模式 (RunOptions Builder) ★★★★☆

**RTK 的做法**：在 [runner.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/runner.rs) 中使用构建器模式：

```rust
pub struct RunOptions<'a> {
    pub tee_label: Option<&'a str>,
    pub filter_stdout_only: bool,
    pub skip_filter_on_failure: bool,
    pub no_trailing_newline: bool,
    pub inherit_stdin: bool,
}

impl<'a> RunOptions<'a> {
    pub fn with_tee(label: &'a str) -> Self { ... }
    pub fn stdout_only() -> Self { ... }
    pub fn tee(mut self, label: &'a str) -> Self { ... }
    pub fn early_exit_on_failure(mut self) -> Self { ... }
}
```

**对当前项目的价值**：

- 当前项目的 `CommandBuilder` 已经初步使用了构建器模式，但可以通过链式调用进一步简化
- 可以添加更多执行选项控制（如超时行为、stderr/stdout 分离、退出码处理策略）

---

#### 2.5 多维度过滤策略 (Filtering Strategies) ★★★★☆

**RTK 的做法**：定义了 5 种过滤策略（参见 [ARCHITECTURE.md](file:///d:/项目/cli/analyzer/ref/rtk-develop/docs/contributing/ARCHITECTURE.md)）：

| 策略                | 描述                   | 适用场景                            |
| ------------------- | ---------------------- | ----------------------------------- |
| Stats Extraction    | 提取统计摘要，丢弃细节 | `cargo build: 3 errors, 5 warnings` |
| Error Only          | 只保留 stderr          | 编译失败场景                        |
| Grouping by Pattern | 按规则分组计数         | lint 输出                           |
| Deduplication       | 去重并标记重复次数     | 日志分析                            |
| Structure Only      | 保留结构，剥离数值     | JSON 输出                           |

**对当前项目的价值**：

- 当前项目只做"全量解析"，没有分层过滤策略
- 可以引入"摘要模式"：先展示统计摘要，用户按需查看详情
- 对于大型项目的 lint 输出（如 ESLint 上千条警告），分组统计远比逐条列出有用

---

#### 2.6 原始输出恢复 (Tee System) ★★★☆☆

**RTK 的做法**：在 [tee.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/tee.rs) 中实现了原始输出保存机制：

- 命令失败时自动保存原始输出到 `~/.local/share/rtk/tee/`
- 可配置保存策略（Never/Failures/Always）
- 自动轮转旧文件（最多保留 20 个）
- 输出的提示信息如 `[full output: ~/.local/share/rtk/tee/12345_cargo_build.log]`

**对当前项目的价值**：

- 当解析器无法正确解析工具输出时，保存原始输出便于调试
- 用户可以检查原始输出来确认分析结果是否正确

---

#### 2.7 正则编译优化 (Lazy Static Regex) ★★★☆☆

**RTK 的做法**：所有正则表达式都使用 `lazy_static!` 或 `OnceLock` 全局编译一次：

```rust
lazy_static! {
    static ref MYPY_DIAG: Regex = Regex::new(
        r"^(.+?):(\d+)(?::\d+)?: (error|warning|note): (.+?)(?:\s+\[(.+)\])?$"
    ).unwrap();
}
```

**对当前项目的价值**：

- 当前项目的 parser 中 regex 在每次 `parse()` 调用时重新编译（如果放在函数内部）
- 应使用 `lazy_static!` 或 `std::sync::OnceLock` 确保正则只编译一次

---

#### 2.8 模块级别 README 规范 ★★★☆☆

**RTK 的做法**：每个模块目录下都有 `README.md`，说明该模块的职责、核心概念和文件组织。代码中有 `//!` 模块级 doc comment。

**对当前项目的价值**：

- 当前项目缺少模块 README
- 添加 README 可以降低新贡献者的上手成本

---

#### 2.9 测试组织方式 (Inline Tests with Fixtures) ★★★☆☆

**RTK 的做法**：

- 测试与实现在同一个文件中（`#[cfg(test)] mod tests { ... }`）
- 测试数据以 fixture 文件形式放在 `tests/fixtures/` 下
- 使用 `include_str!()` 宏在测试中加载 fixture
- 有 token-savings 断言来验证过滤效果

**对当前项目的价值**：

- 当前项目的测试数据也可用 fixture 文件组织
- 在测试用例中验证解析准确率（召回率/精确率）

---

#### 2.10 执行指标跟踪 (Metrics Tracking) ★★★☆☆

**RTK 的做法**：在 [tracking.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/tracking.rs) 中使用 SQLite 记录每次命令执行：

- 记录输入/输出 token 数、执行时间、节省比例
- 支持按天/周/月统计
- 自动清理 90 天前的历史数据

**对当前项目的价值**：

- 可以跟踪分析器的分析耗时、成功/失败率、发现的问题数
- 帮助评估分析器在不同项目上的表现

---

### 3. 具体可复用的代码模式

| 模式                                 | RTK 位置                                                                        | 当前项目可借鉴的改进                                               |
| ------------------------------------ | ------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `resolved_command()` 跨平台命令查找  | [utils.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/utils.rs)      | 当前已有 `resolve_command()`，但 RTK 的实现了 `tool_exists()` 检测 |
| `strip_ansi()` ANSI 去除             | [utils.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/utils.rs)      | 当前 NPM parser 自己实现了 ANSI 剥离，应提取为通用工具函数         |
| `truncate()` 智能截断                | [utils.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/utils.rs)      | 当前缺少统一截断工具，报告中对长消息处理不够                       |
| `exit_code_from_output()` 退出码处理 | [utils.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/utils.rs)      | 当前项目对退出码的处理可以更完善                                   |
| `BlockHandler` 模式                  | [stream.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/stream.rs)    | cargo parser 的 `parse_multiline_error()` 可以重构为此模式         |
| 配置段 + Default                     | [config.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/core/config.rs)    | 项目缺少配置系统                                                   |
| `ParseResult` 三级降级               | [parser/mod.rs](file:///d:/项目/cli/analyzer/ref/rtk-develop/src/parser/mod.rs) | 解析失败不应直接报错，应降级展示                                   |

---

### 4. 总结建议

**高优先级行动项**：

1. **流式处理管道** — 引入 `StreamFilter` + `BlockHandler` 模式，实现边执行边解析
2. **解析降级策略** — 引入 `ParseResult`，解析失败时降级而非报错
3. **配置系统** — 引入 TOML 配置，统一管理分析器行为

**中优先级行动项**：

4. **RunOptions 构建器** — 优化现有 `CommandBuilder`，增加更多执行选项
5. **多维度过滤策略** — 引入摘要模式 + 分组统计能力
6. **Lazy Static Regex** — 确保所有 regex 只编译一次
7. **原始输出恢复** — 失败时保存原始输出便于调试

**低优先级行动项**：

8. **模块 README** — 为各模块添加文档
9. **测试组织** — 使用 fixture 文件组织测试数据
10. **指标跟踪** — 记录分析性能数据

分析完成。以上报告对比了两个项目的架构设计，识别了 **10 个可借鉴的设计思路**，覆盖了从架构模式（流式处理、三级降级）到具体代码实现（Lazy Static Regex、ANSI 剥离工具函数）等多个层面。如果你希望对其中某个设计思路进行更深入的探讨或开始落地实施，我可以进一步提供具体的实现方案。
