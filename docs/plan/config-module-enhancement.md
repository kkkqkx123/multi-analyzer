# 配置模块增强方案

基于 `ref/rtk-develop/src/core/config.rs` 的参考实现，对照当前 `src/config/` 模块与设计文档 `configuration-system-design.md` 的规划，列出缺失功能的补全方案。

---

## 1. 现状分析

### 1.1 当前已实现

| 模块 | 文件 | 功能 |
|------|------|------|
| AppConfig | `src/config/global.rs` | 全局配置：report + filter + commands |
| ReportConfig | `src/config/modules/report.rs` | 报告配置：format / verbose / verbosity / success_short_circuit |
| FilterConfig | `src/config/modules/filter.rs` | 过滤配置：ignore_paths / noise_patterns / keep_patterns / max_lines / max_line_length / strip_ansi / strip_tui_frames |
| CommandConfig | `src/config/modules/commands.rs` | 命令配置：exec + tech_stacks |
| ConfigLoader | `src/config/loader.rs` | 三级加载：全局 > 项目 > 环境变量 |
| ProjectConfigPaths | `src/config/project.rs` | 项目配置查找：analyzer.toml / .analyzer.toml / .analyzer/analyzer.toml |
| EnvLoader | `src/config/env_loader.rs` | ANALYZER_* 环境变量覆盖 |
| FilterRegistry | `src/config/filter_registry.rs` | 三级过滤器注册表：项目 / 用户 / 内置 |
| FilterCompiler | `src/config/filter_compiler.rs` | TOML → OutputPostProcessor 编译 |

### 1.2 设计文档规划但未实现

`docs/configuration-system-design.md` 规划了以下结构，但当前代码中未完整实现：

```rust
pub struct Config {
    pub version: String,                                  // [缺失]
    pub global: GlobalConfig {                            // [简化] ReportConfig 替代
        default_format, filter_warnings, default_output
    },
    pub commands: HashMap<String, CommandConfig> {        // [部分实现] 缺少 description / enabled
        exec, description, tech_stacks, enabled
    },
    pub tech_stacks: HashMap<String, TechStackConfig> {   // [缺失]
        commands, scripts, test_framework
    },
}
```

### 1.3 RTK 有但本模块缺失的能力

| RTK 功能 | 对应文件 | 说明 |
|----------|---------|------|
| `Config::save()` / `create_default()` | `ref/rtk-develop/src/core/config.rs:153-183` | 配置持久化写回 |
| `show_config()` | `ref/rtk-develop/src/core/config.rs:190-206` | 打印完整配置 |
| `TeeConfig` | `ref/rtk-develop/src/core/tee.rs:248-269` | 命令失败时保存原始输出 |
| `LimitsConfig` | `ref/rtk-develop/src/core/config.rs:122-146` | 细粒度输出限制 |
| `DisplayConfig` | `ref/rtk-develop/src/core/config.rs:74-89` | 输出显示控制 |
| `TrackingConfig` | `ref/rtk-develop/src/core/config.rs:56-72` | 执行历史/SQLite 统计 |
| `TelemetryConfig` | `ref/rtk-develop/src/core/config.rs:113-120` | 匿名使用统计 + RGPD consent |
| `HooksConfig` | `ref/rtk-develop/src/core/config.rs:27-54` | 钩子 exclude/transparent_prefixes |

---

## 2. 修改方案

### Phase 1: 核心配置数据模型完善

优先级：高 | 预计影响文件：4

#### 1.1 CommandConfig 补全字段

目标：补充设计文档规划的 `description` 和 `enabled` 字段。

文件变更：

- `src/config/modules/commands.rs`

```rust
// Before:
pub struct CommandConfig {
    pub exec: String,
    #[serde(default)]
    pub tech_stacks: Vec<String>,
}

// After:
pub struct CommandConfig {
    pub exec: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tech_stacks: Vec<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool { true }
```

#### 1.2 AppConfig 增加 version 字段

目标：支持配置文件版本管理，兼容未来格式变更。

文件变更：

- `src/config/global.rs`

```rust
// AppConfig 增加:
#[serde(default = "default_version")]
pub version: String,

fn default_version() -> String { "1.0".to_string() }
```

#### 1.3 新增 TechStackConfig

目标：实现设计文档中技术栈级配置覆盖机制，支持 `tech_stack.<name>.commands.<cmd>` 覆盖、scripts 映射、test_framework 声明。

文件变更：

- 新增 `src/config/modules/tech_stack.rs`
- 修改 `src/config/modules/mod.rs`

数据结构：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TechStackConfig {
    #[serde(default)]
    pub commands: HashMap<String, CommandConfig>,
    #[serde(default)]
    pub scripts: HashMap<String, String>,
    #[serde(default)]
    pub test_framework: Option<String>,
}
```

合并逻辑：`AppConfig` 增加 `tech_stacks: HashMap<String, TechStackConfig>` 字段，`merge_with_project()` 中项目级 tech_stack 覆盖全局级。

#### 1.4 AppConfig 增加 TechStack 字段

文件变更：

- `src/config/global.rs`
- `src/config/project.rs`

```rust
// AppConfig:
#[serde(default)]
pub tech_stacks: HashMap<String, TechStackConfig>,

