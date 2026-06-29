# Config-Discover 桥接与 Limits 消费修改方案

## 1. 问题诊断

### 1.1 discover 模块与 config 系统脱节

**现状**: `discover::registry::classify_command()` 仅查询静态 `rules::RULES` 表。`handle_run()` 调用 `classify_command()` 确定 TechStack + subcommand，若 RULES 表中无匹配则直接报错退出。`main.rs:parse_arguments()` 中对 `config.commands` 的别名查找仅在**用户显式指定 tech_stack + command** 的路径（`analyzer cargo check`）中生效，对 `analyzer run` 路径不可达。

**影响**: 用户在配置文件中定义的 `[commands]` 别名在 `analyzer run` 子命令下不生效。例如：

```toml
[commands]
mylint = { exec = "npm run lint", tech_stacks = ["npm"] }
```

执行 `analyzer run mylint` 会报 `Unrecognized command 'mylint'`。

### 1.2 LimitsConfig 未被消费

**现状**: `LimitsConfig` (grep_max_results, grep_max_per_file, status_max_files, status_max_untracked, passthrough_max_chars) 已定义、加载、合并，但没有任何管道代码读取这些值。`AnalyzeOptions::from_config()` 只从 `FilterConfig` 读过滤参数。

**影响**: 用户修改 `[limits]` 配置段不产生任何效果。但由于 grep/status/walk 等对应操作还在规划阶段，当前最可操作的是让 `AnalyzeOptions` 携带这些值供未来使用。

## 2. 修改方案

### 2.1 discover + config 桥接

**策略**: 最小侵入 — 为 `classify_command()` 和 `rewrite_command()` 增加可选参数，不破坏现有调用方。主路径 `handle_run` 和 `handle_rewrite` 传入 config，测试保持向后兼容。

**实现步骤**:

1. `discover/registry.rs` 中新增 `classify_command_with_config(raw_cmd, commands, tech_stacks) -> Classification`
   - 先尝试静态 RULES 表匹配
   - 未命中时遍历 `commands`，以命令名匹配 command_name
   - 从 `cmd_config.tech_stacks.first()` 解析 TechStack
   - 使用 `cmd_config.exec` 作为 subcommand_template
   - 额外参数从 raw_cmd 剩余部分提取

2. 更新 `handle_run()` 和 `handle_rewrite()` 传递 `&config.commands`

3. 保留原始 `classify_command()` 函数用于测试和向后兼容（内部委托到新函数）

**关键设计决策**:
- 静态 RULES 优先于 config，保持确定性
- `config.commands` 中的每条命令使用 `tech_stacks.first()` 确定 TechStack，若无 tech_stacks 限制则无法确定 TechStack → 不匹配
- config 匹配的 `rule_index` 使用 `usize::MAX` 作为标记

### 2.2 LimitsConfig 消费

**策略**: 扩展 `AnalyzeOptions::from_config()` 读取 `config.limits`，让 limits 值流转到插件层。

**实现步骤**:

1. `core/types.rs` 中 `AnalyzeOptions` 增加 `limits` 字段（或拆分为独立字段）
2. `AnalyzeOptions::from_config()` 从 `config.limits` 复制值

**关键设计决策**:
- 将 `LimitsConfig` 直接内嵌到 `AnalyzeOptions` 中，作为 `pub limits: LimitsConfig` 字段
- 保持单一数据源 (LimitsConfig) 避免字段分散

## 3. 影响范围

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `src/discover/registry.rs` | 修改 | 新增 config-aware 分类函数 |
| `src/discover/mod.rs` | 修改 | 导出新符号 |
| `src/main.rs` | 修改 | handle_run/handle_rewrite 传递 config |
| `src/core/types.rs` | 修改 | AnalyzeOptions 增加 limits 字段 |
