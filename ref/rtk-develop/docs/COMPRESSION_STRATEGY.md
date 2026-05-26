# RTK 命令输出压缩策略分析

## 概述

RTK 对命令输出进行压缩，核心目标是**在保留关键信息的前提下，最小化输出内容的 Token 数量**，从而降低 LLM 上下文消耗。压缩策略覆盖了 60+ 个常用命令，平均节省 Token 60-90%。

文档从**命名压缩**和**内容压缩**两个维度展开分析。

---

## 1. 命名压缩

RTK 通过别名系统将不同写法/工具的命令映射到同一个过滤器，实现复用的同时保持用户习惯。

### 1.1 命令别名映射

| 原始命令 | RTK 路由目标 | 说明 |
|---------|-------------|------|
| `gt status` | → `crate::git::run(GitCommand::Status)` | Graphite CLI (gt) 透明转接到 git 过滤器 |
| `gt diff/show/add/push/pull/fetch/stash` | → 对应 git 命令通道 | gt 未知子命令全部透传给 git |
| `k get pods` | → `kubectl get pods` | `k` 是 `kubectl` 的通用缩写 |
| `pnpm eslint src/` | → `rtk eslint` | 包管理器前缀剥离 |
| `yarn tsc` | → `rtk tsc` | 同上 |
| `npx next build` | → `rtk next build` | npx 前缀剥离 |

**实现机制**：`main.rs` 中的 `try_parse_*` 系列函数（如 `try_parse_git`、`try_parse_kubectl`）尝试将原始命令解析为已知子命令，匹配后映射到对应过滤器。

### 1.2 透明前缀剥离

```rust
// hooks/rewrite_cmd.rs 中的内置前缀
SHELL_PREFIX_BUILTINS = ["noglob", "command", "builtin", "exec", "nocorrect"]
// 用户自定义前缀 (config.toml)
transparent_prefixes = ["docker exec mycontainer", "direnv exec ."]
```

例如：
- `noglob git diff` → 剥离 `noglob` → `git diff` → 过滤后输出
- `docker exec mycontainer git status` → 剥离 `docker exec mycontainer` → `git status` → 过滤后输出

### 1.3 语言生态命名组织

```
src/cmds/
├── git/        → git, gh (Github CLI), glab (Gitlab CLI), gt (Graphite CLI)
├── js/         → tsc, next, pnpm, prettier, prisma, vitest, playwright, lint (eslint)
├── python/     → ruff, pytest, pip, mypy
├── rust/       → cargo (build/test/clippy/check/install/nextest)
├── system/     → grep, find, env, ls, wc, pipe, json, format, log
├── go/         → go, golangci-lint
├── ruby/       → rake, rspec, rubocop
├── dotnet/     → dotnet
├── jvm/        → gradlew
├── cloud/      → aws, curl, psql, wget
```

### 1.4 TOML 过滤器命名匹配

TOML 过滤器使用 **正则表达式** 匹配命令名：

```toml
# 精确匹配
match_command = "^make\\b"            # 匹配 "make"，不匹配 "makefile"
match_command = "^terraform\\s+plan"  # 匹配 terraform plan（含空格）
match_command = "^g(cc|\\+\\+)\\b"    # 匹配 gcc 和 g++
match_command = "^brew\\s+(install|upgrade)\\b"  # 匹配 brew install 或 upgrade
match_command = "^mvn\\s+(compile|package|clean|install)\\b"  # 匹配指定 mvn 子命令
```

---

## 2. 内容压缩策略

内容压缩分为 **8 种核心策略**，可组合使用。

### 2.1 噪音行剥离

最基础的策略：识别并移除对 AI 无价值的行。

**TOML 实现示例**（`make.toml`）：
```toml
strip_lines_matching = [
  "^make\\[\\d+\\]:",       # "make[1]: Entering directory..." 编译指示
  "^\\s*$",                  # 空行
  "^Nothing to be done",     # "Nothing to be done for 'all'"
]
```

