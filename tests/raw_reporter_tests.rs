//! Raw Reporter Tests
//!
//! Unit tests for the RawReporter (pipe-delimited and JSON lines output formats).
//! Verifies correct formatting with various issue configurations and edge cases.

use analyzer::core::AnalysisResult;
use analyzer::core::Issue;
use analyzer::core::IssueLevel;
use analyzer::core::Location;
use analyzer::core::RawReporter;
use analyzer::core::Reporter;

// ============================================================================
// Helper: Create a test AnalysisResult with given issues
// ============================================================================

fn make_result(issues: Vec<Issue>) -> AnalysisResult {
    let mut result = AnalysisResult::new();
    for issue in issues {
        result.add_issue(issue);
    }
    result
}

fn make_issue(
    level: IssueLevel,
    message: &str,
    file: &str,
    line: Option<u32>,
    col: Option<u32>,
    code: Option<&str>,
) -> Issue {
    let mut location = Location::new(file);
    if let Some(l) = line {
        location = location.with_line(l);
    }
    if let Some(c) = col {
        location = location.with_column(c);
    }
    let mut issue = Issue::new(level, message, location);
    if let Some(c) = code {
        issue = issue.with_code(c);
    }
    issue
}

// ============================================================================
// Pipe-delimited tests
// ============================================================================

#[test]
fn test_raw_pipe_empty() {
    let reporter = RawReporter::new();
    let result = AnalysisResult::new();
    let output = reporter.generate(&result).unwrap();
    assert_eq!(output, "");
}

