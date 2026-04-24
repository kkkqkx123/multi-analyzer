# Analyzer - Multi-language Build Tool Error Analyzer

A multilingual build tool error analyzer with a plugin-based architecture, supporting technology stacks such as Cargo, NPM, Maven, Gradle, Mypy, Go, Pytest, and C++ (CMake/GCC/Clang/MSVC).

## Features

- **Multi-language Support**: Analyze errors from various build tools
  - Rust/Cargo: `cargo check`, `cargo clippy`, `cargo test`
  - Python/Mypy: `mypy`, `mypy --strict`
  - Node.js/NPM: `npm lint`, `npm type-check`, `npm audit`
  - Java/Maven: `mvn compile`, `mvn test`
  - Java/Gradle: `gradle compileJava`, `gradle test`
  - Go: `go build`, `go test`, `go vet`
  - Python/Pytest: `pytest`
  - C++/CMake: `cmake configure`, `cmake build`
  - C++/GCC: `gcc compile`
  - C++/Clang: `clang compile`
  - C++/MSVC: `msvc compile`
- **Plugin-based Architecture**: Easily extendable for new tools
- **Multiple Report Formats**: Markdown, JSON, HTML
- **Flexible Filtering**: Filter by warnings or specific file paths
- **Configuration Support**: `.analyzer.toml` for custom configurations

## Installation

### From Source

```bash
cargo build --release
```

The compiled binary will be at `target/release/analyzer`.

### Pre-built

Pre-compiled release packages are provided for Windows users.

## Usage

```bash
# Basic usage - free-form commands
analyzer <tech-stack> "<full command>"

# Analyze Rust project
analyzer cargo "check"
analyzer cargo "clippy --all-targets"
analyzer cargo "test"

# Analyze Python/Mypy project
analyzer mypy "--show-column-numbers ."
analyzer mypy "--strict ."

# Analyze Python/Pytest
analyzer pytest "-v"
analyzer pytest "-v --tb=short"

# Analyze Node.js project
analyzer npm "run lint"
analyzer npm "run typecheck"
analyzer npm "audit"
analyzer pnpm "run lint"
analyzer pnpm "run typecheck"
analyzer yarn "run lint"

# Analyze Java/Maven project
analyzer maven "compile -q"
analyzer maven "test"

# Analyze Java/Gradle project
analyzer gradle "compileJava --quiet"
analyzer gradle "test"

# Analyze Go project
analyzer go "build ./..."
analyzer go "vet ./..."
analyzer go "test -v ./..."

# Analyze C++ project with CMake
analyzer cmake "--build build"

# Analyze C++ project with GCC
analyzer gcc "-fsyntax-only main.cpp"

# Analyze C++ project with Clang
analyzer clang "-fsyntax-only main.cpp"

# Analyze C++ project with MSVC
analyzer msvc "/Zs main.cpp"
```

### Options

- `--filter-warnings`: Filter out all warnings, only show errors
- `--filter-paths <paths>`: Filter errors by file paths (comma-separated)
- `--output <file>`: Specify output file path (default: analysis_report.md)

## Configuration

Create `.analyzer.toml` in your project root to customize behavior:

```toml
version = "1.0"

[global]
default_format = "markdown"
filter_warnings = false

[commands.typecheck]
exec = "npm run typecheck"
description = "Run TypeScript type checker"
tech_stacks = ["npm", "pnpm", "yarn"]

[tech_stack.npm]
test_framework = "jest"
```

## Report Output

The tool generates comprehensive reports in multiple formats:

- **Markdown**: Human-readable reports with statistics and categorization
- **JSON**: Machine-readable format for CI/CD integration
- **HTML**: Styled HTML reports for web viewing

Reports include:

- Summary statistics
- Error and warning type breakdown
- Top files with issues
- Detailed categorization with examples
- Line numbers and descriptions for each error

## Architecture

```
CLI Entry → Core Module → Plugin Module
```

### Core Module (core/)

| Component          | Description                                         |
| ------------------ | --------------------------------------------------- |
| `types.rs`         | Common data types (Issue, Location, AnalysisResult) |
| `parser.rs`        | Output parsing interface                            |
| `analyzer.rs`      | Unified analyzer interface                          |
| `reporter/*`       | Report generation (Markdown/JSON/HTML)              |
| `command.rs`       | Command construction and execution                  |
| `base_analyzer.rs` | Generic analyzer implementation                     |

### Plugin Module (plugins/)

| Plugin    | Description                                      |
| --------- | ------------------------------------------------ |
| Cargo     | Rust/Cargo build analyzer                        |
| Mypy      | Python type checker analyzer                     |
| Pytest    | Python test framework analyzer                   |
| NPM       | Node.js package manager analyzer (npm/pnpm/yarn) |
| Maven     | Java Maven build analyzer                        |
| Gradle    | Java Gradle build analyzer                       |
| Go        | Go build and test analyzer                       |
| C++/CMake | C++ CMake build analyzer                         |
| C++/GCC   | C++ GCC compiler analyzer                        |
| C++/Clang | C++ Clang compiler analyzer                      |
| C++/MSVC  | C++ MSVC compiler analyzer                       |

## Use Cases

- **Code Quality Assessment**: Identify recurring error patterns across your codebase
- **Refactoring Planning**: Focus on files with the most errors/warnings
- **CI/CD Integration**: Automated error reporting in build pipelines
- **Team Onboarding**: Share common error patterns with team members

## License

This project is licensed under the MIT License.