// merge_with_project() 增加 tech_stacks 合并逻辑
```

---

### Phase 2: 配置持久化 (Save/Load/CreateDefault)

优先级：高 | 预计影响文件：2

目标：补齐 RTK 标准的 `Config::save()` / `Config::create_default()` 能力，让用户可通过 CLI 初始化配置。

#### 2.1 Config 持久化方法

文件变更：

- `src/config/global.rs` — 在 `impl AppConfig` 增加方法

```rust
impl AppConfig {
    /// 从全局路径加载
    pub fn load() -> Result<Self> {
        let path = Self::global_config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    /// 保存到全局路径
    pub fn save(&self) -> Result<()> {
        let path = Self::global_config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// 创建默认配置文件，返回路径
    pub fn create_default() -> Result<PathBuf> {
        let config = Self::default();
        config.save()?;
        Ok(Self::global_config_path())
    }

    /// 全局配置路径: ~/.config/analyzer/config.toml
    fn global_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("analyzer")
            .join("config.toml")
    }
}
```

#### 2.2 show_config 命令支持

文件变更：

- `src/config/global.rs`

```rust
/// 打印当前完整配置（含路径信息）
pub fn show_config() -> Result<()> {
    let path = AppConfig::global_config_path();
    println!("Config: {}", path.display());
    println!();
    if path.exists() {
        let config = AppConfig::load()?;
        println!("{}", toml::to_string_pretty(&config)?);
    } else {
        println!("(default config, file not created)");
        println!();
        let config = AppConfig::default();
        println!("{}", toml::to_string_pretty(&config)?);
    }
    Ok(())
}
```

CLI 集成（`src/main.rs` 增加子命令）：
```rust
// analyze config show   → 调用 show_config()
// analyze config init   → 调用 AppConfig::create_default()
```

---

### Phase 3: TeeConfig — 原始输出保存

优先级：中 | 预计影响文件：3

目标：在命令执行失败时，将原始输出保存到磁盘，提供 `[full output: path]` 提示，改善排错体验。

#### 3.1 数据结构

文件变更：

- 新增 `src/config/modules/tee.rs`
- 修改 `src/config/modules/mod.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TeeMode {
    Failures,  // 默认：仅失败时保存
    Always,
    Never,
}

impl Default for TeeMode {
    fn default() -> Self { Self::Failures }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeConfig {
    #[serde(default = "default_tee_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub mode: TeeMode,
    #[serde(default = "default_tee_max_files")]
    pub max_files: usize,    // 默认 20
    #[serde(default = "default_tee_max_file_size")]
    pub max_file_size: usize, // 默认 1MB
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<PathBuf>,
}
```

#### 3.2 TeeConfig 集成到 AppConfig

- 修改 `src/config/global.rs`：`AppConfig` 增加 `tee: TeeConfig` 字段
- 修改 `src/config/loader.rs`：load 时加载 TeeConfig

#### 3.3 Tee 写入核心逻辑

文件变更：

- 新增 `src/config/tee_writer.rs`

参考 `ref/rtk-develop/src/core/tee.rs` 实现：

```rust
/// 写入原始输出到磁盘，自动轮转旧文件
/// 返回 `[full output: ~/path]` 或 None（条件不满足时）
pub fn tee_raw(raw: &str, command_slug: &str, exit_code: i32) -> Option<String>;
```

功能要点：
- 约 500 字节以下的小输出不保存
- 按 exit_code 决定是否保存（Failures 模式下 exit != 0 才保存）
- 文件命名：`{epoch}_{sanitized_slug}.log`
- 超过 `max_files` 时自动清理旧文件
- 超过 `max_file_size` 时截断并标注
- 支持 `ANALYZER_TEE=0` / `ANALYZER_TEE_DIR` 环境变量覆盖

#### 3.4 Tee 集成到命令执行流程

- 修改 `src/core/command.rs`：在 `execute()` 返回前调用 `tee_raw()`

---

### Phase 4: 命令发现增强 — TechStack 配置驱动

优先级：中 | 预计影响文件：3

目标：让 discover 模块能根据 `TechStackConfig.scripts` 映射识别 npm scripts 的实际测试/检查框架。

#### 4.1 脚本映射查询接口

文件变更：

- `src/config/global.rs` 或新增 `src/config/resolver.rs`

```rust
impl AppConfig {
    /// 根据技术栈名 + 脚本名，查找实际框架
    /// 例如: ("npm", "test") → Some("jest")
    pub fn resolve_script(&self, tech_stack: &str, script: &str) -> Option<String>;
    
