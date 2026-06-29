# TUI 格式输出处理设计方案

## 1. 概述

### 1.1 背景

monorepo 工具（如 Turborepo、Nx）和部分 CLI 工具在终端环境中会输出 TUI（Terminal User Interface）格式的内容，包含 Unicode 框线字符、ANSI 转义码、进度动画、缓存状态等装饰性信息。这些噪声会严重干扰构建错误分析器的解析准确性。

当前 `multi-analyzer` 在 `core/utils.rs` 和 `plugins/npm/parser.rs` 中已分别实现了部分 TUI 过滤能力，但存在代码重复、管道不完整、缺少声明式配置等问题。

### 1.2 参考来源

设计参考 `ref/rtk-develop/` 中 RTK 项目对 TUI 输出的三层处理策略：
- **TOML 声明式过滤器** (`src/filters/turbo.toml` + `src/core/toml_filter.rs`)
- **流式过滤引擎** (`src/core/stream.rs`)
- **Runner 模式路由** (`src/core/runner.rs`)

### 1.3 目标

1. 统一 TUI 预处理逻辑，消除 `utils.rs` 与各 parser 之间的重复
2. 将 `OutputPostProcessor` 升级为与 RTK 对齐的 8 阶段管道
3. 支持 TOML 声明式配置，新工具支持只需添加配置文件

---

## 2. RTK TUI 处理策略分析

### 2.1 三层递进策略

```
用户执行: rtk turbo run lint
            │
            ▼
     Clap CLI 解析
       ╱          ╲
   已知命令      未知命令 (如 turbo)
   (Rust原生)      │
              run_fallback()
                  │
         ┌────────┴────────┐
         │ TOML Filter 查找 │
         │ 1. .rtk/filters.toml
         │ 2. ~/.config/rtk/filters.toml
         │ 3. 内置 (59个,编译嵌入)
         └────────┬────────┘
                  │
            匹配？──否──→ Passthrough (TTY继承)
                  │是
                  ▼
         apply_filter() 8阶段管道
```

| 策略 | 适用场景 | 机制 |
|------|---------|------|
| Passthrough | 需要终端交互的 TUI 工具 | stdin/stdout/stderr 全 inherit |
| TOML Filter | turbo, terraform, ps 等 | 声明式 8 阶段管道 |
| Rust Native | git, cargo, npm 等 | `BlockHandler`/`LineHandler` 状态机 |

### 2.2 turbo.toml 过滤器剖析

```toml
[filters.turbo]
description = "Compact Turborepo output — strip cache status noise, keep task results"
match_command = "^turbo\\b"
strip_ansi = true
strip_lines_matching = [
  "^\\s*$",                                    # 空行
  "^\\s*cache (hit|miss|bypass)",              # 缓存噪音
  "^\\s*\\d+ packages in scope",               # 范围统计
  "^\\s*Tasks:\\s+\\d+",                       # 任务汇总
  "^\\s*Duration:\\s+",                        # 耗时
  "^\\s*Remote caching (enabled|disabled)",    # 配置状态
]
truncate_lines_at = 150
max_lines = 50
on_empty = "turbo: ok"
```

**设计原则**：
- **保留核心**：子任务的实际输出（lint 错误、构建日志）是唯一有价值的内容
- **剥离装饰**：框线、缓存标记、统计摘要全部丢弃
- **降级保护**：`on_empty` 确保过滤为空时返回确认信号，而非空字符串
- **上限保护**：`max_lines` / `truncate_lines_at` 防止输出爆炸

### 2.3 8 阶段过滤管道（toml_filter.rs:436-533）

```
strip_ansi → replace → match_output(短路) → strip/keep_lines → truncate_lines_at → head/tail_lines → max_lines → on_empty
```

各阶段说明：

| 阶段 | 功能 | 关键设计 |
|------|------|---------|
| 1. strip_ansi | 去除 ANSI 转义码 | 对所有 TUI 输出必须先做 |
| 2. replace | 逐行正则替换（链式） | 支持反向引用 `$1` |
| 3. match_output | 全量输出短路匹配 | 首条匹配即返回，`unless` 防误判 |
| 4. strip/keep_lines | 行级过滤（互斥） | 使用 `RegexSet` 批量匹配 |
| 5. truncate_lines_at | 每行截断至 N 字符 | Unicode 安全截断 |
| 6. head/tail_lines | 首/尾 N 行保留 | 含省略消息 `... (N lines omitted)` |
| 7. max_lines | 绝对行数上限 | 在 head/tail 之后应用 |
| 8. on_empty | 空结果降级消息 | 结果为空时返回预设字符串 |

