//! Test Common Module
//! Provides test utilities and shared logic

#![cfg(test)]
#![allow(dead_code)]

pub mod vs_env;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Get project root directory
pub fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Get test data root directory
pub fn data_dir() -> PathBuf {
    project_root().join("tests/data")
}

/// Get raw command output directory (generated at runtime)
pub fn raw_output_dir() -> PathBuf {
    let dir = data_dir().join("raw_output");
    fs::create_dir_all(&dir).expect("Failed to create raw_output directory");
    dir
}

/// Get samples output directory
pub fn samples_dir() -> PathBuf {
    data_dir().join("samples")
}

/// Get reports output directory
pub fn reports_dir() -> PathBuf {
    let dir = data_dir().join("reports");
    fs::create_dir_all(&dir).expect("Failed to create reports directory");
    dir
}

/// Get fixtures directory
pub fn fixtures_dir() -> PathBuf {
    data_dir().join("fixtures")
}

/// Save raw command output to file
pub fn save_raw_output(name: &str, content: &str) -> PathBuf {
    let output_path = raw_output_dir().join(format!("{}.txt", name));
    fs::write(&output_path, content).expect("Failed to write raw output file");
    println!("Raw output saved to: {}", output_path.display());
    output_path
}

/// Read sample file content
pub fn read_sample(name: &str) -> String {
    let sample_path = samples_dir().join(format!("{}.txt", name));
    fs::read_to_string(&sample_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read sample file {}: {}",
            sample_path.display(),
            e
        )
    })
}

/// Generate Markdown report
pub fn generate_report(
    name: &str,
    tool_name: &str,
    command: &str,
    issues: &[analyzer::core::Issue],
    raw_output_path: Option<&str>,
) -> PathBuf {
    use analyzer::core::{AnalysisResult, IssueLevel};

    let report_path = reports_dir().join(format!("{}_report.md", name));
    let result = AnalysisResult::from_issues(issues.to_vec());

    let mut report = String::new();

    // Title
    report.push_str(&format!("# {} Analysis Report\n\n", tool_name));

    // Command executed
    report.push_str(&format!("**Command**: `{}`\n\n", command));

    // Summary statistics
    report.push_str("## Summary\n\n");
    report.push_str(&format!("- **Total Issues**: {}\n", issues.len()));

    let error_count = result.issues_by_level.get(&IssueLevel::Error).unwrap_or(&0);
    let warning_count = result
        .issues_by_level
        .get(&IssueLevel::Warning)
        .unwrap_or(&0);
    let info_count = result.issues_by_level.get(&IssueLevel::Info).unwrap_or(&0);

    report.push_str(&format!("- **Errors**: {}\n", error_count));
    report.push_str(&format!("- **Warnings**: {}\n", warning_count));
    report.push_str(&format!("- **Info**: {}\n", info_count));

    // File statistics
    let mut files_with_issues: HashMap<&str, usize> = HashMap::new();
    for issue in issues {
        *files_with_issues
            .entry(issue.location.file_path.as_str())
            .or_insert(0) += 1;
    }

    report.push_str(&format!(
        "- **Files with Issues**: {}\n\n",
        files_with_issues.len()
    ));

    // Issues grouped by file
    if !issues.is_empty() {
        report.push_str("## Issue Details (Grouped by File)\n\n");

        let mut file_issues: HashMap<&str, Vec<&analyzer::core::Issue>> = HashMap::new();
        for issue in issues {
            file_issues
                .entry(issue.location.file_path.as_str())
                .or_default()
                .push(issue);
        }

        for (file_path, file_issues_list) in file_issues {
            report.push_str(&format!("### {}\n\n", file_path));
            report.push_str("| Line | Column | Level | Message |\n");
            report.push_str("|------|--------|-------|---------|\n");

            for issue in file_issues_list {
                let line = issue
                    .location
                    .line_number
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let column = issue
                    .location
                    .column_number
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let level = format!("{:?}", issue.level);
                let message = issue.message.replace("|", "\\|").replace("\n", " ");

                report.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    line, column, level, message
                ));
            }
            report.push('\n');
        }
    }

    // Raw output link
    if let Some(path) = raw_output_path {
        report.push_str("## Raw Output\n\n");
        report.push_str(&format!(
            "View raw command output: [{}]({})\n\n",
            path, path
        ));
    }

    fs::write(&report_path, report).expect("Failed to write report file");
    println!("Report saved to: {}", report_path.display());
    report_path
}

