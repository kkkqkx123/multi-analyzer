# rtk-develop `src/cmds` 命令系统分析报告

> 分析日期: 2026-04-18
> 基准代码: `ref/rtk-develop/src/cmds/`

---

## 一、概述

rtk-develop 的 `src/cmds` 是**命令输出过滤/压缩层**，核心职责是：

- 拦截对各类工具链、CLI 的调用
- 执行实际命令
- 对原始输出进行智能过滤和压缩（减少 token 消耗，保留关键信息）
- 通过 tee 机制提供被截断内容的恢复路径

整个系统分为 **10 个生态模块**，共 **~50+ 个命令过滤器**。

---

## 二、按生态模块分类

### 2.1 `system` — 系统通用工具（15 个命令）

| 文件 | 功能 | 核心机制 |
|------|------|---------|
| `read.rs` | 读取源码文件（支持语言感知过滤） | `core/filter` 去掉注释/空行，支持 `FilterLevel` |
| `ls.rs` | 增强的文件列表（紧凑格式，8进制权限） | 替代原生 `ls`，紧凑格式 |
| `grep_cmd.rs` | 搜索（基于 ripgrep） | 通过 `rg` 实现，分组按文件输出 |
| `find_cmd.rs` | 文件查找 | 基于 `ignore` crate，`.gitignore` 感知 |
| `tree.rs` | 目录树（代理原生 tree） | 自动排除噪音目录 |
| `wc_cmd.rs` | 行/词/字节计数 | 紧凑格式化 wc 输出 |
| `env_cmd.rs` | 环境变量查看 | 自动隐藏敏感值（key/secret/password/token） |
| `pipe_cmd.rs` | 管道过滤器（自动检测输出类型） | 自动识别 cargo-test/pytest/go-test 等输出 |
| `summary.rs` | 命令输出的启发式摘要 | 检测输出类型（测试/构建/日志/列表/JSON） |
| `json_cmd.rs` | JSON 结构检查（不显示值） | 节省大量 token |
| `log_cmd.rs` | 日志去重 | 识别时间戳/UUID/HEX，压缩重复行 |
| `format_cmd.rs` | 代码格式化调度器 | 自动检测 prettier/ruff/black |
| `local_llm.rs` | 源码启发式摘要 | 识别 imports/functions/structs/traits |
| `deps.rs` | 项目依赖摘要 | 解析 Cargo.toml/package.json/requirements.txt |
| `constants.rs` | 噪音目录列表 | node_modules/.git/target 等 |

### 2.2 `git` — Git 生态（5 个命令）

| 文件 | 功能 | 核心机制 |
|------|------|---------|
| `git.rs` | Git 命令（主模块 ~3000 行） | 支持 status/diff/log/add/commit/push/show/checkout/branch/merge 等 |
| `diff_cmd.rs` | 文件差异比较 | 独立于 git 的文件对比 |
| `gh_cmd.rs` | GitHub CLI | 过滤 Markdown 噪声（HTML注释、徽章等） |
| `glab_cmd.rs` | GitLab CLI | 同 gh_cmd，适配 GitLab 差异 |
| `gt_cmd.rs` | Graphite CLI（栈式工作流） | 过滤 Email/分支名/PR 行 |

### 2.3 `js` — JavaScript/TypeScript 生态（9 个命令）

| 文件 | 功能 | 核心机制 |
|------|------|---------|
| `npm_cmd.rs` | npm 包管理 | 自动注入 `run` 子命令 |
| `pnpm_cmd.rs` | pnpm 包管理 | 依赖树解析、JSON 输出提取 |
| `tsc_cmd.rs` | TypeScript 编译 | 块流式过滤（BlockStreamFilter） |
| `lint_cmd.rs` | ESLint/Biome 代码检查 | JSON 输出解析，按规则分组 |
| `prettier_cmd.rs` | Prettier 格式化 | 只显示需格式化的文件列表 |
| `next_cmd.rs` | Next.js 构建 | 抽取路由指标和包大小 |
| `vitest_cmd.rs` | Vitest 测试 | JSON 输出解析，只显示失败用例 |
| `playwright_cmd.rs` | Playwright E2E 测试 | JSON 报告解析 |
| `prisma_cmd.rs` | Prisma ORM CLI | 去除 ASCII art 和冗余装饰 |

