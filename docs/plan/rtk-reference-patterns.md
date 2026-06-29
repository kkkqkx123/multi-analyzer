# RTK 参考模式

本文档记录 RTK 项目中可直接复用的设计模式，供拦截命令功能实现时参考。

## 1. 命令重写引擎

### 1.1 核心函数签名

来源：`ref/rtk-develop/src/discover/registry.rs`

```rust
/// 重写单个命令段（已拆分后的）
pub fn rewrite_command(
    cmd: &str,
    excluded: &[String],        // 黑名单 (排除列表)
    transparent_prefixes: &[String], // 透明前缀 (不改写的命令前缀)
) -> Option<String>;

/// 对复合命令的每个 segment 独立重写
/// "cmd1 && cmd2" → 分别 rewrite，仅改写左侧
pub fn rewrite_compound(cmd: &str, excluded, transparent) -> Option<String>;
```

### 1.2 分类结果枚举

```rust
pub enum Classification {
    Supported {
        rtk_equivalent: &'static str,
        category: &'static str,
        estimated_savings_pct: f64,
        status: RtkStatus,
    },
    Unsupported { base_command: String },
    Ignored,
}
```

> 映射到 multi-analyzer: `Classification::Matched { tech_stack, subcommand, extra_args }` / `Unmatched { base_command }`

### 1.3 规则表数据结构

```rust
struct RtkRule {
    pattern: &'static str,          // 匹配正则
    rtk_cmd: &'static str,          // rtk 等价命令
    rewrite_prefixes: &'static [&'static str],
    category: &'static str,
    savings_pct: f64,
    subcmd_savings: &'static [(&'static str, f64)],
    subcmd_status: &'static [(&'static str, RtkStatus)],
}
```

> 映射到 multi-analyzer: 将 `rtk_cmd` 换为 `tech_stack: TechStack` + `subcommand_template`，去掉 savings 相关字段。

### 1.4 复合命令运算符处理

```rust
// 运算符分割策略
// && || ; → 两侧都改写
// |       → 仅改写左侧 (管道右侧依赖左侧格式)
// &       → 左侧改写 (后台命令)

pub fn split_on_operators(cmd: &str) -> Vec<Segment>;
```

---

## 2. 命令执行 Runner

来源：`ref/rtk-develop/src/core/runner.rs`

### 2.1 运行模式枚举

```rust
pub enum RunMode<'a> {
    Filtered(Box<dyn Fn(&str) -> String + 'a>),  // 批量过滤
    Streamed(Box<dyn StreamFilter + 'a>),          // 流式过滤
    Passthrough,                                    // 透传
}
```

> 映射到 multi-analyzer: 当前 `CommandBuilder::execute()` 是批处理模式，需要增加 `Streamed` 模式用于实时输出场景。

### 2.2 流式执行

```rust
pub fn run_streaming(
    cmd: &mut Command,
    stdin_mode: StdinMode,
    filter_mode: FilterMode,
) -> Result<StreamResult>;
```

---

## 3. 退出码协议

来源：`ref/rtk-develop/src/hooks/rewrite_cmd.rs`

| 退出码 | stdout | 含义 |
|--------|--------|------|
| 0 | rewritten | Rewrite 成功且 Allow → 自动执行 |
| 1 | (none) | 无匹配 → 透传 |
| 2 | (none) | Deny → 阻止 |
| 3 | rewritten | Ask/Default → 改写但用户确认 |

> 映射到 multi-analyzer: 退出码含义调整为：
> - 0 → 成功
> - 1 → 无匹配 / 执行失败
> - 2 → 子命令不支持

---

## 4. JSON 输出数据格式

来源：`ref/rtk-develop/src/hooks/hook_cmd.rs`

### 4.1 Claude Code 格式

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "RTK auto-rewrite",
    "updatedInput": {
      "command": "rtk git status",
      "timeout": 30000,
      "description": "Check repo status"
    }
  }
}
```

### 4.2 Cursor 格式

```json
{
  "continue": true,
  "permission": "allow",
  "updated_input": {
    "command": "rtk git status"
  }
}
```

### 4.3 无改写时返回

```json
{}
```

> 映射到 multi-analyzer: 这些格式用于 AI Agent hook 层（未来扩展预留），当前 CLI 模式不直接使用这些 JSON 协议，但 raw/raw-json 输出格式的字段命名保持一致。

---

## 5. TOML Filter 配置格式

来源：`ref/rtk-develop/.rtk/filters.toml` 和 `src/filters/*.toml`

### 5.1 Schema

```toml
schema_version = 1

[filters.<name>]
description = "Description text"
match_command = "^(regex pattern)"    # 匹配命令的正则
strip_ansi = true                      # 剥离 ANSI
replace = [
  { pattern = "regex", replacement = "replacement" }
]
match_output = [
  { pattern = "BUILD SUCCESS", message = "Build succeeded" }
]
strip_lines_matching = ["^\\[INFO\\] Installing"]
keep_lines_matching = ["ERROR", "WARN"]
truncate_lines_at = 200
head_lines = 50
tail_lines = 20
max_lines = 30
on_empty = "all good"
filter_stderr = false
```

### 5.2 8 阶段处理管道

1. `strip_ansi` - 去除 ANSI 转义码
2. `replace` - 逐行正则替换
3. `match_output` - 全量匹配短路
4. `strip_lines_matching` / `keep_lines_matching` - 行过滤
5. `truncate_lines_at` - 行截断
6. `head_lines` / `tail_lines` - 头尾保留
7. `max_lines` - 绝对上限
8. `on_empty` - 空结果替换消息

> 映射到 multi-analyzer: 当前 `FilterConfig` 已支持部分过滤能力（noise_patterns / keep_patterns）。TOML filter 作为可选增强，可在后续阶段添加。

---

## 6. 支持的 AI Agent 类型 (参考，暂不实现)

RTK 使用三层集成深度：

| 层级 | 机制 | Agent |
|------|------|-------|
| Full Hook | Shell 脚本或 Rust 二进制通过 Agent API 拦截 | Claude Code, Cursor, Copilot, Gemini CLI |
| Plugin | TS/JS/Python 插件系统中运行 | OpenCode, Hermes, Pi |
| Rules File | Prompt 级指令文件 | Cline, Windsurf, Codex CLI, KiloCode |

> 当前 multi-analyzer 仅专注 CLI 模式。

---

## 7. 关键文件路径速查

| RTK 文件 | 参考用途 |
|----------|---------|
| `ref/rtk-develop/src/discover/rules.rs` | 规则表数据结构和 RULES 定义 |
| `ref/rtk-develop/src/discover/registry.rs` | `rewrite_command()` 实现 |
| `ref/rtk-develop/src/discover/lexer.rs` | 复合命令拆分 (TokenKind, split_on_operators) |
| `ref/rtk-develop/src/core/runner.rs` | RunMode, run(), run_streaming() |
| `ref/rtk-develop/src/hooks/rewrite_cmd.rs` | 退出码协议 |
| `ref/rtk-develop/src/hooks/hook_cmd.rs` | JSON 输出格式 |
| `ref/rtk-develop/src/hooks/permissions.rs` | 权限模型 |
| `ref/rtk-develop/src/core/toml_filter.rs` | TOML 过滤引擎 (8 阶段管道) |
| `ref/rtk-develop/hooks/opencode/rtk.ts` | OpenCode 插件实现参考 |
