# Core Module

Core traits and types for the analyzer.

## Structure

| Component | Description |
|-----------|-------------|
| `types.rs` | Core data types (`Issue`, `Location`, `AnalysisResult`, etc.) |
| `parser.rs` | `OutputParser` trait and `BaseParser` implementation |
| `analyzer.rs` | `BuildAnalyzer` trait for plugin-based analysis |
| `utils.rs` | Shared utility functions (ANSI stripping, text processing) |
| `command.rs` | Command construction, execution, and cross-platform lookup |
| `reporter/` | Report generation (Markdown, JSON, HTML) |
| `test_analyzer.rs` | `TestAnalyzer` and `TestOutputParser` traits |

## Key Concepts

1. **Plugin Architecture**: `BuildAnalyzer` trait provides analysis, `OutputParser` provides parsing. Each plugin implements both.
2. **Report Format**: Analysis results are converted to Markdown / JSON / HTML via the reporter module.
3. **Extending**: Add a new plugin in `src/plugins/`, implement `BuildAnalyzer` + `OutputParser`, register in `plugins/mod.rs`.