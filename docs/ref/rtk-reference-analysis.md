# RTK 参考项目设计分析及分阶段改造方案

## 概述

本文档基于 `ref/rtk-develop` (RTK) 项目与当前 analyzer 项目的对比分析，总结可借鉴的设计思路，并制定分阶段实施计划。

### 核心发现

| 维度     | RTK                                      | 当前项目                                 |
| -------- | ---------------------------------------- | ---------------------------------------- |
| 定位     | CLI 输出过滤/压缩代理                    | 构建错误分析器                           |
| 核心流程 | 执行命令 -> 流式过滤 -> 压缩输出 -> 跟踪 | 执行命令 -> 缓冲输出 -> 解析 -> 生成报告 |
| 模块组织 | 按生态系统（git/js/python/go/rust/...）  | 按技术栈（cargo/npm/python/go/cpp/...）  |
| 扩展方式 | 在 `src/cmds/` 下添加新模块              | 在 `src/plugins/` 下实现 Trait           |

### 10 个可借鉴的设计思路

| 优先级 | 设计思路           | RTK 参考位置                                                            | 核心价值             |
| ------ | ------------------ | ----------------------------------------------------------------------- | -------------------- |
| ★★★★★  | 流式输出处理       | [stream.rs](../ref/rtk-develop/src/core/stream.rs)                      | 实时解析、低内存占用 |
| ★★★★★  | 解析降级策略       | [parser/mod.rs](../ref/rtk-develop/src/parser/mod.rs)                   | 解析失败不阻塞       |
| ★★★★☆  | 配置系统           | [config.rs](../ref/rtk-develop/src/core/config.rs)                      | 可定制行为           |
| ★★★★☆  | Runner 构建器模式  | [runner.rs](../ref/rtk-develop/src/core/runner.rs)                      | 灵活的执行选项       |
| ★★★★☆  | 多维度过滤策略     | [ARCHITECTURE.md](../ref/rtk-develop/docs/contributing/ARCHITECTURE.md) | 分层摘要展示         |
| ★★★☆☆  | 原始输出恢复 (Tee) | [tee.rs](../ref/rtk-develop/src/core/tee.rs)                            | 失败时可调试         |
| ★★★☆☆  | Lazy Static Regex  | 各命令模块                                                              | 避免重复编译         |
| ★★★☆☆  | 模块 README 规范   | 各模块目录                                                              | 降低上手成本         |
| ★★★☆☆  | 内联测试 + Fixture | 各 `*_cmd.rs`                                                           | 测试更易维护         |
| ★★★☆☆  | 执行指标跟踪       | [tracking.rs](../ref/rtk-develop/src/core/tracking.rs)                  | 性能可观测           |

## 分阶段实施计划

### Phase 1: 快速优化（低风险，立即可做）

| #   | 任务              | 涉及文件                                      | 说明                                                                   |
| --- | ----------------- | --------------------------------------------- | ---------------------------------------------------------------------- |
| 1.1 | 提取公共工具函数  | 新建 `src/core/utils.rs`                      | 将 ANSI 剥离、智能截断、跨平台命令检测等通用功能从各 parser 中提取出来 |
| 1.2 | Lazy Static Regex | `src/plugins/cpp/parser.rs` 及其他 parser     | 将 Regex 改为全局一次性编译                                            |
| 1.3 | 清理冗余代码      | 各文件                                        | 审查并删除未使用的函数和字段                                           |
| 1.4 | 添加模块 README   | `src/core/README.md`, `src/plugins/README.md` | 简要说明模块职责和核心概念                                             |

### Phase 2: 核心层增强（中等影响）

| #   | 任务                      | 涉及文件                  | 说明                                                   |
| --- | ------------------------- | ------------------------- | ------------------------------------------------------ |
| 2.1 | 引入 ParseResult 降级策略 | `src/core/parser.rs`      | 将返回值从 `Vec<Issue>` 改为 `ParseResult<Vec<Issue>>` |
| 2.2 | 引入配置系统              | 新建 `src/core/config.rs` | 基于 toml 的配置系统                                   |
| 2.3 | RunOptions 构建器         | `src/core/command.rs`     | 扩展 CommandBuilder，添加链式调用选项                  |

### Phase 3: 流式处理管道（重大架构变化）

| #   | 任务                    | 涉及文件                  | 说明                             |
| --- | ----------------------- | ------------------------- | -------------------------------- |
| 3.1 | 引入 StreamFilter Trait | 新建 `src/core/stream.rs` | 定义核心 streaming trait         |
| 3.2 | 重构 CommandBuilder     | `src/core/command.rs`     | 添加流式执行模式                 |
| 3.3 | 迁移 Cargo Analyzer     | `src/plugins/cargo/`      | 使用 BlockHandler 处理多行错误块 |
| 3.4 | 迁移 Go Analyzer        | `src/plugins/go/`         | 使用流式替代当前的行遍历         |
| 3.5 | 迁移 C++ Parser         | `src/plugins/cpp/`        | 使用流式替代全量正则匹配         |

### Phase 4: 高级功能（新增能力）

| #   | 任务           | 涉及文件                    | 说明                       |
| --- | -------------- | --------------------------- | -------------------------- |
| 4.1 | Tee 系统       | 新建 `src/core/tee.rs`      | 命令失败时自动保存原始输出 |
| 4.2 | 错误摘要模式   | `src/core/reporter/`        | 新增分组/摘要策略          |
| 4.3 | Verbosity 级别 | `src/core/`                 | 引入三级 Verbosity         |
| 4.4 | 指标跟踪       | 新建 `src/core/tracking.rs` | 记录分析耗时、成功率等     |

### 依赖关系

```
Phase 1 ──→ Phase 2 ──→ Phase 3
                 │            │
                 └──→ Phase 4 ─┘
```

- Phase 1 无外部依赖，可立即开始
- Phase 2.1 影响所有 parser，需要同步修改
- Phase 3 依赖 Phase 2.3
- Phase 4.1 依赖 Phase 2.2
- Phase 3 和 Phase 4 可并行推进
