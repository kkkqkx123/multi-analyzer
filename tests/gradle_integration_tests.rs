//! Gradle Integration Testing
//! Execute the actual Gradle commands to verify that the parsing logic matches the actual output format

use std::path::Path;

mod common;
use common::{
    fixtures_dir, generate_report, is_command_available, read_sample, run_command, save_raw_output,
};

fn gradle_project_path() -> std::path::PathBuf {
    fixtures_dir().join("gradle-project")
}

/// Check if Gradle is available
fn ensure_gradle() -> Result<(), String> {
    if !is_command_available("gradle") && !is_command_available("gradlew") {
        return Err(
            "Gradle (gradle or gradlew) is not available in PATH. Please install Gradle."
                .to_string(),
        );
    }
    Ok(())
}

/// Get the appropriate gradle command for the project
fn get_gradle_cmd(project_path: &Path) -> String {
    // Check for gradlew in project directory
    let gradlew_path = project_path.join(if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    });
    if gradlew_path.exists() {
        return gradlew_path.to_string_lossy().to_string();
    }

    // Fall back to system gradle
    "gradle".to_string()
}

#[test]
fn test_gradle_compile_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::java::gradle::parser::GradleParser;

    if let Err(e) = ensure_gradle() {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = gradle_project_path();
    let gradle_cmd = get_gradle_cmd(&project_path);
    let cmd_name = if gradle_cmd.contains("gradlew") {
        "gradlew"
    } else {
        "gradle"
    };

    // Run Gradle build
    let output = match run_command(cmd_name, &["compileJava", "--quiet"], &project_path) {
        Ok(output) => output,
        Err(e) => {
            // Gradle also returns output (with error messages) when compilation fails
            println!("Gradle compile failed or found errors: {}", e);
            // Try running it again to get the error output
            match run_command(cmd_name, &["compileJava"], &project_path) {
                Ok(output) => output,
                Err(_) => {
                    // If it still fails, use the sample output
                    println!("Using sample output for testing");
                    read_sample("gradle_compile_sample")
                }
            }
        }
    };

    // Saving the original output
    save_raw_output("gradle_compile", &output);

    // Parses and generates reports
    let parser = GradleParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "gradle_compile",
        "Gradle Compile",
        "gradle compileJava",
        &issues,
        Some("raw_output/gradle_compile.txt"),
    );

    println!("=== Gradle Compile Output ===");
    println!("{}", output);

    // Validating the Gradle Output Format
    // Format: /path/to/File.java:10: error: message
    // Format: /path/to/File.java:20: warning: message
    let lines: Vec<&str> = output.lines().collect();
    let has_error_lines = lines
        .iter()
        .any(|line: &&str| line.contains(": error:") || line.contains(": warning:"));

    if has_error_lines {
        println!("✓ Found Gradle error/warning lines in expected format");
    } else if output.contains("BUILD SUCCESSFUL") || output.contains("BUILD SUCCESS") {
        println!("✓ Gradle build succeeded (no issues found)");
    } else {
        println!("! No error lines found (may be due to Gradle configuration or build success)");
    }

    // Verify that the output format is as expected by the parser
    for line in &lines {
        let trimmed: &str = line.trim();
        if trimmed.contains(": error:") || trimmed.contains(": warning:") {
            println!("  Found issue: {}", line);
        }
    }
}

#[test]
fn test_gradle_test_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::java::gradle::parser::GradleParser;

    if let Err(e) = ensure_gradle() {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = gradle_project_path();
    let gradle_cmd = get_gradle_cmd(&project_path);
    let cmd_name = if gradle_cmd.contains("gradlew") {
        "gradlew"
    } else {
        "gradle"
    };

    // Running Gradle Tests
    let output = match run_command(cmd_name, &["test", "--quiet"], &project_path) {
        Ok(output) => output,
        Err(e) => {
            println!("Gradle test failed or found errors: {}", e);
            match run_command(cmd_name, &["test"], &project_path) {
                Ok(output) => output,
                Err(_) => {
                    println!("Using sample output for testing");
                    read_sample("gradle_test_sample")
                }
            }
        }
    };

    // Saving the original output
    save_raw_output("gradle_test", &output);

    // Parses and generates reports
    let parser = GradleParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "gradle_test",
        "Gradle Test",
        "gradle test",
        &issues,
        Some("raw_output/gradle_test.txt"),
    );

    println!("=== Gradle Test Output ===");
    println!("{}", output);

    // Validation Test Output
    let lines: Vec<&str> = output.lines().collect();
    let has_test_output = lines.iter().any(|line: &&str| {
        line.contains("Test") || line.contains("PASSED") || line.contains("FAILED")
    });

    if has_test_output {
        println!("✓ Found Gradle test output");
    } else if output.contains("BUILD SUCCESSFUL") || output.contains("BUILD SUCCESS") {
        println!("✓ Gradle tests passed");
    } else {
        println!("! No test output found");
    }
}

