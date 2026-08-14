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
    fn detect_report_type(
        &self,
        result: &AnalysisResult,
        tech_stack: Option<&str>,
    ) -> (String, String) {
        // Tech-stack driven titles take priority over message heuristics
        if let Some(ts) = tech_stack {
            let lower = ts.to_lowercase();
            if lower.contains("rubocop") || lower.contains("ruby") || lower.contains("rails") {
                return (
                    "RuboCop Report".to_string(),
                    "RuboCop Issues Summary".to_string(),
                );
            }
            if lower.contains("rspec") {
                return ("RSpec Report".to_string(), "Test Issues Summary".to_string());
            }
        }

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
            m.contains("eslint")
                || m.contains("clippy")
                || m.contains("lint")
                || m.contains("style")
        });

        if is_lint {
            return ("Lint Report".to_string(), "Lint Issues Summary".to_string());
        }

        // Defaults to a generic analysis report
        ("Analysis Report".to_string(), "Issues Summary".to_string())
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
        let (title, summary_title) = self.detect_report_type(result, options.tech_stack.as_deref());
        report.push_str(&format!("# {}\n\n", title));

        // summaries
        report.push_str(&format!("## {}\n\n", summary_title));

        if result.total_issues == 0 {
            report.push_str("✅ No issues found.\n\n");
            return Ok(report);
        }

        report.push_str(&format!("- **Total**: {}\n", result.total_issues));

        // Statistics by level, sorted by severity
        let level_order = [
            IssueLevel::Error,
            IssueLevel::Warning,
            IssueLevel::Info,
            IssueLevel::Hint,
        ];
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

        report.push_str(&format!(
            "- **Categories**: {}\n",
            result.unique_patterns.len()
        ));
        report.push_str(&format!(
            "- **Files Affected**: {}",
            result.issues_by_file.len()
        ));

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
            let package_count = result
                .issues_by_package
                .keys()
                .filter(|k| *k != "unknown")
                .count();
            report.push_str(&format!("\n- **Packages Affected**: {}", package_count));
        }
        report.push_str("\n\n");

        // In minimal mode, skip details sections
        if options.verbose.is_minimal() && result.total_issues > 0 {
            // Still show top files in minimal mode
            report.push_str("**Top Files:**\n\n");
            let mut files: Vec<_> = result.issues_by_file.iter().collect();
            files.sort_by_key(|b| std::cmp::Reverse(b.1.len()));
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
            let type_limit = if options.verbose.is_verbose() {
                types.len()
            } else {
                20
            };
            for (issue_type, count) in types.iter().take(type_limit) {
                report.push_str(&format!("- **{}**: {} occurrence(s)\n", issue_type, count));
            }
            report.push('\n');
        }

        // NEW: Statistics by package (if we have package information)
        if has_package_info {
            report.push_str("## Details by Package\n\n");

            let mut packages: Vec<_> = result.issues_by_package.iter().collect();
            packages.sort_by_key(|b| std::cmp::Reverse(b.1.len()));

            let package_limit = if options.verbose.is_verbose() {
                packages.len()
            } else {
                10
            };

            for (package_name, issues) in packages.iter().take(package_limit) {
                if *package_name == "unknown" {
                    continue;
                }

                report.push_str(&format!(
                    "### Package: `{}` ({} issue(s))\n\n",
                    package_name,
                    issues.len()
                ));

                // Group issues by file within this package
                let mut files: HashMap<String, Vec<&Issue>> = HashMap::new();
                for issue in issues.iter() {
                    files
                        .entry(issue.location.file_path.clone())
                        .or_default()
                        .push(issue);
                }

                let mut file_list: Vec<_> = files.iter().collect();
                file_list.sort_by_key(|b| std::cmp::Reverse(b.1.len()));

                let file_limit = if options.verbose.is_verbose() {
                    file_list.len()
                } else {
                    5
                };

                for (file_path, file_issues) in file_list.iter().take(file_limit) {
                    report.push_str(&format!(
                        "#### `{}` ({} item(s))\n\n",
                        file_path,
                        file_issues.len()
                    ));

                    let issue_limit = if options.verbose.is_verbose() {
                        file_issues.len()
                    } else {
                        5
                    };
                    for issue in file_issues.iter().take(issue_limit) {
                        let location =
                            match (issue.location.line_number, issue.location.column_number) {
                                (Some(line), Some(col)) => format!("{}:{}", line, col),
                                (Some(line), None) => format!("{}", line),
                                _ => "-".to_string(),
                            };

                        let code = issue
                            .code
                            .as_ref()
                            .map(|c| format!(" `[{}]`", c))
                            .unwrap_or_default();
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
            files.sort_by_key(|b| std::cmp::Reverse(b.1.len()));

            // In verbose mode, show all files; otherwise limit to 20
            let file_limit = if options.verbose.is_verbose() {
                files.len()
            } else {
                20
            };
            for (file_path, issues) in files.iter().take(file_limit) {
                report.push_str(&format!(
                    "### `{}` ({} item(s))\n\n",
                    file_path,
                    issues.len()
                ));

                // In verbose mode, show all issues; otherwise limit to 10
                let issue_limit = if options.verbose.is_verbose() {
                    issues.len()
                } else {
                    10
                };
                for issue in issues.iter().take(issue_limit) {
                    let location = match (issue.location.line_number, issue.location.column_number)
                    {
                        (Some(line), Some(col)) => format!("{}:{}", line, col),
                        (Some(line), None) => format!("{}", line),
                        _ => "-".to_string(),
                    };

                    let code = issue
                        .code
                        .as_ref()
                        .map(|c| format!(" `[{}]`", c))
                        .unwrap_or_default();
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
        self.generate_test_report_internal(result)
    }

    fn generate_test_report_with_options(
        &self,
        result: &TestAnalysisResult,
        _options: ReportOptions,
    ) -> Result<String, ReporterError> {
        self.generate_test_report_internal(result)
    }
}

impl MarkdownReporter {
    fn generate_test_report_internal(&self, result: &TestAnalysisResult) -> Result<String, ReporterError> {
        let mut report = String::new();

        // Selection of titles based on test results
        let all_passed = result.all_passed();
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

            // "calculated" reports how many per-case detail entries the parser
            // collected; the runner-declared total stays authoritative and may
            // be higher when details are aggregated at class granularity.
            let collected = result.collected_tests();
            report.push_str(&format!(
                "- **Total**: {} test(s) (calculated: {})\n",
                summary.total, collected
            ));
            report.push_str(&format!(
                "- **Passed**: ✅ {} ({:.1}%)\n",
                summary.passed, pass_rate
            ));
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
            if summary.execution_time().is_some() {
                report.push_str(&format!("- **Duration**: {}\n", summary.execution_time_formatted()));
            }
            report.push('\n');
        }

        // Failed Test Details
        if !result.failed_tests.is_empty() {
            report.push_str(&format!(
                "## Failed Tests ({} item(s))\n\n",
                result.failed_tests.len()
            ));
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
            report.push_str(&format!(
                "## Ignored Tests ({} item(s))\n\n",
                result.ignored_tests.len()
            ));
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
            report.push_str(&format!(
                "## Passed Tests ({} item(s))\n\n",
                result.passed_tests.len()
            ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Issue, IssueLevel, Location, TestSummary, TestStatus, TestCase};
    use crate::core::Verbosity;

    fn single_error_result() -> AnalysisResult {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Error,
            "undefined reference to `foo`",
            Location::new("src/main.rs").with_line(10).with_column(5),
        ));
        r
    }

    fn multi_issue_result() -> AnalysisResult {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(IssueLevel::Error, "type mismatch", Location::new("src/main.rs").with_line(10)).with_code("E0308"));
        r.add_issue(Issue::new(IssueLevel::Warning, "unused var", Location::new("src/lib.rs").with_line(20)).with_code("W0001"));
        r.add_issue(Issue::new(IssueLevel::Info, "consider refactoring", Location::new("src/lib.rs").with_line(25)));
        r
    }

    fn security_audit_result() -> AnalysisResult {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Error,
            "npm audit security vulnerability",
            Location::new("package.json"),
        ));
        r
    }

    fn type_check_result() -> AnalysisResult {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Error,
            "type mismatch: expected String",
            Location::new("src/main.ts"),
        ));
        r
    }

    fn lint_result() -> AnalysisResult {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "clippy::style issue",
            Location::new("src/main.rs"),
        ));
        r
    }

    // ── detect_report_type ──────────────────────────────────────────

    #[test]
    fn test_detect_report_type_default() {
        let reporter = MarkdownReporter::new();
        let (title, _) = reporter.detect_report_type(&single_error_result(), None);
        assert_eq!(title, "Analysis Report");
    }

    #[test]
    fn test_detect_report_type_security() {
        let reporter = MarkdownReporter::new();
        let (title, _) = reporter.detect_report_type(&security_audit_result(), None);
        assert_eq!(title, "Security Audit Report");
    }

    #[test]
    fn test_detect_report_type_check() {
        let reporter = MarkdownReporter::new();
        let (title, _) = reporter.detect_report_type(&type_check_result(), None);
        assert_eq!(title, "Type Check Report");
    }

    #[test]
    fn test_detect_report_type_lint() {
        let reporter = MarkdownReporter::new();
        let (title, _) = reporter.detect_report_type(&lint_result(), None);
        assert_eq!(title, "Lint Report");
    }

    // ── Empty result ────────────────────────────────────────────────

    #[test]
    fn test_generate_empty_result() {
        let reporter = MarkdownReporter::new();
        let result = AnalysisResult::new();
        let report = reporter.generate(&result).unwrap();
        assert!(report.contains("No issues found"));
        assert!(report.contains("Analysis Report"));
    }

    // ── Basic report ────────────────────────────────────────────────

    #[test]
    fn test_generate_single_error() {
        let reporter = MarkdownReporter::new();
        let report = reporter.generate(&single_error_result()).unwrap();
        assert!(report.contains("undefined reference"));
        assert!(report.contains("src/main.rs"));
        assert!(report.contains("Total"));
        assert!(report.contains("error"));
    }

    #[test]
    fn test_generate_multi_issue() {
        let reporter = MarkdownReporter::new();
        let report = reporter.generate(&multi_issue_result()).unwrap();
        assert!(report.contains("type mismatch"));
        assert!(report.contains("unused var"));
        assert!(report.contains("consider refactoring"));
        assert!(report.contains("E0308"));
        assert!(report.contains("W0001"));
    }

    #[test]
    fn test_generate_with_top_error_codes() {
        let reporter = MarkdownReporter::new();
        let report = reporter.generate(&multi_issue_result()).unwrap();
        assert!(report.contains("Top Error Codes"));
        assert!(report.contains("E0308"));
        assert!(report.contains("W0001"));
    }

    // ── Package grouping ────────────────────────────────────────────

    #[test]
    fn test_generate_with_package_info() {
        let reporter = MarkdownReporter::new();
        let mut r = AnalysisResult::new();
        r.add_issue(
            Issue::new(IssueLevel::Error, "err in pkg-a", Location::new("a/src/main.rs"))
                .with_package("pkg-a"),
        );
        r.add_issue(
            Issue::new(IssueLevel::Error, "err in pkg-b", Location::new("b/src/main.rs"))
                .with_package("pkg-b"),
        );
        let report = reporter.generate(&r).unwrap();
        assert!(report.contains("Details by Package"));
        assert!(report.contains("pkg-a"));
        assert!(report.contains("pkg-b"));
    }

    // ── Verbose / Minimal modes ─────────────────────────────────────

    #[test]
    fn test_generate_minimal_mode() {
        let reporter = MarkdownReporter::new();
        let opts = ReportOptions {
            verbose: Verbosity::Minimal,
            ..Default::default()
        };
        let report = reporter.generate_with_options(&multi_issue_result(), opts).unwrap();
        // Minimal mode should still show summary and top files
        assert!(report.contains("Total"));
        assert!(report.contains("Top Files"));
        // But should not show detailed breakdown by category
        assert!(!report.contains("Breakdown by Category"));
    }

    #[test]
    fn test_generate_verbose_mode() {
        let reporter = MarkdownReporter::new();
        let opts = ReportOptions {
            verbose: Verbosity::Verbose,
            ..Default::default()
        };
        let report = reporter.generate_with_options(&multi_issue_result(), opts).unwrap();
        assert!(report.contains("Breakdown by Category"));
        assert!(report.contains("Details by File"));
    }

    // ── Test report ─────────────────────────────────────────────────

    #[test]
    fn test_generate_test_report_all_passed() {
        let reporter = MarkdownReporter::new();
        let result = TestAnalysisResult::from_compile_result(AnalysisResult::new());
        let report = reporter.generate_test_report(&result).unwrap();
        assert!(report.contains("All Passed"));
    }

    #[test]
    fn test_generate_test_report_with_failures() {
        let reporter = MarkdownReporter::new();
        let mut result = TestAnalysisResult::from_compile_result(AnalysisResult::new());
        result.test_summary = Some(TestSummary {
            total: 5,
            passed: 3,
            failed: 2,
            ignored: 0,
            measured: 0,
            filtered: 0,
            execution_time: None,
        });
        result.failed_tests = vec![
            TestCase::new("test_fail", TestStatus::Failed)
                .with_failure_details("assertion failed: 1 != 2"),
        ];
        let report = reporter.generate_test_report(&result).unwrap();
        assert!(report.contains("Issues Found"));
        assert!(report.contains("test_fail"));
        assert!(report.contains("assertion failed"));
    }

    #[test]
    fn test_generate_test_report_with_ignored() {
        let reporter = MarkdownReporter::new();
        let mut result = TestAnalysisResult::from_compile_result(AnalysisResult::new());
        result.ignored_tests = vec![
            TestCase::new("test_skip", TestStatus::Ignored(Some("not ready".to_string()))),
        ];
        result.passed_tests = vec![
            TestCase::new("test_pass", TestStatus::Passed),
        ];
        let report = reporter.generate_test_report(&result).unwrap();
        assert!(report.contains("Ignored Tests"));
        assert!(report.contains("test_skip"));
        assert!(report.contains("not ready"));
        assert!(report.contains("Passed Tests"));
    }

    // ── Short-circuit ───────────────────────────────────────────────

    #[test]
    fn test_generate_with_options_short_circuit_markdown() {
        let reporter = MarkdownReporter::new();
        let empty = AnalysisResult::new();
        let opts = ReportOptions {
            success_short_circuit: true,
            tech_stack: Some("cargo check".to_string()),
            ..Default::default()
        };
        let report = reporter.generate_with_options(&empty, opts).unwrap();
        assert_eq!(report, "cargo check: no issues found");
    }
}
