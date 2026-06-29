//! Go Integration Tests
//! Execute actual Go commands, verify parsing logic matches actual output format

use std::path::PathBuf;

mod common;
use common::{
    fixtures_dir, generate_report, generate_test_report, is_command_available, run_command,
    save_raw_output,
};

fn go_project_path() -> PathBuf {
    fixtures_dir().join("go-project")
}

/// Check if Go is available
fn ensure_go() -> Result<(), String> {
    if !is_command_available("go") {
        return Err(
            "Go is not installed. Please install Go from https://golang.org/dl/".to_string(),
        );
    }
    Ok(())
}

/// Check if golangci-lint is available
fn ensure_golangci_lint() -> Result<(), String> {
    if !is_command_available("golangci-lint") {
        return Err("golangci-lint is not installed. Please install it from https://golangci-lint.run/usage/install/".to_string());
    }
    Ok(())
}

#[test]
fn test_go_build_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::go::parser::GoParser;

    if let Err(e) = ensure_go() {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = go_project_path();

    // Run go build
    let output = match run_command("go", &["build", "./..."], &project_path) {
        Ok(output) => output,
        Err(e) => {
            // go build may fail with compilation errors, which is expected
            println!("go build returned errors (expected): {}", e);
            e
        }
    };

    // Save raw output
    save_raw_output("go_build", &output);

    // Parse and generate report
    let parser = GoParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "go_build",
        "Go Build",
        "go build ./...",
        &issues,
        Some("raw_output/go_build.txt"),
    );

    println!("=== Go Build Output ===");
    println!("{}", output);

    // Verify go build output format
    let lines: Vec<&str> = output.lines().collect();
    let has_build_errors = lines.iter().any(|line| {
        // Format: file:line:col: message
        let parts: Vec<&str> = line.split(':').collect();
        parts.len() >= 3 && !line.starts_with('#')
    });

    if has_build_errors {
        println!("✓ Found go build error lines in expected format (file:line:col: message)");
    } else if output.is_empty() {
        println!("✓ Go build succeeded with no output");
    } else {
        println!("! Unexpected go build output format");
    }

    // Verify output format matches parser expectations
    for line in &lines {
        if !line.starts_with('#') && line.contains(':') {
            let parts: Vec<&str> = line.splitn(4, ':').collect();
            if parts.len() >= 3 && parts[1].trim().parse::<u32>().is_ok() {
                println!("  Found issue: {}", line);
            }
        }
    }
}

#[test]
fn test_go_vet_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::go::parser::GoParser;

    if let Err(e) = ensure_go() {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = go_project_path();

    // Run go vet
    let output = match run_command("go", &["vet", "./..."], &project_path) {
        Ok(output) => output,
        Err(e) => {
            println!("go vet returned errors: {}", e);
            e
        }
    };

    // Save raw output
    save_raw_output("go_vet", &output);

    // Parse and generate report
    let parser = GoParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "go_vet",
        "Go Vet",
        "go vet ./...",
        &issues,
        Some("raw_output/go_vet.txt"),
    );

    println!("=== Go Vet Output ===");
    println!("{}", output);

    // Verify go vet output format
    for line in output.lines() {
        if !line.starts_with('#') && line.contains(':') {
            let parts: Vec<&str> = line.splitn(5, ':').collect();
            if parts.len() >= 4
                && parts[1].trim().parse::<u32>().is_ok()
                && parts[2].trim().parse::<u32>().is_ok()
            {
                println!("  Found vet issue: {}", line);
            }
        }
    }
}

#[test]
fn test_golangci_lint_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::go::parser::GoParser;

    if let Err(e) = ensure_golangci_lint() {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = go_project_path();

    // Run golangci-lint
    let output = match run_command("golangci-lint", &["run", "./..."], &project_path) {
        Ok(output) => output,
        Err(e) => {
            println!("golangci-lint returned errors: {}", e);
            e
        }
    };

    // Save raw output
    save_raw_output("golangci_lint", &output);

    // Parse and generate report
    let parser = GoParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "golangci_lint",
        "Golangci-lint",
        "golangci-lint run ./...",
        &issues,
        Some("raw_output/golangci_lint.txt"),
    );

    println!("=== Golangci-lint Output ===");
    println!("{}", output);

    // Verify golangci-lint output format
    for line in output.lines() {
        if line.contains(':') && !line.starts_with('#') {
            // Format: file:line:col: message (linter)
            let parts: Vec<&str> = line.splitn(5, ':').collect();
            if parts.len() >= 4 {
                println!("  Found lint issue: {}", line);
            }
        }
    }
}

