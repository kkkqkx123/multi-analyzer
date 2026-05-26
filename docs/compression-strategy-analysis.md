# Analyzer 输出压缩策略分析

## 概述

本文档基于 RTK 项目的命令输出压缩策略（参考 `ref/rtk-develop/docs/COMPRESSION_STRATEGY.md`），分析当前 Analyzer 项目可在输出处理中引入的压缩与精简策略。

Analyzer 的核心定位是**构建错误分析器**，与 RTK 的定位（**CLI 输出过滤/压缩代理**）有所不同。因此部分 RTK 策略需要调整后适用，部分策略则直接适用于 Analyzer 的管道处理。

---

## 当前项目已有能力

### 已实现的压缩/精简能力

| 能力      | 实现位置                             | 说明                                |
| --------- | ------------------------------------ | ----------------------------------- |
| ANSI 剥离 | `core/utils.rs::strip_ansi()`        | 移除终端颜色转义码                  |
| 基础截断  | `core/utils.rs::truncate()`          | 按字符数截断并追加 `...`            |
| 输出摘要  | `core/utils.rs::summarize_output()`  | 保留前 N 行，追加 `... (+M more)`   |
| 降级策略  | `core/parser.rs::ParseResult`        | Full/Degraded/Passthrough 三级降级  |
| 管道处理  | `core/stream.rs::ProcessingPipeline` | Parse → Filter → Analyze 阶段化处理 |
| 级别过滤  | `core/stream.rs::LevelFilter`        | 仅保留 Error 级别的问题             |
| 路径过滤  | `core/stream.rs::IncludePathsFilter` | 按文件路径模式过滤                  |
| 执行跟踪  | `core/tracking.rs::History`          | 记录分析耗时、问题数等              |

### 与 RTK 的能力差距

| 维度            | RTK                                   | 当前项目                          |
| --------------- | ------------------------------------- | --------------------------------- |
| 噪音行剥离      | TOML 配置 `strip_lines_matching`      | 仅在 NpmParser 中硬编码剥离部分行 |
| 成功短路        | `match_output` 匹配成功模式替换为单行 | 无此能力                          |
| Keep-lines 模式 | `keep_lines_matching` 保留特定行      | 无                                |
| 结构化摘要      | 解析 JSON/测试输出后生成极简摘要      | 基本解析，但无摘要生成            |
| Token 预估      | `estimate_tokens()` 估算 LLM 消耗     | 无                                |
| 路径缩短        | `compact_path()` 缩短绝对路径         | 无                                |
| 代码智能截断    | `smart_truncate()` 语言感知截断       | 无                                |
| TOML 过滤器     | 60+ 命令的 TOML 过滤配置              | 无                                |
| 命令别名        | 自动剥离透明前缀                      | 无                                |
| 头尾保留        | `head_lines` / `tail_lines`           | 只有 `summarize_output`           |

---

## 可借鉴的压缩策略分析

### 1. 噪音行剥离（Noise Line Stripping）

**RTK 做法**：通过 TOML 过滤器配置 `strip_lines_matching`，使用正则表达式匹配并移除命令输出中的噪音行（如 `make[N]:`、`[INFO] ---`、`==> Downloading` 等）。

**当前项目现状**：

- NpmParser 中有硬编码的噪音行剥离（如 TUI border 行、cache hit 行、update notification 行）
- 其他 Parser 无此能力
- 无统一/可配置的噪音行剥离机制

**应用建议**：★★★★★（高价值，低实施成本）

| 措施                                        | 说明                                                                                                   |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| 在 `core/utils.rs` 中添加通用噪音行过滤函数 | 接受行列表 + 正则模式列表，返回过滤后的行                                                              |
| 为各技术栈定义默认噪音模式                  | Cargo: `Compiling` / `Finished` / `Locking`；NPM: TUI border / cache hit；C++: `In file included from` |
| 通过配置系统支持自定义噪音模式              | 扩展 `config.rs` 中的 `FilterConfig`                                                                   |

**示例 API**：

