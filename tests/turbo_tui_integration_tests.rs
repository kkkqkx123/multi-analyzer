//! Integration tests for Turbo TUI mode output parsing
//! Tests that analyzer can correctly capture ESLint output when using turbo with "ui": "tui"

use std::path::PathBuf;

mod common;
use common::{fixtures_dir, generate_report, is_command_available, run_command, save_raw_output};

fn turbo_project_path() -> PathBuf {
    fixtures_dir().join("sample_turbo_project")
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
fn test_turbo_tui_output_capture() {
    use analyzer::core::BuildAnalyzer;
    use analyzer::plugins::npm::NpmAnalyzer;

    if !is_command_available("npm") {
        println!("Skipping test: npm is not available");
        return;
    }

    let project_path = turbo_project_path();
    let original_dir = std::env::current_dir().expect("Failed to get current directory");

    // Change to the turbo project directory
    std::env::set_current_dir(&project_path).expect("Failed to change directory");

    let analyzer = NpmAnalyzer::npm();
    let options = default_analyze_options("run lint");

    let result = analyzer.analyze(&options);

    // Restore original directory
    let _ = std::env::set_current_dir(&original_dir);

    // The analyze should complete without error
    assert!(result.is_ok(), "Analyzer should complete without error");

    let analysis = result.unwrap();

    // Print debug info
    println!("Total issues found: {}", analysis.total_issues);
    println!("Errors: {}", analysis.errors().len());
    println!("Warnings: {}", analysis.warnings().len());

    // If the underlying command itself failed to execute (e.g. the fixture's
    // toolchain — turbo/eslint — is not installed in this environment), there is
    // nothing for the parser to capture. That is an environment limitation, not a
    // regression in output capture, so we skip rather than fail.
    if analysis.command_failed {
        // Even when the underlying toolchain is missing, the analyzer must
        // surface the *raw* command error (not silently report success). This
        // is the design requirement: "output the original error when dependencies
        // are missing". We assert the fallback issue carries that raw text.
        let surfaced = analysis.issues_by_file.values().flatten().any(|i| {
            i.message.contains("Command failed")
                && (i.message.contains("turbo")
                    || i.message.contains("not found")
                    || i.message.contains("npm error"))
        });
        assert!(
            surfaced,
            "When the toolchain is missing, the analyzer should surface the raw command error"
        );
        println!(
            "Skipping ESLint-capture assertion: `npm run lint` failed to execute (exit code {:?}); \
             the fixture's toolchain is likely not installed in this environment.",
            analysis.exit_code
        );
        return;
    }

    // When the command succeeded, we expect ESLint issues to be captured. A
    // near-empty result indicates turbo's TUI mode swallowed the output.
    if analysis.total_issues == 0 || (analysis.total_issues == 1 && !analysis.errors().is_empty()) {
        panic!(
            "ESLint issues were not captured despite a successful run. This may be due to turbo TUI mode \
             interfering with output capture. Try running with CI=true or --output-logs=full"
        );
    }
}

#[test]
fn test_turbo_real_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::npm::parser::NpmParser;

    if !is_command_available("npm") {
        println!("Skipping test: npm is not available");
        return;
    }

    let project_path = turbo_project_path();

    // Run actual npm run lint command
    let output = match run_command("npm", &["run", "lint"], &project_path) {
        Ok(output) => output,
        Err(e) => {
            println!(
                "npm run lint failed (expected if there are lint errors): {}",
                e
            );
            // Try to get output even if command failed
            e.to_string()
        }
    };

    // Save raw output
    save_raw_output("turbo_real_output", &output);

    println!("=== Turbo Real Output ===");
    println!("{}", output);
    println!("========================");

    // Parse output
    let parser = NpmParser::new();
    let issues = parser.parse(&output).data_or_default_owned();

    // Generate report
    generate_report(
        "turbo_real_output",
        "Turbo with TUI",
        "npm run lint",
        &issues,
        Some("raw_output/turbo_real_output.txt"),
    );

    println!("Parsed {} issues from real turbo output", issues.len());

    // Save the result for analysis
    if issues.is_empty() {
        println!("WARNING: No issues parsed from turbo output. This may indicate:");
        println!("  1. Turbo TUI mode is interfering with output capture");
        println!("  2. The output format is different than expected");
        println!("  3. ESLint is not configured or has no issues to report");
    }
}

#[test]
fn test_turbo_tui_output_format() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::npm::parser::NpmParser;

    // Simulate the output format that turbo with TUI might produce
    // This tests if our parser can handle the format
    let turbo_tui_output = r#"
