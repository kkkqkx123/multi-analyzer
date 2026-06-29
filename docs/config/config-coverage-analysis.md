# Configuration Coverage Analysis

## 1. 当前已具备配置化的功能

| 功能模块 | Config 结构体 | 定义位置 | 消费位置 | 环境变量 | 状态 |
|----------|-------------|---------|---------|---------|------|
| 报告输出 | `ReportConfig` | `config/modules/report.rs` | `types.rs:604-627` from_config, `main.rs` CLI | `ANALYZER_FORMAT`, `ANALYZER_VERBOSITY`, `ANALYZER_VERBOSE` | **完整** |
| 通用过滤 | `FilterConfig` | `config/modules/filter.rs` | `types.rs:618-623` from_config | `ANALYZER_STRIP_ANSI`, `ANALYZER_MAX_LINES`, `ANALYZER_MAX_LINE_LENGTH` | **完整** |
| 命令覆盖 | `CommandConfig` | `config/modules/commands.rs` | `main.rs:669` 命令查找 | 无 | **完整** |
| 技术栈脚本映射 | `TechStackConfig` | `config/modules/tech_stack.rs` | `main.rs:435,681` resolve_script/test_framework_for | 无 | **完整** |
| TOML 过滤器 | `FilterRegistry` + `TomlFilterConfig` | `config/filter_registry.rs` | `stream.rs:94`, `npm/parser.rs:19` | 无 | **完整** |
| 输出 Tee 保存 | `TeeConfig` | `config/modules/tee.rs` | `tee_writer.rs:135` tee_raw | `ANALYZER_TEE`, `ANALYZER_TEE_DIR`, `ANALYZER_TEE_MODE` | **完整** |
| 分层加载 | `ConfigLoader` | `config/loader.rs` | `main.rs` 入口 | 无 | **完整** |
| 配置持久化 | save/load/create_default/show_config | `config/global.rs` | `main.rs` CLI subcommand | 无 | **完整** |

## 2. 需要补充配置支持的功能

### 2.1 LimitsConfig 未被消费 (高优先级)

**现状**: `LimitsConfig` 结构定义完成 (`config/modules/limits.rs`)，包含 `grep_max_results`、`grep_max_per_file`、`status_max_files`、`status_max_untracked`、`passthrough_max_chars` 五个字段。`AppConfig` 已集成该字段，config loader 会正常加载，但**没有任何管道代码使用这些值**。

**影响**: grep/status/passthrough 操作使用硬编码默认值，用户无法通过配置文件调整输出限制。

**建议修改**:
- `src/core/command.rs` 或 `src/core/utils.rs` 中，在执行 grep/status 类命令时读取 `config.limits.*` 替代硬编码常量
- `src/core/stream.rs` 中 passthrough 输出时使用 `config.limits.passthrough_max_chars`

### 2.2 discover 模块未与配置系统桥接 (高优先级)

**现状**: `src/discover/rules.rs` 使用完全静态的 `RULES` 表 (`[CommandRule; N]`) 做命令匹配。`TechStackConfig.scripts` 和 `test_framework` 的查询仅在 `main.rs` 中对已匹配的命令名做**二次 lookup**，而非驱动命令发现本身。

**问题**:
- 用户无法通过配置添加新的命令发现规则——必须修改源码
- `tech_stacks.<name>.commands.<cmd>` 中定义的自定义命令，discover 引擎不认识
- 这导致配置系统说的"支持自定义命令"实际上只对用户显式传入的命令名生效，对拦截/重写的 shell 命令不生效

**建议修改**:
- 在 `discover::registry::classify_command()` 调用链路中注入 `&AppConfig`
- 当静态 RULES 未匹配时，fallback 到配置中的 `tech_stacks` 查找
- `rewrite_command()` 也应根据配置中的 `exec` 覆盖决定最终命令

### 2.3 env_loader 覆盖不完整 (中优先级)

**现状**: `src/config/env_loader.rs` 覆盖了 format、verbosity、verbose、strip_ansi、max_lines、max_line_length、tee 相关变量，但以下环境变量缺失：