### 2.4 match_output 短路机制

```rust
// 如果整个输出匹配 "BUILD SUCCESS"，直接返回确认消息
match_output = [
  { pattern = "BUILD SUCCESS", message = "Build succeeded" }
]
// unless 防止在有错误时误短路
match_output = [
  { pattern = ".*", message = "all good", unless = "(?i)error|fail" }
]
```

---

## 3. 当前项目能力评估

### 3.1 已实现能力

| 能力 | 实现位置 | 行数 |
|------|---------|------|
| ANSI 剥离 | `core/utils.rs:strip_ansi()` | 24-26 |
| TUI 框线过滤 | `core/utils.rs:filter_tui_frame_lines()` | 84-107 |
| TUI 框线检测 | `core/utils.rs:is_tui_border_line()` | 73-78 |
| 框线字符集 | `core/utils.rs:TUI_BORDER_CHARS` | 8-16 |
| 噪音行过滤 | `core/utils.rs:filter_noise_lines()` | 41-54 |
| 保留行过滤 | `core/utils.rs:keep_matching_lines()` | 57-70 |
| 行截断 | `core/utils.rs:truncate_output()` | 112-120 |
| 后处理管道 | `core/utils.rs:OutputPostProcessor` | 124-229 |
| TOML 配置类型 | `config/modules/filter.rs:FilterConfig` | - |

### 3.2 存在的问题

| 问题 | 严重程度 | 说明 |
|------|---------|------|
| TUI 逻辑重复 | 高 | `NpmParser` 硬编码了与 `utils.rs` 重复的框线字符检测（164-267行、275-317行） |
| 管道不完整 | 中 | `OutputPostProcessor` 缺少 `replace`、`match_output`、`on_empty` 阶段 |
| 缺少声明式配置 | 中 | turbo 的过滤规则硬编码在 NpmParser 中，其他 parser 无法复用 |
| 仅缓冲模式 | 低 | 无流式过滤能力，大规模输出时内存占用高 |
| 缺少短路机制 | 中 | 成功后仍需遍历所有阶段，无法快速返回 |

---

## 4. 设计方案

### 4.1 架构总览

```
命令输出（含TUI噪声）
    │
    ▼
┌──────────────────────────────────────┐
│        OutputPostProcessor           │
│  (升级为 8 阶段管道 + TOML配置驱动)   │
│                                      │
│  strip_ansi → replace →              │
│  match_output(短路) →                │
│  strip_tui_frames →                  │
│  strip/keep_lines →                  │
│  truncate_lines_at →                 │
│  head/tail_lines →                   │
│  max_lines → on_empty                │
└──────────────────────────────────────┘
    │
    ▼
各 Parser 解析纯净输出
```

### 4.2 第一阶段：消除重复，统一 TUI 预处理（P0）

**目标**：NpmParser 中的 TUI 处理逻辑全部委托给 `OutputPostProcessor`。

**改动范围**：

1. **`core/utils.rs`** — `OutputPostProcessor` 增加 `replace_patterns` 和 `on_empty_message` 字段：

```rust
pub struct OutputPostProcessor {
    pub strip_ansi: bool,
    pub strip_tui_frames: bool,
    pub replace_patterns: Vec<(String, String)>,  // 新增：逐行替换规则
    pub short_circuits: Vec<ShortCircuitRule>,     // 新增：短路匹配规则
    pub max_lines: Option<usize>,
    pub max_line_length: Option<usize>,
    pub noise_patterns: Vec<String>,
    pub keep_patterns: Vec<String>,
    pub on_empty_message: Option<String>,          // 新增：空结果降级消息
}

pub struct ShortCircuitRule {
    pub pattern: String,            // 匹配模式
    pub message: String,            // 匹配时返回的消息
    pub unless: Option<String>,     // 排除条件
}
```

2. **`core/utils.rs`** — `process()` 方法扩展为完整的 8 阶段管道：

```
strip_ansi → replace → short_circuit → strip_tui_frames → noise/keep → truncate_lines → max_lines → on_empty
```