### 2.4 `python` — Python 生态（4 个命令）

| 文件 | 功能 | 核心机制 |
|------|------|---------|
| `pip_cmd.rs` | pip/uv 包管理 | pip list/outdated 格式化，pip 不可用时降级到 uv |
| `pytest_cmd.rs` | pytest 测试 | 状态机解析输出，只显示失败 |
| `mypy_cmd.rs` | mypy 类型检查 | 解析错误分组 |
| `ruff_cmd.rs` | Ruff 检查/格式化 | JSON 输出解析 |

### 2.5 `rust` — Rust 生态（2 个命令）

| 文件 | 功能 | 核心机制 |
|------|------|---------|
| `cargo_cmd.rs` | Cargo 构建、测试、检查 | `CargoCommand` 枚举（build/test/check/clippy/nextest），BlockHandler 模式 |
| `runner.rs` | 任意命令运行器 | 捕获 stderr 或测试失败 |

### 2.6 `go` — Go 生态（2 个命令）

| 文件 | 功能 | 核心机制 |
|------|------|---------|
| `go_cmd.rs` | Go 测试、构建、vet | 自动注入 `-json` 标志 |
| `golangci_cmd.rs` | golangci-lint 代码检查 | JSON 输出按规则分组 |

### 2.7 `jvm` — JVM 生态（1 个命令）

| 文件 | 功能 | 核心机制 |
|------|------|---------|
| `gradlew_cmd.rs` | Gradle 包装器 | 支持 build/test/connectedCheck/lint/dependencies |

### 2.8 `dotnet` — .NET 生态（4 个命令）

| 文件 | 功能 | 核心机制 |
|------|------|---------|
| `dotnet_cmd.rs` | dotnet CLI | build/test/restore/format 过滤 |
| `dotnet_trx.rs` | .trx 测试结果解析 | XML 解析 |
| `binlog.rs` | MSBuild 二进制日志 | 错误/警告/测试结果提取 |
| `dotnet_format_report.rs` | dotnet format 报告 | JSON 报告解析 |

### 2.9 `ruby` — Ruby 生态（3 个命令）

| 文件 | 功能 | 核心机制 |
|------|------|---------|
| `rake_cmd.rs` | Rake 测试 | Minitest 输出过滤 |
| `rspec_cmd.rs` | RSpec 测试 | JSON 格式注入，失败信息提取 |
| `rubocop_cmd.rs` | RuboCop 检查 | JSON 格式注入，按严重度排序 |

### 2.10 `cloud` — 云/基础设施工具（5 个命令）

| 文件 | 功能 | 核心机制 |
|------|------|---------|
| `aws_cmd.rs` | AWS CLI | 支持 STS/S3/EC2/ECS/RDS/CloudFormation/Lambda/IAM/DynamoDB 等 |
| `container.rs` | Docker & kubectl | Docker ps/images/logs; kubectl pods/services/logs |
| `curl_cmd.rs` | curl | 截断非 JSON 响应（保留 tee 恢复路径） |
| `wget_cmd.rs` | wget | 去除进度条，显示结果摘要 |
| `psql_cmd.rs` | PostgreSQL | 表格/展开格式压缩 |

---

## 三、当前 Analyzer 项目架构

### 当前架构总览

```
src/
├── core/               # 核心框架
│   ├── mod.rs          # 导出模块
│   ├── analyzer.rs     # BuildAnalyzer trait + PluginRegistry
│   ├── command.rs      # CommandBuilder 命令构建/执行
│   ├── config.rs       # 配置系统
│   ├── parser.rs       # OutputParser trait + BaseParser
│   ├── types.rs        # Issue / AnalysisResult / TestSummary 等类型定义
│   ├── utils.rs        # OutputPostProcessor（行过滤、ANSI剥离、截断）
│   ├── stream.rs       # 分析管道（PipelineStage + run_analysis_pipeline）
│   ├── test_analyzer.rs
│   └── reporter/       # 报告生成（html / json / markdown）
├── plugins/            # 技术栈分析器
│   ├── cargo/          # Rust Cargo
│   ├── cpp/            # C++（clang/gcc/cmake）
│   ├── jvm/            # Java（maven/gradle）
│   ├── npm/            # Node.js
│   └── python/         # Python（mypy/pytest）
├── config/             # 配置模块
├── lib.rs
└── main.rs
```

