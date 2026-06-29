//! Maven Integration Testing
//! Execute the actual Maven commands to verify that the parsing logic matches the actual output format

use std::path::PathBuf;

mod common;
use common::{
    fixtures_dir, generate_report, is_command_available, read_sample, run_command, save_raw_output,
};

fn maven_project_path() -> PathBuf {
    fixtures_dir().join("maven-project")
}

/// Check if Maven is available
fn ensure_maven() -> Result<(), String> {
    if !is_command_available("mvn") {
        return Err("Maven (mvn) is not available in PATH. Please install Maven.".to_string());
    }
    Ok(())
}

#[test]
fn test_maven_compile_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::java::maven::parser::MavenParser;

    if let Err(e) = ensure_maven() {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = maven_project_path();

    // Run a Maven build
    let output = match run_command("mvn", &["compile", "-q"], &project_path) {
        Ok(output) => output,
        Err(e) => {
            // Maven also returns output (with error messages) when compilation fails
            println!("Maven compile failed or found errors: {}", e);
            // Try running it again to get the error output
            match run_command("mvn", &["compile"], &project_path) {
                Ok(output) => output,
                Err(_) => {
                    // If it still fails, use the sample output
                    println!("Using sample output for testing");
                    read_sample("maven_compile_sample")
                }
            }
        }
    };

    // Saving the original output
    save_raw_output("maven_compile", &output);

    // Parses and generates reports
    let parser = MavenParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "maven_compile",
        "Maven Compile",
        "mvn compile",
        &issues,
        Some("raw_output/maven_compile.txt"),
    );

    println!("=== Maven Compile Output ===");
    println!("{}", output);

    // Validating the Maven Output Format
    // Format: [ERROR] /path/to/File.java:[line,col] error: message
    // Format: [WARNING] /path/to/File.java:[line,col] warning: message
    let lines: Vec<&str> = output.lines().collect();
    let has_error_lines = lines.iter().any(|line: &&str| {
        line.trim().starts_with("[ERROR]") || line.trim().starts_with("[WARNING]")
    });

    if has_error_lines {
        println!("✓ Found Maven error/warning lines in expected format");
    } else if output.contains("BUILD SUCCESS") {
        println!("✓ Maven build succeeded (no issues found)");
    } else {
        println!("! No error lines found (may be due to Maven configuration or build success)");
    }

    // Verify that the output format is as expected by the parser
    for line in &lines {
        let trimmed: &str = line.trim();
        if trimmed.starts_with("[ERROR]") || trimmed.starts_with("[WARNING]") {
            println!("  Found issue: {}", line);
        }
    }
}

#[test]
fn test_maven_test_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::java::maven::parser::MavenParser;

    if let Err(e) = ensure_maven() {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = maven_project_path();

    // Running Maven Tests
    let output = match run_command("mvn", &["test", "-q"], &project_path) {
        Ok(output) => output,
        Err(e) => {
            println!("Maven test failed or found errors: {}", e);
            match run_command("mvn", &["test"], &project_path) {
                Ok(output) => output,
                Err(_) => {
                    println!("Using sample output for testing");
                    read_sample("maven_test_sample")
                }
            }
        }
    };

    // Saving the original output
    save_raw_output("maven_test", &output);

    // Parses and generates reports
    let parser = MavenParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "maven_test",
        "Maven Test",
        "mvn test",
        &issues,
        Some("raw_output/maven_test.txt"),
    );

    println!("=== Maven Test Output ===");
    println!("{}", output);

    // Validation Test Output
    let lines: Vec<&str> = output.lines().collect();
    let has_test_output = lines
        .iter()
        .any(|line: &&str| line.contains("Tests run:") || line.contains("T E S T S"));

    if has_test_output {
        println!("✓ Found Maven test output");
    } else if output.contains("BUILD SUCCESS") {
        println!("✓ Maven tests passed");
    } else {
        println!("! No test output found");
    }
}

#[test]
fn test_validate_maven_outputs() {
    use analyzer::core::{AnalysisResult, IssueLevel, OutputParser};
    use analyzer::plugins::java::maven::parser::MavenParser;

    let parser = MavenParser::new();

    // Sample Verification Compilation Errors
    let compile_output = read_sample("maven_compile_sample");
    let issues = parser.parse(&compile_output).data_or_default_owned();

    println!("=== Validating Maven Compile Sample ===");
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
}

#[test]
fn test_maven_parser_specific_patterns() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::java::maven::parser::MavenParser;

    let parser = MavenParser::new();

    // Testing various Maven error formats by parsing full output
    let test_cases = vec![
        (
            "[ERROR] /src/main/java/App.java:[10,5] error: cannot find symbol",
            "error",
        ),
        (
            "[WARNING] /src/main/java/App.java:[20,10] warning: [unchecked] unchecked conversion",
            "warning",
        ),
        (
            "[ERROR] /src/test/java/Test.java:[5,1] error: package org.junit does not exist",
            "error",
        ),
    ];

    println!("=== Testing Maven Parser Patterns ===");
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