**效果**：
```
输入 (100 tokens):
  make[1]: Entering directory '/home/user'
  gcc -O2 foo.c
  make[1]: Leaving directory '/home/user'

输出 (10 tokens):
  gcc -O2 foo.c

节省: ~90%
```

**典型噪音行模式**（来自多个 TOML 过滤器）：

| 工具 | 剥离内容 |
|------|---------|
| `make` | `make[N]: Entering/Leaving`、空行、`Nothing to be done` |
| `terraform plan` | `Refreshing state...`、`Acquiring/Releasing state lock`、空行 |
| `mvn build` | `[INFO] ---`、`[INFO] Building`、`Downloading/Downloaded`、`Progress` |
| `gcc` | `In file included from`、`from `、`N warnings generated` |
| `brew install` | `==> Downloading`、`==> Pouring`、`Already downloaded:`、`###` |
| `dotnet build` | `Microsoft (R)`、`Copyright (C)`、`Determining projects` |
| `biome` | `Checked N files`、`Fixed N files`、空行 |

### 2.2 成功短路

当命令执行成功时，将整个输出替换为一行确认信息，实现最大压缩。

**TOML 实现**（`dotnet-build.toml`）：
```toml
match_output = [
  { pattern = "0 Warning\\(s\\)\\n\\s+0 Error\\(s\\)", message = "ok (build succeeded)" },
]
```

**效果**：
```
输入 (~100 tokens):
  Microsoft (R) Build Engine version 17.8.3
  Copyright (C) Microsoft Corporation.
    Determining projects to restore...
    All projects are up-to-date for restore.
    MyApp -> /home/user/MyApp/bin/Debug/net8.0/MyApp.dll
  Build succeeded.
      0 Warning(s)
      0 Error(s)
  Time Elapsed 00:00:02.34

输出 (5 tokens):
  ok (build succeeded)

节省: ~95%
```

**其他成功短路示例**：

| 过滤器 | 匹配模式 | 替换消息 |
|--------|---------|---------|
| `dotnet-build` | `0 Warning(s) + 0 Error(s)` | `ok (build succeeded)` |
| `brew-install` | `already installed` | `ok (already installed)` |
| `make` | 全部行被剥离 | `make: ok` |
| `biome` | 空输出 | `biome: ok` |
| `terraform-plan` | 空输出 | `terraform plan: no changes detected` |
| `gcc` | 空输出 | `gcc: ok` |

### 2.3 行数截断

控制最大输出行数，超出行数自动裁剪。

```toml
max_lines = 50   # make.toml
max_lines = 80   # terraform-plan.toml
max_lines = 20   # brew-install.toml
max_lines = 40   # dotnet-build.toml
```

**空输出替换**：当过滤后无内容时，`on_empty` 配置提供占位消息。

### 2.4 保留行匹配

某些场景下，保留特定行比剥离噪音行更高效。

**TOML 语法**（`sshd.toml`）：
```toml
keep_lines_matching = [
  "^error:",
  "^fatal:",
  "^warning:",
  "^\\[ERROR\\]",
]
```

### 2.5 行截断

对超长行按字符数截断，并保持 UTF-8 安全。

```toml
truncate_lines_at = 200   # 每行最多 200 字符
```

**Rust 代码实现**（`grep_cmd.rs`）中的智能截断：
```rust
// 在匹配关键词周围保留上下文，而非简单截头
let start = char_pos.saturating_sub(max_len / 3);  // 保留匹配词前 1/3
let end = (start + max_len).min(char_len);
// 输出: "...before match_keyword after..."
```

### 2.6 结构化解析与摘要

针对有结构输出的命令，解析结构后生成极简摘要。

#### 示例一：TypeScript 编译 (`tsc_cmd.rs`)

**策略**：流式+块处理，统计错误分布，输出摘要而非逐行错误。

```rust
// 状态机：统计每个错误码的出现次数
fn is_block_start(&mut self, line: &str) -> bool {
    if let Some(caps) = TSC_ERROR.captures(line) {
        self.error_count += 1;
        self.files.insert(caps[1].to_string());
        *self.code_counts.entry(caps[5].to_string()).or_insert(0) += 1;
        true
    } else { false }
}

// 输出摘要
fn format_summary(&self, _exit_code: i32, _raw: &str) -> Option<String> {
    format!("TypeScript: {} errors in {} files\nTop codes: TS2322, TS2345\n", ...)
}
```