```rust
// core/utils.rs
pub fn filter_noise_lines(lines: &[&str], patterns: &[&str]) -> Vec<String> {
    let re_list: Vec<Regex> = patterns.iter()
        .map(|p| Regex::new(p).unwrap())
        .collect();
    lines.iter()
        .filter(|line| !re_list.iter().any(|re| re.is_match(line.trim())))
        .map(|s| s.to_string())
        .collect()
}
```

**预期效果**：

```
输入 (200 tokens):
  Compiling serde v1.0.0
  Compiling tokio v1.0.0
    Checking foo v0.1.0
  error[E0308]: mismatched types
   --> src/main.rs:10:5
  Compiling bar v0.2.0

输出 (50 tokens):
  error[E0308]: mismatched types
   --> src/main.rs:10:5

节省: ~75%
```

---

### 2. 成功短路（Success Short-Circuit）

**RTK 做法**：当命令执行成功且无问题时，将整个输出替换为一行确认消息（如 `ok (build succeeded)`），实现 ~95% 的 Token 节省。

**当前项目现状**：

- 当 `AnalysisResult.total_issues == 0` 时，报告仍然输出完整格式（标题 + 摘要 + 统计）
- 无"零问题短路"概念

**应用建议**：★★★★★（高价值，低实施成本）

**实现方案**：

```rust
// 在 reporter 层增加短路检测
fn generate_report(result: &AnalysisResult) -> String {
    if result.total_issues == 0 {
        return format!("✅ {}: no issues found", tech_stack_name);
    }
    // 正常报告生成...
}
```

**预期效果**：

```
输入 (150 tokens):
  # Analysis Report
  ## Issues Summary
  - **Total**: 0
  - **Categories**: 0
  - **Files Affected**: 0

输出 (5 tokens):
  cargo check: no issues found

节省: ~97%
```

---

### 3. 结构化摘要与聚合（Structured Summarization）

**RTK 做法**：针对有结构输出的命令（tsc、ruff、cargo test 等），解析输出结构后生成极简摘要而非逐行输出。例如 TypeScript 编译输出从 100+ 行错误缩减为 `TypeScript: 15 errors in 8 files\nTop codes: TS2322(5), TS2345(4)`。

**当前项目现状**：

- `AnalysisResult` 已有 `issues_by_type`、`issues_by_file`、`issues_by_level` 等聚合结构
- Markdown 报告已按文件/类型分组展示
- 但报告中未提供"Top N 错误码"或高频错误摘要

**应用建议**：★★★★☆（高价值，中等实施成本）

**改进方向**：

| 改进                | 说明                                                           |
| ------------------- | -------------------------------------------------------------- |
| 增加高频错误码排名  | 在报告摘要中添加 Top N 错误码及其出现次数                      |
| 按错误模式聚合      | 对相似消息进行模糊聚合（如 `undefined variable X` 视为同类型） |
| 摘要级别控制        | 支持 `--summary` 模式，仅输出统计摘要                          |
| 按 Package 聚合统计 | 已部分支持，可增强为摘要形式                                   |

**报告增强示例**：

```markdown
# Type Check Report

## Issues Summary

- **Total**: 15
- **Errors**: 12 | **Warnings**: 3
- **Files**: 8 | **Categories**: 5

## Top Error Codes

- `TS2322` (Type mismatch): 5 occurrences
- `TS2345` (Argument type): 4 occurrences
- `TS18046` (Unknown type): 3 occurrences
```

---

### 4. 路径缩短（Path Shortening）

**RTK 做法**：`compact_path()` 函数将绝对路径缩短为相对路径，如 `/home/user/project/src/main.py` → `src/main.py`。同时支持缩短 ARN 等长标识符。

**当前项目现状**：

- 各 Parser 直接使用命令输出中的原始文件路径
- 无统一路径缩短机制

**应用建议**：★★★★☆（高价值，低实施成本）

**实现方案**：

