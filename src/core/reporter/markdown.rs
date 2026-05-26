//! Markdown Report Generator

use super::{ReportOptions, Reporter, ReporterError};
use crate::core::types::{AnalysisResult, Issue, IssueLevel, TestAnalysisResult, TestStatus};
use std::collections::HashMap;

/// Markdown Report Generator
pub struct MarkdownReporter;

impl MarkdownReporter {
    pub fn new() -> Self {
        Self
    }

    /// Detects the report type and returns the appropriate title
    fn detect_report_type(&self, result: &AnalysisResult) -> (String, String) {
        // Collect all issue messages for type determination
        let all_messages: Vec<String> = result
            .issues_by_file
            .values()
            .flatten()
            .map(|i| i.message.to_lowercase())
            .collect();

        // Determining whether a security audit report
        let is_security_audit = all_messages.iter().any(|m| {
            m.contains("security vulnerability")
                || m.contains("severity: high")
                || m.contains("severity: critical")
                || m.contains("npm audit")
        });

        if is_security_audit {
            return (
                "Security Audit Report".to_string(),
                "Vulnerability Summary".to_string(),
            );
        }

        // Determining whether a type check report
        let is_type_check = all_messages.iter().any(|m| {
            m.contains("type")
                || m.contains("typescript")
                || m.contains("type mismatch")
                || m.contains("expected")
                || m.contains("mypy")
        });

        if is_type_check {
            return (
                "Type Check Report".to_string(),
                "Type Issues Summary".to_string(),
            );
        }

        // Determining if a Lint Report
        let is_lint = all_messages.iter().any(|m| {
            m.contains("eslint") || m.contains("clippy") || m.contains("lint") || m.contains("style")
        });

        if is_lint {
            return ("Lint Report".to_string(), "Lint Issues Summary".to_string());
        }

        // Defaults to a generic analysis report
        (
            "Analysis Report".to_string(),
            "Issues Summary".to_string(),
        )
    }
}

