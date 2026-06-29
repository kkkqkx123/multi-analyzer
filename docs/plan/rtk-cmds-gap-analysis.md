# RTK 命令覆盖差距分析

> 分析日期: 2026-06-25
> 基准: `ref/rtk-develop/src/cmds/` vs 当前 `src/plugins/`
> 前置文档: `docs/ref/rtk-cmds-analysis.md`

---

## 一、现状概述

`rtk-cmds-analysis.md` 撰写时项目仅有 Cargo/NPM/Mypy 三个插件，后续已补齐 Go、Dotnet、Ruby、C++ 等。当前 9 个生态共 15 个分析器已覆盖 RTK `cmds/` 中大部分构建错误相关命令。

RTK `cmds/` 中 `system/`、`cloud/`、`git/` 三个生态（~25 命令）属于 CLI 输出压缩场景，超出"构建错误分析"范畴，无需引入。

---

## 二、命令覆盖对照表

### Rust

| RTK 命令 | 当前项目 | 状态 |
|---------|---------|------|
| `cargo build` | `cargo` 插件 (`check`) | 已覆盖 |
| `cargo check` | `cargo` 插件 | 已覆盖 |
| `cargo clippy` | `cargo` 插件 | 已覆盖 |
| `cargo test` | `cargo` 插件 (TestAnalyzer) | 已覆盖 |
| `cargo nextest` | -- | **缺失** |
| `cargo install` | -- | 非构建错误分析范畴 |

### Go

| RTK 命令 | 当前项目 | 状态 |
|---------|---------|------|
| `go build` | `go` 插件 | 已覆盖 |
| `go test` | `go` 插件 (TestAnalyzer) | 已覆盖 |
| `go vet` | `go` 插件 (自动检测) | 已覆盖 |
| `golangci-lint` | `go` 插件 (自动检测) | 已覆盖 |

### .NET

| RTK 命令 | 当前项目 | 状态 |
|---------|---------|------|
| `dotnet build` | `dotnet` 插件 | 已覆盖 |
| `dotnet test` | `dotnet` 插件 (TestAnalyzer) | 已覆盖 |
| `dotnet restore` | -- | **缺失** |
| `dotnet format` | -- | **缺失** |

### Ruby

| RTK 命令 | 当前项目 | 状态 |
|---------|---------|------|
| `rspec` | `ruby` 插件 (JSON + 文本) | 已覆盖 |
| `rubocop` | `ruby` 插件 (JSON 解析) | 已覆盖 |
| `rake` | `ruby` 插件 (自动检测) | 已覆盖 |

### JavaScript / TypeScript

| RTK 命令 | 当前项目 | 状态 |
|---------|---------|------|
| `tsc` | `npm` 插件 (format 自动检测) | 已覆盖 |
| `eslint` | `npm` 插件 (format 自动检测) | 已覆盖 |
| `jest` | `npm` 插件 (TestAnalyzer) | 已覆盖 |
| `vitest` | `npm` 插件 (TestAnalyzer) | 已覆盖 |
| `npm` / `pnpm` | `npm` 插件 (包管理器设置) | 已覆盖 |
| `prettier` | -- | **缺失** |
| `next build` | -- | **缺失** |
| `playwright` | -- | **缺失** |
| `prisma` | -- | **缺失** |

### Python

| RTK 命令 | 当前项目 | 状态 |
|---------|---------|------|
| `mypy` | `mypy` 插件 | 已覆盖 |
| `pytest` | `pytest` 插件 (TestAnalyzer) | 已覆盖 |
| `ruff` | -- | **缺失** |
| `pip` | -- | 非构建错误分析范畴 |

### JVM

| RTK 命令 | 当前项目 | 状态 |
|---------|---------|------|
| `gradlew` | `gradle` 插件 | 已覆盖 |
| -- | `maven` 插件 | 超出 RTK |

---

## 三、核心架构增强状态