    /// 查找技术栈的 test_framework
    /// 例如: ("pnpm") → Some("vitest")
    pub fn test_framework_for(&self, tech_stack: &str) -> Option<String>;
}
```

#### 4.2 集成到 discover 模块

- 修改 `src/discover/` 模块：使用 resolver 方法替代硬编码的脚本→框架映射

---

### Phase 5: LimitsConfig — 细粒度输出限制

优先级：低 | 预计影响文件：2

目标：增加针对不同操作的细粒度输出限制，与 RTK 的 `LimitsConfig` 对齐。

#### 5.1 数据结构

文件变更：

- 新增 `src/config/modules/limits.rs`
- 修改 `src/config/modules/mod.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    #[serde(default = "default_grep_max_results")]
    pub grep_max_results: usize,    // 默认 200
    #[serde(default = "default_grep_max_per_file")]
    pub grep_max_per_file: usize,   // 默认 25
    #[serde(default = "default_status_max_files")]
    pub status_max_files: usize,    // 默认 15
    #[serde(default = "default_status_max_untracked")]
    pub status_max_untracked: usize, // 默认 10
    #[serde(default = "default_passthrough_max_chars")]
    pub passthrough_max_chars: usize, // 默认 2000
}
```

#### 5.2 集成到 AppConfig

- 修改 `src/config/global.rs`：增加 `limits: LimitsConfig` 字段
- 修改 `src/config/loader.rs`：load 时加载 LimitsConfig

---

### Phase 6: 扩展预留 (Optional)

以下功能根据实际需求决定是否在本轮实现：

| 功能 | RTK 参考 | 说明 |
|------|---------|------|
| DisplayConfig | `ref/rtk-develop/src/core/config.rs:74-89` | colors / emoji / max_width |
| TrackingConfig | `ref/rtk-develop/src/core/config.rs:56-72` | SQLite 执行历史、统计 |
| TelemetryConfig | `ref/rtk-develop/src/core/config.rs:113-120` | 匿名使用统计 + consent |
| HooksConfig | `ref/rtk-develop/src/core/config.rs:27-54` | exclude_commands / transparent_prefixes |
| 二进制目录配置层 | `configuration-system-design.md:26-30` | binary-dir/analyzer.toml |
| 命令别名 | `configuration-system-design.md` Phase 3 | alias 映射 |
| 配置验证 | `configuration-system-design.md` Phase 3 | 检查命令格式/技术栈合法性 |

---

## 3. 实施顺序

```
Phase 1 (核心数据模型)
  ├── 1.1 CommandConfig 补全
  ├── 1.2 version 字段
  ├── 1.3 TechStackConfig 新增
  └── 1.4 AppConfig 集成 TechStack

Phase 2 (配置持久化)
  ├── 2.1 save / load / create_default
  └── 2.2 show_config 命令

Phase 3 (Tee 输出保存)
  ├── 3.1 TeeConfig 数据结构
  ├── 3.2 AppConfig 集成
  ├── 3.3 tee_writer.rs
  └── 3.4 command.rs 集成

Phase 4 (TechStack 脚本映射)
  ├── 4.1 resolve_script / test_framework_for
  └── 4.2 discover 集成

Phase 5 (LimitsConfig)
  ├── 5.1 数据结构
  └── 5.2 AppConfig 集成

Phase 6 (扩展预留)
  └── 按需选择
```

---

## 4. 关键文件变更清单

| 文件 | 变更类型 | Phase |
|------|---------|-------|
| `src/config/modules/commands.rs` | 修改 | 1 |
| `src/config/modules/tech_stack.rs` | 新增 | 1 |
| `src/config/modules/tee.rs` | 新增 | 3 |
| `src/config/modules/limits.rs` | 新增 | 5 |
| `src/config/modules/mod.rs` | 修改 | 1, 3, 5 |
| `src/config/global.rs` | 修改 | 1, 2, 3, 4, 5 |
| `src/config/project.rs` | 修改 | 1 |
| `src/config/loader.rs` | 修改 | 3, 5 |
| `src/config/tee_writer.rs` | 新增 | 3 |
| `src/config/resolver.rs` | 新增 | 4 |
| `src/core/command.rs` | 修改 | 3 |
| `src/discover/` | 修改 | 4 |
| `src/main.rs` | 修改 | 2 |
| `tests/` | 新增测试 | 1-5 |

---

## 5. 向后兼容保证

1. 所有新增字段使用 `#[serde(default)]`，旧配置文件可正常解析
2. `ConfigLoader::load()` 行为不变；新增的 `AppConfig::load()` 为独立加载方法
3. `TeeConfig` 默认 `enabled: true` + `mode: Failures`，不影响正常流程
4. `CommandConfig` 新增字段均有缺省值，现有 toml 配置无需修改

---

*文档版本: 1.0*
*最后更新: 2026-06-24*
*参考: `ref/rtk-develop/src/core/config.rs`, `docs/configuration-system-design.md`*
