//! NPM Error Handling Tests
//! Test NPM analyzer behavior when commands fail and no issues are parsed

use std::path::PathBuf;
use std::fs;

mod common;
use common::{fixtures_dir, is_command_available, save_raw_output};

fn npm_project_path() -> PathBuf {
    fixtures_dir().join("npm-project")
}

/// Create a temporary package.json with a broken lint script
fn create_broken_lint_project() -> PathBuf {
    let temp_dir = std::env::temp_dir().join("analyzer_test_broken_lint");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
    
    let package_json = r#"{
    "name": "test-broken-lint",
    "version": "1.0.0",
    "scripts": {
        "lint": "eslint-nonexistent-command src/"
    }
}"#;
    
    fs::write(temp_dir.join("package.json"), package_json)
        .expect("Failed to write package.json");
    
    temp_dir
}

/// Create a temporary package.json with a lint script that fails but has parseable output
#[allow(dead_code)]
fn create_lint_with_errors_project() -> PathBuf {
    let temp_dir = std::env::temp_dir().join("analyzer_test_lint_errors");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
    fs::create_dir_all(temp_dir.join("src")).expect("Failed to create src directory");
    
    let package_json = r#"{
    "name": "test-lint-errors",
    "version": "1.0.0",
    "scripts": {
        "lint": "echo 'src/index.js:1:1: error: Missing semicolon semi'"
    }
}"#;
    
    let js_file = "const x = 1"; // Missing semicolon
    
    fs::write(temp_dir.join("package.json"), package_json)
        .expect("Failed to write package.json");
    fs::write(temp_dir.join("src/index.js"), js_file)
        .expect("Failed to write index.js");
    
    temp_dir
}

fn default_analyze_options(subcommand: &str) -> analyzer::core::AnalyzeOptions {
    analyzer::core::AnalyzeOptions {
        subcommand: Some(analyzer::core::SubCommand::new(subcommand)),
        filter_warnings: false,
        filter_paths: vec![],
        output_file: None,
        verbosity: analyzer::core::Verbosity::Normal,
        source_dir: None,
        build_dir: None,
        cmake_generator: None,
        target: None,
        target_files: vec![],
        include_paths: vec![],
        defines: vec![],
        cpp_standard: None,
        json_output: false,
        report_format: analyzer::core::ReportFormat::Markdown,
        ..analyzer::core::AnalyzeOptions::default()
    }
}