3. **`plugins/npm/parser.rs`** — 在 `parse()` 入口调用 `OutputPostProcessor`，删除内联的 TUI 检测逻辑（`extract_package_and_content` 中的框线判断、`strip_turbo_prefixes` 中的重复过滤）。

4. **`plugins/npm/parser.rs`** — `extract_package_and_content` 仅保留包名前缀剥离逻辑，移除所有 TUI 框线/缓存/更新通知检测。

### 4.3 第二阶段：TOML 声明式配置（P1）

**目标**：参考 RTK 的 `filters/*.toml`，支持 TOML 配置文件驱动过滤规则。

**配置格式**：

```toml
# .analyzer/filters.toml
schema_version = 1

[filters.turbo]
description = "Strip Turborepo TUI decoration, keep task results"
match_command = "^(turbo|pnpm exec turbo|npx turbo)\\b"
strip_ansi = true
strip_tui_frames = true
strip_lines_matching = [
  "^\\s*cache (hit|miss|bypass)",
  "^\\s*\\d+ packages in scope",
  "^\\s*Tasks:\\s+\\d+",
  "^\\s*Duration:\\s+",
  "^\\s*Remote caching (enabled|disabled)",
  "^\\s*Cached:\\s+",
  "^\\s*Time:\\s+",
  "^\\s*Failed:\\s+",
  "^\\s*ERROR\\s+run failed:",
  "^(╭|╰|╮|╯).*",           # TUI 框线
  "^(┌|└|─).*",             # TUI 任务框
]
replace = [
  { pattern = "^[^:]+:[^:]+:\\s*", replacement = "" }  # 剥离 "web:lint:" 前缀
]
max_lines = 100
truncate_lines_at = 200
on_empty = "turbo: all tasks completed successfully"

[[tests.turbo]]
name = "strips cache noise, keeps lint errors"
input = """
web:lint: 
web:lint: > web@1.0.0 lint
web:lint: > eslint src/
web:lint:    4:7   error    'x' is assigned but never used
 Tasks:    1 successful, 1 total
"""
expected = "    4:7   error    'x' is assigned but never used"
```

**加载优先级**（与 RTK 一致）：

```
1. .analyzer/filters.toml          — 项目本地
2. ~/.config/analyzer/filters.toml — 用户全局
3. 内置默认过滤器                   — 编译时嵌入
4. 直通（无过滤）                    — 无匹配时
```

**新增文件**：

| 文件 | 用途 |
|------|------|
| `src/config/filter_registry.rs` | TOML 过滤器注册表和查找 |
| `src/config/filter_compiler.rs` | TOML → `OutputPostProcessor` 编译 |
| `src/filters/turbo.toml` | 内置 turbo 过滤器 |
| `src/filters/make.toml` | 内置 make 过滤器（预留） |
| `build.rs` | 编译时拼接 `src/filters/*.toml` |

### 4.4 第三阶段：流式过滤模式（P2，预留）

**目标**：支持逐行实时过滤，降低大输出场景的内存占用。

**设计方案**（参考 RTK `stream.rs`）：

```rust
pub enum FilterMode<'a> {
    Buffered(Box<dyn Fn(&str) -> String + 'a>),   // 当前模式
    Streaming(Box<dyn LineFilter + 'a>),            // 新增：逐行过滤
    Passthrough,                                     // 完全不拦截
}

pub trait LineFilter {
    fn feed_line(&mut self, line: &str) -> Option<String>;  // 返回过滤后的行
    fn flush(&mut self) -> Vec<String>;                      // 尾部输出
}
```

`CommandBuilder` 增加 `execute_streaming()` 方法，通过 `mpsc::channel` 在两个线程中分别读取 stdout/stderr，主线程逐行处理。

### 4.5 各 Parser 的改造清单

| Parser | 现状 | 改造后 |
|--------|------|--------|
| `npm/parser.rs` | 内联 TUI 检测 + 框线过滤 + 前缀剥离 | 移除 TUI 逻辑，仅保留 ESLint 格式解析；预处理委托 `OutputPostProcessor` |
| `cargo/parser.rs` | 无 TUI 处理 | 增加 `OutputPostProcessor` 预处理入口（预留） |
| `go/parser.rs` | 无 TUI 处理 | 增加 `OutputPostProcessor` 预处理入口（预留） |
| `cpp/parser.rs` | 无 TUI 处理 | 增加 `OutputPostProcessor` 预处理入口（预留） |
| `mypy/parser.rs` | 无 TUI 处理 | 增加 `OutputPostProcessor` 预处理入口（预留） |

