# Config & Stream Module Integration Plan

## 1. Configuration Module Integration (`src/core/config.rs`)

### 1.1 Current State

The current `config.rs` provides a TOML-based configuration schema:

```
Config
├── report: ReportConfig      (format, verbose, verbosity)
├── commands: HashMap          (exec, tech_stacks per command)
└── filter: FilterConfig       (ignore_paths, noise_patterns, keep_patterns,
                                max_lines, max_line_length, strip_ansi)
```

`Config::load()` scans for `analyzer.toml` / `.analyzer.toml` / `.analyzer/config.toml` and deserializes them. It is **not wired** into `main.rs` — the module exists but is dead code (`#![allow(dead_code)]`).

### 1.2 What Needs Configuration

The following CLI-controlled behaviors are suitable for configuration file support:

| Feature           | Current CLI Option             | Config Mapping          | Priority     |
| ----------------- | ------------------------------ | ----------------------- | ------------ |
| Report format     | `--format`                     | `report.format`         | CLI > Config |
| Verbosity         | implicit (none)                | `report.verbosity`      | Config only  |
| Noise patterns    | `--filter-noise` (nonexistent) | `filter.noise_patterns` | Config only  |
| Keep patterns     | (none)                         | `filter.keep_patterns`  | Config only  |
| ANSI stripping    | (always on)                    | `filter.strip_ansi`     | Config only  |
| Max output lines  | (none)                         | `filter.max_lines`      | Config only  |
| Ignore paths      | (none)                         | `filter.ignore_paths`   | Config only  |
| Command overrides | (none)                         | `commands.<name>.exec`  | Config only  |

Items marked "Config only" are features available only via TOML config — they have no CLI equivalent, which is why they remain unused.

### 1.3 Integration Approach

The goal is to **seed `AnalyzeOptions` from TOML config**, then let CLI args override specific fields. This requires:

1. Load `Config` in `main.rs` early in `parse_arguments()`
2. Map `Config` fields → `AnalyzeOptions` defaults
3. Apply CLI arg overrides on top

#### Pseudocode for main.rs integration

```rust
fn parse_arguments() -> (TechStack, AnalyzeOptions) {
    // Step 1: Load from config file first
    let config = Config::load();
    let mut options = AnalyzeOptions::from_config(&config);

    // Step 2: Parse CLI args, overriding seeded values
    for i in 0..args.len() {
        match arg {
            "--format" => /* override options.report_format */,
            "--output" => /* override options.output_file */,
            // ... existing CLI parsing ...
        }
    }
    // ...
}
```

#### 1.3.1 `AnalyzeOptions::from_config()` Implementation

```rust
impl AnalyzeOptions {
    pub fn from_config(config: &Config) -> Self {
        let mut options = AnalyzeOptions::default();

        // Report settings
        options.report_format = match config.report.format.as_str() {
            "json" => ReportFormat::Json,
            "html" => ReportFormat::Html,
            _ => ReportFormat::Markdown,
        };
        options.verbosity = match config.report.verbosity.as_str() {
            "minimal" => Verbosity::Minimal,
            "verbose" => Verbosity::Verbose,
            _ => Verbosity::Normal,
        };

        // Filter settings (will be used when wiring filter into analysis)
        // options.filter_paths = config.filter.ignore_paths.clone(); // possible mapping

        options
    }
}
```

#### 1.3.2 Why This Is Safe to Defer

The TOML config integration is **non-breaking** — without a config file present, `Config::load()` returns `Config::default()`, and `AnalyzeOptions::from_config(&Config::default())` is equivalent to `AnalyzeOptions::default()`. So adding config loading is purely additive and backward-compatible.

#### 1.3.3 Concrete Implementation Steps (for later phase)

1. Add `AnalyzeOptions::from_config(&Config) -> Self` in `types.rs`
2. In `main.rs`, call `Config::load()` before `parse_arguments()`
3. Pass config-merged options through the existing flow
4. Remove `#[allow(dead_code)]` from `config.rs`

---

## 2. Stream Module Integration (`src/core/stream.rs`)

### 2.1 Current State

`stream.rs` defines a **processing pipeline** with stages:

```
Pipeline Stages:
  raw output  →  [ParseStage]   →  [FilterStage]   →  [AnalyzeStage]  →  AnalysisResult
                      │                  │                   │
                  (parser)        (path/level filter)   (from_issues)
```

Each stage implements `PipelineStage<Input, Output>` and returns `StageResult<T>` with degradation support (`Complete / Degraded / Failed`).

The pipeline is currently **dead code** because all plugins manually do:

```rust
let issues = self.parser.parse(&output).data_or_default_owned();
let result = AnalysisResult::from_issues(issues);
Ok(self.filter_issues(result, options))
```

Instead of using the pipeline abstraction.

### 2.2 Integration Approach: Optional Utility Function

The stream pipeline should remain **optional** — plugins CAN use it, but are not required to. The cleanest integration is to provide a **standalone convenience function** in `stream.rs` that plugins can optionally call:

```rust
/// Process command output through the full pipeline:
///   parse → filter (by options) → analyze
///
/// This is an optional helper. Plugins can either call this function
/// or manually do the parse-filter-analyze steps themselves.
pub fn run_analysis_pipeline(
    parser: &dyn OutputParser,
    output: &str,
    options: &AnalyzeOptions,
) -> Result<AnalysisResult, AnalyzerError> {
    // 1) Parse
    let issues = parser.parse(output).data_or_default_owned();
    // 2) Filter (using shared filter_by_options)
    let result = AnalysisResult::from_issues(issues);
    Ok(result.filter_by_options(options))
}
```

Wait — this is actually identical to what plugins already do. The pipeline's real value is in its **stage-based degradation model** and **extensibility** (adding new stages in between). But since plugins currently have no need for degradation, the integration should be pragmatic:

#### 2.2.1 Provide a Factory Function That Builds a Configured Pipeline

````rust
/// Build a fully configured pipeline from AnalyzeOptions.
/// The pipeline handles parse → filter → analyze, and supports
/// degradation reporting.
///
/// # Usage (optional, in any plugin's analyze()):
/// ```ignore
/// let mut pipeline = build_pipeline(&self.parser, options);
/// match pipeline.run(&self.parser, &output, None) {
///     StageResult::Complete(result) => Ok(result),
///     StageResult::Degraded(result, _warnings) => Ok(result),
///     StageResult::Failed(_warnings) => Err(...),
/// }
/// ```
pub fn build_pipeline<'a>(
    _parser: &'a dyn OutputParser,
    _options: &AnalyzeOptions,
) -> ProcessingPipeline {
    ProcessingPipeline::new()
}
````

#### 2.2.2 Enhance `ProcessingPipeline::run()` to Support Post-Processing

Currently `ProcessingPipeline::run()` doesn't apply `OutputPostProcessor` (ANSI stripping, noise filtering, line truncation). Add this as an optional stage.

### 2.3 Concrete Integration Steps

1. **Add `build_analysis_pipeline()` function** in `stream.rs` — a factory that builds a pipeline from `AnalyzeOptions`
2. **Enhance `ProcessingPipeline::run()`** to accept optional `OutputPostProcessor` stage
3. **Demonstrate in one plugin** (e.g., `GoAnalyzer`) by replacing manual parse-filter with pipeline call
4. **Keep `#[allow(dead_code)]`** on the pipeline infrastructure itself, remove it only from the factory function

### 2.4 Why Stream Is Optional

The pipeline abstraction adds value when:

- You need **degradation reporting** (warnings from each stage)
- You want to **add/remove stages** without changing plugin code
- You need **consistent error handling** across all plugins

For simple cases (which covers all current plugins), the manual 3-line flow is more readable. The pipeline is a **future-proofing abstraction** that becomes valuable when analysis logic grows more complex.

### 2.5 Module Dependency Flow After Integration

```
Plugin::analyze()
    │
    ├── (optional) → stream::build_analysis_pipeline(parser, options)
    │                    │
    │                    ├── OutputPostProcessor (from utils)
    │                    ├── parser.parse()
    │                    └── AnalysisResult::filter_by_options()
    │
    └── (direct)   → parser.parse() + AnalysisResult::filter_by_options()
```