#[test]
fn test_npm_analyzer_command_failure_no_issues() {
    use analyzer::plugins::npm::NpmAnalyzer;
    use analyzer::core::BuildAnalyzer;
    
    if !is_command_available("npm") {
        println!("Skipping test: npm is not available");
        return;
    }

    let temp_dir = create_broken_lint_project();
    let original_dir = std::env::current_dir().expect("Failed to get current directory");
    
    // Change to the temp directory
    std::env::set_current_dir(&temp_dir).expect("Failed to change directory");
    
    let analyzer = NpmAnalyzer::npm();
    let options = default_analyze_options("run lint");
    
    let result = analyzer.analyze(&options);
    
    // Restore original directory (use stored path directly)
    let _ = std::env::set_current_dir(&original_dir);
    
    // The analyze should succeed (not return Err) even if command fails
    assert!(result.is_ok(), "Analyzer should not fail even when command fails");
    
    let analysis_result = result.unwrap();
    // Should have 0 issues since the broken command output won't be parseable
    assert_eq!(analysis_result.total_issues, 0, "Should have 0 parsed issues");
    
    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_npm_analyzer_parses_issues_from_failed_command() {
    use analyzer::plugins::npm::parser::NpmParser;
    use analyzer::core::OutputParser;
    
    // Test that the parser can extract issues from ESLint-style output
    let eslint_output = r#"
src/index.js:1:1: error: Missing semicolon semi
src/index.js:2:5: warning: Unused variable 'foo' no-unused-vars

2 problems (1 error, 1 warning)
"#;
    
    let parser = NpmParser::new();
    let issues = parser.parse(eslint_output).data_or_default_owned();
    
    // Should parse 2 issues
    assert_eq!(issues.len(), 2, "Should parse 2 issues from ESLint output");
    
    // Save for debugging
    save_raw_output("npm_eslint_parsed", eslint_output);
}

#[test]
fn test_npm_analyzer_handles_config_error() {
    use analyzer::plugins::npm::parser::NpmParser;
    use analyzer::core::OutputParser;
    
    // Test parser behavior with config error output (no parseable issues)
    let config_error_output = r#"
> test@1.0.0 lint
> eslint src/

Oops! Something went wrong! :(ESLint: 8.57.1

ESLint couldn't find the config "@typescript-eslint/recommended" to extend from.
Please check that the name of the config is correct.
"#;
    
    let parser = NpmParser::new();
    let issues = parser.parse(config_error_output).data_or_default_owned();
    
    // Should have 0 issues since config errors aren't in ESLint format
    assert_eq!(issues.len(), 0, "Should have 0 issues for config errors");
    
    // Save for debugging
    save_raw_output("npm_config_error", config_error_output);
}

#[test]
fn test_npm_analyzer_with_real_project() {
    use analyzer::plugins::npm::NpmAnalyzer;
    use analyzer::core::BuildAnalyzer;
    
    if !is_command_available("npm") {
        println!("Skipping test: npm is not available");
        return;
    }

    let project_path = npm_project_path();
    let original_dir = std::env::current_dir().expect("Failed to get current directory");
    
    // Change to the npm project directory
    std::env::set_current_dir(&project_path).expect("Failed to change directory");
    
    let analyzer = NpmAnalyzer::npm();
    let options = default_analyze_options("run lint");
    
    let result = analyzer.analyze(&options);
    
    // Restore original directory (use stored path directly)
    let _ = std::env::set_current_dir(&original_dir);
    
    // The analyze should succeed (not return Err)
    assert!(result.is_ok(), "Analyzer should complete without error");
    
    let analysis_result = result.unwrap();
    println!("Found {} issues", analysis_result.total_issues);
    
    // The test project may or may not have issues depending on ESLint config
    // The important thing is that the analyzer doesn't panic or return Err
}

#[test]
fn test_npm_parser_typescript_errors() {
    use analyzer::plugins::npm::parser::NpmParser;
    use analyzer::core::OutputParser;
    
    // Test TypeScript error format parsing
    let tsc_output = r#"
src/index.ts(1,7): error TS2322: Type 'string' is not assignable to type 'number'.
src/utils.ts(5,3): error TS2345: Argument of type 'any' is not assignable to parameter of type 'string'.
"#;
    
    let parser = NpmParser::new();
    let issues = parser.parse(tsc_output).data_or_default_owned();
    
    // Should parse 2 TypeScript errors
    assert_eq!(issues.len(), 2, "Should parse 2 TypeScript errors");
    
    // Verify file paths
    let file_paths: Vec<&str> = issues.iter()
        .map(|i| i.location.file_path.as_str())
        .collect();
    assert!(file_paths.contains(&"src/index.ts"), "Should contain src/index.ts");
    assert!(file_paths.contains(&"src/utils.ts"), "Should contain src/utils.ts");
    
    save_raw_output("npm_typescript_errors", tsc_output);
}

#[test]
fn test_npm_parser_audit_output() {
    use analyzer::plugins::npm::parser::NpmParser;
    use analyzer::core::OutputParser;
    
    // Test npm audit output parsing
    let audit_output = r#"
npm error code EAUDIT
npm error audit No lockfile found
npm error A complete log of this run can be found in: /path/to/log

found 0 vulnerabilities
"#;
    
    let parser = NpmParser::new();
    let issues = parser.parse(audit_output).data_or_default_owned();
    
    // Audit errors should be parsed
    assert!(issues.len() > 0, "Should parse npm audit errors");
    
    save_raw_output("npm_audit_parsed", audit_output);
}

#[test]
fn test_pnpm_analyzer_execution() {
    use analyzer::plugins::npm::NpmAnalyzer;
    use analyzer::core::BuildAnalyzer;
    
    if !is_command_available("pnpm") {
        println!("Skipping test: pnpm is not available");
        return;
    }

    let project_path = npm_project_path();
    let original_dir = std::env::current_dir().expect("Failed to get current directory");
    
    // Change to the npm project directory
    std::env::set_current_dir(&project_path).expect("Failed to change directory");
    
    let analyzer = NpmAnalyzer::pnpm();
    let options = default_analyze_options("run lint");
    
    let result = analyzer.analyze(&options);
    
    // Restore original directory (use stored path directly)
    let _ = std::env::set_current_dir(&original_dir);
    
    // Should complete without error
    assert!(result.is_ok(), "Pnpm analyzer should complete without error");
}

#[test]
fn test_npm_parser_eslint_verbose_format() {
    use analyzer::plugins::npm::parser::NpmParser;
    use analyzer::core::OutputParser;
    
    // Test ESLint verbose format with file path on separate line
    // This is the format used by turbo and other tools
    let eslint_verbose_output = r#"D:\项目\agent\graph-agent\apps\cli-app\src\commands\agent\index.ts
    13:31  warning  'parseAndValidateAgentLoopConfig' is defined but never used. Allowed unused vars must match /^_/u  @typescript-eslint/no-unused-vars
    14:10  warning  'readFileSync' is defined but never used. Allowed unused vars must match /^_/u                     @typescript-eslint/no-unused-vars
    83:32  warning  Unexpected any. Specify a different type                               @typescript-eslint/no-explicit-any
    92:22  warning  'error' is defined but never used                                      @typescript-eslint/no-unused-vars
   177:32  warning  Unexpected any. Specify a different type                               @typescript-eslint/no-explicit-any

D:\项目\agent\graph-agent\apps\cli-app\src\commands\agent\utils.ts
    10:5   error    'foo' is assigned a value but never used                              @typescript-eslint/no-unused-vars
"#;
    
    let parser = NpmParser::new();
    let issues = parser.parse(eslint_verbose_output).data_or_default_owned();
    
    // Should parse all 6 issues
    assert_eq!(issues.len(), 6, "Should parse 6 issues from ESLint verbose output");
    
    // Verify first issue details
    let first_issue = &issues[0];
    assert_eq!(first_issue.location.file_path, "D:\\项目\\agent\\graph-agent\\apps\\cli-app\\src\\commands\\agent\\index.ts");
    assert_eq!(first_issue.location.line_number, Some(13));
    assert_eq!(first_issue.location.column_number, Some(31));
    assert!(matches!(first_issue.level, analyzer::core::IssueLevel::Warning));
    assert!(first_issue.message.contains("parseAndValidateAgentLoopConfig"));
    
    // Verify second file's issue
    let last_issue = &issues[5];
    assert_eq!(last_issue.location.file_path, "D:\\项目\\agent\\graph-agent\\apps\\cli-app\\src\\commands\\agent\\utils.ts");
    assert_eq!(last_issue.location.line_number, Some(10));
    assert!(matches!(last_issue.level, analyzer::core::IssueLevel::Error));
    
    // Save for debugging
    save_raw_output("npm_eslint_verbose", eslint_verbose_output);
}

#[test]
fn test_npm_parser_eslint_message_extraction() {
    use analyzer::plugins::npm::parser::NpmParser;
    use analyzer::core::OutputParser;
    
    // Test that rule names are correctly separated from messages
    let eslint_output = r#"src/test.ts
    1:1  warning  'foo' is defined but never used. Allowed unused vars must match /^_/u  @typescript-eslint/no-unused-vars
    2:1  error    Unexpected any. Specify a different type                               @typescript-eslint/no-explicit-any
"#;
    
    let parser = NpmParser::new();
    let issues = parser.parse(eslint_output).data_or_default_owned();
    
    assert_eq!(issues.len(), 2, "Should parse 2 issues");
    
    // Check that rule names are not included in the message
    let first_msg = &issues[0].message;
    assert!(first_msg.contains("'foo' is defined but never used"));
    assert!(!first_msg.contains("@typescript-eslint"), "Rule name should be extracted from message");
    
    let second_msg = &issues[1].message;
    assert!(second_msg.contains("Unexpected any"));
    assert!(!second_msg.contains("@typescript-eslint"), "Rule name should be extracted from message");
}