### 已覆盖的技术栈

| 技术栈 | 状态 | 输出 → Issue 的解析器 |
|--------|------|----------------------|
| Rust / Cargo | ✅ 有 | `plugins/cargo/` |
| Python / mypy / pytest | ✅ 有 | `plugins/python/` |
| Node.js / npm | ✅ 有 | `plugins/npm/` |
| Java / Maven | ✅ 有 | `plugins/jvm/` |
| Java / Gradle | ✅ 有 | `plugins/jvm/` |
| C++ / CMake | ✅ 有 | `plugins/cpp/` |
| C++ / GCC | ✅ 有 | `plugins/cpp/` |
| C++ / Clang | ✅ 有 | `plugins/cpp/` |
| Go | ❌ 无 | — |
| .NET | ❌ 无 | — |
| Ruby | ❌ 无 | — |
| TypeScript / tsc | ❌ 无 | — |

### 已有的过滤能力

当前 `src/core/` 已经内置了基础的通用过滤能力：

| 组件 | 路径 | 功能 |
|------|------|------|
| `OutputPostProcessor` | `core/utils.rs` | ANSI 剥离、噪音行过滤、保留行匹配、行长度/行数截断 |
| `LineFilter` | `core/stream.rs` | PipelineStage：前缀排除 + 最大行数 |
| `ProcessingPipeline` | `core/stream.rs` | 可组合的 `ParseStage → FilterStage → AnalyzeStage` |
| `ParseResult<T>::Full/Degraded/Passthrough` | `core/parser.rs` | 三档解析结果（完整/降级/透传） |

---

## 四、关键决策：是否引入 `cmds/` 过滤层？

### 4.1 决策结论：**不引入 `cmds/` 层**

经过深入对比，当前架构方向是正确的，不需要将 rtk-develop 的 `cmds/` 模式照搬过来。原因如下。

### 4.2 根本差异：两个项目解决的抽象层级不同

| 维度 | **analyzer** | **rtk-develop `cmds/`** |
|------|-------------|------------------------|
| **核心输出** | 结构化数据 (`Vec<Issue>` → `AnalysisResult`) | 压缩后的文本字符串 |
| **最终目标** | 提取错误/警告 → 分类统计 → 生成报告 | 压缩任意 CLI 输出 → 减少 LLM token |
| **输出终点** | HTML / 终端摘要报告 | 标准输出（终端或管道喂给 LLM） |
| **原始输出处理** | 解析后丢弃（已转为 Issue） | 通过 tee 文件保留完整输出 |
| **过滤粒度** | 通用行级过滤（noise/keep/max_lines） | 每个命令独立定制的块级过滤 |
| **处理时机** | 命令结束 → 一次性解析 | 支持实时流式逐行/逐块过滤 |

**一句话概括**：analyzer 要把命令输出**解析成结构化 Issue 数据**；rtk-develop 要把命令输出**压缩成更短的人读文本**。这是两个不同的抽象层面。

### 4.3 如果引入 `cmds/` 层的代价

```
当前流程（简洁清晰）：
  CommandBuilder.run() → raw_string → parser.parse() → Vec<Issue> → AnalysisResult → Report

加入 cmds/ 后的流程（多一层无意义的过滤）：
  CommandBuilder.run() → cmds::filter() → filtered_string → parser.parse() → Vec<Issue> → AnalysisResult → Report
                              ↑
                 parser 只关注少数关键行，OutputPostProcessor 已经能做到；
                 中间的 cmds::filter() 对结构化解析没有额外价值
```

具体代价：
- **增加维护负担**：维护 ~50+ 命令过滤器，每个都需要单独测试
- **抽象冲突**：`cmds/` 输出文本给 LLM 看，analyzer 需要结构化数据 — 最终还是要再解析一遍
- **多数 `cmds/` 命令不在 analyzer 范围内**：system（read/ls/grep/tree）、cloud（aws/docker/curl/wget）、git 都不属于「构建错误分析」
- **每个生态已有现成解析方案**：Go test 自带 JSON 输出、.NET MSBuild 有结构化日志、Ruby RSpec 支持 JSON 格式 — 直接集成比自己写过滤层更可靠

### 4.4 那从 rtk-develop 可以借鉴什么？

**不是整个 `cmds/` 层，而是个别设计模式：**

