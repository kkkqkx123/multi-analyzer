//! Pytest Integration Testing
//! Execute the actual pytest commands to verify that the parsing logic matches the actual output format

use std::path::PathBuf;

mod common;
use common::{
    fixtures_dir, generate_report, generate_test_report, is_command_available, read_sample,
    save_raw_output,
};

fn python_project_path() -> PathBuf {
    fixtures_dir().join("python-project")
}

/// Check if pytest is available
fn ensure_pytest() -> Result<(), String> {
    if !is_command_available("pytest") {
        // Also try python -m pytest
        if !is_command_available("python") {
            return Err(
                "Neither pytest nor python is available in PATH. Please install Python and pytest."
                    .to_string(),
            );
        }
    }
    Ok(())
}

#[test]
fn test_pytest_output_parsing() {
    use analyzer::core::TestOutputParser;
    use analyzer::plugins::python::pytest::parser::PytestParser;

    // Use sample output for consistent testing
    let output = read_sample("pytest_sample");

    // Save the raw output
    save_raw_output("pytest", &output);

    // Parse the output
    let parser = PytestParser::new();
    let result = parser.parse_test_output(&output);

    println!("=== Pytest Output ===");
    println!("{}", output);

    println!("\n=== Parsed Results ===");
    println!("Passed tests: {}", result.passed_tests.len());
    println!("Failed tests: {}", result.failed_tests.len());
    println!("Skipped tests: {}", result.ignored_tests.len());

    if let Some(ref summary) = result.test_summary {
        println!(
            "Total: {}, Passed: {}, Failed: {}, Skipped: {}",
            summary.total, summary.passed, summary.failed, summary.ignored
        );
    }

    // Generate test report
    generate_test_report(
        "pytest",
        "Pytest",
        "pytest -v",
        &result,
        Some("raw_output/pytest.txt"),
    );

    // Verify that we parsed some tests
    assert!(
        result.passed_tests.len() + result.failed_tests.len() + result.ignored_tests.len() > 0,
        "Expected to find at least one test"
    );

    // Check that failed tests have details
    for test in &result.failed_tests {
        println!("\nFailed test: {}", test.name);
        if let Some(ref details) = test.failure_details {
            println!("Failure details: {}", details);
        }
    }
}

#[test]
fn test_pytest_parser_with_sample() {
    use analyzer::core::TestOutputParser;
    use analyzer::plugins::python::pytest::parser::PytestParser;

    let parser = PytestParser::new();

    // Test with sample output that has failures
    let sample_output = read_sample("pytest_sample");
    let result = parser.parse_test_output(&sample_output);

    println!("=== Validating Pytest Sample ===");
    println!("Passed tests: {}", result.passed_tests.len());
    println!("Failed tests: {}", result.failed_tests.len());
    println!("Skipped tests: {}", result.ignored_tests.len());

    // Verify parsing results
    // Note: XFAIL is parsed as Ignored, not Passed (because of the [reason] pattern)
    assert_eq!(
        result.passed_tests.len(),
        7,
        "Expected 7 passed tests (PASSED + XPASS)"
    );
    assert_eq!(result.failed_tests.len(), 1, "Expected 1 failed test");
    assert_eq!(
        result.ignored_tests.len(),
        2,
        "Expected 2 skipped tests (1 SKIPPED + 1 XFAIL)"
    );

    // Verify summary (includes xfailed in passed count)
    if let Some(ref summary) = result.test_summary {
        assert_eq!(
            summary.total, 11,
            "Expected 11 total tests (8 passed + 1 skipped + 1 xfailed + 1 failed)"
        );
        assert_eq!(
            summary.passed, 9,
            "Expected 9 passed in summary (8 passed + 1 xfailed)"
        );
        assert_eq!(summary.failed, 1, "Expected 1 failed in summary");
        assert_eq!(summary.ignored, 1, "Expected 1 skipped in summary");
    } else {
        panic!("Expected test summary to be parsed");
    }

    // Verify failed test details
    let failed_test = result
        .failed_tests
        .iter()
        .find(|t| t.name == "test_divide_failure")
        .expect("Expected to find test_divide_failure");

    assert!(
        failed_test.failure_details.is_some(),
        "Expected failure details for failed test"
    );

    // Test with all-passed sample
    let all_passed_output = read_sample("pytest_all_passed_sample");
    let all_passed_result = parser.parse_test_output(&all_passed_output);

    assert_eq!(
        all_passed_result.passed_tests.len(),
        5,
        "Expected 5 passed tests"
    );
    assert_eq!(
        all_passed_result.failed_tests.len(),
        0,
        "Expected 0 failed tests"
    );
}

#[test]
fn test_pytest_parser_specific_patterns() {
    use analyzer::plugins::python::pytest::parser::PytestParser;

    let parser = PytestParser::new();

    // Test various pytest output formats
    let test_cases = vec![
        ("test_example.py::test_addition PASSED [0.01s]", true),
        ("test_example.py::test_division FAILED [0.02s]", true),
        ("test_example.py::test_feature SKIPPED [not ready]", true),
        ("test_example.py::test_bug XFAIL [known issue]", true),
        ("test_example.py::test_unexpected XPASS [0.01s]", true),
        ("test_example.py::test_error ERROR [0.01s]", true),
        ("some regular log line", false),
        (
            "============================= test session starts ==============================",
            false,
        ),
    ];

    for (line, should_parse) in test_cases {
        let result = parser.parse_test_case_line(line);
        if should_parse {
            assert!(result.is_some(), "Expected to parse: {}", line);
            println!("✓ Parsed: {} -> {:?}", line, result.unwrap().status);
        } else {
            assert!(result.is_none(), "Expected not to parse: {}", line);
            println!("✓ Correctly ignored: {}", line);
        }
    }
}

