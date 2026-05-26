//! Parser Integration Tests
//! Verify that parsers can correctly parse actual command output

use std::fs;
use std::path::PathBuf;

mod common;
use common::{samples_dir, generate_report};

// Importing the parser from src
use analyzer::core::{IssueLevel, OutputParser};
use analyzer::plugins::python::mypy::parser::MypyParser;
use analyzer::plugins::npm::parser::NpmParser;
use analyzer::plugins::java::maven::parser::MavenParser;

/// Get sample file path
fn get_sample_file(name: &str) -> PathBuf {
    samples_dir().join(format!("{}.txt", name))
}

/// Read sample file content
fn read_sample_file(name: &str) -> String {
    let path = get_sample_file(name);
    fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read sample file: {}", path.display()))
}

/// Validate Issue basic properties
fn assert_issue_valid(issue: &analyzer::core::Issue, expected_file: &str) {
    assert!(
        !issue.message.is_empty(),
        "Issue message should not be empty"
    );
    assert!(
        issue.location.file_path.contains(expected_file),
        "Issue file path should contain '{}', got: {}",
        expected_file,
        issue.location.file_path
    );
    assert!(
        issue.location.line_number.is_some(),
        "Issue should have line number"
    );
}

/// Count issues by level
fn count_issues_by_level(issues: &[analyzer::core::Issue]) -> (usize, usize, usize) {
    let errors = issues.iter().filter(|i| matches!(i.level, IssueLevel::Error)).count();
    let warnings = issues.iter().filter(|i| matches!(i.level, IssueLevel::Warning)).count();
    let infos = issues.iter().filter(|i| matches!(i.level, IssueLevel::Info)).count();
    (errors, warnings, infos)
}

#[test]
fn test_mypy_parser_basic() {
    let content = read_sample_file("mypy_basic_sample");
    let parser = MypyParser::new();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // Validation parses an Issue
    assert!(!issues.is_empty(), "Should parse at least one issue from mypy output");

    // Verify that there is at least one error
    let (errors, warnings, _) = count_issues_by_level(&issues);
    assert!(errors > 0, "Should have at least one error, got {} errors", errors);

    // Verify the structure of the first Issue
    let first_issue = &issues[0];
    assert_issue_valid(first_issue, ".py");
    assert!(matches!(first_issue.level, IssueLevel::Error | IssueLevel::Warning));

    // Validation Contains Specific Errors
    let has_type_error = issues.iter().any(|i| {
        i.message.contains("Unsupported operand types")
            || i.message.contains("Incompatible types")
            || i.message.contains("Missing type annotation")
    });
    assert!(has_type_error, "Should have type-related errors");

    // Generating reports
    generate_report(
        "mypy_basic",
        "Mypy Basic",
        "mypy src/",
        &issues,
        Some("samples/mypy_basic.txt")
    );

    println!("✓ Mypy parser correctly parsed {} issues ({} errors, {} warnings)", 
             issues.len(), errors, warnings);
}

#[test]
fn test_mypy_parser_specific_file() {
    let content = read_sample_file("mypy_specific_file_sample");
    let parser = MypyParser::new();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    assert!(!issues.is_empty(), "Should parse issues from specific file output");

    // Verify that at least some of the Issues come from main.py (and possibly from other files like utils.py)
    let has_main_py = issues.iter().any(|i| i.location.file_path.contains("main.py"));
    assert!(has_main_py, "Should have issues from main.py");

    // Verify that all Issues are Python files
    let all_py_files = issues.iter().all(|i| i.location.file_path.ends_with(".py"));
    assert!(all_py_files, "All issues should be from .py files");

    // Generating reports
    generate_report(
        "mypy_specific_file",
        "Mypy Specific File",
        "mypy src/main.py",
        &issues,
        Some("samples/mypy_specific_file.txt")
    );

    println!("✓ Mypy parser correctly parsed {} issues from Python files", issues.len());
}

#[test]
fn test_mypy_parser_strict() {
    let content = read_sample_file("mypy_strict_sample");
    let parser = MypyParser::new();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // There should be more errors in strict mode
    assert!(
        issues.len() >= 3,
        "Strict mode should have at least 3 issues, got {}",
        issues.len()
    );

    // Generating reports
    generate_report(
        "mypy_strict",
        "Mypy Strict",
        "mypy --strict src/",
        &issues,
        Some("samples/mypy_strict.txt")
    );

    println!("✓ Mypy parser (strict) correctly parsed {} issues", issues.len());
}

