# Filter Configuration Guide

## 1. 概述

Filter 是 analyzer 的输出后处理系统，用于在命令执行后对原始输出进行清洗、裁剪和格式化，去除构建工具输出的噪音（如 ANSI 转义码、TUI 边框、进度条、统计摘要等），只保留对代码分析有价值的内容。

## 2. 架构设计

### 2.1 过滤器来源与优先级

```
                    优先级 (由高到低)
┌─────────────────────────────────────────────────────────────┐
│  项目本地: <project>/.analyzer/filters.toml                  │  最高
│  - 向上遍历目录树查找                                        │
│  - 每个项目可独立定制过滤规则                                │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│  用户全局: ~/.config/analyzer/filters.toml                   │  中等
│  - 适用于当前用户的所有项目                                  │
│  - 默认过滤器和个性化设置                                    │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│  内置默认: src/filters/*.toml (编译时嵌入)                   │  最低
│  - 通过 build.rs 在编译时拼接所有 .toml 文件                  │
│  - 通过 include_str!() 嵌入二进制                            │
│  - 确保工具在任何环境下都有可用的默认行为                    │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 编译流程

```
src/filters/*.toml           build.rs                二进制
      │                         │                      │
      ├─ turbo.toml ──────────→│                      │
      ├─ (future) ... ────────→│ 拼接所有 .toml 文件    │
      └─ (future) ... ────────→│                      │
                                ↓                      │
                     $OUT_DIR/builtin_filters.toml    │
                                │                      │
                                └── include_str!() ───→│
                                (compile time embed)   │
```

### 2.3 运行时匹配流程

```
命令字符串 (如 "turbo run lint")
        │
        ↓
  FilterRegistry::find_filter()
        │
        ├── 1. 遍历 project_filters，正则匹配 match_command → 命中则返回
        ├── 2. 遍历 user_filters，正则匹配 match_command → 命中则返回
        ├── 3. 遍历 builtin_filters，正则匹配 match_command → 命中则返回
        └── 4. 无匹配 → None（直通，不做任何处理）
                │
                ↓
      compile_toml_filter()
                │
                ↓
      OutputPostProcessor 实例
      (strip_ansi, strip_tui_frames,
       strip_lines, keep_lines, replace,
       short_circuit, max_lines, truncate, on_empty)
```

## 3. 配置文件格式

### 3.1 顶层结构

```toml
# schema_version 可选，用于未来格式兼容
# schema_version = 1

[filters.<filter-name>]
description = "简短描述此过滤器的用途"
match_command = "匹配命令的正则表达式"
strip_ansi = true
strip_tui_frames = true
# ... 更多字段
```

### 3.2 完整字段说明

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `description` | string | 否 | - | 过滤器用途描述 |
| `match_command` | string (regex) | **是** | - | 匹配命令行的正则表达式 |
| `strip_ansi` | bool | 否 | false | 是否去除 ANSI 转义码（颜色/光标控制） |
| `strip_tui_frames` | bool | 否 | false | 是否去除 TUI 边框字符（`╭──╮ │ ── ├──┤` 等） |
| `strip_lines_matching` | string[] | 否 | [] | 移除完全匹配这些正则的行（噪音行） |
| `keep_lines_matching` | string[] | 否 | [] | 仅保留匹配这些正则的行 |
| `replace` | array | 否 | [] | 正则替换规则列表 |
| `short_circuit` | array | 否 | [] | 成功短路规则，命中后直接返回简短消息 |
| `max_lines` | usize | 否 | 0 (不限) | 输出最大行数（0 = 不限制） |
| `truncate_lines_at` | usize | 否 | 0 (不限) | 单行最大字符数（0 = 不截断） |
| `on_empty` | string | 否 | - | 过滤后输出为空时的回退消息 |

### 3.3 replace 规则

```toml
[[filters.<name>.replace]]
pattern = "匹配的正则表达式"
replacement = "替换成的文本"
```

### 3.4 short_circuit 规则

```toml
[[filters.<name>.short_circuit]]
pattern = "匹配的正则表达式"
message = "短路后输出的简短消息"
# unless 可选：如果输出中匹配此正则，则不触发短路
unless = "排除匹配的正则"
```

## 4. 内置过滤器示例

### 4.1 turbo

当前项目内置的唯一过滤器，用于处理 Turborepo 的 TUI 输出：

```toml
# src/filters/turbo.toml
[filters.turbo]
description = "Strip Turborepo TUI decoration, keep task results"
match_command = "^(turbo|pnpm exec turbo|npx turbo)\\b"
strip_ansi = true
strip_tui_frames = true

strip_lines_matching = [
    "^\\s*cache (hit|miss|bypass)",
    "^\\s*replaying logs",
    "^\\s*\\d+ packages in scope",
    "^\\s*Running \\w+ in \\d+ packages",
    "^\\s*Remote caching",
    "^\\s*Tasks:\\s*\\d+",
    "^\\s*Cached:\\s*",
    "^\\s*Time:\\s*",
    "^\\s*Duration:\\s*",
    "^\\s*Failed:\\s*",
    "^\\s*ERROR\\s+run failed:",
    "^\\s*\\d+ problems?\\s*\\(",
    "^\\s*Update available",
    "^\\s*Changelog:",
    "^\\s*Follow @turborepo",
    "^\\s*> .+@\\d+\\.\\d+\\.\\d+ \\w+",
    "^\\s*\\S+@\\S+ \\w+ >",
]

[[filters.turbo.short_circuit]]
pattern = "(?i)BUILD SUCCESS"
message = "Build succeeded"
unless = "(?i)error|fail|warning"

[[filters.turbo.short_circuit]]
pattern = "(?i)build succeeded"
message = "Build succeeded"
unless = "(?i)error|fail"

max_lines = 100
truncate_lines_at = 200
on_empty = "turbo: all tasks completed successfully"
```

## 5. 用户自定义过滤器

### 5.1 项目级添加新过滤器

在项目根目录创建 `.analyzer/filters.toml`：

```toml
# 新增一个 CMake 构建输出过滤器
[filters.cmake]
description = "Strip CMake configure noise, keep errors"
match_command = "^cmake\\b"
strip_ansi = true
strip_lines_matching = [
    "^-- The C compiler identification",
    "^-- The CXX compiler identification",
    "^-- Detecting C compiler",
    "^-- Detecting CXX compiler",
    "^-- Check for working",
    "^-- Configuring done",
    "^-- Generating done",
    "^-- Build files",
]

[filters.cmake.short_circuit]
pattern = "(?i)Build files have been written"
message = "CMake configure succeeded"

max_lines = 50
truncate_lines_at = 300
```

### 5.2 覆盖内置过滤器

在 `.analyzer/filters.toml` 中使用相同的 filter name 即可覆盖：

```toml
# 覆盖内置的 turbo 过滤器，增加更多噪音行过滤
[filters.turbo]
match_command = "^(turbo|pnpm exec turbo|npx turbo)\\b"
strip_ansi = true
strip_tui_frames = true
strip_lines_matching = [
    # ... 自定义规则
]
max_lines = 200
```

### 5.3 用户全局默认

在 `~/.config/analyzer/filters.toml` 中定义适用于所有项目的过滤器。

## 6. 代码结构映射

| 文件 | 功能 |
|------|------|
| `src/filters/*.toml` | 内置过滤器源码定义 |
| `build.rs` | 编译时拼接所有 .toml 为 builtin_filters.toml |
| `src/config/filter_registry.rs` | FilterRegistry: 三级加载与查找 |
| `src/config/filter_compiler.rs` | TOML → OutputPostProcessor 编译 |
| `src/config/modules/filter.rs` | FilterConfig: 项目/用户级过滤配置结构 |
| `src/core/utils.rs` | OutputPostProcessor: 运行时后处理引擎 |

## 7. 命令行集成

filter 的匹配在 `stream::run_analysis_pipeline()` 中自动触发。当命令执行完毕后：

1. 根据当前执行的命令行字符串，调用 `FilterRegistry::find_filter()` 查找匹配的过滤器
2. 如果找到，调用 `compile_toml_filter()` 生成 `OutputPostProcessor`
3. 对输出进行后处理（去 ANSI、去 TUI 边框、行过滤、替换、短路等）
4. 将处理后的输出传递给 Reporter 生成报告

用户无需手动指定过滤器——系统根据实际执行的命令自动匹配。

## 8. 扩展现有过滤器

参考项目 RTK (`ref/rtk-develop/src/filters/`) 包含约 60 个内置过滤器，覆盖以下类别：

| 类别 | 示例 |
|------|------|
| JavaScript/TypeScript | turbo, biome, oxlint, basedpyright |
| Rust/Cargo | (可通过 TOML 添加) |
| Python | uv-sync, pre-commit, yamllint |
| Java/JVM | gradle, mvn-build, liquibase |
| Go | (可通过 TOML 添加) |
| .NET | dotnet-build |
| 容器/基础设施 | helm, terraform-plan, tofu-plan, tofu-validate, skopeo, hadolint |
| 系统工具 | systemctl-status, df, du, ps, stat, iptables |
| 包管理 | brew-install, composer-install, poetry-install, bundle-install |
| Shell/脚本 | shellcheck, make, just, task, jq |
| Git/版本控制 | jj, yadm |
| 其他 | ssh, rsync, ping, mise, ansible-playbook, jira |

要添加新的内置过滤器，只需在 `src/filters/` 下创建对应的 `.toml` 文件，重新编译即可。`build.rs` 会自动发现并嵌入。

---

*最后更新: 2026-06-27*