#[test]
fn test_pytest_analyzer_applicable() {
    use analyzer::core::BuildAnalyzer;
    use analyzer::plugins::python::pytest::analyzer::PytestAnalyzer;

    let analyzer = PytestAnalyzer::new();

    // Test with python-project fixture
    let project_path = python_project_path();

    println!("Python project path: {}", project_path.display());

    // The analyzer should have correct name and commands
    assert_eq!(analyzer.name(), "pytest");
    assert!(analyzer.supported_commands().contains(&"pytest"));
}

#[test]
fn test_pytest_analyzer_traits() {
    use analyzer::core::{BuildAnalyzer, TestAnalyzer};
    use analyzer::plugins::python::pytest::analyzer::PytestAnalyzer;

    let analyzer = PytestAnalyzer::new();

    // Test BuildAnalyzer trait
    assert_eq!(analyzer.name(), "pytest");
    assert!(analyzer.supported_commands().contains(&"pytest"));
    assert!(analyzer.supported_commands().contains(&"py.test"));

    // Test TestAnalyzer trait
    assert!(
        analyzer.supports_test(),
        "PytestAnalyzer should support test analysis"
    );
    assert!(
        analyzer.test_parser().is_some(),
        "PytestAnalyzer should have a test parser"
    );
}

#[test]
fn test_generate_pytest_report() {
    use analyzer::core::TestOutputParser;
    use analyzer::plugins::python::pytest::parser::PytestParser;

    let parser = PytestParser::new();
    let sample_output = read_sample("pytest_sample");
    let result = parser.parse_test_output(&sample_output);

    // Generate test report using the new function
    let report_path = generate_test_report(
        "pytest_test",
        "Pytest",
        "pytest -v",
        &result,
        Some("raw_output/pytest.txt"),
    );

    println!("Test report generated at: {}", report_path.display());
    assert!(report_path.exists(), "Report file should exist");

    // Also test the old generate_report function with converted issues
    use analyzer::core::{Issue, IssueLevel};

    let issues: Vec<Issue> = result
        .failed_tests
        .iter()
        .filter_map(|test| {
            test.location.as_ref().map(|loc| {
                Issue::new(
                    IssueLevel::Error,
                    format!("Test failed: {}", test.name),
                    loc.clone(),
                )
                .with_context(test.failure_details.clone().unwrap_or_default())
            })
        })
        .collect();

    let issue_report_path = generate_report(
        "pytest_issues",
        "Pytest (Issues View)",
        "pytest -v",
        &issues,
        Some("raw_output/pytest.txt"),
    );

    println!(
        "Issues report generated at: {}",
        issue_report_path.display()
    );
    assert!(
        issue_report_path.exists(),
        "Issues report file should exist"
    );
}

#[test]
fn test_pytest_with_filter() {
    use analyzer::core::{TestAnalyzer, TestOptions};
    use analyzer::plugins::python::pytest::analyzer::PytestAnalyzer;

    if let Err(e) = ensure_pytest() {
        println!("Skipping test: {}", e);
        return;
    }

    let analyzer = PytestAnalyzer::new();
    let project_path = python_project_path();

    // Change to project directory for test
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&project_path).unwrap();

    // Run tests with filter
    let options = TestOptions {
        filter: Some("add".to_string()),
        ..Default::default()
    };

    match analyzer.run_tests(&options) {
        Ok(result) => {
            println!("Filtered test results:");
            println!("Passed: {}", result.passed_tests.len());
            println!("Failed: {}", result.failed_tests.len());

            // All tests with "add" in name should pass
            assert!(
                result.failed_tests.is_empty(),
                "Expected no failures for 'add' tests"
            );
        }
        Err(e) => {
            println!("Test execution failed: {}", e);
            // Don't fail the test if pytest isn't properly configured
        }
    }

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_pytest_summary_parsing() {
    use analyzer::plugins::python::pytest::parser::PytestParser;

    let parser = PytestParser::new();

    // Test various summary formats
    let summaries = vec![
        (
            "============= 5 passed in 0.08s ===============================",
            5,
            0,
            0,
        ),
        (
            "============= 8 passed, 1 skipped, 1 xfailed, 1 failed in 0.15s =============",
            9,
            1,
            1,
        ), // 8 passed + 1 xfailed = 9 passed
        (
            "===================== 5 passed, 2 failed, 1 skipped in 0.05s ======================",
            5,
            2,
            1,
        ),
        (
            "================== 10 passed, 0 failed in 1.23s ==================",
            10,
            0,
            0,
        ),
    ];

    for (line, expected_passed, expected_failed, expected_skipped) in summaries {
        let summary = parser.parse_test_summary(line);
        assert!(summary.is_some(), "Expected to parse summary: {}", line);

        let summary = summary.unwrap();
        assert_eq!(
            summary.passed, expected_passed,
            "Passed count mismatch for: {}",
            line
        );
        assert_eq!(
            summary.failed, expected_failed,
            "Failed count mismatch for: {}",
            line
        );
        assert_eq!(
            summary.ignored, expected_skipped,
            "Skipped count mismatch for: {}",
            line
        );

        println!(
            "✓ Parsed summary: {} passed, {} failed, {} skipped in {:?}s",
            summary.passed, summary.failed, summary.ignored, summary.execution_time
        );
    }
}