impl Default for MarkdownReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownReporter {
    /// Internal method to generate report with optional truncation
    fn generate_internal(
        &self,
        result: &AnalysisResult,
        options: ReportOptions,
    ) -> Result<String, ReporterError> {
        // Success short-circuit: if no issues found and short-circuit is enabled,
        // output a single-line confirmation instead of full markdown report
        if options.success_short_circuit && result.total_issues == 0 {
            if let Some(msg) = options.short_circuit_message() {
                return Ok(msg);
            }
        }

        let mut report = String::new();

        // Detect the report type and set the appropriate title
        let (title, summary_title) = self.detect_report_type(result);
        report.push_str(&format!("# {}\n\n", title));

        // summaries
        report.push_str(&format!("## {}\n\n", summary_title));

        if result.total_issues == 0 {
            report.push_str("✅ No issues found.\n\n");
            return Ok(report);
        }

        report.push_str(&format!("- **Total**: {}\n", result.total_issues));

        // Statistics by level, sorted by severity
        let level_order = [IssueLevel::Error, IssueLevel::Warning, IssueLevel::Info, IssueLevel::Hint];
        for level in &level_order {
            if let Some(count) = result.issues_by_level.get(level) {
                let icon = match level {
                    IssueLevel::Error => "❌",
                    IssueLevel::Warning => "⚠️",
                    IssueLevel::Info => "ℹ️",
                    IssueLevel::Hint => "💡",
                };
                report.push_str(&format!("- **{}** {}: {}\n", icon, level, count));
            }
        }

        report.push_str(&format!("- **Categories**: {}\n", result.unique_patterns.len()));
        report.push_str(&format!("- **Files Affected**: {}", result.issues_by_file.len()));

        // Add top error codes breakdown
        let top_codes = result.top_error_codes(5);
        if !top_codes.is_empty() {
            report.push_str("\n\n### Top Error Codes\n\n");
            for (code, count) in &top_codes {
                report.push_str(&format!("- `{}`: {} occurrence(s)\n", code, count));
            }
        }
        
        // Add package count if we have package information
        let has_package_info = result.issues_by_package.keys().any(|k| k != "unknown");
        if has_package_info {
            let package_count = result.issues_by_package.keys().filter(|k| *k != "unknown").count();
            report.push_str(&format!("\n- **Packages Affected**: {}", package_count));
        }
        report.push_str("\n\n");

        // In minimal mode, skip details sections
        if options.verbose.is_minimal() && result.total_issues > 0 {
            // Still show top files in minimal mode
            report.push_str("**Top Files:**\n\n");
            let mut files: Vec<_> = result.issues_by_file.iter().collect();
            files.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
            for (file_path, issues) in files.iter().take(5) {
                report.push_str(&format!("- `{}`: {} issue(s)\n", file_path, issues.len()));
            }
            report.push('\n');
            return Ok(report);
        }

        // Statistics by type
        if !result.issues_by_type.is_empty() {
            report.push_str("## Breakdown by Category\n\n");
            let mut types: Vec<_> = result.issues_by_type.iter().collect();
            types.sort_by(|a, b| b.1.cmp(a.1));

            // In verbose mode, show all types; otherwise limit to 20
            let type_limit = if options.verbose.is_verbose() { types.len() } else { 20 };
            for (issue_type, count) in types.iter().take(type_limit) {
                report.push_str(&format!("- **{}**: {} occurrence(s)\n", issue_type, count));
            }
            report.push('\n');
        }

        // NEW: Statistics by package (if we have package information)
        if has_package_info {
            report.push_str("## Details by Package\n\n");
            
            let mut packages: Vec<_> = result.issues_by_package.iter().collect();
            packages.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
            
            let package_limit = if options.verbose.is_verbose() { packages.len() } else { 10 };
            
            for (package_name, issues) in packages.iter().take(package_limit) {
                if *package_name == "unknown" {
                    continue;
                }
                
                report.push_str(&format!("### Package: `{}` ({} issue(s))\n\n", 
                    package_name, issues.len()));
                
                // Group issues by file within this package
                let mut files: HashMap<String, Vec<&Issue>> = HashMap::new();
                for issue in issues.iter() {
                    files.entry(issue.location.file_path.clone())
                        .or_default()
                        .push(issue);
                }
                
                let mut file_list: Vec<_> = files.iter().collect();
                file_list.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
                
                let file_limit = if options.verbose.is_verbose() { file_list.len() } else { 5 };
                
                for (file_path, file_issues) in file_list.iter().take(file_limit) {
                    report.push_str(&format!("#### `{}` ({} item(s))\n\n", file_path, file_issues.len()));
                    
                    let issue_limit = if options.verbose.is_verbose() { file_issues.len() } else { 5 };
                    for issue in file_issues.iter().take(issue_limit) {
                        let location = match (issue.location.line_number, issue.location.column_number) {
                            (Some(line), Some(col)) => format!("{}:{}", line, col),
                            (Some(line), None) => format!("{}", line),
                            _ => "-".to_string(),
                        };

                        let code = issue.code.as_ref().map(|c| format!(" `[{}]`", c)).unwrap_or_default();
                        let level_icon = match issue.level {
                            IssueLevel::Error => "❌",
                            IssueLevel::Warning => "⚠️",
                            IssueLevel::Info => "ℹ️",
                            IssueLevel::Hint => "💡",
                        };

                        report.push_str(&format!(
                            "- {} **{}**{} at line {}: {}\n",
                            level_icon, issue.level, code, location, issue.message
                        ));
                    }
                    
                    if !options.verbose.is_verbose() && file_issues.len() > 5 {
                        report.push_str(&format!("- ... and {} more\n", file_issues.len() - 5));
                    }
                    report.push('\n');
                }
                
                if !options.verbose.is_verbose() && file_list.len() > 5 {
                    report.push_str(&format!(
                        "*... and {} more files in this package*\n\n",
                        file_list.len() - 5
                    ));
                }
            }
            
            if !options.verbose.is_verbose() && packages.len() > 10 {
                report.push_str(&format!(
                    "*... and {} more packages (use --verbose to see all)*\n\n",
                    packages.len() - 10
                ));
            }
        }

        // Statistics by document (fallback if no package info or for detailed view)
        if !result.issues_by_file.is_empty() && !has_package_info {
            report.push_str("## Details by File\n\n");
            let mut files: Vec<_> = result.issues_by_file.iter().collect();
            files.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

            // In verbose mode, show all files; otherwise limit to 20
            let file_limit = if options.verbose.is_verbose() { files.len() } else { 20 };
            for (file_path, issues) in files.iter().take(file_limit) {
                report.push_str(&format!("### `{}` ({} item(s))\n\n", file_path, issues.len()));

                // In verbose mode, show all issues; otherwise limit to 10
                let issue_limit = if options.verbose.is_verbose() { issues.len() } else { 10 };
                for issue in issues.iter().take(issue_limit) {
                    let location = match (issue.location.line_number, issue.location.column_number) {
                        (Some(line), Some(col)) => format!("{}:{}", line, col),
                        (Some(line), None) => format!("{}", line),
                        _ => "-".to_string(),
                    };

                    let code = issue.code.as_ref().map(|c| format!(" `[{}]`", c)).unwrap_or_default();
                    let level_icon = match issue.level {
                        IssueLevel::Error => "❌",
                        IssueLevel::Warning => "⚠️",
                        IssueLevel::Info => "ℹ️",
                        IssueLevel::Hint => "💡",
                    };

                    report.push_str(&format!(
                        "- {} **{}**{} at line {}: {}\n",
                        level_icon, issue.level, code, location, issue.message
                    ));
                }

                if !options.verbose.is_verbose() && issues.len() > 10 {
                    report.push_str(&format!("- ... and {} more\n", issues.len() - 10));
                }

                report.push('\n');
            }

            // Show message if files were truncated
            if !options.verbose.is_verbose() && files.len() > 20 {
                report.push_str(&format!(
                    "*... and {} more files (use --verbose to see all)*\n\n",
                    files.len() - 20
                ));
            }
        }

        Ok(report)
    }
}

