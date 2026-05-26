# Configuration Module Design

## 1. Configuration Hierarchy

```
CLI Arguments (highest priority)
    ↑ overrides
Environment Variables (ANALYZER_*)
    ↑ overrides
Project Config (<project>/analyzer.toml / .analyzer.toml / .analyzer/config.toml)
    ↑ overrides
Global Config (~/.config/analyzer/config.toml)
    ↑ overrides
Default Values (lowest priority)
```

## 2. Module Structure

```
src/config/
├── mod.rs                   # Module exports
├── global.rs                # AppConfig (global config entry point)
├── project.rs               # ProjectAppConfig (project-level overrides)
├── loader.rs                # ConfigLoader (hierarchical loading)
├── env_loader.rs            # Environment variable loading & override
├── serde_helpers.rs         # Custom serde helpers
└── modules/
    ├── mod.rs               # Sub-module unified exports
    ├── report.rs            # Report settings
    ├── filter.rs            # Filter settings (migrated from core/config.rs)
    └── commands.rs          # Command overrides (migrated from core/config.rs)
```

## 3. Data Flow

```text
TOML Config File(s)
    ↓ deserialize
AppConfig / ProjectAppConfig
    ↓ merge
AppConfig (merged)
    ↓ AnalyzeOptions::from_config()
AnalyzeOptions (seeded from config)
    ↓ CLI args override
AnalyzeOptions (final, CLI takes precedence)
    ↓
Plugin::analyze() + stream::run_analysis_pipeline()
```

## 4. Key Design Decisions

### 4.1 No Global Singleton

Unlike the reference architecture (ref/config/settings.rs), this design does NOT use a global singleton.
The analyzer is a CLI tool, not a long-running service — config is loaded once at startup and passed
through function parameters. This avoids unnecessary complexity.

### 4.2 Config vs AnalyzeOptions Separation

- `AppConfig` / `ProjectAppConfig`: TOML-serializable config structures. Designed for persistence.
- `AnalyzeOptions`: Runtime options struct consumed by plugins. Contains CLI-specific fields
  (like `output_file`, `filter_paths`) that don't belong in persistent config.
- Bridge: `AnalyzeOptions::from_config(&AppConfig)` maps config → runtime options.

### 4.3 Backward Compatibility

Without any config file present, `ConfigLoader::load()` returns `AppConfig::default()`,
and `AnalyzeOptions::from_config(&AppConfig::default())` is equivalent to
`AnalyzeOptions::default()`. Adding config loading is purely additive.

## 5. What Becomes Configurable

| Category | Items | Config Source |
|----------|-------|---------------|
| Report | format, verbosity, verbose, success_short_circuit | Global + Project |
| Filter | ignore_paths, noise_patterns, keep_patterns, max_lines, max_line_length, strip_ansi | Global + Project |
| Commands | per-tech-stack exec override, tech_stacks | Global + Project |
| CLI-only | --output, --filter-paths, --filter-warnings, cargo workspace/feature options | CLI args only |

## 6. Implementation Stages

1. Create `src/config/` directory + sub-modules, migrate existing config content
2. Implement `ConfigLoader` with hierarchical loading
3. Implement `env_loader.rs` for environment variable support
4. Add `AnalyzeOptions::from_config()` in `types.rs`
5. Integrate into `main.rs` with CLI override
6. Clean up old `core/config.rs`, remove `#[allow(dead_code)]`
7. Build and fix compilation errors