| 缺失的环境变量 | 对应字段 | 预期行为 |
|--------------|---------|---------|
| `ANALYZER_MAX_FILES` | `TeeConfig.max_files` | 覆盖 tee 最大文件数 |
| `ANALYZER_MAX_FILE_SIZE` | `TeeConfig.max_file_size` | 覆盖 tee 最大文件大小 |
| `ANALYZER_SUCCESS_SHORT_CIRCUIT` | `ReportConfig.success_short_circuit` | 控制成功短路行为 |
| `ANALYZER_OUTPUT` | (CLI --output 对应的持久化) | 覆盖默认输出路径 |
| `ANALYZER_FILTER_PATHS` | `AnalyzeOptions.filter_paths` | 逗号分隔的路径过滤列表 |

### 2.4 配置验证缺失 (中优先级)

**现状**: 配置文件解析失败时仅打印 warning 并使用默认值，但没有对配置值的**语义合法性**进行验证：

| 验证目标 | 问题 |
|----------|------|
| `match_command` 正则 | 无效正则不会在加载时报错，运行时 `find_filter()` 中 `Regex::new()` 失败时静默跳过 |
| `exec` 命令格式 | 无格式检查 |
| `tech_stacks` 引用一致性 | 命令引用的 tech_stack 可能在 `tech_stacks` 表中不存在 |
| `strip_lines_matching` 正则 | 无效正则会在运行时 panic 或静默跳过 |
| `short_circuit.unless` 正则 | 同上 |
| `replace.pattern` 正则 | 同上 |

**建议**:
- 在 `ConfigLoader::load()` 后增加 `validate()` 步骤
- 在 `FilterRegistry::load()` 中对每个 `TomlFilterConfig` 预编译正则，当场报告错误
- 对 config 中的 tech_stack 引用做完整性检查

### 2.5 缺少 DisplayConfig (低优先级)

**现状**: 无输出显示控制（颜色、emoji、最大宽度等）。

**RTK 参考**: `ref/rtk-develop/src/core/config.rs:74-89` 定义了 `DisplayConfig` 控制 colors / emoji / max_width。

**适用场景**: Markdown reporter 的终端输出美化、HTML reporter 的 inline style 控制。

### 2.6 缺少 TrackingConfig (低优先级)

**现状**: `core::tracking::stats()` 提供执行统计，但无法通过配置关闭或调整。SQLite 存储路径无配置控制。

**RTK 参考**: `ref/rtk-develop/src/core/config.rs:56-72` 定义了 `TrackingConfig` 控制 enabled / database_path / retention_days。

### 2.7 缺少 HooksConfig (低优先级)

**现状**: 项目支持 agent hooks（见 `ref/rtk-develop/hooks/`），但缺少 `exclude_commands`、`transparent_prefixes` 等配置项。

**RTK 参考**: `ref/rtk-develop/src/core/config.rs:27-54`。

**说明**: 此功能取决于 hooks 模块本身是否已从 RTK 迁移过来。如果 hooks 模块尚未集成，此配置项暂不需要。

### 2.8 命令别名系统未实现 (低优先级)

**现状**: 设计文档 Phase 3 规划了命令别名（`[alias]` section），但未实现。

**适用场景**: 用户习惯使用 `analyzer check` 等价于 `analyzer cargo check`。

**建议**: 在 `AppConfig` 中增加 `alias: HashMap<String, String>` 字段，在 CLI 解析阶段做别名展开。

### 2.9 二进制目录配置层未实现 (低优先级)

**现状**: 设计文档规划了 `<binary_dir>/analyzer.toml` 配置层 (优先级介于全局和项目之间)，但未实现。

**适用场景**: 分发 analyzer 二进制时附带默认配置文件，适用于组织级统一配置。

## 3. 优先级建议

```
Phase 1 (高) — 功能正确性
├── LimitsConfig 消费 — grep/status 限制生效
└── discover + 配置桥接 — 配置真正驱动命令发现

Phase 2 (中) — 健壮性 & 可维护性
├── env_loader 变量补全
└── 配置验证 (预编译正则 + 引用检查)

Phase 3 (低) — 体验增强
├── DisplayConfig (colors/emoji)
├── TrackingConfig (执行统计持久化)
├── 命令别名系统
└── 二进制目录配置层

Phase 4 (条件) — 取决于上游模块
└── HooksConfig (需 hooks 模块先集成)
```

## 4. 现状总结

配置系统的基础架构已经很扎实：分层加载、环境变量覆盖、TOML 序列化/反序列化、Filter/Report/Command/TechStack/Tee/Limits 六大模块定义齐全。主要差距在于 **LimitsConfig 未被消费**和 **discover 模块与配置系统脱节**这两个功能性问题，其余缺失项属于锦上添花的体验增强。

---

*最后更新: 2026-06-27*