impl Reporter for MarkdownReporter {
    fn generate(&self, result: &AnalysisResult) -> Result<String, ReporterError> {
        self.generate_internal(result, ReportOptions::default())
    }

    fn generate_with_options(
        &self,
        result: &AnalysisResult,
        options: ReportOptions,
    ) -> Result<String, ReporterError> {
        self.generate_internal(result, options)
    }

    fn generate_test_report(&self, result: &TestAnalysisResult) -> Result<String, ReporterError> {
        let mut report = String::new();

        // Selection of titles based on test results
        let all_passed = result.all_passed();
        let total_tests = result.total_tests();
        if all_passed {
            report.push_str("# ✅ Test Report - All Passed\n\n");
        } else {
            report.push_str("# ❌ Test Report - Issues Found\n\n");
        }

        // Test Summary
        if let Some(ref summary) = result.test_summary {
            report.push_str("## Summary\n\n");

            // Calculating the pass rate
            let pass_rate = if summary.total > 0 {
                (summary.passed as f64 / summary.total as f64) * 100.0
            } else {
                0.0
            };

            report.push_str(&format!("- **Total**: {} test(s) (calculated: {})\n", summary.total, total_tests));
            report.push_str(&format!("- **Passed**: ✅ {} ({:.1}%)\n", summary.passed, pass_rate));
            if summary.failed > 0 {
                report.push_str(&format!("- **Failed**: ❌ {}\n", summary.failed));
            }
            if summary.ignored > 0 {
                report.push_str(&format!("- **Ignored**: 🔕 {}\n", summary.ignored));
            }
            if summary.measured > 0 {
                report.push_str(&format!("- **Measured**: {}\n", summary.measured));
            }
            if summary.filtered > 0 {
                report.push_str(&format!("- **Filtered out**: {}\n", summary.filtered));
            }
            report.push('\n');
        }

        // Failed Test Details
        if !result.failed_tests.is_empty() {
            report.push_str(&format!("## Failed Tests ({} item(s))\n\n", result.failed_tests.len()));
            for (idx, test) in result.failed_tests.iter().enumerate() {
                report.push_str(&format!("### {}. `{}`\n\n", idx + 1, test.name));

                if let Some(ref location) = test.location {
                    report.push_str(&format!(
                        "📍 **Location**: `{}:{}`\n\n",
                        location.file_path,
                        location
                            .line_number
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    ));
                }

                if let Some(ref details) = test.failure_details {
                    report.push_str("🔍 **Failure Details**:\n");
                    report.push_str("```\n");
                    report.push_str(details);
                    report.push_str("\n```\n\n");
                }
            }
        }

        // Neglected Tests
        if !result.ignored_tests.is_empty() {
            report.push_str(&format!("## Ignored Tests ({} item(s))\n\n", result.ignored_tests.len()));
            for test in &result.ignored_tests {
                let reason = match &test.status {
                    TestStatus::Ignored(Some(r)) => format!(" - *Reason: {}*", r),
                    _ => String::new(),
                };
                report.push_str(&format!("- `{}`{}\n", test.name, reason));
            }
            report.push('\n');
        }

        // Passed Tests (summary only if there are many)
        if !result.passed_tests.is_empty() {
            report.push_str(&format!("## Passed Tests ({} item(s))\n\n", result.passed_tests.len()));
            if result.passed_tests.len() <= 10 {
                // List all passed tests if there are few
                for test in &result.passed_tests {
                    report.push_str(&format!("- ✅ `{}`\n", test.name));
                }
            } else {
                // Just show count if there are many
                report.push_str(&format!("✅ {} tests passed\n", result.passed_tests.len()));
            }
            report.push('\n');
        }

        // Test output availability indicator
        if result.has_test_output {
            report.push_str("---\n*Test output was successfully captured*\n");
        }

        // Compilation issues (if any)
        if result.compile_result.total_issues > 0 {
            report.push_str("## Build Issues\n\n");
            report.push_str("The following issues were found during compilation:\n\n");
            report.push_str(&self.generate(&result.compile_result)?);
        }

        Ok(report)
    }
}