web:lint: 
web:lint: > web@1.0.0 lint
web:lint: > eslint src/
web:lint: 
web:lint: D:\项目\cli\analyzer\tests\data\fixtures\sample_turbo_project\apps\web\src\index.ts
web:lint:    4:7   error    'unusedVariable' is assigned a value but never used       @typescript-eslint/no-unused-vars
web:lint:    7:24  error    'unusedParam' is defined but never used                   @typescript-eslint/no-unused-vars
web:lint:   12:24  warning  Unexpected any. Specify a different type                  @typescript-eslint/no-explicit-any
web:lint:   12:36  warning  Unexpected any. Specify a different type                  @typescript-eslint/no-explicit-any
web:lint:   17:10  error    'unusedFunction' is defined but never used                @typescript-eslint/no-unused-vars
web:lint: 
web:lint: ✖ 5 problems (3 errors, 2 warnings)
web:lint: 

 Tasks:    1 successful, 1 total
Cached:    0 cached, 1 total
  Time:    1.234s 
"#;

    // Save raw output
    save_raw_output("turbo_tui_sample", turbo_tui_output);

    let parser = NpmParser::new();
    let issues = parser.parse(turbo_tui_output).data_or_default_owned();

    // Generate report
    generate_report(
        "turbo_tui_sample",
        "Turbo TUI Sample",
        "turbo run lint",
        &issues,
        Some("raw_output/turbo_tui_sample.txt"),
    );

    println!("Parsed {} issues from turbo TUI sample", issues.len());

    // Should parse the ESLint issues
    assert!(
        issues.len() >= 5,
        "Should parse at least 5 ESLint issues from turbo TUI output, found {}",
        issues.len()
    );
}

#[test]
fn test_turbo_stream_output_format() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::npm::parser::NpmParser;

    // Simulate the output format with turbo stream prefixes
    // This is what turbo outputs when using --output-logs=full
    let turbo_stream_output = r#"
web:lint: 
web:lint: > web@1.0.0 lint
web:lint: > eslint src/
web:lint: 
web:lint: D:\project\apps\web\src\index.ts
web:lint:   4:7   error  'unusedVariable' is assigned a value but never used  @typescript-eslint/no-unused-vars
web:lint:   7:24  error  'unusedParam' is defined but never used              @typescript-eslint/no-unused-vars
web:lint: 
web:lint: ✖ 2 problems (2 errors, 0 warnings)
web:lint: 
web:lint: ERROR: command finished with error: command (D:\project\apps\web) pnpm run lint exited (1)

 Tasks:    0 successful, 1 total
Cached:    0 cached, 1 total
  Time:    1.234s 
Failed:    web#lint

 ERROR  run failed: command  exited (1)
"#;

    // Save raw output
    save_raw_output("turbo_stream_sample", turbo_stream_output);

    let parser = NpmParser::new();
    let issues = parser.parse(turbo_stream_output).data_or_default_owned();

    // Generate report
    generate_report(
        "turbo_stream_sample",
        "Turbo Stream Sample",
        "turbo run lint --output-logs=full",
        &issues,
        Some("raw_output/turbo_stream_sample.txt"),
    );

    println!("Parsed {} issues from turbo stream output", issues.len());

    // Should parse the ESLint issues
    assert!(
        issues.len() >= 2,
        "Should parse at least 2 ESLint issues from turbo stream output, found {}",
        issues.len()
    );
}

#[test]
fn test_strip_turbo_prefix() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::npm::parser::NpmParser;

    // Test that lines with turbo prefix are correctly parsed
    let output_with_prefix = r#"web:lint:    4:7   error    'unusedVariable' is assigned a value but never used  @typescript-eslint/no-unused-vars
app:lint:     10:5  warning  Unexpected any                                     @typescript-eslint/no-explicit-any
"#;

    // Save raw output
    save_raw_output("turbo_prefix_test", output_with_prefix);

    let parser = NpmParser::new();
    let issues = parser.parse(output_with_prefix).data_or_default_owned();

    // Generate report
    generate_report(
        "turbo_prefix_test",
        "Turbo Prefix Test",
        "turbo run lint",
        &issues,
        Some("raw_output/turbo_prefix_test.txt"),
    );

    println!("Parsed {} issues with turbo prefix", issues.len());

    assert_eq!(issues.len(), 2, "Should parse 2 issues with turbo prefix");

    // Verify the first issue
    let first = &issues[0];
    assert_eq!(first.location.line_number, Some(4));
    assert!(matches!(first.level, analyzer::core::IssueLevel::Error));
    assert!(first.message.contains("unusedVariable"));

    // Verify the second issue
    let second = &issues[1];
    assert_eq!(second.location.line_number, Some(10));
    assert!(matches!(second.level, analyzer::core::IssueLevel::Warning));
    assert!(second.message.contains("Unexpected any"));
}