```rust
// core/utils.rs
pub fn compact_path(path: &str, base_dir: Option<&str>) -> String {
    let base = base_dir.unwrap_or(".");
    if let Ok(relative) = std::path::Path::new(path).strip_prefix(base) {
        return relative.to_string_lossy().to_string();
    }
    // 尝试从已知的公共前缀之后开始截断
    if let Some(pos) = path.rfind("/src/") {
        return path[pos + 1..].to_string();
    }
    if let Some(pos) = path.rfind("\\src\\") {
        return path[pos + 1..].to_string();
    }
    path.to_string()
}
```

**预期效果**：

```
输入: D:\projects\myapp\src\components\Button.tsx
输出: src/components/Button.tsx
节省: ~60% 路径长度
```

---

### 5. 保留行匹配（Keep Lines Matching）

**RTK 做法**：通过 `keep_lines_matching` 配置只保留特定模式的行（如 `^error:`、`^fatal:`、`^warning:`），比噪音剥离更高效。

**当前项目现状**：无此能力。所有 Parser 尝试解析所有行。

**应用建议**：★★★☆☆（中等价值，低实施成本）

**适用场景**：

- 当命令输出包含大量无关信息，而关键信息仅出现在特定格式行时
- 如 `mvn build` 的输出中仅 `[ERROR]` 行是关键信息

**实现方案**：

```rust
pub fn keep_matching_lines(output: &str, patterns: &[&str]) -> String {
    let re_list: Vec<Regex> = patterns.iter()
        .map(|p| Regex::new(p).unwrap())
        .collect();
    output.lines()
        .filter(|line| re_list.iter().any(|re| re.is_match(line)))
        .collect::<Vec<_>>()
        .join("\n")
}
```

---

### 6. 行数截断 + 头尾保留（Line Count Truncation with Head/Tail）

**RTK 做法**：同时支持 `head_lines`（保留开头 N 行）、`tail_lines`（保留结尾 N 行）、`max_lines`（总行数上限）。

**当前项目现状**：仅有 `summarize_output()` 保留开头 N 行。

**应用建议**：★★★☆☆（中等价值，低实施成本）

**改进方向**：

```rust
pub enum OutputTruncation {
    Head(usize),       // 保留开头 N 行
    Tail(usize),       // 保留结尾 N 行
    HeadTail { head: usize, tail: usize },  // 保留开头 + 结尾
    Max(usize),        // 总行数上限（按比例保留头尾）
}
```

**适用场景**：

- Cargo test 输出：`tail` 保留最后的 test result 摘要
- 编译错误：`head` 保留错误头部信息

---

### 7. 行截断（Per-Line Truncation）

**RTK 做法**：对超长行按字符数截断，并支持**上下文感知截断**——在匹配关键词周围保留上下文，而非简单截头。

**当前项目现状**：`truncate()` 按字符数简单截断，无上下文感知。

**应用建议**：★★★☆☆（中等价值，中等实施成本）

**改进方向**：

```rust
/// 智能截断：在匹配关键词周围保留上下文
pub fn smart_truncate_line(line: &str, max_len: usize, keyword: Option<&str>) -> String {
    let char_count = line.chars().count();
    if char_count <= max_len {
        return line.to_string();
    }
    if let Some(kw) = keyword {
        if let Some(pos) = line.find(kw) {
            let start = pos.saturating_sub(max_len / 3);
            let end = (start + max_len).min(char_count);
            return format!("...{}...", &line[start..end]);
        }
    }
    format!("{}...", line.chars().take(max_len.saturating_sub(3)).collect::<String>())
}
```

---

### 8. TOML 过滤器配置系统（TOML Filter Configuration）

**RTK 做法**：每个命令有独立的 TOML 配置文件（如 `make.toml`、`terraform-plan.toml`），配置噪音行模式、成功匹配模式、最大行数等。

**当前项目现状**：

- 有 `core/config.rs` 但仅支持基本配置（报告格式、命令覆盖、忽略路径）
- 无按技术栈/命令的细化过滤配置

**应用建议**：★★★☆☆（中等价值，高实施成本）

**扩展配置示例**：

