# Plugins Module

Technology-stack-specific analyzer implementations.

## Available Plugins

| Plugin | Directory | Status |
|--------|-----------|--------|
| Cargo (Rust) | `cargo/` | Done — `check`, `clippy`, `test` |
| NPM/Node.js | `npm/` | Done — `lint`, `type-check`, `audit` |
| Python | `python/` | Done — `mypy`, `pytest` |
| Go | `go/` | Done — `build`, `vet`, `test`, `golangci-lint` |
| Java | `java/` | Done — `maven`, `gradle` |
| C/C++ | `cpp/` | Done — `cmake`, `gcc`, `clang`, `msvc` |

## Adding a New Plugin

1. Create `src/plugins/<name>/` directory
2. Implement `BuildAnalyzer` in `analyzer.rs`
3. Implement `OutputParser` in `parser.rs`
4. Create `<name>.rs` exporting your types
5. Register in `src/plugins.rs`

## Conventions

- Each plugin has its own `OutputParser` implementation
- Use `BaseParser` for standard `file:line:col:` format parsing
- Keep parsers stateless (no mutable shared state)