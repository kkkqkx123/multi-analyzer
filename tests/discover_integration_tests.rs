//! Discover Integration Tests
//!
//! End-to-end tests for the command classification and rewrite engines.
//! Verifies that classify_command, rewrite_command, and the lexer
//! correctly handle real-world shell commands across all supported tech stacks.

use analyzer::discover::lexer::split_on_operators;
use analyzer::discover::{classify_command, rewrite_command, Classification};

// ============================================================================
// Helper assertions
// ============================================================================

/// Assert that a command is Matched with expected tech_stack name
fn assert_classified_as(raw_cmd: &str, expected_tech: &str) -> Classification {
    let result = classify_command(raw_cmd);
    match &result {
        Classification::Matched { tech_stack, .. } => {
            assert_eq!(
                tech_stack.as_str(),
                expected_tech,
                "Command '{}' classified as '{}', expected '{}'",
                raw_cmd,
                tech_stack.as_str(),
                expected_tech
            );
        }
        Classification::Unmatched { base_command } => {
            panic!(
                "Command '{}' was Unmatched (base='{}'), expected Matched with tech='{}'",
                raw_cmd, base_command, expected_tech
            );
        }
    }
    result
}

/// Assert that a command is Unmatched
fn assert_unmatched(raw_cmd: &str) {
    let result = classify_command(raw_cmd);
    assert!(
        matches!(result, Classification::Unmatched { .. }),
        "Expected '{}' to be Unmatched but got Matched",
        raw_cmd
    );
}

/// Assert subcommand and extra_args on a Matched classification
fn assert_subcommand(result: &Classification, expected_sub: &str) {
    if let Classification::Matched { subcommand, .. } = result {
        assert_eq!(
            subcommand, expected_sub,
            "Expected subcommand '{}', got '{}'",
            expected_sub, subcommand
        );
    }
}

/// Assert extra_args contain specific values
fn assert_extra_args(result: &Classification, expected_args: &[&str]) {
    if let Classification::Matched { extra_args, .. } = result {
        assert_eq!(
            extra_args, expected_args,
            "Expected extra_args {:?}, got {:?}",
            expected_args, extra_args
        );
    }
}

// ============================================================================
// Rust / Cargo
// ============================================================================

#[test]
fn test_cargo_check() {
    let r = assert_classified_as("cargo check", "cargo");
    assert_subcommand(&r, "check");
}

#[test]
fn test_cargo_check_all_targets() {
    let r = assert_classified_as("cargo check --all-targets --workspace", "cargo");
    assert_subcommand(&r, "check");
    assert_extra_args(&r, &["--all-targets", "--workspace"]);
}

#[test]
fn test_cargo_clippy() {
    let r = assert_classified_as("cargo clippy -- -D warnings", "cargo");
    assert_subcommand(&r, "clippy");
    assert_extra_args(&r, &["--", "-D", "warnings"]);
}

#[test]
fn test_cargo_test_specific() {
    let r = assert_classified_as("cargo test --package my_crate -- --nocapture", "cargo");
    assert_subcommand(&r, "test");
    assert_extra_args(&r, &["--package", "my_crate", "--", "--nocapture"]);
}

#[test]
fn test_cargo_build_release() {
    let r = assert_classified_as("cargo build --release", "cargo");
    assert_subcommand(&r, "build");
    assert_extra_args(&r, &["--release"]);
}

#[test]
fn test_cargo_fmt() {
    let r = assert_classified_as("cargo fmt --check", "cargo");
    assert_subcommand(&r, "fmt");
}

#[test]
fn test_cargo_nextest_run() {
    let r = assert_classified_as("cargo nextest run", "cargo");
    assert_subcommand(&r, "nextest run");
}

#[test]
fn test_cargo_nextest_list() {
    let r = assert_classified_as("cargo nextest list", "cargo");
    assert_subcommand(&r, "nextest list");
}

#[test]
fn test_cargo_nextest_archive() {
    let r = assert_classified_as("cargo nextest archive", "cargo");
    assert_subcommand(&r, "nextest archive");
}

#[test]
fn test_cargo_with_env_prefix() {
    let r = assert_classified_as("RUST_BACKTRACE=1 CARGO_INCREMENTAL=0 cargo test", "cargo");
    assert_subcommand(&r, "test");
    assert!(extra_args_is_empty(&r));
}