**效果**：
```
输入 (~500 tokens): 100+ 行详细的类型错误报告
输出 (~30 tokens):  "TypeScript: 15 errors in 8 files\nTop codes: TS2322(5), TS2345(4), TS18046(3)"

节省: ~94%
```

#### 示例二：Ruff lint (`ruff_cmd.rs`)

**策略**：解析 JSON 输出，按文件聚合，压缩路径。

```rust
fn filter_ruff_check_json(output: &str) -> String {
    // 1. 解析 JSON 结构
    // 2. 按文件分组
    // 3. 压缩路径 (compact_path)
    // 4. 统计：总问题数 N，涉及文件 M，其中 K 个可自动修复
    // 5. 行数上限控制 (MAX_VIOLATIONS=50)
}
```

**效果**：
```
输入 (~1000 tokens): JSON 数组，200 条诊断
输出 (~150 tokens):  "200 issues in 15 files (50 fixable)\n  src/main.py: F401(3), E501(2)\n  ... +150 more"

节省: ~85%
```

#### 示例三：Cargo test (`cargo_cmd.rs`)

**策略**：解析测试输出，按测试套件聚合统计。

```rust
struct AggregatedTestResult {
    name: String,
    passed: usize,
    failed: usize,
    ignored: usize,
    filtered: usize,
    measured: usize,
}

// 输出: "crate::module::test_name: 42 passed, 3 failed, 1 ignored"
```

**效果**：
```
输入 (~2000 tokens): 逐行测试输出，含详细 pass/fail 消息
输出 (~50 tokens):   "test result: 142 passed; 3 failed; 2 ignored; 5 filtered out"

节省: ~97%
```

#### 示例四：Cargo install (`cargo_cmd.rs`)

**策略**：状态机逐行分类，剥离无关信息。

```rust
// 跳过所有编译中间行:
if trimmed.starts_with("Compiling") { compiled += 1; continue; }
if trimmed.starts_with("Downloading") { continue; }
if trimmed.starts_with("Finished") { continue; }
if trimmed.starts_with("Locking") { continue; }
// 保留: Installing 行、Installed 行、错误块、PATH 警告
```

**效果**：
```
输入 (~300 tokens): "Compiling serde v1.0.0\n Compiling tokio v1.0.0\n ... (200 行)"
输出 (~30 tokens):   "serde v1.0.0 (replaced v0.9.0)\nWarning: be sure to add /path to your PATH"

节省: ~90%
```

#### 示例五：Next.js build (`next_cmd.rs`)

**策略**：正则提取路由/包大小/时间，生成结构化摘要。

```
输入 (~500 tokens): 详细的 Next.js 构建日志
输出 (~50 tokens):
  Next.js Build
  ═══════════════════════════════════════
  15 routes (12 static, 3 dynamic)
  Bundles:
    /dashboard      156 kB  (+66%)
    /                132 kB
  Time: 34.2s | Errors: 0 | Warnings: 2

节省: ~90%
```

### 2.7 代码内容智能截断

针对代码文件内容，使用语言感知的智能截断。

```rust
fn smart_truncate(content: &str, max_lines: usize, lang: &Language) -> String {
    // 优先级保留：
    // 1. 函数/类型签名 (FUNC_SIGNATURE)
    // 2. import/use 导入
    // 3. pub/export 关键字行
    // 4. 花括号 { }
    // 普通行最多保留 max_lines/2 条
    // 末尾统一: "[N more lines]"
}
```

**效果**：
```
输入 (200 行代码文件):
  line_1
  line_2
  ...
  fn important_function() {  ← 保留（函数签名）
      let x = 1;
      let y = 2;             ← 保留（前 10 条非重要行）
      ...
  }
  ...

输出 (20 行):
  line_1
  line_2
  pub fn important_function() {
      let x = 1;
      ...
  }
  [180 more lines]

节省: ~90%
```