```toml
# analyzer.toml
[filter.cargo]
strip_lines = [
  "^Compiling\\s",
  "^Finished\\s",
  "^Locking\\s",
  "^Downloading\\s",
]
on_empty = "cargo: ok"
max_lines = 80

[filter."npm:lint"]
keep_lines = [
  "^error:",
  "^warning:",
]
on_empty = "npm lint: no issues"
```

---

### 9. Token 预估（Token Estimation）

**RTK 做法**：`tracking::estimate_tokens()` 通过 `text.len() / 4` 估算输出 Token 数，用于衡量压缩效果。

**当前项目现状**：无 Token 预估。

**应用建议**：★★☆☆☆（低价值，低实施成本）

**实现方案**：

```rust
pub fn estimate_tokens(text: &str) -> usize {
    // 简单估算：中英文混合场景每 4 字符约 1 token
    text.len() / 4
}
```

---

### 10. 代码内容智能截断（Smart Code Truncation）

**RTK 做法**：对代码文件内容进行语言感知截断，优先保留函数签名、`pub`/`export` 关键字行、`import`/`use` 导入行等关键结构。

**当前项目现状**：不适用的可能性高。Analyzer 处理的是命令输出而非代码文件内容。

**应用建议**：★★☆☆☆（低价值，仅在代码审查场景有用）

---

### 11. 命令别名与透明前缀剥离（Command Alias & Transparent Prefix Stripping）

**RTK 做法**：自动剥离 `noglob`、`command`、`builtin`、`exec`、`nocorrect` 等 Shell 前缀，以及用户自定义的透明前缀。

**当前项目现状**：analyzer 通过 CLI 参数直接指定 `tech-stack` 和 `command`，不涉及 Shell 命令重写。

**应用建议**：★☆☆☆☆（低价值，与当前 CLI 设计不匹配）

---

### 12. 通用后处理增强（Post-Processing Enhancement）

**RTK 做法**：所有输出经过 ANSI 剥离、Token 预估、路径缩短、URL 缩短等通用后处理。

**当前项目现状**：仅有 ANSI 剥离。路径缩短、URL 缩短缺失。

**应用建议**：★★★★☆（高价值，低实施成本）

**实现方案**：在 `core/utils.rs` 中添加后处理管道：

```rust
pub struct OutputPostProcessor {
    strip_ansi: bool,
    compact_paths: bool,
    max_lines: Option<usize>,
    max_line_length: Option<usize>,
}

impl OutputPostProcessor {
    pub fn process(&self, output: &str) -> String {
        let mut result = output.to_string();
        if self.strip_ansi {
            result = strip_ansi(&result);
        }
        // 更多处理...
        result
    }
}
```

---

## 综合优先级排序

基于价值/成本比的综合排序：

| 优先级 | 策略                      | 价值  | 成本 | 实施阶段 |
| ------ | ------------------------- | ----- | ---- | -------- |
| P0     | 成功短路                  | ★★★★★ | 低   | Phase 1  |
| P0     | 噪音行剥离（统一机制）    | ★★★★★ | 低   | Phase 1  |
| P1     | 路径缩短                  | ★★★★☆ | 低   | Phase 1  |
| P1     | 结构化摘要增强            | ★★★★☆ | 中   | Phase 2  |
| P1     | 通用后处理管道            | ★★★★☆ | 低   | Phase 1  |
| P2     | 保留行匹配                | ★★★☆☆ | 低   | Phase 2  |
| P2     | 行数截断增强（Head/Tail） | ★★★☆☆ | 低   | Phase 2  |
| P2     | Token 预估                | ★★☆☆☆ | 低   | Phase 2  |
| P3     | 行智能截断                | ★★★☆☆ | 中   | Phase 3  |
| P3     | TOML 过滤器配置           | ★★★☆☆ | 高   | Phase 3  |
| P4     | 代码内容智能截断          | ★★☆☆☆ | 中   | 暂不实施 |
| P4     | 命令别名/前缀剥离         | ★☆☆☆☆ | 低   | 暂不实施 |

---

## 各技术栈的噪音行模式总结

为方便实施噪音行剥离，整理各技术栈输出的常见噪音行模式：

### Cargo