根据 `docs/ref/rtk-reference-analysis.md` 的 Phase 1-4 计划，逐一检查实现状态：

### 已实现

| 增强项 | 位置 | 状态 |
|-------|------|------|
| ParseResult 三态降级 (Phase 2.1) | `core/parser.rs:14` | 已实现 |
| 配置系统 (Phase 2.2) | `config/global.rs` + `config/modules/` | 已实现 |
| RunOptions 构建器 (Phase 2.3) | `core/command.rs` CommandBuilder | 已实现 |
| LineFilter 流式过滤 (Phase 3.1) | `core/stream.rs:69` | 已实现 |
| 流式执行模式 (Phase 3.2) | `core/stream.rs` execute_streaming | 已实现 |
| BlockCollector trait (Phase 3.3) | `core/parser.rs:345` | 已实现 |
| Tee 系统 (Phase 4.1) | `config/tee_writer.rs` | 已实现 |
| Verbosity 三级 (Phase 4.3) | `core/types.rs:505` | 已实现 |
| 公共工具函数 (Phase 1.1) | `core/utils.rs` OutputPostProcessor | 已实现 |

### Tee 系统详情

`config/tee_writer.rs` (224 行完整实现):
- 三种模式: `Failures` (默认) / `Always` / `Never`
- 环境变量覆盖: `ANALYZER_TEE_DIR`、`ANALYZER_TEE=0`
- 文件管理: 默认最大 20 个文件，每个最大 1MB，自动清理
- 集成点: `core/command.rs:254` 调用 `tee_and_hint()`

### 未实现

| 增强项 | Phase | 说明 |
|-------|-------|------|
| 指标跟踪 (tracking) | Phase 4.4 | 分析耗时、成功率、token 消耗统计 |
| 声明式规则系统 | discover | 将各 parser 中硬编码的匹配模式数据化 |
| 输入源抽象 (LogProvider) | P2 | CI 日志 / GitHub Actions 日志等非 CLI 输入源 |

---

## 四、需补充的命令

按优先级排序：

### 高优先级

| 命令 | 生态 | 理由 | 实现方案 |
|------|------|------|---------|
| **`ruff`** | Python | 当前最主流 Python linter，几乎所有新项目都在使用 | 新建 `plugins/python/ruff/`，解析 JSON 输出 (`--output-format json`) |
| **`cargo nextest`** | Rust | Rust 生态下一代测试运行器，社区采用率快速上升 | 在 `plugins/cargo/` 中扩展，nextest 输出格式与 `cargo test` 类似 |

### 中优先级

| 命令 | 生态 | 理由 | 实现方案 |
|------|------|------|---------|
| **`prettier`** | JS/TS | 代码格式检查，常与 ESLint 配合使用 | 在 `plugins/npm/parser.rs` 中增加 `prettier --check` 输出格式解析 |
| **`next build`** | JS/TS | Next.js 构建输出有特定格式 | 在 `plugins/npm/parser.rs` 中增加 Next.js 路由/编译错误格式 |
| **`dotnet format`** | .NET | .NET 代码格式检查 | 在 `plugins/dotnet/parser.rs` 中增加 format report JSON 解析 |

### 低优先级

| 命令 | 生态 | 理由 |
|------|------|------|
| `playwright` | JS/TS | E2E 测试，JSON reporter 已标准化，延迟需求 |
| `prisma` | JS/TS | ORM 迁移错误，频次低 |
| `dotnet restore` | .NET | 依赖还原错误，build 命令已能捕获大部分相关问题 |

---

## 五、建议路线

1. **短期** (本次): 实现 `ruff` 分析器 + `cargo nextest` 扩展
2. **中期**: 实现 `prettier` 和 `next build` 输出格式支持 (在已有 npm parser 中扩展)
3. **长期**: 按需引入指标跟踪系统；探索声明式规则降低新 parser 开发成本
