# RTK discover 模块分析报告

## 概述

本文档分析 RTK 项目中 `ref/rtk-develop/src/discover/` 模块的架构设计与实现，提炼可供当前 Analyzer 项目借鉴的设计思路。

discover 模块是 RTK 中负责**命令分类与扫描分析**的核心模块，服务于两条业务路径：

| 路径                | 用途                                                    | 调用时机   |
| ------------------- | ------------------------------------------------------- | ---------- |
| **Rewrite (重写)**  | LLM agent 执行命令时判断是否需要重写为 `rtk <cmd>` 格式 | 实时热路径 |
| **Discover (扫描)** | 扫描历史会话中未被优化的命令，生成优化建议报告          | 离线分析   |

两条路径共享同一套分类逻辑，体现了"一次实现、多处复用"的设计原则。

---

## 模块架构

```
discover/
├── mod.rs       # 编排层 — 统筹 provider → registry → report 的完整流程
├── rules.rs     # 规则层 — 60+ 条声明式匹配规则
├── registry.rs  # 引擎层 — 命令分类、重写、命令链拆分（核心业务逻辑）
├── provider.rs  # 数据层 — 从会话文件提取命令历史
├── report.rs    # 展示层 — Text/JSON 报告生成
├── lexer.rs     # 基础设施 — Shell 命令词法解析器
└── README.md    # 文档
```

### 数据流

```
discover_sessions()
  → 递归扫描 ~/.claude/projects/ 目录下的 JSONL 文件
  → extract_commands() 解析 tool_use/tool_result 提取命令
    → split_command_chain() 拆分复合命令（&&, ||, ;, |）
      → classify_command() 匹配规则并分类
        → Supported / Unsupported / Ignored
          → Bucket 聚合统计
            → DiscoverReport 报告生成
```

---

## 各模块分析

### 1. lexer.rs — Shell 词法解析器

**解决的问题**：Shell 命令不是简单的空格分割字符串，包含引号、转义、重定向、管道等语法。

**实现特点**：

- 单遍状态机：char 迭代器 + peekable，单次遍历完成全部 Tokenization
- 状态追踪：quote（' / "）、escaped（\ 转义）两个状态
- Token 类型体系：

  | TokenKind  | 含义                | 示例                                 |
  | ---------- | ------------------- | ------------------------------------ | --- | ------ |
  | `Arg`      | 普通参数（含 $VAR） | `git`, `status`, `$HOME`             |
  | `Operator` | 控制运算符          | `&&`, `                              |     | `, `;` |
  | `Pipe`     | 管道                | `                                    | `   |
  | `Redirect` | 重定向              | `>`, `>>`, `<`, `<<`, `2>&1`         |
  | `Shellism` | Shell 特有语法      | `*`, `?`, `$()`, `${}`, `` ` ``, `!` |

- 每个 Token 记录 `offset` 字节偏移，便于后续在原始字符串中定位

**对当前项目的参考价值**：Parser 当前使用逐行正则匹配。如果引入前置词法分析步骤，可以更鲁棒地处理编译器错误输出的结构化信息。

---

### 2. rules.rs — 声明式规则定义

**核心设计**：纯数据驱动，所有匹配逻辑集中在静态表中。

```rust
pub struct RtkRule {
    pub pattern: &'static str,                        // 正则匹配模式
    pub rtk_cmd: &'static str,                        // 对应 RTK 命令
    pub rewrite_prefixes: &'static [&'static str],    // 需要替换的前缀
    pub category: &'static str,                       // 分类标签
    pub savings_pct: f64,                             // 默认节省百分比
    pub subcmd_savings: &'static [(&'static str, f64)],       // 子命令级节省覆盖
    pub subcmd_status: &'static [(&'static str, RtkStatus)],  // 子命令级状态覆盖
}
```

**设计特点**：

- 同类别工具共享一条规则（如 `git` 和 `yadm`）
- 子命令级别的覆盖机制（如 `diff` 节省 80% > 默认 70%）
- 三种支持状态：`Existing`（有专属过滤）、`Passthrough`（通用透传）、`NotSupported`（不支持）

**对当前项目的参考价值**：当前每个 Plugin 的 parser 实现都是硬编码的解析逻辑。如果可以引入声明式规则定义——比如每种错误模式的匹配模式、严重级别、提取方式用结构化数据表达——那么新增技术栈支持只需添加规则条目，无需写新的 parser 实现。

---

### 3. registry.rs — 规则引擎

这是模块核心，实现 `classify_command()` 和 `rewrite_command()`。

**命令分类流程**：

```
命令字符串
  → 忽略列表检查 (IGNORED_EXACT + IGNORED_PREFIXES)
  → 剥离环境变量前缀 (sudo, env, VAR=val)
  → 规范化：绝对路径去前缀 (/usr/bin/grep → grep)
  → 剥离 git 全局选项 (-C <path>, -c <key=val>)
  → 剥离 golangci-lint 全局选项
  → 检查重定向操作 (cat/head/tail + >/>> 时跳过)
  → RegexSet 批量匹配（所有规则并行匹配）
  → 提取子命令 → 查找 subcmd_savings/subcmd_status 覆盖
  → 返回 Classification::Supported / Unsupported / Ignored