### 2.8 通用后处理

所有输出经过以下通用处理：

| 处理 | 说明 | 实现位置 |
|------|------|---------|
| ANSI 剥离 | 移除终端颜色转义码 | `utils::strip_ansi()` |
| Token 预估计数 | `text.len() / 4` 估算 Token 数 | `tracking::estimate_tokens()` |
| 路径缩短 | `/home/user/project/src/main.py` → `src/main.py` | `compact_path()` 函数 |
| URL 缩短 | 格式化显示，缩短 ARN | `utils::shorten_arn()` |
| 截断符 | 超长输出末尾添加 `[N more lines]` | `filter.rs::smart_truncate()` |

---

## 3. 综合压缩效果实测

来自项目单元测试的 Token 节省验证：

| 命令 | 策略组合 | 原始 Token | 过滤后 Token | 节省率 |
|------|---------|-----------|-------------|-------|
| `ruff check` (200 条诊断) | JSON 解析+文件聚合+上限截断 | ~2500 | ~250 | **90%** |
| `cargo test` (全通过) | 状态机+摘要聚合 | ~1500 | ~50 | **97%** |
| `cargo build` (错误) | 块过滤+编译行剥离 | ~800 | ~120 | **85%** |
| `tsc` (15 错误) | 块汇总+错误码统计 | ~3000 | ~80 | **97%** |
| `dotnet build` (成功) | 成功短路 | ~150 | ~5 | **97%** |
| `terraform plan` | 噪音行剥离 | ~500 | ~100 | **80%** |
| `gt log` | 图结构摘要+数量截断 | ~200 | ~30 | **85%** |
| `next build` | 结构化摘要 | ~500 | ~50 | **90%** |
| `pip install` | 噪音行剥离 | ~300 | ~60 | **80%** |
| `brew install` | 成功短路+噪音剥离 | ~200 | ~10 | **95%** |

---

## 4. 策略选择决策树

```
输出内容
  │
  ├── 命令执行成功?
  │     ├── 是 → 有 "match_output" 成功模式?
  │     │         ├── 是 → 短路为单行确认消息 ← 最高节省
  │     │         └── 否 → 继续下行分析
  │     └── 否 → 保留错误信息
  │
  ├── 输出有结构化格式?
  │     ├── JSON       → 解析后按维度聚合 ← Rust 代码过滤
  │     ├── 测试报告   → 按套件聚合统计  ← Rust 代码过滤
  │     ├── 编译输出   → 按错误块聚合    ← TOML / Rust 代码过滤
  │     └── 表格式     → 提取关键列      ← Rust 代码过滤
  │
  ├── 输出包含噪音行?
  │     ├── 是 → strip_lines_matching (TOML)
  │     └── 否 → 保留全部
  │
  ├── 输出超长?
  │     ├── 是 → max_lines / head_lines / tail_lines / smart_truncate
  │     └── 否 → 保持完整
  │
  └── 输出为空?
        ├── 是 → on_empty 占位消息
        └── 否 → 最终输出
```

---

## 5. 总结

RTK 的压缩策略遵循三个核心原则：

| 原则 | 说明 | 示例 |
|------|------|------|
| **语义压缩** | 理解输出含义，提取关键信息 | JSON 解析后按文件聚合 |
| **信号优先** | 保留错误和警告，剥离噪音 | 保留 `error:` 行，剥离 `make[N]:` 行 |
| **最低可用** | 找到"刚好够用"的输出量 | 成功时整个输出替换为 `ok` 消息 |

三种过滤层级的典型节省率：

| 层级 | 实现方式 | 节省率 | 适用场景 |
|------|---------|-------|---------|
| 基础层 (TOML) | 正则行匹配+截断 | 60-80% | 通用工具输出 |
| 增强层 (Rust 代码) | 结构化解析+摘要 | 80-95% | 常用开发工具 |
| 极致层 (短路) | 成功确认替换 | 95-97% | 例行操作 |