#[test]
fn test_cargo_with_quoted_env() {
    let r = assert_classified_as(r#"RUSTFLAGS="-C target-cpu=native" cargo build"#, "cargo");
    assert_subcommand(&r, "build");
}

// ============================================================================
// Node.js / NPM
// ============================================================================

#[test]
fn test_npm_run_lint() {
    let r = assert_classified_as("npm run lint", "npm");
    assert_subcommand(&r, "run lint");
}

#[test]
fn test_npm_lint_shorthand() {
    let r = assert_classified_as("npm lint", "npm");
    assert_subcommand(&r, "run lint");
}

#[test]
fn test_npm_typecheck() {
    let r = assert_classified_as("npm run typecheck", "npm");
    assert_subcommand(&r, "run typecheck");
}

#[test]
fn test_npm_audit() {
    let r = assert_classified_as("npm audit --production", "npm");
    assert_subcommand(&r, "run audit");
}

#[test]
fn test_npm_custom_script() {
    // "npm run build" doesn't match lint|typecheck|audit|test, falls through to catch-all
    let r = assert_classified_as("npm run build", "npm");
    // The fallback rule pattern "^npm\s+(?:run\s+)?(\S+)" with template "run {1}"
    // captures "build" as group 1, producing "run build"
    assert_subcommand(&r, "run build");
}

#[test]
fn test_npm_custom_fallback() {
    let r = assert_classified_as("npm install", "npm");
    assert_subcommand(&r, "run install");
}

// ============================================================================
// Node.js / PNPM
// ============================================================================

#[test]
fn test_pnpm_run_lint() {
    // pnpm template is "{2}", extracts the subcommand without the "pnpm" prefix
    let r = assert_classified_as("pnpm run lint", "pnpm");
    assert_subcommand(&r, "lint");
}

#[test]
fn test_pnpm_typecheck() {
    let r = assert_classified_as("pnpm run typecheck", "pnpm");
    assert_subcommand(&r, "typecheck");
}

#[test]
fn test_pnpm_exec_tsc() {
    let r = assert_classified_as("pnpm exec tsc", "pnpm");
    assert_subcommand(&r, "exec tsc");
}

#[test]
fn test_pnpm_audit() {
    let r = assert_classified_as("pnpm audit", "pnpm");
    assert_subcommand(&r, "audit");
}

// ============================================================================
// Node.js / Yarn
// ============================================================================

#[test]
fn test_yarn_run_lint() {
    let r = assert_classified_as("yarn run lint", "yarn");
    assert_subcommand(&r, "run lint");
}

#[test]
fn test_yarn_test() {
    let r = assert_classified_as("yarn test --coverage", "yarn");
    assert_subcommand(&r, "run test");
}

// ============================================================================
// Python / Mypy
// ============================================================================

#[test]
fn test_mypy_basic() {
    let r = assert_classified_as("mypy src/", "mypy");
    assert_subcommand(&r, "src/");
}

#[test]
fn test_mypy_strict() {
    let r = assert_classified_as("mypy --strict --ignore-missing-imports src/", "mypy");
    assert_subcommand(&r, "--strict --ignore-missing-imports src/");
}

// ============================================================================
// Python / Pytest
// ============================================================================

#[test]
fn test_pytest_basic() {
    let r = assert_classified_as("pytest", "pytest");
    assert_subcommand(&r, "pytest");
}

#[test]
fn test_pytest_verbose() {
    let r = assert_classified_as("pytest -v -x tests/", "pytest");
    assert_subcommand(&r, "-v -x tests/");
}

// ============================================================================
// Python / Ruff
// ============================================================================

#[test]
fn test_ruff_check() {
    let r = assert_classified_as("ruff check src/", "ruff");
    assert_subcommand(&r, "check");
    assert_extra_args(&r, &["src/"]);
}

#[test]
fn test_ruff_format() {
    let r = assert_classified_as("ruff format src/", "ruff");
    assert_subcommand(&r, "format");
}

// ============================================================================
// Python / Black
// ============================================================================

#[test]
fn test_black_basic() {
    let r = assert_classified_as("black src/", "black");
    assert_subcommand(&r, "src/");
}

// ============================================================================
// Go
// ============================================================================

#[test]
fn test_go_build() {
    let r = assert_classified_as("go build ./...", "go");
    assert_subcommand(&r, "build");
}

#[test]
fn test_go_test_race() {
    let r = assert_classified_as("go test -race -count=1 ./...", "go");
    assert_subcommand(&r, "test");
}

#[test]
fn test_go_vet() {
    let r = assert_classified_as("go vet ./...", "go");
    assert_subcommand(&r, "vet");
}

#[test]
fn test_golangci_lint() {
    let r = assert_classified_as("golangci-lint run --timeout=5m", "golangci-lint");
    assert_subcommand(&r, "run");
}

#[test]
fn test_gofmt() {
    let r = assert_classified_as("gofmt -l src/", "go");
    assert_subcommand(&r, "fmt");
    assert_extra_args(&r, &["-l", "src/"]);
}

// ============================================================================
// Java / Maven
// ============================================================================

#[test]
fn test_mvn_compile() {
    let r = assert_classified_as("mvn compile", "maven");
    assert_subcommand(&r, "compile");
}

#[test]
fn test_mvn_test() {
    let r = assert_classified_as("mvn test -pl core -DskipTests=false", "maven");
    assert_subcommand(&r, "test");
}

#[test]
fn test_mvn_verify() {
    let r = assert_classified_as("mvn verify", "maven");
    assert_subcommand(&r, "verify");
}

#[test]
fn test_mvn_package() {
    let r = assert_classified_as("mvn package -DskipTests", "maven");
    assert_subcommand(&r, "package");
}

// ============================================================================
// Java / Gradle
// ============================================================================

#[test]
fn test_gradle_compile() {
    let r = assert_classified_as("gradle compileJava", "gradle");
    assert_subcommand(&r, "compileJava");
}

#[test]
fn test_gradle_test() {
    let r = assert_classified_as("gradle test --info", "gradle");
    assert_subcommand(&r, "test");
}

#[test]
fn test_gradle_check() {
    let r = assert_classified_as("gradle check", "gradle");
    assert_subcommand(&r, "check");
}

#[test]
fn test_gradlew_test() {
    let r = assert_classified_as("gradlew test", "gradle");
    assert_subcommand(&r, "test");
}

// ============================================================================
// .NET / Dotnet
// ============================================================================

#[test]
fn test_dotnet_build() {
    let r = assert_classified_as("dotnet build", "dotnet");
    assert_subcommand(&r, "build");
}

#[test]
fn test_dotnet_test() {
    let r = assert_classified_as("dotnet test --filter Category=Unit", "dotnet");
    assert_subcommand(&r, "test");
}

// ============================================================================
// Ruby
// ============================================================================

#[test]
fn test_rubocop() {
    let r = assert_classified_as("rubocop", "rubocop");
    assert_subcommand(&r, "rubocop");
}

#[test]
fn test_rubocop_with_args() {
    let r = assert_classified_as("rubocop --auto-correct app/", "rubocop");
    assert_subcommand(&r, "--auto-correct app/");
}

#[test]
fn test_rspec_basic() {
    let r = assert_classified_as("rspec spec/", "rspec");
    assert_subcommand(&r, "spec/");
}

#[test]
fn test_rspec_bundle_exec() {
    let r = assert_classified_as("bundle exec rspec spec/models/", "rspec");
    assert_subcommand(&r, "spec/models/");
}

// ============================================================================
// C++ / CMake
// ============================================================================

#[test]
fn test_cmake_build() {
    let r = assert_classified_as("cmake --build build/", "cmake");
    assert_subcommand(&r, "--build");
}

#[test]
fn test_cmake_configure() {
    let r = assert_classified_as("cmake --configure -G Ninja", "cmake");
    assert_subcommand(&r, "--configure");
}

// ============================================================================
// C++ / GCC
// ============================================================================

#[test]
fn test_gcc_compile() {
    let r = assert_classified_as("gcc -c src/main.c -o main.o", "gcc");
    assert_subcommand(&r, "compile");
}

#[test]
fn test_gpp_compile() {
    // g++ is matched by the Gcc rule (pattern groups gcc and g++ together)
    let r = assert_classified_as("g++ -c src/main.cpp -std=c++17 -o main.o", "gcc");
    assert_subcommand(&r, "compile");
}

// ============================================================================
// C++ / Clang
// ============================================================================

#[test]
fn test_clang_compile() {
    let r = assert_classified_as("clang -c src/main.c -o main.o", "clang");
    assert_subcommand(&r, "compile");
}

#[test]
fn test_clangpp_compile() {
    let r = assert_classified_as("clang++ -c src/main.cpp -o main.o", "clang");
    assert_subcommand(&r, "compile");
}

// ============================================================================
// C++ / MSVC
// ============================================================================

#[test]
fn test_cl_exe() {
    let r = assert_classified_as("cl.exe /c src/main.cpp", "msvc");
    assert_subcommand(&r, "compile");
}

#[test]
fn test_msvc() {
    let r = assert_classified_as("msvc /c src/main.cpp", "msvc");
    assert_subcommand(&r, "compile");
}

// ============================================================================
// Unmatched commands
// ============================================================================

#[test]
fn test_unmatched_echo() {
    assert_unmatched("echo hello");
}

#[test]
fn test_unmatched_ls() {
    assert_unmatched("ls -la");
}

#[test]
fn test_unmatched_unknown_tool() {
    assert_unmatched("unknown-tool run");
}

#[test]
fn test_unmatched_empty() {
    assert_unmatched("");
}

// ============================================================================
// Edge cases
// ============================================================================

#[test]
fn test_command_with_leading_whitespace() {
    let r = assert_classified_as("   cargo check", "cargo");
    assert_subcommand(&r, "check");
}

#[test]
fn test_case_insensitive() {
    let r = assert_classified_as("CARGO CHECK", "cargo");
    // Regex is case-insensitive, but capture groups preserve original case
    assert_subcommand(&r, "CHECK");
}

#[test]
fn test_mixed_case() {
    let r = assert_classified_as("Go Test ./...", "go");
    // Regex is case-insensitive, but capture groups preserve original case
    assert_subcommand(&r, "Test");
}

// ============================================================================
// Rewrite command tests
// ============================================================================

#[test]
fn test_rewrite_cargo() {
    let result = rewrite_command("cargo check --all-targets");
    assert!(result.is_some());
    let (ts, sub, extra) = result.unwrap();
    assert_eq!(ts.as_str(), "cargo");
    assert_eq!(sub, "check");
    assert!(!extra.is_empty());
    assert!(extra.contains(&"--all-targets".to_string()));
}

#[test]
fn test_rewrite_npm() {
    let result = rewrite_command("npm run lint");
    assert!(result.is_some());
    let (ts, sub, _) = result.unwrap();
    assert_eq!(ts.as_str(), "npm");
    assert_eq!(sub, "run lint");
}

#[test]
fn test_rewrite_pytest() {
    let result = rewrite_command("pytest -v");
    assert!(result.is_some());
    let (ts, sub, extra) = result.unwrap();
    assert_eq!(ts.as_str(), "pytest");
    assert_eq!(sub, "-v");
    assert!(extra.is_empty());
}

#[test]
fn test_rewrite_unrecognized() {
    assert!(rewrite_command("echo hello").is_none());
}

#[test]
fn test_rewrite_with_env() {
    let result = rewrite_command("RUST_LOG=debug cargo test");
    assert!(result.is_some());
    let (ts, sub, _) = result.unwrap();
    assert_eq!(ts.as_str(), "cargo");
    assert_eq!(sub, "test");
}

// ============================================================================
// Lexer (compound command) tests
// ============================================================================

#[test]
fn test_lexer_double_ampersand() {
    let segments = split_on_operators("cargo check && cargo test");
    assert_eq!(segments, vec!["cargo check", "cargo test"]);
}

#[test]
fn test_lexer_double_pipe() {
    let segments = split_on_operators("cargo build || echo failed");
    assert_eq!(segments, vec!["cargo build", "echo failed"]);
}

#[test]
fn test_lexer_semicolon() {
    let segments = split_on_operators("npm run lint; npm run test");
    assert_eq!(segments, vec!["npm run lint", "npm run test"]);
}

#[test]
fn test_lexer_pipe() {
    let segments = split_on_operators("mypy src/ | tee mypy_output.txt");
    assert_eq!(segments, vec!["mypy src/", "tee mypy_output.txt"]);
}

#[test]
fn test_lexer_quoted_operators() {
    let segments = split_on_operators("echo \"a && b\" && cargo check");
    assert_eq!(segments, vec!["echo \"a && b\"", "cargo check"]);
}

#[test]
fn test_lexer_single_quotes() {
    let segments = split_on_operators("echo 'foo || bar' && cargo test");
    assert_eq!(segments, vec!["echo 'foo || bar'", "cargo test"]);
}

#[test]
fn test_lexer_triple_chain() {
    let segments = split_on_operators("cargo fmt && cargo check && cargo test");
    assert_eq!(segments, vec!["cargo fmt", "cargo check", "cargo test"]);
}

#[test]
fn test_lexer_empty() {
    assert!(split_on_operators("").is_empty());
}

#[test]
fn test_lexer_simple_command() {
    let segments = split_on_operators("cargo build");
    assert_eq!(segments, vec!["cargo build"]);
}

// ============================================================================
// is_matched helper
// ============================================================================

#[test]
fn test_is_matched_true() {
    let result = classify_command("cargo check");
    assert!(result.is_matched());
}

#[test]
fn test_is_matched_false() {
    let result = classify_command("echo hello");
    assert!(!result.is_matched());
}

// ============================================================================
// Helper
// ============================================================================

fn extra_args_is_empty(result: &Classification) -> bool {
    match result {
        Classification::Matched { extra_args, .. } => extra_args.is_empty(),
        _ => false,
    }
}
