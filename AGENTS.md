# AGENTS.md - Project Architecture Overview

## Overview

A multilingual build tool error analyzer with a plugin-based architecture, supporting technology stacks such as Cargo, NPM, and Mypy.

## Core Architecture

```
CLI Entry → Core Module → Plugin Module
```

### Core Module (core/)

| Component          | Responsibility                                            |
| ------------------ | --------------------------------------------------------- |
| `types.rs`         | Common data types (e.g., Issue, Location, AnalysisResult) |
| `parser.rs`        | Output parsing interface                                  |
| `analyzer.rs`      | Unified analyzer interface                                |
| `reporter/*`       | Report generation (Markdown/JSON/Text/HTML)               |
| `command.rs`       | Command construction and execution                        |
| `base_analyzer.rs` | Generic analyzer implementation                           |

### Plugin Module (plugins/)

| Plugin | Status      | Supported Commands              |
| ------ | ----------- | ------------------------------- |
| Cargo  | Implemented | `check`, `clippy`, `check-test` |
| NPM    | Implemented | `lint`, `type-check`, `audit`   |
| Mypy   | Implemented | `mypy`, `mypy --strict`         |

...

## Data Flow

```
User Input → CLI Parsing → Plugin Selection → Command Execution → Output Parsing → Filtering & Statistics → Report Generation
```

## Extending with New Plugins

1. Create a directory under `plugins/`
2. Implement `BuildAnalyzer` and `OutputParser`
3. Register the plugin in `plugins.rs`

## Error Types

- `CommandFailed` - Command execution failure
- `ParseError` - Parsing error
- `IoError` - I/O error
- `NotApplicable` - Analyzer not applicable

## Language

Always use English in code, comments, logging, error info or other string literal. Use Chinese in docs (except code block)
**Never use any Chinese in any code files or code block.**