#[test]
fn test_turbo_tui_with_borders() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::npm::parser::NpmParser;

    // Test parsing real TUI format with border characters
    // This is what turbo outputs when running in a real terminal with TUI mode
    let turbo_tui_with_borders = r#"╭──────────────────────────────────────────────────────────────────────────╮    
│                                                                          │    
│                     Update available v2.8.3 ≫ v2.9.6                     │    
╰──────────────────────────────────────────────────────────────────────────╯    
• turbo 2.8.3
• Packages in scope: @graph-agent/storage
• Running lint in 1 packages
• Remote caching disabled
┌─ @graph-agent/storage#lint > cache hit, replaying logs f184635148ed46e6 

> @graph-agent/storage@1.0.0 lint D:\project\packages\storage
> eslint . --ext .ts


D:\project\packages\storage\src\json\base-json-storage.ts
   16:3   warning  'CompressionResult' is defined but never used  @typescript-eslint/no-unused-vars
   422:17  warning  'id' is assigned a value but never used        @typescript-eslint/no-unused-vars

✖ 7 problems (0 errors, 7 warnings)
└─ @graph-agent/storage#lint ──"#;

    // Save raw output
    save_raw_output("turbo_tui_borders", turbo_tui_with_borders);

    let parser = NpmParser::new();
    let issues = parser.parse(turbo_tui_with_borders).data_or_default_owned();

    // Generate report
    generate_report(
        "turbo_tui_borders",
        "Turbo TUI with Borders",
        "turbo run lint (TUI mode)",
        &issues,
        Some("raw_output/turbo_tui_borders.txt"),
    );

    println!("Parsed {} issues from turbo TUI with borders", issues.len());

    // Should parse the ESLint issues (2 warnings)
    assert!(
        issues.len() >= 2,
        "Should parse at least 2 ESLint issues from turbo TUI with borders, found {}",
        issues.len()
    );

    // Verify the first issue
    let first = &issues[0];
    assert_eq!(first.location.line_number, Some(16));
    assert_eq!(first.location.column_number, Some(3));
    assert!(matches!(first.level, analyzer::core::IssueLevel::Warning));
    assert!(first.message.contains("CompressionResult"));

    // Verify the second issue
    let second = &issues[1];
    assert_eq!(second.location.line_number, Some(422));
    assert_eq!(second.location.column_number, Some(17));
    assert!(matches!(second.level, analyzer::core::IssueLevel::Warning));
    assert!(second.message.contains("id"));
}

#[test]
fn test_turbo_tui_scoped_packages() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::npm::parser::NpmParser;

    // Test parsing output with scoped packages (@scope/package#task format)
    let turbo_scoped_output = r#"┌─ @graph-agent/script-executors#lint > cache hit, replaying logs 30d596d992b3838b

> @graph-agent/script-executors@1.0.0 lint D:\project\packages\script-executors
> eslint . --ext .ts


D:\project\packages\script-executors\src\core\base\BaseScriptExecutor.ts
   167:12  warning  'script' is defined but never used  @typescript-eslint/no-unused-vars

D:\project\packages\script-executors\src\core\types.ts
   6:3  warning  'Script' is defined but never used  @typescript-eslint/no-unused-vars
   8:3  warning  'ScriptExecutionOptions' is defined but never used  @typescript-eslint/no-unused-vars
   9:3  warning  'ScriptExecutionResult' is defined but never used  @typescript-eslint/no-unused-vars

✖ 4 problems (0 errors, 4 warnings)
└─ @graph-agent/script-executors#lint ──"#;

    // Save raw output
    save_raw_output("turbo_scoped_packages", turbo_scoped_output);

    let parser = NpmParser::new();
    let issues = parser.parse(turbo_scoped_output).data_or_default_owned();

    // Generate report
    generate_report(
        "turbo_scoped_packages",
        "Turbo Scoped Packages",
        "turbo run lint (scoped packages)",
        &issues,
        Some("raw_output/turbo_scoped_packages.txt"),
    );

    println!("Parsed {} issues from turbo scoped packages", issues.len());

    // Should parse 4 ESLint issues
    assert_eq!(
        issues.len(),
        4,
        "Should parse exactly 4 ESLint issues from turbo scoped packages output, found {}",
        issues.len()
    );

    // Verify all issues are warnings
    for issue in &issues {
        assert!(matches!(issue.level, analyzer::core::IssueLevel::Warning));
    }

    // Verify file paths
    assert!(issues[0]
        .location
        .file_path
        .contains("BaseScriptExecutor.ts"));
    assert!(issues[1].location.file_path.contains("types.ts"));
}
