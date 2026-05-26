# RTK 架构文档

## 概述

**RTK (Rust Token Killer)** 是一个高性能 CLI 代理工具，核心目标是在命令输出到达 LLM 上下文之前进行过滤和压缩，节省 60-90% 的 Token 消耗。

- 语言：Rust (edition 2021)
- 许可证：Apache 2.0
- 版本：0.40.0
- 仓库：https://github.com/rtk-ai/rtk

---

## 目录

1. [设计原则](#设计原则)
2. [模块组织](#模块组织)
3. [六阶段执行流程](#六阶段执行流程)
4. [核心模块详解](#核心模块详解)
5. [命令模块体系](#命令模块体系)
6. [过滤器体系](#过滤器体系)
7. [流式处理架构](#流式处理架构)
8. [跟踪与遥测系统](#跟踪与遥测系统)
9. [Hook 系统](#hook-系统)
10. [配置系统](#配置系统)
11. [构建优化](#构建优化)
12. [关键架构决策](#关键架构决策)

---

## 设计原则

1. **透明代理** — RTK 作为透明代理运行，不改变用户习惯，自动拦截并过滤命令输出
2. **按语言生态组织** — 每个编程语言生态（JS、Python、Rust、Go、Ruby 等）拥有独立的命令模块
3. **流式处理优先** — 子进程输出通过流式管道处理，支持实时过滤，避免内存爆炸
4. **可扩展过滤器** — 支持 TOML 定义的内置过滤器和 Rust 代码过滤器两种方式
5. **零配置起步** — 安装即用，配置可选
6. **CI/CD 安全** — 保留原始退出码，确保管道行为不变

---

## 模块组织

```
src/
├── main.rs              # CLI 入口：clap 定义、命令分发、fallback 解析
├── core/                # 核心基础设施
│   ├── config.rs        # 用户配置 (config.toml) 读取
│   ├── constants.rs     # 全局常量
│   ├── runner.rs        # 通用命令执行骨架
│   ├── stream.rs        # 流式 I/O 处理核心
│   ├── filter.rs        # 代码级过滤器策略
│   ├── toml_filter.rs   # TOML 过滤器引擎
│   ├── tee.rs           # 输出旁路（tee）系统
│   ├── tracking.rs      # SQLite 跟踪系统
│   ├── telemetry.rs     # 遥测数据采集
│   ├── telemetry_cmd.rs # 遥测命令
│   ├── truncate.rs      # 输出截断工具
│   ├── display_helpers.rs # 显示辅助
│   └── utils.rs         # 通用工具函数
├── cmds/                # 按语言生态组织的命令过滤器
│   ├── cloud/           # 云工具 (aws, curl, psql, wget)
│   ├── dotnet/          # .NET 工具 (dotnet build/test/format)
│   ├── git/             # Git 工具 (git, gh, glab)
│   ├── go/              # Go 工具 (go, golangci-lint)
│   ├── js/              # JS/TS 工具 (eslint, tsc, next, prettier, vitest...)
│   ├── jvm/             # JVM 工具 (gradlew)
│   ├── python/          # Python 工具 (ruff, pytest, pip, mypy)
│   ├── ruby/            # Ruby 工具 (rake, rspec, rubocop)
│   ├── rust/            # Rust 工具 (cargo)
│   └── system/          # 系统工具 (find, grep, ls, env, tree...)
├── discover/            # AI 编码会话扫描
│   ├── lexer.rs         # 会话文件词法分析
│   ├── provider.rs      # 会话提供者 (Claude Code)
│   ├── registry.rs      # 命令分类注册表
│   ├── rules.rs         # 匹配规则
│   └── report.rs        # 报告生成
├── learn/               # 从反复错误中学习
│   ├── detector.rs      # 命令模式检测
│   └── report.rs        # 学习报告
├── hooks/               # AI 代理 Hook 系统
│   ├── init.rs          # Hook 初始化
│   ├── hook_cmd.rs      # Hook 命令
│   ├── hook_check.rs    # Hook 校验
│   ├── hook_audit_cmd.rs # Hook 审计
│   ├── rewrite_cmd.rs   # 命令重写
│   ├── verify_cmd.rs    # Hook 验证
│   ├── trust.rs         # 信任管理
│   ├── permissions.rs   # 权限管理
│   ├── integrity.rs     # 完整性校验
│   └── constants.rs     # Hook 常量
├── analytics/           # Token 节省分析
│   ├── gain.rs          # 收益计算
│   ├── cc_economics.rs  # Claude Code 经济分析
│   ├── ccusage.rs       # Claude Code 用量
│   └── session_cmd.rs   # 会话命令分析
├── parser/              # 输出解析
│   ├── types.rs         # 解析类型定义
│   └── formatter.rs     # 输出格式化
├── filters/             # TOML 内置过滤器定义 (60+ 个 .toml 文件)
└── discover/            # (同上，发现/推荐模块)
```

---

## 六阶段执行流程

每次 RTK 运行都遵循六个阶段：

```
用户命令输入
    │
    ▼
┌──────────────────────────────────────────────────────┐
│ 阶段 1: 命令行解析 (CLI Parsing)                      │
│                                                      │
│ • 使用 clap 解析主命令                                │
│ • 如 clap 解析失败，启用 fallback 模式                 │
│   (将未知命令作为子进程运行)                           │
│ • 支持多级子命令 (如 rtk hook check <agent> <cmd>)     │
│ • 特殊命令通过别名匹配 (gt → git, pnpm → js)           │
└──────────────────────────────────────────────────────┘
    │
    ▼
┌──────────────────────────────────────────────────────┐
│ 阶段 2: 命令路由 (Command Routing)                     │
│                                                      │
│ • 根据命令类型路由到对应的 cmds/* 模块                 │
│ • 如果命令不在已知列表中，走 fallback 直通              │
│ • 如果启用了 hook，尝试命令重写                        │
│   (如 git commit → rtk git commit)                   │
│ • 自动检测包管理器 (pnpm/yarn/npm, uv/pip)              │
└──────────────────────────────────────────────────────┘
    │
    ▼
┌──────────────────────────────────────────────────────┐
│ 阶段 3: 子进程执行 (Subprocess Execution)              │
│                                                      │
│ • 通过 core::runner::run() 创建子进程                  │
│ • 捕获 stdout 和 stderr                               │
│ • 支持三种执行模式:                                    │
│   - Filtered: 捕获全部输出后应用过滤器                  │
│   - Streamed: 逐行流式过滤                             │
│   - Passthrough: 原始输出直通                          │
│ • 支持 stdin 继承 (用于管道输入)                        │
└──────────────────────────────────────────────────────┘
    │
    ▼
┌──────────────────────────────────────────────────────┐
│ 阶段 4: 输出过滤 (Output Filtering)                   │
│                                                      │
│ • 根据命令类型选择过滤策略                              │
│ • 三种过滤方式:                                        │
│   1. Rust 代码过滤 (cmds/*/*_cmd.rs)                  │
│   2. TOML 模式过滤 (filters/*.toml)                   │
│   3. 通用内容过滤 (core/filter.rs)                    │
│ • 流式过滤支持逐行处理                                 │
│ • 异常时可选跳过过滤 (保留原始输出)                     │
└──────────────────────────────────────────────────────┘
    │
    ▼
┌──────────────────────────────────────────────────────┐
│ 阶段 5: 输出呈现 (Output Presentation)                │
│                                                      │
│ • 打印过滤后的输出到 stdout                            │
│ • 如启用 tee 功能，将原始输出保存到文件                  │
│ • 在过滤输出末尾添加 tee 提示 (显示文件路径)            │
│ • 保留原始退出码 (对 CI/CD 至关重要)                   │
└──────────────────────────────────────────────────────┘
    │
    ▼
┌──────────────────────────────────────────────────────┐
│ 阶段 6: 跟踪记录 (Tracking)                           │
│                                                      │
│ • 通过 TimedExecution 记录执行时间                     │
│ • 计算原始 Token 和过滤后 Token 数量                   │
│ • 记录到 SQLite 数据库 (history.db)                   │
│ • 支持按项目路径过滤统计                               │
│ • 自动清理超过 history_days 的旧记录                   │
└──────────────────────────────────────────────────────┘
```

---

## 核心模块详解

### `core::runner` — 通用命令执行骨架

提供统一的命令执行入口，封装了子进程创建、输出捕获、过滤、跟踪等完整流程。

关键结构：
- `RunMode`：三种执行模式枚举（Filtered / Streamed / Passthrough）
- `RunOptions`：运行选项（tee 标签、仅过滤 stdout、失败跳过过滤、继承 stdin 等）
- `run()`：核心函数，根据 RunMode 分发到不同的执行路径
- `run_filtered()` / `run_streamed()` / `run_passthrough()`：便捷包装

```rust
pub enum RunMode<'a> {
    Filtered(Box<dyn Fn(&str) -> String + 'a>),   // 捕获后过滤
    Streamed(Box<dyn StreamFilter + 'a>),           // 流式过滤
    Passthrough,                                     // 原始直通
}
```

### `core::stream` — 流式 I/O 处理核心

负责子进程的异步读取、管道数据路由和流式过滤。

关键抽象：
- `StreamFilter` trait — 流式过滤器的核心接口（feed_line / flush / on_exit）
- `BlockStreamFilter<H: BlockHandler>` — 基于块的流式过滤器，按块聚合后输出摘要
- `LineStreamFilter<H: LineHandler>` — 基于行的流式过滤器，逐行判断保留/跳过
- `RegexBlockFilter` — 基于正则表达式匹配的块过滤器，按开始/连续/结束模式过滤
- `run_streaming()` — 核心 I/O 函数，创建子进程并使用独立线程异步读取 stdout/stderr

管道数据路由逻辑：
```
子进程 stdout ──→ 线程1: 行读取器 ──→ 主通道 (用于过滤 + stdout 输出)
子进程 stderr ──→ 线程2: 行读取器 ──→ 直接输出到 stderr (不经过过滤)
```

### `core::filter` — 代码级过滤器

定义了三档过滤策略和语言感知的代码截断。

- `FilterLevel`：过滤级别枚举（NoFilter / Minimal / Aggressive）
- `FilterStrategy` trait：过滤器策略接口
- `Language`：编程语言枚举，支持从其文件扩展名推导
- `CommentPatterns`：各种语言的注释模式定义
- `smart_truncate()`：智能截断函数，优先保留函数签名、导入、pub/export 等关键行

### `core::toml_filter` — TOML 过滤器引擎

支持通过 TOML 格式定义过滤规则，无需编写 Rust 代码。

- `TomlFilterRegistry`：全局过滤器注册表（构建时由 `build.rs` 合并 `src/filters/*.toml`）
- `CompiledFilter`：编译后的过滤器定义，包含：
  - `match_command`：匹配的正则表达式模式
  - `strip_lines_matching` / `keep_lines_matching`：行过滤模式
  - `truncate_lines_at` / `head_lines` / `tail_lines`：截断模式
  - `replace` / `replace_lines`：替换模式
  - 内联测试用例
- `apply_filter()`：对 stdout 应用编译后的过滤器
- `find_matching_filter()`：根据命令名查找匹配的 TOML 过滤器

### `core::tracking` — SQLite 跟踪系统

使用 SQLite 数据库记录每次命令执行的 Token 节省数据。

- `Tracker`：跟踪器结构体，管理数据库连接
- `CommandRecord`：命令执行记录（命令名、原始/输出/节省 Token、执行时间等）
- `GainSummary`：收益汇总（总 Token、节省百分比、美元成本等）
- `TimedExecution`：计时执行包装器，自动计算耗时和 Token 节省
- 支持按项目路径过滤、按天/周/月统计、自动清理旧数据

### `core::tee` — 输出旁路系统

将原始命令输出保存到文件，供后续 AI 代理参考。

- `TeeMode`：三种模式（Disabled / FailuresOnly / Always）
- `tee_and_hint()`：保存原始输出并返回文件路径提示
- 文件存储在 `~/.rtk/tee/` 目录，自动清理旧文件
- 支持 UTF-8 安全截断（避免在字符中间截断）

---

## 命令模块体系

每个命令模块通常包含一个 `mod.rs` 和若干 `*_cmd.rs` 文件，按照统一的模式实现：

### 典型命令模块结构

```rust
// cmds/python/ruff_cmd.rs (示例)
pub fn run_ruff_check(...) -> Result<i32> {
    // 1. 构造命令
    let mut cmd = Command::new("ruff");
    cmd.arg("check").args(args);
    
    // 2. 通过 runner 执行
    run_filtered(cmd, "ruff", &args_str, |output| {
        // 3. 过滤逻辑
        let parsed: Vec<Violation> = serde_json::from_str(output)?;
        format!("Found {} violations", parsed.len())
    }, opts)
}
```

### 命令路由流程

```
main.rs: run_cli()
  │
  ├── Commands::Git { .. }     → cmds::git::diff_cmd::run()
  ├── Commands::Rust { .. }    → cmds::rust::cargo_cmd::run()
  ├── Commands::Python { .. }  → cmds::python::mod.rs 分发
  │     ├── ruff → ruff_cmd::run()
  │     ├── pytest → pytest_cmd::run()
  │     ├── pip → pip_cmd::run()
  │     └── mypy → mypy_cmd::run()
  ├── Commands::Js { .. }      → cmds::js::mod.rs 分发
  │     ├── tsc → tsc_cmd::run()
  │     ├── lint → lint_cmd::run()
  │     ├── next → next_cmd::run()
  │     └── ...
  ├── Commands::System { .. }  → cmds::system::mod.rs 分发
  │     ├── grep → grep_cmd::run()
  │     ├── find → find_cmd::run()
  │     ├── ls → ls.rs
  │     └── ...
  ├── Commands::Hook { .. }    → hooks:: hook_cmd / hook_check 等
  ├── Commands::Discover { .. } → discover::run()
  ├── Commands::Learn { .. }   → learn::run()
  ├── Commands::Gain { .. }    → analytics::gain
  └── fallback                 → run_fallback() 直通原始命令
```

### 包管理器自动检测

JS/TS 模块实现了包管理器自动检测，支持 pnpm、yarn、npm：

```rust
// 检测顺序:
// 1. pnpm-lock.yaml 存在 → 使用 pnpm exec
// 2. yarn.lock 存在     → 使用 yarn exec
// 3. 兜底               → 使用 npx --no-install
```

Python 模块同样支持 uv/pip 自动检测，优先使用 uv（如果可用）。

---

## 过滤器体系

RTK 提供三层过滤机制：

### 第一层：TOML 模式过滤器 (`filters/*.toml`)

用于通用命令的输出修剪，无需编写 Rust 代码。构建时由 `build.rs` 自动合并所有 TOML 文件。

TOML 过滤器结构：
```toml
schema_version = 1

[filters.terraform-plan]
match_command = "^terraform\\s+plan"
head_lines = 200
strip_lines_matching = [
    "^\s+│",
    "^\s+└",
    "Reading\\.\\.\\.",
]

[[tests.terraform-plan]]
name = "filter plan output"
input = """
Terraform used the selected providers...
"""
expected = """
Terraform used the selected providers...
"""
```

### 第二层：Rust 命令过滤器 (`cmds/*/*_cmd.rs`)

针对特定编程工具的输出进行定制过滤，过滤逻辑嵌入 Rust 代码。

主要策略：
| 策略 | 说明 | 典型节省 |
|------|------|---------|
| JSON 解析 | 解析 JSON 输出，提取关键结构 | 80%+ |
| 状态机 | 跟踪行状态，提取摘要 | 90%+ |
| 正则匹配 | 匹配并跳过/保留特定行 | 60-80% |
| 智能截断 | 保留关键行，截断冗余 | 70%+ |

### 第三层：通用内容过滤器 (`core/filter.rs`)

对所有命令输出通用的后处理：
- 注释剥离（按编程语言识别）
- 智能截断（保留函数签名、导入等关键结构）
- URL/路径缩短

### 过滤器优先级

```
命令匹配成功?
  ├── 是 → TOML 过滤器存在?
  │         ├── 是 → 应用 TOML 过滤器
  │         └── 否 → 应用 Rust 代码过滤器
  └── 否 → 直通 (passthrough)
            (或应用通用内容过滤器，取决于配置)
```

---

## 流式处理架构

### 核心抽象

```
StreamFilter trait
    │
    ├── BlockStreamFilter<H: BlockHandler>
    │     • 按块聚合：找到起始行后，连续收集直到块结束
    │     • 输出摘要 (format_summary)，而非完整内容
    │     • 适用场景：编译器构建输出、测试运行输出
    │
    └── LineStreamFilter<H: LineHandler>
          • 逐行处理：每行独立判断保留或跳过
          • 可累加状态 (observe_line)
          • 适用场景：lint 输出、日志过滤
```

### BlockHandler trait

```rust
pub trait BlockHandler {
    fn should_skip(&mut self, line: &str) -> bool;          // 是否跳过此行
    fn is_block_start(&mut self, line: &str) -> bool;       // 块开始?
    fn is_block_continuation(&mut self, line: &str, block: &[String]) -> bool;  // 块连续?
    fn format_summary(&self, exit_code: i32, raw: &str) -> Option<String>;  // 输出摘要
}
```

### 流式管道架构

```
子进程 (stdout)
    │
    ▼
┌──────────────────┐
│ 独立读取线程      │  从子进程 stdout 读取行
│ (io::BufReader)   │
└────────┬─────────┘
         │ 行数据 (String)
         ▼
┌──────────────────┐
│ StreamFilter     │  逐行或逐块过滤
│ (feed_line)      │
└────────┬─────────┘
         │ 过滤后的行 (Option<String>)
         ▼
┌──────────────────┐
│ stdout 输出      │  立即输出到终端
│ (print!)         │
└──────────────────┘

子进程结束时:
┌──────────────────┐
│ flush()          │  输出剩余缓冲
│ on_exit()        │  输出摘要/统计
└──────────────────┘
```

---

## 跟踪与遥测系统

### SQLite Schema

```sql
CREATE TABLE commands (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    command       TEXT NOT NULL,
    rtk_command   TEXT NOT NULL,
    input_tokens  INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    saved_tokens  INTEGER NOT NULL,
    exec_time_ms  INTEGER NOT NULL,
    timestamp     DATETIME DEFAULT CURRENT_TIMESTAMP,
    project_path  TEXT,
    exit_code     INTEGER
);

CREATE TABLE parse_failures (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    command    TEXT NOT NULL,
    error_line TEXT,
    timestamp  DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### Token 估算

- Token 计数使用简单估算：`text.len() / 4`（平均每 4 个字符 ≈ 1 token）
- 记录字段：`input_tokens`（原始输出）、`output_tokens`（过滤后输出）、`saved_tokens`（节省量）

### 收益报告 (`rtk gain`)

```
$ rtk gain
┌──────────────────────────────────────────────────────┐
│ Token 节省统计                                        │
├──────────────────────────────────────────────────────┤
│ 总节省 Token:    1,234,567  (78.3%)                   │
│ 总成本节省:      $12.35                               │
│ 最近 7 天:       +234,567  (82.1%)                    │
│ 执行次数:        1,234 次                              │
└──────────────────────────────────────────────────────┘
```

---

## Hook 系统

RTK 的 Hook 系统允许它自动拦截 AI 编码代理中的命令执行，透明地应用过滤。

### 支持的 AI 代理

Claude Code、Cursor、Windsurf、Cline、Kilocode、Copilot、Codex、OpenCode、Hermes、Pi 等 15+ 种 AI 编码代理。

### Hook 工作原理

```
AI 代理执行命令
    │
    ▼
┌──────────────────────────────────────────────────────┐
│ Hook 触发                                            │
│                                                      │
│ 1. hook check: 分析命令，检查是否可以受益于 RTK       │
│ 2. 如果可以，重写命令:                                │
│    git status → rtk git status                       │
│ 3. 如果不行，直通原始命令                              │
└──────────────────────────────────────────────────────┘
```

### 命令重写引擎

- 自动检测 RTK 支持的已知命令
- 透明前缀剥离（如 `docker exec mycontainer`、`noglob`、`builtin`）
- 包管理器感知（`pnpm exec -- eslint` → `rtk eslint`）
- 支持 `RTK_DISABLED` 环境变量绕过

### Hook 安装

`rtk init` 命令支持一次性安装 (`-g`) 和按代理安装 (`--agent claude`) 两种方式。

---

## 配置系统

### 配置文件路径

`~/.rtk/config.toml`（由 `dirs` crate 解析到标准配置目录）

### 配置结构

```toml
[tracking]
enabled = true
history_days = 90

[display]
colors = true
emoji = true
max_width = 120

[filters]
ignore_dirs = ["node_modules", ".git"]
ignore_patterns = ["*.lock"]

[tee]
mode = "failures_only"  # disabled | failures_only | always
max_files = 20
max_file_size = 1048576

[telemetry]
enabled = false

[hooks]
exclude_commands = ["curl"]
transparent_prefixes = ["docker exec mycontainer"]

[limits]
max_output_chars = 100000
```

### 初始化流程

```
rtk init
  │
  ├── 创建 ~/.rtk/ 目录
  ├── 写入默认 config.toml
  ├── 创建 SQLite 数据库 (history.db)
  ├── 创建 filters.toml (如果存在内置过滤器)
  └── 安装 Hook (如果指定 --agent)
```

---

## 构建优化

### 构建配置 (`Cargo.toml`)

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### 构建时过滤器合并 (`build.rs`)

构建时自动扫描 `src/filters/` 目录下的所有 `.toml` 文件，合并为一个内置过滤器文件，检测重复名称。

### Windows 特定优化

为 Windows 构建设置 8MB 线程栈（`/STACK:8388608`），避免 clap 命令图初始化时栈溢出。

### 性能指标

| 指标 | 值 |
|------|-----|
| 二进制大小 (strip 后) | ~5MB |
| 启动时间 | < 50ms |
| 输出处理延迟 | < 10ms (流式) |
| 内存峰值 | < 50MB |
| Token 节省 | 60-90% |

---

## 关键架构决策

### 为什么用 Rust？
- 性能：零成本抽象，接近 C 的性能
- 安全性：所有权系统防止内存错误
- 生态：clap (CLI)、serde (序列化)、rusqlite (数据库)

### 为什么用 SQLite？
- 零配置：无需外部数据库服务
- 嵌入式：直接集成到二进制文件
- 可靠性：ACID 事务保证数据一致性

### 为什么用 anyhow 处理错误？
- 简洁：`anyhow::Result<T>` 统一错误类型
- 上下文：`.with_context(|| format!("..."))` 提供丰富的错误消息
- 兼容性：自动包装所有实现 `std::error::Error` 的类型

### 为什么用 clap 解析命令行？
- 派生宏：通过 `#[derive(Parser)]` 声明式定义参数
- 自动补全：支持 shell 自动补全生成
- 错误处理：自动格式化友好错误消息

### 为什么用 TOML 做过滤器？
- 声明式：非 Rust 开发者也可以编写过滤器
- 安全：无需编译，构建时静态嵌入
- 可测试：每个过滤器附带内联测试用例

---

## 扩展指南

### 添加一个新的 TOML 过滤器

1. 在 `src/filters/` 下创建 `<tool>.toml`
2. 定义 `match_command`（正则匹配命令名）
3. 添加过滤规则（strip / keep / truncate / replace）
4. 添加内联测试用例
5. 运行 `cargo test` 验证

### 添加一个新的 Rust 命令过滤器

1. 在对应语言生态目录下创建 `<tool>_cmd.rs`
2. 实现过滤函数（`fn run_xxx() -> Result<i32>`）
3. 在 `mod.rs` 中注册分发
4. 在 `main.rs` 中注册命令枚举
5. 运行 `cargo check` 验证

### 添加一个新的语言生态模块

1. 在 `src/cmds/` 下创建 `<lang>/` 目录
2. 创建 `mod.rs` 和命令文件
3. 在 `src/cmds/mod.rs` 中注册模块
4. 在 `main.rs` 的 `Commands` 枚举中添加新变体
5. 实现分发逻辑

---

## 资源

- [官方文档](https://www.rtk-ai.app)
- [GitHub 仓库](https://github.com/rtk-ai/rtk)
- [架构参考](ARCHITECTURE.md)（英文版）
- [编码规范](CODING_PRACTICES.md)
- [技术说明](TECHNICAL.md)