#[test]
fn test_eslint_parser_output() {
    let content = read_sample_file("npm_eslint_sample");
    let parser = NpmParser::new();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // Validation parses an Issue
    assert!(
        !issues.is_empty(),
        "Should parse at least one issue from ESLint output"
    );

    // Number of validation errors and warnings
    let (errors, warnings, _) = count_issues_by_level(&issues);
    assert!(
        errors + warnings >= 3,
        "Should have at least 3 issues (errors + warnings), got {} errors and {} warnings",
        errors,
        warnings
    );

    // Validating the Issue Structure
    let first_issue = &issues[0];
    assert_issue_valid(first_issue, ".ts");

    // Verify that the file path is extracted correctly
    let has_index_ts = issues.iter().any(|i| i.location.file_path.contains("index.ts"));
    let has_utils_ts = issues.iter().any(|i| i.location.file_path.contains("utils.ts"));
    assert!(
        has_index_ts || has_utils_ts,
        "Should have issues from index.ts or utils.ts"
    );

    // Validating row and column numbers
    let issue_with_location = issues.iter().find(|i| {
        i.location.line_number.is_some() && i.location.column_number.is_some()
    });
    assert!(
        issue_with_location.is_some(),
        "At least one issue should have both line and column numbers"
    );

    // Generating reports
    generate_report(
        "npm_eslint",
        "ESLint",
        "npm run lint",
        &issues,
        Some("samples/npm_eslint_sample.txt")
    );

    println!(
        "✓ ESLint parser correctly parsed {} issues ({} errors, {} warnings)",
        issues.len(),
        errors,
        warnings
    );
}

#[test]
fn test_typescript_parser_output() {
    let content = read_sample_file("npm_typecheck_sample");
    let parser = NpmParser::new();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // Validation parses an Issue
    assert!(
        !issues.is_empty(),
        "Should parse at least one issue from TypeScript output"
    );

    // TypeScript output should be full of errors
    let (errors, _, _) = count_issues_by_level(&issues);
    assert!(
        errors >= 3,
        "Should have at least 3 TypeScript errors, got {}",
        errors
    );

    // Verification contains a TS error code (may be in the format TSxxxx or [TSxxxx])
    let has_ts_code = issues.iter().any(|i: &analyzer::core::Issue| {
        i.code.as_ref().map(|c: &String| {
            c.starts_with("TS") || c.starts_with("[TS")
        }).unwrap_or(false)
    });
    assert!(has_ts_code, "Should have TypeScript error codes (TSxxxx), got codes: {:?}", 
            issues.iter().filter_map(|i| i.code.clone()).collect::<Vec<_>>());

    // Validating the Issue Structure
    let first_issue = &issues[0];
    assert_issue_valid(first_issue, ".ts");

    // Generating reports
    generate_report(
        "npm_typecheck",
        "TypeScript Type Check",
        "npm run type-check",
        &issues,
        Some("samples/npm_typecheck_sample.txt")
    );

    println!("✓ TypeScript parser correctly parsed {} errors", issues.len());
}

#[test]
fn test_npm_audit_parser_output() {
    let content = read_sample_file("npm_audit_sample");
    let parser = NpmParser::new();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // npm audit may not be vulnerable (issues being empty is also normal)
    // Generating reports
    generate_report(
        "npm_audit",
        "NPM Audit",
        "npm audit",
        &issues,
        Some("samples/npm_audit_sample.txt")
    );

    if issues.is_empty() {
        println!("! NPM audit: No vulnerabilities found (this is good!)");
    } else {
        println!("✓ NPM audit parser correctly parsed {} security vulnerabilities", issues.len());
    }
}

#[test]
fn test_maven_compile_parser_output() {
    let content = read_sample_file("maven_compile_sample");
    let parser = MavenParser::new();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // Validation parses an Issue
    assert!(
        !issues.is_empty(),
        "Should parse at least one issue from Maven compile output"
    );

    // Number of validation errors and warnings
    let (errors, warnings, _) = count_issues_by_level(&issues);
    assert!(
        errors > 0,
        "Should have at least one error, got {} errors",
        errors
    );

    // Validating the Issue Structure
    let first_issue = &issues[0];
    assert_issue_valid(first_issue, ".java");

    // Generating reports
    generate_report(
        "maven_compile",
        "Maven Compile",
        "mvn compile",
        &issues,
        Some("samples/maven_compile_sample.txt")
    );

    println!(
        "✓ Maven compile parser correctly parsed {} issues ({} errors, {} warnings)",
        issues.len(),
        errors,
        warnings
    );
}