/// Generate test report for test frameworks (pytest, cargo test, etc.)
pub fn generate_test_report(
    name: &str,
    tool_name: &str,
    command: &str,
    test_output: &analyzer::core::ParsedTestOutput,
    raw_output_path: Option<&str>,
) -> PathBuf {
    use analyzer::core::TestStatus;

    let report_path = reports_dir().join(format!("{}_report.md", name));

    let mut report = String::new();

    // Title
    report.push_str(&format!("# {} Test Report\n\n", tool_name));

    // Command executed
    report.push_str(&format!("**Command**: `{}`\n\n", command));

    // Summary statistics
    report.push_str("## Summary\n\n");

    let total = test_output.passed_tests.len()
        + test_output.failed_tests.len()
        + test_output.ignored_tests.len();

    report.push_str(&format!("- **Total Tests**: {}\n", total));
    report.push_str(&format!(
        "- **Passed**: {}\n",
        test_output.passed_tests.len()
    ));
    report.push_str(&format!(
        "- **Failed**: {}\n",
        test_output.failed_tests.len()
    ));
    report.push_str(&format!(
        "- **Skipped/Ignored**: {}\n",
        test_output.ignored_tests.len()
    ));

    report.push('\n');

    // Failed tests details
    if !test_output.failed_tests.is_empty() {
        report.push_str("## Failed Tests\n\n");
        report.push_str("| Test Name | File | Line | Error |\n");
        report.push_str("|-----------|------|------|-------|\n");

        for test in &test_output.failed_tests {
            let file = test
                .location
                .as_ref()
                .map(|l| l.file_path.as_str())
                .unwrap_or("-");
            let line = test
                .location
                .as_ref()
                .and_then(|l| l.line_number)
                .map(|l| l.to_string())
                .unwrap_or_else(|| "-".to_string());
            let error = test
                .failure_details
                .as_ref()
                .map(|d| d.lines().next().unwrap_or(d.as_str()).to_string())
                .unwrap_or_else(|| "-".to_string());
            let error_short = if error.len() > 50 {
                format!("{}...", &error[..50])
            } else {
                error
            };

            report.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                test.name,
                file,
                line,
                error_short.replace("|", "\\|")
            ));
        }
        report.push('\n');
    }

    // Passed tests
    if !test_output.passed_tests.is_empty() {
        report.push_str("## Passed Tests\n\n");
        report.push_str("| Test Name | File |\n");
        report.push_str("|-----------|------|\n");

        for test in &test_output.passed_tests {
            let file = test
                .location
                .as_ref()
                .map(|l| l.file_path.as_str())
                .unwrap_or("-");

            report.push_str(&format!("| {} | {} |\n", test.name, file));
        }
        report.push('\n');
    }

    // Skipped/Ignored tests
    if !test_output.ignored_tests.is_empty() {
        report.push_str("## Skipped/Ignored Tests\n\n");
        report.push_str("| Test Name | File | Reason |\n");
        report.push_str("|-----------|------|--------|\n");

        for test in &test_output.ignored_tests {
            let file = test
                .location
                .as_ref()
                .map(|l| l.file_path.as_str())
                .unwrap_or("-");
            let reason = match &test.status {
                TestStatus::Ignored(Some(r)) => r.as_str(),
                TestStatus::Ignored(None) => "No reason provided",
                _ => "-",
            };

            report.push_str(&format!("| {} | {} | {} |\n", test.name, file, reason));
        }
        report.push('\n');
    }

    // Failure details
    if !test_output.failed_tests.is_empty() {
        report.push_str("## Failure Details\n\n");

        for test in &test_output.failed_tests {
            report.push_str(&format!("### {}\n\n", test.name));

            if let Some(ref location) = test.location {
                report.push_str(&format!("**File**: `{}`\n\n", location.file_path));
                if let Some(line) = location.line_number {
                    report.push_str(&format!("**Line**: {}\n\n", line));
                }
            }

            if let Some(ref details) = test.failure_details {
                report.push_str("**Error Details**:\n\n");
                report.push_str("```\n");
                report.push_str(details);
                report.push_str("\n```\n\n");
            }
        }
    }

    // Raw output link
    if let Some(path) = raw_output_path {
        report.push_str("## Raw Output\n\n");
        report.push_str(&format!(
            "View raw command output: [{}]({})\n\n",
            path, path
        ));
    }

    fs::write(&report_path, report).expect("Failed to write report file");
    println!("Test report saved to: {}", report_path.display());
    report_path
}

/// Resolve command full path (cross-platform)
/// On Windows, prioritize .cmd, .bat, .exe extensions
pub fn resolve_command(cmd: &str) -> Option<PathBuf> {
    // If it's already a path, return directly
    if cmd.contains('/') || cmd.contains('\\') {
        return Some(PathBuf::from(cmd));
    }

    // Use which/where to find the command
    #[cfg(windows)]
    let check_cmd = "where";
    #[cfg(not(windows))]
    let check_cmd = "which";

    let output = Command::new(check_cmd).arg(cmd).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let paths: Vec<PathBuf> = stdout.lines().map(PathBuf::from).collect();

    #[cfg(windows)]
    {
        // On Windows, prioritize executables with extensions
        // Priority: .cmd > .bat > .exe > others
        let priority = [".cmd", ".bat", ".exe"];
        for ext in &priority {
            if let Some(path) = paths.iter().find(|p| {
                p.extension()
                    .map(|e| e.to_string_lossy().to_lowercase() == ext.trim_start_matches('.'))
                    .unwrap_or(false)
            }) {
                return Some(path.clone());
            }
        }
    }

    // Default: return first found path
    paths.into_iter().next()
}

/// Check if command is available
pub fn is_command_available(cmd: &str) -> bool {
    resolve_command(cmd).is_some()
}

/// Run command and return output (using resolved full path)
pub fn run_command(cmd: &str, args: &[&str], cwd: &PathBuf) -> Result<String, String> {
    // Resolve command path
    let cmd_path =
        resolve_command(cmd).ok_or_else(|| format!("Command '{}' not found in PATH", cmd))?;

    println!("Executing: {} with args {:?}", cmd_path.display(), args);

    let output = Command::new(&cmd_path)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", cmd, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Merge stdout and stderr
    let full_output = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{}{}", stdout, stderr)
    };

    Ok(full_output)
}