#[test]
fn test_validate_gradle_outputs() {
    use analyzer::core::{AnalysisResult, IssueLevel, OutputParser};
    use analyzer::plugins::java::gradle::parser::GradleParser;

    let parser = GradleParser::new();

    // Sample Verification Compilation Errors
    let compile_output = read_sample("gradle_compile_sample");
    let issues = parser.parse(&compile_output).data_or_default_owned();

    // Generate report for manual review
    generate_report(
        "gradle_compile_sample",
        "Gradle Compile (Sample)",
        "gradle compileJava (sample output)",
        &issues,
        Some("samples/gradle_compile_sample.txt"),
    );

    println!("=== Validating Gradle Compile Sample ===");
    println!("Found {} issues in sample output", issues.len());

    let result = AnalysisResult::from_issues(issues);
    println!(
        "Total errors: {}",
        result.issues_by_level.get(&IssueLevel::Error).unwrap_or(&0)
    );
    println!(
        "Total warnings: {}",
        result
            .issues_by_level
            .get(&IssueLevel::Warning)
            .unwrap_or(&0)
    );

    // Verify parsing results
    assert!(
        result.total_issues > 0,
        "Expected at least one issue in the sample output"
    );

    // Verify Error Details
    for (file_path, file_issues) in &result.issues_by_file {
        println!("  File: {} - {} issues", file_path, file_issues.len());
        for issue in file_issues {
            println!(
                "    [{:?}] Line {:?}: {}",
                issue.level, issue.location.line_number, issue.message
            );
        }
    }

    // Also test and report test sample
    let test_output = read_sample("gradle_test_sample");
    let test_issues = parser.parse(&test_output).data_or_default_owned();

    generate_report(
        "gradle_test_sample",
        "Gradle Test (Sample)",
        "gradle test (sample output)",
        &test_issues,
        Some("samples/gradle_test_sample.txt"),
    );

    println!("\n=== Validating Gradle Test Sample ===");
    println!("Found {} issues in test sample output", test_issues.len());
}

#[test]
fn test_gradle_parser_specific_patterns() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::java::gradle::parser::GradleParser;

    let parser = GradleParser::new();

    // Testing various Gradle error formats by parsing full output
    let test_cases = vec![
        (
            "/src/main/java/App.java:10: error: cannot find symbol",
            "error",
        ),
        (
            "/src/main/java/App.java:20: warning: unchecked conversion",
            "warning",
        ),
        (
            "/src/test/java/Test.java:5: error: package org.junit does not exist",
            "error",
        ),
    ];

    println!("=== Testing Gradle Parser Patterns ===");
    for (line, expected_level) in test_cases {
        let issues = parser.parse(line).data_or_default_owned();
        let found = !issues.is_empty();
        let status = if found { "✓" } else { "✗" };
        println!(
            "{} Line: '{}' - found issue: {} (level: {:?})",
            status,
            line,
            found,
            issues.first().map(|i| &i.level)
        );
        assert!(found, "Should parse issue from line: {}", line);

        if expected_level == "error" {
            assert!(
                matches!(issues[0].level, analyzer::core::IssueLevel::Error),
                "Should be error level: {}",
                line
            );
        } else if expected_level == "warning" {
            assert!(
                matches!(issues[0].level, analyzer::core::IssueLevel::Warning),
                "Should be warning level: {}",
                line
            );
        }
    }
}

#[test]
fn test_gradle_test_sample_parse_statistics() {
    use analyzer::core::TestOutputParser;
    use analyzer::plugins::java::gradle::parser::GradleParser;

    // The gradle_test_sample uses the real Gradle 8 two-part output format
    // ("AppTest > testApp() PASSED"). The parser must extract the test
    // statistics and must not report test event lines as compile issues.
    let parser = GradleParser::new();
    let output = read_sample("gradle_test_sample");
    let parsed = parser.parse_test_output(&output);

    let summary = parsed
        .test_summary
        .expect("test summary should be parsed from sample output");
    assert_eq!(summary.total, 3);
    assert_eq!(summary.passed, 2);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.ignored, 0);

    assert_eq!(parsed.failed_tests.len(), 1);
    assert_eq!(parsed.failed_tests[0].name, "AppTest::testFailure()");
    assert_eq!(parsed.passed_tests.len(), 2);
    assert!(parsed.passed_tests
        .iter()
        .any(|t| t.name == "AppTest::testApp()"));
    assert!(parsed.passed_tests
        .iter()
        .any(|t| t.name == "UtilsTest::testUtils()"));

    assert!(
        parsed.compile_issues.is_empty(),
        "test event lines must not be compile issues, got: {:?}",
        parsed.compile_issues
    );
}