| 可借鉴的模式 | 来源文件 | analyzer 中如何落地 |
|-------------|---------|-------------------|
| **BlockHandler** 块级收集 | `cargo_cmd.rs` | 在 `core/` 加轻量 `BlockCollector` trait（~5个方法），供 parser 按需实现 |
| **流式输出处理** | `runner.rs` | 给 `CommandBuilder` 增加 `exec_streamed()`（行级回调），用于长时间构建 |
| **Tee 文件恢复** | 多个文件使用 | 可选的 `--save-raw` 选项，保存完整原始输出 |
| **JSON 输出自动注入** | `go_cmd.rs`、`rubocop_cmd.rs` | parser 层自动添加 `-json` / `--format json` 标志 |

---

## 五、真正需要补充的内容

### 5.1 🎯 高优先级 — 补充技术栈分析器（Parser 覆盖度）

这是 analyzer 的核心职责，当前缺失：

| 缺失模块 | 目标输出格式 | 实现方案 |
|---------|-------------|---------|
| **Go 分析器** | `go test -json`、`go build` 错误行 | 新增 `plugins/go/`，核心行解析 + 测试 JSON |
| **.NET 分析器** | MSBuild 格式、`dotnet test` 输出 | 新增 `plugins/dotnet/`，解析 MSBuild 错误格式 |
| **Ruby 分析器** | `rspec --format json`、`rubocop --format json` | 新增 `plugins/ruby/`，JSON 解析 |
| **TypeScript 分析器** | `tsc` 标准错误格式 | 新增 `plugins/typescript/` 或合并到 `plugins/npm/` |

### 5.2 🟢 中优先级 — 轻量增强现有 `core/`

这些改动可以小步迭代，不需要引入 `cmds/` 层：

| 改进项 | 位置 | 改动量 |
|-------|------|-------|
| 增加 `BlockCollector` trait | `core/parser.rs` | ~30 行 |
| `CommandBuilder` 增加流式回调 | `core/command.rs` | ~40 行 |
| `AnalyzeOptions` 增加 `--save-raw` | `core/types.rs` | ~10 行 |
| `OutputPostProcessor` 增加去重选项 | `core/utils.rs` | ~20 行 |

### 5.3 ❌ 不需要做的

| 不必做的事 | 原因 |
|-----------|------|
| 引入 `src/cmds/` 目录 | 多一层无意义的过滤，与 parser 抽象冲突 |
| 为每个工具写独立过滤器 | `OutputPostProcessor` 的通用过滤 + 各 Parser 的特定解析已足够 |
| 实现 system/read/grep/find 等 | 不属于「构建错误分析」范畴 |
| 实现 cloud/aws/docker/kubectl | 不属于「构建错误分析」范畴 |

---

## 六、总结

| 维度 | rtk-develop `src/cmds` | 当前 analyzer | 建议 |
|------|----------------------|---------------|------|
| 命令过滤模块 | 10 生态，~50+ 命令 | ❌ 无此概念 | ❌ **不引入**。不属于 analyzer 职责 |
| 技术栈覆盖 | Rust/JS/Python/Go/JVM/.NET/Ruby | Rust/JS/Python/JVM/C++ | ✅ **补充** Go/.NET/Ruby/TypeScript |
| 输出过滤粒度 | 行级、块级、流式、JSON 解析 | `OutputPostProcessor` + parser | ✅ **增强** `BlockCollector` + 流式回调 |
| token 压缩 | 各类定制压缩逻辑 | 仅 `max_lines` / `max_line_length` | ✅ 按需增强 `OutputPostProcessor` |
| tee 恢复机制 | 完整实现 | ❌ 无 | ⚠️ 可选：加 `--save-raw` |
| 准实时流式处理 | 多命令支持 | ❌ 仅阻塞执行 | ⚠️ 可选：`exec_streamed` |

### 建议路线

1. **短期**: 补充 Go 和 .NET 分析器（各 1 个 `plugins/` 模块）
2. **中期**: 补充 Ruby 和 TypeScript 分析器；在 `core/` 中实现 `BlockCollector` 提升 cargo 错误块解析质量
3. **远期**: 按需增强 `OutputPostProcessor` 和 `CommandBuilder` 的流式能力

**不需要 `cmds/` 层。当前架构方向是正确的。**