统一入口模式：

```rust
// 每个 parser 的 parse() 方法统一改为：
pub fn parse(&self, raw_output: &str) -> ParseResult<Vec<Issue>> {
    let processor = self.get_post_processor();  // 从配置/Toml 构建
    let clean = processor.process(raw_output);
    self.parse_clean_output(&clean)
}
```

---

## 5. 实施计划

### Phase 1：统一 TUI 预处理（1-2 天）

| # | 任务 | 涉及文件 | 测试验证 |
|---|------|---------|---------|
| 1.1 | `OutputPostProcessor` 增加 `replace`、`short_circuit`、`on_empty` 字段 | `core/utils.rs` | 单元测试 |
| 1.2 | `process()` 扩展为完整 8 阶段管道 | `core/utils.rs` | 单元测试 |
| 1.3 | NpmParser 移除内联 TUI 检测，改用 `OutputPostProcessor` | `plugins/npm/parser.rs` | `turbo_tui_integration_tests.rs` 全量通过 |
| 1.4 | 添加 `OutputPostProcessor` 的默认 turbo 配置工厂方法 | `core/utils.rs` | 集成测试 |

### Phase 2：TOML 配置系统（2-3 天）

| # | 任务 | 涉及文件 |
|---|------|---------|
| 2.1 | 实现 TOML 配置反序列化类型 | `config/filter_registry.rs` |
| 2.2 | 实现 TOML → `OutputPostProcessor` 编译 | `config/filter_compiler.rs` |
| 2.3 | 实现三优先级加载（项目/全局/内置） | `config/filter_registry.rs` |
| 2.4 | 创建内置过滤器文件 | `src/filters/turbo.toml` |
| 2.5 | build.rs 编译时拼接 | `build.rs` |
| 2.6 | 各 Parser 接入 TOML 配置 | `plugins/*/parser.rs` |

### Phase 3：流式过滤（预留，3-5 天）

| # | 任务 | 涉及文件 |
|---|------|---------|
| 3.1 | 定义 `LineFilter` trait | `core/stream.rs`（新建） |
| 3.2 | `CommandBuilder` 增加 `execute_streaming()` | `core/command.rs` |
| 3.3 | 迁移 Cargo/NPM Analyzer 适配流式接口 | `plugins/cargo/`、`plugins/npm/` |

### 依赖关系

```
Phase 1 ──→ Phase 2 ──→ Phase 3
```

Phase 1 和 Phase 2 可部分并行（1.1-1.2 完成后即可开始 Phase 2）。

---

## 6. 测试策略

### 6.1 单元测试

- `OutputPostProcessor` 每个阶段的独立测试
- `filter_tui_frame_lines` 各种框线格式覆盖
- `strip_ansi` 复杂 ANSI 序列覆盖
- TOML 配置解析边界测试

### 6.2 集成测试

- 复用现有 `tests/turbo_tui_integration_tests.rs` 的全部测试用例
- 新增：构造含 TUI 框线的多种 monorepo 工具输出格式
- 新增：验证 `on_empty` 降级行为

### 6.3 回归测试

- `cargo test` 全部通过
- NPM Parser 的 1372 行解析逻辑在改造后行为不变

---

## 7. 关键设计决策

1. **优先级：修正现有重复 > 新增能力**
   先统一 NpmParser 与 utils.rs 的 TUI 处理，再扩展管道。避免在混乱基座上叠加新功能。

2. **OutputPostProcessor 作为统一入口**
   所有 Parser 的 `parse()` 方法都通过 `OutputPostProcessor` 预处理原始输出，确保 TUI 剥离行为一致。

3. **TOML 配置为可选项**
   Parser 可以硬编码默认的 `OutputPostProcessor` 配置作为 fallback；TOML 配置用于覆盖和扩展。不影响现有代码路径。

4. **流式过滤延迟到 Phase 3**
   当前缓冲模式满足需求，流式模式仅在需要实时反馈或超大输出时启用。

5. **内联测试跟随过滤器**
   参考 RTK 的 `[[tests.<filter-name>]]` 模式，TOML 过滤器中内嵌测试用例，确保过滤器行为的正确性可验证。
