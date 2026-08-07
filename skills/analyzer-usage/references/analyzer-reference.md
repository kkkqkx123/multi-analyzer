# Analyzer Reference

Detailed reference for advanced options and configuration features.
For quick start and common usage, see the main [SKILL.md](../SKILL.md).

## Global Options (Full Reference)

| Option | Description |
| ------ | ----------- |
| `-h, --help` | Show help message |
| `--version` | Show version |
| `--filter-warnings` | Filter out all warnings, only show errors |
| `--filter-paths <paths>` | Filter errors by file paths (comma-separated) |
| `--verbose` | Show detailed progress information on stderr |
| `-q, --quiet` | Suppress all informational messages (stderr) |
| `-o, --output, --file, -f <file>` | Write report to file instead of stdout |
| `--format <format>` | Report format: `markdown`, `json`, `html`, `raw`, `raw-json` |
| `--no-short-circuit` | Disable success short-circuit (always show full report) |
| `--max-issues <N>` | Limit analysis to the first N issues (default: unlimited) |
| `--stdout` | **No-op** (stdout is the default; kept for backward compatibility) |

## Report Formats (Detail)

| Format | Description | Extension | Use Case |
| ------ | ----------- | --------- | -------- |
| `markdown` | Human-readable Markdown report with issue grouping, severity levels, and statistics | `.md` | General reading, documentation |
| `json` | Structured JSON report with metadata, summary, and issue details | `.json` | Machine consumption, CI pipelines |
| `html` | HTML report with styled output, suitable for CI/browser viewing | `.html` | Browser preview, CI artifacts |
| `raw` | Pipe-delimited: `LEVEL\|CODE\|FILE:LINE:COL\|MESSAGE` | `.txt` | Grep/awk pipelines, terminal display |
| `raw-json` | JSON lines (one JSON object per line), streaming-friendly | `.jsonl` | Stream processing, log aggregation |

## Run / Rewrite Exit Codes

### Run Exit Codes

| Code | Meaning |
| ---- | ------- |
| 0 | Success (rewritten and executed successfully) |
| 1 | No matching rule / execution failed |
| 2 | Subcommand not supported |

### Rewrite Exit Codes

| Code | Meaning |
| ---- | ------- |
| 0 | Successfully rewritten (command printed to stdout) |
| 1 | No matching rule / execution failed |

## Cargo Crate-Specific Options

### Workspace Options

| Option | Description |
| ------ | ----------- |
| `--workspace` | Analyze all workspace members |
| `-p, --package <SPEC>` | Analyze specific package (can be used multiple times) |
| `--exclude <SPEC>` | Exclude specific package from analysis |

### Target Options

| Option | Description |
| ------ | ----------- |
| `--lib` | Analyze only the library target |
| `--bin <NAME>` | Analyze specific binary target |
| `--bins` | Analyze all binary targets |
| `--test <NAME>` | Analyze specific test target |
| `--tests` | Analyze all test targets |
| `--example <NAME>` | Analyze specific example target |
| `--examples` | Analyze all example targets |
| `--bench <NAME>` | Analyze specific benchmark target |
| `--benches` | Analyze all benchmark targets |
| `--all-targets` | Analyze all targets |

### Feature Options

| Option | Description |
| ------ | ----------- |
| `--features <FEATURES>` | Space-separated list of features to enable |
| `--all-features` | Enable all available features |
| `--no-default-features` | Do not enable the default feature |

### Cargo Examples (Advanced)

```bash
# Workspace analysis
analyzer cargo check --workspace
analyzer cargo check --package my-crate

# Target-specific analysis
analyzer cargo check --lib
analyzer cargo check --bin my-app
analyzer cargo check --tests --all-features
analyzer cargo clippy --workspace --all-targets
analyzer cargo check --package foo --features "feat1 feat2"

# With output options
analyzer cargo "test" --filter-warnings
analyzer cargo "check" --format json -o report.json
```

## C++ Build Options

| Option | Description |
| ------ | ----------- |
| `--source-dir <DIR>` | Source directory for CMake/GCC/Clang builds |
| `--build-dir <DIR>` | Build directory for CMake builds |
| `--cmake-generator <GEN>` | CMake generator (e.g. "Ninja", "Unix Makefiles") |
| `--target <NAME>` | Build target name |
| `--target-files <FILES>` | Comma-separated target source files |
| `-I, --include-path <DIR>` | Add include search path (repeatable) |
| `-D, --define <MACRO>` | Add preprocessor define (repeatable) |
| `--cpp-std <STANDARD>` | C++ standard (e.g. c++17, c++20) |