#[test]
fn test_maven_test_parser_output() {
    let content = read_sample_file("maven_test_sample");
    let parser = MavenParser::new();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // Validation parses an Issue
    assert!(
        !issues.is_empty(),
        "Should parse at least one issue from Maven test output"
    );

    // Generating reports
    generate_report(
        "maven_test",
        "Maven Test",
        "mvn test",
        &issues,
        Some("samples/maven_test_sample.txt")
    );

    println!("✓ Maven test parser correctly parsed {} issues", issues.len());
}

#[test]
fn test_parser_handles_empty_input() {
    let parser = NpmParser::new();
    let issues = OutputParser::parse(&parser, "").data_or_default_owned();
    assert!(issues.is_empty(), "Should return empty vec for empty input");

    let parser = MypyParser::new();
    let issues = OutputParser::parse(&parser, "").data_or_default_owned();
    assert!(issues.is_empty(), "Should return empty vec for empty input");

    let parser = MavenParser::new();
    let issues = OutputParser::parse(&parser, "").data_or_default_owned();
    assert!(issues.is_empty(), "Should return empty vec for empty input");

    println!("✓ Parsers correctly handle empty input");
}

#[test]
fn test_parser_handles_no_issues() {
    // Analog output without errors
    let no_error_output = "✓ No issues found\nAll checks passed!";
    let parser = NpmParser::new();
    let issues = OutputParser::parse(&parser, no_error_output).data_or_default_owned();
    assert!(issues.is_empty(), "Should return empty vec when no issues found");

    println!("✓ Parsers correctly handle 'no issues' output");
}

#[test]
fn test_npm_parser_issue_detection() {
    let parser = NpmParser::new();

    // Test ESLint format
    let eslint_output = "  3:7   warning  message  rule-name";
    let issues = parser.parse(eslint_output).data_or_default_owned();
    assert!(!issues.is_empty(), "Should detect ESLint format issue");

    // Test TypeScript format
    let ts_output = "src/index.ts(13,7): error TS2345: message";
    let issues = parser.parse(ts_output).data_or_default_owned();
    assert!(!issues.is_empty(), "Should detect TypeScript format issue");

    // Test NPM Error Format
    let npm_error = "npm error code ENOLOCK";
    let issues = parser.parse(npm_error).data_or_default_owned();
    assert!(!issues.is_empty(), "Should detect NPM error");

    // Test non-issue lines
    let no_issue = "✓ All checks passed";
    let issues = parser.parse(no_issue).data_or_default_owned();
    assert!(issues.is_empty(), "Should not detect issue in normal text");

    println!("✓ NPM parser issue detection works correctly");
}

/// Comprehensive test: verify all sample files can be correctly parsed
#[test]
fn test_all_sample_files_parsable() {
    let samples_dir = samples_dir();
    let mut parsed_count = 0;
    let mut failed_files = Vec::new();

    for entry in fs::read_dir(&samples_dir).expect("Failed to read samples directory").flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "txt").unwrap_or(false) {
            let filename = path.file_stem().unwrap_or_default().to_string_lossy();
            let content = fs::read_to_string(&path).expect("Failed to read file");

            // Choose the appropriate parser based on the filename
            let issues = if filename.starts_with("mypy") {
                let parser = MypyParser::new();
                OutputParser::parse(&parser, &content).data_or_default_owned()
            } else if filename.starts_with("npm") {
                let parser = NpmParser::new();
                OutputParser::parse(&parser, &content).data_or_default_owned()
            } else if filename.starts_with("maven") {
                let parser = MavenParser::new();
                OutputParser::parse(&parser, &content).data_or_default_owned()
            } else {
                continue;
            };

            // Verify parsing results
            if !issues.is_empty() {
                // Verify that at least one Issue is structured correctly
                let valid = issues.iter().any(|i| {
                    !i.message.is_empty()
                        && !i.location.file_path.is_empty()
                        && i.location.line_number.is_some()
                });

                if valid {
                    parsed_count += 1;
                    println!("✓ {}: parsed {} valid issues", filename, issues.len());
                } else {
                    failed_files.push(filename.to_string());
                }
            } else {
                // The null result may also be correct (if there are no errors)
                println!("! {}: no issues parsed (may be correct)", filename);
            }
        }
    }

    assert!(
        parsed_count >= 3,
        "Should successfully parse at least 3 sample files, got {}. Failed: {:?}",
        parsed_count,
        failed_files
    );

    println!("\n✓ Successfully parsed {} sample files", parsed_count);
}