#[test]
fn test_raw_pipe_single_error() {
    let reporter = RawReporter::new();
    let issue = make_issue(
        IssueLevel::Error,
        "mismatched types",
        "src/main.rs",
        Some(42),
        Some(10),
        Some("E0308"),
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert_eq!(output, "error|E0308|src/main.rs:42:10|mismatched types\n");
}

#[test]
fn test_raw_pipe_single_warning() {
    let reporter = RawReporter::new();
    let issue = make_issue(
        IssueLevel::Warning,
        "unused variable: x",
        "src/lib.rs",
        Some(15),
        None,
        Some("unused_variables"),
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert_eq!(
        output,
        "warning|unused_variables|src/lib.rs:15:|unused variable: x\n"
    );
}

#[test]
fn test_raw_pipe_no_code() {
    let reporter = RawReporter::new();
    let issue = make_issue(
        IssueLevel::Info,
        "generating docs",
        "src/docs.rs",
        Some(1),
        Some(1),
        None,
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert_eq!(output, "info|-|src/docs.rs:1:1|generating docs\n");
}

#[test]
fn test_raw_pipe_no_line_no_col() {
    let reporter = RawReporter::new();
    let issue = make_issue(
        IssueLevel::Error,
        "build failed",
        "Cargo.toml",
        None,
        None,
        Some("BUILD_ERR"),
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert_eq!(output, "error|BUILD_ERR|Cargo.toml::|build failed\n");
}

#[test]
fn test_raw_pipe_multiple_issues() {
    let reporter = RawReporter::new();
    let issues = vec![
        make_issue(
            IssueLevel::Error,
            "type error",
            "src/main.rs",
            Some(1),
            Some(5),
            Some("E0001"),
        ),
        make_issue(
            IssueLevel::Warning,
            "dead code",
            "src/lib.rs",
            Some(10),
            Some(1),
            Some("dead_code"),
        ),
        make_issue(
            IssueLevel::Hint,
            "try adding a return",
            "src/main.rs",
            Some(2),
            Some(1),
            None,
        ),
    ];
    let result = make_result(issues);
    let output = reporter.generate(&result).unwrap();
    let lines: Vec<&str> = output.trim().split('\n').collect();
    assert_eq!(lines.len(), 3);
    // Check content regardless of order (HashMap iteration is non-deterministic)
    let all_output = output.clone();
    assert!(all_output.contains("error|E0001|src/main.rs:1:5|type error"));
    assert!(all_output.contains("warning|dead_code|src/lib.rs:10:1|dead code"));
    assert!(all_output.contains("hint|-|src/main.rs:2:1|try adding a return"));
}

#[test]
fn test_raw_pipe_message_with_pipe() {
    let reporter = RawReporter::new();
    let issue = make_issue(
        IssueLevel::Error,
        "expected | got mismatch",
        "src/foo.rs",
        Some(7),
        Some(3),
        None,
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert!(output.contains("expected | got mismatch"));
}

// ============================================================================
// JSON lines tests
// ============================================================================

#[test]
fn test_raw_json_empty() {
    let reporter = RawReporter::new_json_lines();
    let result = AnalysisResult::new();
    let output = reporter.generate(&result).unwrap();
    assert_eq!(output, "");
}

#[test]
fn test_raw_json_single_error() {
    let reporter = RawReporter::new_json_lines();
    let issue = make_issue(
        IssueLevel::Error,
        "mismatched types",
        "src/main.rs",
        Some(42),
        Some(10),
        Some("E0308"),
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert!(output.contains("\"level\":\"error\""));
    assert!(output.contains("\"code\":\"E0308\""));
    assert!(output.contains("\"file\":\"src/main.rs\""));
    assert!(output.contains("\"line\":42"));
    assert!(output.contains("\"column\":10"));
    assert!(output.contains("\"message\":\"mismatched types\""));
}

#[test]
fn test_raw_json_no_code_no_line_no_col() {
    let reporter = RawReporter::new_json_lines();
    let issue = make_issue(
        IssueLevel::Warning,
        "some generic warning",
        "build.gradle",
        None,
        None,
        None,
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert!(output.contains("\"level\":\"warning\""));
    assert!(output.contains("\"code\":null"));
    assert!(output.contains("\"file\":\"build.gradle\""));
    assert!(output.contains("\"line\":null"));
    assert!(output.contains("\"column\":null"));
    assert!(output.contains("\"message\":\"some generic warning\""));
}

#[test]
fn test_raw_json_each_line_is_valid_json() {
    let reporter = RawReporter::new_json_lines();
    let issues = vec![
        make_issue(
            IssueLevel::Error,
            "error one",
            "a.rs",
            Some(1),
            Some(1),
            Some("E1"),
        ),
        make_issue(
            IssueLevel::Error,
            "error two",
            "b.rs",
            Some(2),
            Some(2),
            Some("E2"),
        ),
    ];
    let result = make_result(issues);
    let output = reporter.generate(&result).unwrap();
    for line in output.trim().split('\n') {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
        assert!(
            parsed.is_ok(),
            "Expected valid JSON, got parse error for: {}",
            line
        );
    }
}

#[test]
fn test_raw_json_message_with_quotes() {
    let reporter = RawReporter::new_json_lines();
    let issue = make_issue(
        IssueLevel::Error,
        r#"user "admin" not found"#,
        "src/auth.rs",
        Some(10),
        Some(5),
        None,
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert!(output.contains(r#""message":"user \"admin\" not found""#));
}

#[test]
fn test_raw_json_message_with_backslash() {
    let reporter = RawReporter::new_json_lines();
    let issue = make_issue(
        IssueLevel::Error,
        r"path\to\file not found",
        "src/main.rs",
        Some(1),
        Some(1),
        None,
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert!(output.contains(r"path\to\file"));
}

#[test]
fn test_raw_json_file_with_quotes() {
    let reporter = RawReporter::new_json_lines();
    let issue = make_issue(
        IssueLevel::Error,
        "test",
        r#"src/"weird_name".rs"#,
        Some(1),
        Some(1),
        Some("E1"),
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert!(output.contains(r#""file":"src/\"weird_name\".rs""#));
}

// ============================================================================
// Multiple issues ordering
// ============================================================================

#[test]
fn test_raw_pipe_multiple_files() {
    let reporter = RawReporter::new();
    let issues = vec![
        make_issue(
            IssueLevel::Error,
            "err a",
            "a.rs",
            Some(1),
            Some(1),
            Some("EA"),
        ),
        make_issue(
            IssueLevel::Error,
            "err b",
            "b.rs",
            Some(2),
            Some(2),
            Some("EB"),
        ),
        make_issue(
            IssueLevel::Error,
            "err c",
            "c.rs",
            Some(3),
            Some(3),
            Some("EC"),
        ),
    ];
    let result = make_result(issues);
    let output = reporter.generate(&result).unwrap();
    let lines: Vec<&str> = output.trim().split('\n').collect();
    assert_eq!(lines.len(), 3);
}

// ============================================================================
// Hint and Info levels
// ============================================================================

#[test]
fn test_raw_pipe_hint_level() {
    let reporter = RawReporter::new();
    let issue = make_issue(
        IssueLevel::Hint,
        "consider using a match expression",
        "src/main.rs",
        Some(5),
        Some(1),
        None,
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert!(output.starts_with("hint|"));
}

#[test]
fn test_raw_json_info_level() {
    let reporter = RawReporter::new_json_lines();
    let issue = make_issue(
        IssueLevel::Info,
        "compilation completed",
        "src/main.rs",
        Some(1),
        Some(1),
        Some("INFO001"),
    );
    let result = make_result(vec![issue]);
    let output = reporter.generate(&result).unwrap();
    assert!(output.contains("\"level\":\"info\""));
}