```

**关键优化**：

- **`RegexSet`**：所有规则编译为一次并行匹配，避免逐个尝试
- **`lazy_static!`**：正则表达式只编译一次
- **匹配取最后一个**：`matches.last()` 支持规则覆盖

**对当前项目的参考价值**：如果一个 Parser 需要匹配大量模式（如 Cargo 的数百种 error codes），可以使用 `RegexSet` 批量匹配优化性能。

---

### 4. provider.rs — 数据层抽象

**SessionProvider Trait**：

```rust
pub trait SessionProvider {
    fn discover_sessions(&self, project_filter: Option<&str>, since_days: Option<u64>) -> Result<Vec<PathBuf>>;
    fn extract_commands(&self, path: &Path) -> Result<Vec<ExtractedCommand>>;
}
```

**设计考虑**：

- Trait 抽象便于未来扩展到 OpenCode、Cursor 等其他会话格式
- `walkdir` 递归遍历支持 sub-agents 子目录
- `try_exists` 优雅处理目录不存在的情况
- 路径编码适配 Claude Code 的目录命名规则

**对当前项目的参考价值**：如果需要支持多种输入源（CI 日志文件、本地命令输出、GitHub Actions 日志），可以引入类似的 `LogProvider` trait，解耦"输入方式"和"解析逻辑"。

---

### 5. report.rs — 报告层

**报告数据结构**：

```rust
pub struct DiscoverReport {
    pub sessions_scanned: usize,
    pub total_commands: usize,
    pub already_rtk: usize,
    pub supported: Vec<SupportedEntry>,
    pub unsupported: Vec<UnsupportedEntry>,
    pub rtk_disabled_count: usize,
    pub agent_status: AgentIntegrationStatus,
}
```

**特点**：

- 同时支持 Text（人类可读表格）和 JSON（机器解析）
- 包含系统配置检测（Cursor 钩子、Hermes 插件是否安装）
- 支持 RTK_DISABLED 滥用警告

---

### 6. mod.rs — 编排层

**聚合策略**（Bucket 模式）：

```rust
struct SupportedBucket {
    rtk_equivalent: &'static str,           // 分组键
    count: usize,                            // 频次统计
    total_output_tokens: usize,              // 加权累加
    total_raw_output_tokens: usize,          // 用于计算加权平均节省率
    command_counts: HashMap<String, usize>,  // 子命令频次
}
```

**特点**：

- 支持不同子命令有不同的节省率
- 最终按预估节省 token 数降序排列
- 使用加权平均计算有效节省率

**当前项目已实现的对应能力**：`AnalysisResult` 的 `issues_by_type`、`issues_by_file`、`issues_by_package` 等多维度聚合。

---

## 对当前项目的参考总结

| 参考点           | RTK 做法                  | 当前项目状态     | 优先级 | 实施建议                   |
| ---------------- | ------------------------- | ---------------- | ------ | -------------------------- |
| 声明式规则       | rules.rs 数据驱动         | 各 Parser 硬编码 | P1     | 将错误模式定义为结构化规则 |
| Token 化解析     | lexer → classify          | 逐行正则匹配     | P2     | 引入前置词法分析步骤       |
| ParseResult 三态 | Full/Degraded/Passthrough | 已实现           | ✅     | 保持完善                   |
| 输入源抽象       | SessionProvider trait     | CLI 直接执行     | P2     | 引入 LogProvider trait     |
| 批量匹配优化     | RegexSet                  | 未使用           | P2     | 用于多模式匹配场景         |
| Bucket 聚合      | 加权统计 + 排序           | 已部分实现       | ✅     | 可引入节省/影响估算        |
| 配置感知检测     | AgentIntegrationStatus    | 未实现           | P3     | 检测项目环境配置           |
| 权重估算         | 分类平均 tokens           | 未实现           | P3     | 估算修复成本优先级         |

### 已实现的改进

以下策略已在压缩策略分析后实施到当前项目中：

| 策略          | 实现位置                               | 功能                                 |
| ------------- | -------------------------------------- | ------------------------------------ |
| 噪音行剥离    | `core/utils.rs::filter_noise_lines()`  | 按正则模式过滤噪音行                 |
| 保留行匹配    | `core/utils.rs::keep_matching_lines()` | 仅保留匹配模式的行                   |
| 行数截断      | `core/utils.rs::truncate_output()`     | 支持 Head/Tail/HeadTail/Max 四种模式 |
| 智能截断      | `core/utils.rs::smart_truncate_line()` | 关键词上下文感知截断                 |
| 路径缩短      | `core/utils.rs::compact_path()`        | 绝对路径 → 相对路径                  |
| Token 预估    | `core/utils.rs::estimate_tokens()`     | 简单 Token 计数                      |
| 后处理管道    | `core/utils.rs::OutputPostProcessor`   | 链式输出后处理                       |
| 成功短路      | `core/reporter/mod.rs::ReportOptions`  | 零问题时单行确认输出                 |
| TOML 过滤配置 | `core/config.rs::FilterConfig`         | 噪音/保留/截断配置                   |

---

## 关键设计原则

1. **分类逻辑共享**：一套规则同时服务于 rewrite 和 discover 两个场景
2. **从具体到抽象**：先有 lexer 词法分析建立基础，再构建规则引擎
3. **数据驱动**：规则是数据而非代码，扩展时只需添加数据条目
4. **优雅降级**：Real 值不可用时 fallback 到估算值，不阻塞流程
5. **关注点分离**：provider/registry/report 各层职责明确，可独立演进