## Test Analysis

The analyzer detects test subcommands (commands containing "test") and automatically runs test-specific analysis. When a test command is detected:

1. The test framework is resolved from the configuration (if declared)
2. Test output is parsed for pass/fail/ignore status
3. A combined report is generated showing compile issues + test results

```bash
# Run test analysis with framework detection
analyzer cargo "test" --verbose

# Run with custom output format
analyzer pytest "-v" --format json -o test_report.json

# Test analysis with config-defined framework
# (.analyzer.toml: [tech_stacks.pnpm] test_framework = "vitest")
analyzer pnpm "test" --verbose
```

## Configuration (Full Reference)

### Configuration File Locations

- **Global**: `~/.config/analyzer/config.toml`
- **Project**: `.analyzer.toml` in the project root (overrides global)

### Global Configuration (Default Values)

```toml
version = "1.0"

[report]
format = "markdown"
verbosity = "normal"
success_short_circuit = true

[filter]
strip_ansi = true
strip_tui_frames = true
max_lines = 0
max_line_length = 0
noise_patterns = []
keep_patterns = []

[tee]
enabled = true
mode = "failures"
max_files = 20
max_file_size = 1048576
```

### Project Configuration Example

```toml
version = "1.0"

[report]
format = "json"
verbosity = "verbose"
success_short_circuit = false

[filter]
strip_ansi = false
noise_patterns = ["warning: unused import"]

[commands.typecheck]
exec = "npm run typecheck"
description = "Run TypeScript type checker"
tech_stacks = ["npm", "pnpm", "yarn"]
enabled = true

[tech_stacks.npm]
test_framework = "jest"

[tech_stacks.pnpm]
test_framework = "vitest"

[tech_stacks.npm.scripts]
test = "jest"
lint = "eslint"
```

### Configuration Sections

| Section | Description |
| ------- | ----------- |
| `[report]` | Report format, verbosity, and short-circuit behavior |
| `[filter]` | Output filtering: ANSI stripping, TUI frame stripping, line limits, noise/keep patterns |
| `[commands.<name>]` | Command aliases: exec command, description, restricted tech stacks, enabled flag |
| `[tech_stacks.<name>]` | Tech stack settings: test framework, script-to-framework mapping |
| `[tee]` | Tee output settings: enable/disable, mode (failures/always/never), file limits |

### Command Aliases

Define custom command aliases in `.analyzer.toml`:

```toml
[commands.lint]
exec = "eslint src/"
description = "Run ESLint on source"
tech_stacks = ["npm", "pnpm", "yarn"]
enabled = true

[commands.ci]
exec = "cargo check --workspace --all-targets"
description = "CI check"
tech_stacks = ["cargo"]
```

Then use them directly:

```bash
analyzer npm "lint"
analyzer cargo "ci"
```

### Script Resolution

Map npm/pnpm/yarn script names to actual test frameworks:

```toml
[tech_stacks.pnpm]
test_framework = "vitest"

[tech_stacks.pnpm.scripts]
test = "vitest run"
lint = "eslint src/"
```

## Command Discovery Engine

The analyzer includes a built-in discovery engine that maps raw shell commands to tech stacks. This powers the `run` and `rewrite` subcommands.

```bash
# Auto-detect and analyze
analyzer run "cargo check --all-targets"

# Preview what would be analyzed
analyzer rewrite "npm run lint"
```

The discovery engine supports:
- Compound command splitting (`&&`, `||`, `;`, `|`, `&`) — only the first segment is analyzed
- Configuration-based command aliases
- Pattern matching against a built-in rules table for all supported tech stacks

### Discovery Constraints

- The input must be a **build tool command** from a supported tech stack (e.g. `cargo check`, `npm run lint`, `mvn test`).
- **Shell builtins** (`cd`, `ls`, `echo`, `cat`, `cp`, `mv`, `rm`, `mkdir`, etc.) are NOT supported and will result in exit code 1.
- **General shell commands** (`git`, `curl`, `wget`, `python`, `node`, etc.) that are not in the supported tech stacks will fail.
- **Compound commands** are handled: only the first segment before `&&`, `||`, `;`, `|`, or `&` is rewritten. A note is printed for any remaining segments.
- **Environment variable prefixes** (`ENV=val cmd`) are stripped automatically.