#[test]
fn test_go_test_output() {
    use analyzer::core::TestOutputParser;
    use analyzer::plugins::go::parser::GoParser;

    if let Err(e) = ensure_go() {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = go_project_path();

    // Run go test
    let output = match run_command("go", &["test", "-v", "./..."], &project_path) {
        Ok(output) => output,
        Err(e) => {
            println!("go test returned errors: {}", e);
            e
        }
    };

    // Save raw output
    save_raw_output("go_test", &output);

    // Parse and generate test report
    let parser = GoParser::new();
    let test_output = parser.parse_test_output(&output);
    generate_test_report(
        "go_test",
        "Go Test",
        "go test -v ./...",
        &test_output,
        Some("raw_output/go_test.txt"),
    );

    println!("=== Go Test Output ===");
    println!("{}", output);

    // Verify test output format
    let passed_count = output.lines().filter(|l| l.contains("--- PASS:")).count();
    let failed_count = output.lines().filter(|l| l.contains("--- FAIL:")).count();
    let skip_count = output.lines().filter(|l| l.contains("--- SKIP:")).count();

    println!("Test Summary:");
    println!("  Passed: {}", passed_count);
    println!("  Failed: {}", failed_count);
    println!("  Skipped: {}", skip_count);

    if passed_count > 0 {
        println!("✓ Found passed test lines");
    }
    if failed_count > 0 {
        println!("✓ Found failed test lines");
    }
    if skip_count > 0 {
        println!("✓ Found skipped test lines");
    }
}

#[test]
fn test_go_analyzer_traits() {
    use analyzer::core::BuildAnalyzer;
    use analyzer::plugins::go::analyzer::GoAnalyzer;

    let analyzer = GoAnalyzer::new();

    // Test basic trait methods
    assert_eq!(analyzer.name(), "go");
    assert!(analyzer.supported_commands().contains(&"go"));
    assert!(analyzer.supported_commands().contains(&"golang"));

    println!("✓ GoAnalyzer implements BuildAnalyzer correctly");
}

#[test]
fn test_go_parser_specific_patterns() {
    use analyzer::core::{IssueLevel, OutputParser};
    use analyzer::plugins::go::parser::GoParser;

    let parser = GoParser::new();

    // Test go build error format
    let build_error = "./main.go:10:5: undefined: someVariable";
    let issues = parser.parse(build_error).data_or_default_owned();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].location.file_path, "./main.go");
    assert_eq!(issues[0].location.line_number, Some(10));
    assert_eq!(issues[0].location.column_number, Some(5));
    assert!(matches!(issues[0].level, IssueLevel::Error));

    // Test go vet error format
    let vet_error = "./main.go:15:10: Printf format %s has arg x of wrong type int";
    let issues = parser.parse(vet_error).data_or_default_owned();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].location.file_path, "./main.go");
    assert_eq!(issues[0].location.line_number, Some(15));
    assert_eq!(issues[0].location.column_number, Some(10));
    assert!(matches!(issues[0].level, IssueLevel::Warning));

    // Test golangci-lint error format
    let lint_error = "main.go:20:3: Error return value of `fmt.Println` is not checked (errcheck)";
    let issues = parser.parse(lint_error).data_or_default_owned();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].location.file_path, "main.go");
    assert_eq!(issues[0].location.line_number, Some(20));
    assert_eq!(issues[0].location.column_number, Some(3));
    assert_eq!(issues[0].context, Some("linter: errcheck".to_string()));

    // Test golangci-lint with error code
    let lint_with_code = "main.go:25:5: SA1000: invalid regular expression (staticcheck)";
    let issues = parser.parse(lint_with_code).data_or_default_owned();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, Some("[SA1000]".to_string()));
    assert_eq!(issues[0].message, "invalid regular expression");

    println!("✓ GoParser correctly parses all Go tool output formats");
}

#[test]
fn test_go_test_parser() {
    use analyzer::core::TestOutputParser;
    use analyzer::plugins::go::parser::GoParser;

    let parser = GoParser::new();

    let test_output = r#"
=== RUN   TestAdd
--- PASS: TestAdd (0.01s)
=== RUN   TestDivide
--- FAIL: TestDivide (0.02s)
=== RUN   TestIntegration
--- SKIP: TestIntegration (0.00s) requires external service
PASS
ok  	example.com/myproject	0.052s
"#;

    let result = parser.parse_test_output(test_output);

    assert_eq!(result.passed_tests.len(), 1);
    assert_eq!(result.failed_tests.len(), 1);
    assert_eq!(result.ignored_tests.len(), 1);

    // Check passed test
    assert_eq!(result.passed_tests[0].name, "TestAdd");
    assert_eq!(result.passed_tests[0].execution_time, Some(0.01));

    // Check failed test
    assert_eq!(result.failed_tests[0].name, "TestDivide");
    assert_eq!(result.failed_tests[0].execution_time, Some(0.02));

    // Check skipped test
    assert_eq!(result.ignored_tests[0].name, "TestIntegration");

    println!("✓ GoParser correctly parses go test output");
}

#[test]
fn test_validate_go_outputs() {
    use analyzer::core::{OutputParser, TestOutputParser};
    use analyzer::plugins::go::parser::GoParser;

    let parser = GoParser::new();

    // Read sample files and validate parsing
    let samples = vec![
        ("go_build_sample.txt", "go build"),
        ("go_vet_sample.txt", "go vet"),
        ("golangci_lint_sample.txt", "golangci-lint"),
    ];

    for (filename, tool) in samples {
        let sample_path = common::samples_dir().join(filename);
        if let Ok(content) = std::fs::read_to_string(&sample_path) {
            let issues = parser.parse(&content).data_or_default_owned();
            println!(
                "✓ {}: Parsed {} issues from {}",
                tool,
                issues.len(),
                filename
            );

            // Generate report for sample
            let report_name = filename.replace("_sample.txt", "_sample");
            generate_report(
                &report_name,
                &format!("Go {} (Sample)", tool),
                tool,
                &issues,
                Some(&format!("samples/{}", filename)),
            );
        } else {
            println!("! Sample file not found: {}", sample_path.display());
        }
    }

    // Test go test sample
    let test_sample_path = common::samples_dir().join("go_test_sample.txt");
    if let Ok(content) = std::fs::read_to_string(&test_sample_path) {
        let test_output = parser.parse_test_output(&content);
        println!(
            "✓ go test: Parsed {} passed, {} failed, {} skipped from sample",
            test_output.passed_tests.len(),
            test_output.failed_tests.len(),
            test_output.ignored_tests.len()
        );

        // Generate test report for sample
        generate_test_report(
            "go_test_sample",
            "Go Test (Sample)",
            "go test -v ./...",
            &test_output,
            Some("samples/go_test_sample.txt"),
        );
    }
}