| 模式             | 说明                                      |
| ---------------- | ----------------------------------------- |
| `^Compiling\s`   | 编译中提示（如 `Compiling serde v1.0.0`） |
| `^Finished\s`    | 编译完成提示                              |
| `^Locking\s`     | 锁定文件更新提示                          |
| `^Downloading\s` | 下载依赖提示                              |
| `^\s+Blocking\s` | 等待下载提示                              |
| `^\s*$`          | 空行                                      |

### Maven

| 模式                 | 说明                     |
| -------------------- | ------------------------ | -------- |
| `^\[INFO\]\s`        | 信息日志（大部分可忽略） |
| `^\[INFO\]\s+---\s+` | 分隔线                   |
| `^Download(ing       | ed)\s`                   | 下载进度 |
| `^Progress\s`        | 下载进度条               |
| `^\s*$`              | 空行                     |
| `^\[WARNING\]`       | 警告（按需保留）         |

### Gradle

| 模式             | 说明         |
| ---------------- | ------------ | -------- |
| `^> Configure\s` | 配置阶段提示 |
| `^Download(ing   | ed)\s`       | 下载提示 |

### npm/pnpm/yarn

| 模式                               | 说明       |
| ---------------------------------- | ---------- |
| `^╭\|╰\|┌\|└\|│\|─\|├\|┤\|•\|>.*$` | TUI 装饰行 |
| `.*cache hit.*`                    | 缓存命中   |
| `.*replaying logs.*`               | 日志重放   |
| `.*Update available.*`             | 更新通知   |
| `.*Changelog:.*`                   | 更新日志   |

### Python (mypy/pytest)

| 模式                 | 说明         |
| -------------------- | ------------ |
| `^\s*$`              | 空行         |
| `^Found \d+ errors?` | 重复的汇总行 |

### Go

| 模式    | 说明                                         |
| ------- | -------------------------------------------- |
| `^\#\s` | 包编译指示行（如 `# example.com/myproject`） |
| `^\s*$` | 空行                                         |

### C/C++ (GCC/Clang/MSVC)

| 模式                       | 说明       |
| -------------------------- | ---------- |
| `^In file included from\s` | 包含链追踪 |
| `^\s+from\s`               | 包含链延续 |
| `^\d+ warnings? generated` | 警告计数行 |

---

## 实施路径建议

### Phase 1: 快速见效（预计 1-2 天）

1. **成功短路**：在 Reporter 层检查 `total_issues == 0` 时输出单行确认
2. **噪音行剥离**：在 `core/utils.rs` 添加通用噪音行过滤器
3. **路径缩短**：添加 `compact_path()` 函数，在各 Parser 输出中应用
4. **通用后处理管道**：在 `core/stream.rs` 中新增后处理阶段

### Phase 2: 核心增强（预计 3-5 天）

1. **结构化摘要**：增强 Reporter 支持 Top N 错误码排名、错误模式聚合
2. **保留行匹配**：添加 `keep_matching_lines()` 工具函数
3. **行数截断增强**：支持 Head/Tail 截断模式
4. **Token 预估**：添加简单 Token 计数，可选展示在报告中

### Phase 3: 高级功能（预计 1-2 周）

1. **TOML 过滤器**：扩展配置系统支持按命令的噪音行/保留行/最大行数配置
2. **行智能截断**：实现上下文感知的行截断
3. **Verbosity 级别**：引入三级 Verbosity（summary/normal/verbose）

---

## 效果预估

| 策略       | 节省率 | 适用命令                              |
| ---------- | ------ | ------------------------------------- |
| 成功短路   | 95-97% | 全部（成功时）                        |
| 噪音行剥离 | 60-80% | Cargo build、Maven build、NPM install |
| 结构化摘要 | 80-95% | Cargo check/clippy、Mypy、TSC         |
| 路径缩短   | 20-40% | 全部                                  |
| 保留行匹配 | 70-90% | 详细日志类命令                        |

**综合期望**：当前项目输出的 Token 消耗可降低 50-80%，具体取决于命令类型和成功/失败状态。
