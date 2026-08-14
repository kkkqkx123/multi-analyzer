//! HTML Report Generator

use super::{Reporter, ReporterError, ReportOptions};
use crate::core::types::{AnalysisResult, IssueLevel, TestAnalysisResult};

/// Escape HTML-sensitive characters so issue/test content cannot break markup.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// HTML Report Generator
pub struct HtmlReporter;

impl HtmlReporter {
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
        ("Analysis Report".to_string(), "Summary".to_string())
    }
}

impl Default for HtmlReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for HtmlReporter {
    fn generate(&self, result: &AnalysisResult) -> Result<String, ReporterError> {
        self.generate_with_options(result, ReportOptions::default())
    }

    fn generate_with_options(
        &self,
        result: &AnalysisResult,
        options: ReportOptions,
    ) -> Result<String, ReporterError> {
        // Success short-circuit: if no issues found and short-circuit is enabled,
        // output a single-line confirmation instead of a full HTML report
        if options.success_short_circuit && result.total_issues == 0 {
            if let Some(msg) = options.short_circuit_message() {
                return Ok(msg);
            }
        }

        let mut html = String::new();

        // Type of test report
        let (title, summary_title) = self.detect_report_type(result, options.tech_stack.as_deref());

        html.push_str("<!DOCTYPE html>\n");
        html.push_str("<html>\n<head>\n");
        html.push_str("<meta charset=\"UTF-8\">\n");
        html.push_str(&format!("<title>{}</title>\n", title));
        html.push_str("<style>\n");
        html.push_str("body { font-family: Arial, sans-serif; margin: 40px; }\n");
        html.push_str("h1 { color: #333; }\n");
        html.push_str(".error { color: #d32f2f; }\n");
        html.push_str(".warning { color: #f57c00; }\n");
        html.push_str(".info { color: #1976d2; }\n");
        html.push_str("table { border-collapse: collapse; width: 100%; margin: 20px 0; }\n");
        html.push_str("th, td { border: 1px solid #ddd; padding: 12px; text-align: left; }\n");
        html.push_str("th { background-color: #f5f5f5; }\n");
        html.push_str("</style>\n");
        html.push_str("</head>\n<body>\n");

        html.push_str(&format!("<h1>{}</h1>\n", title));

        // summaries
        html.push_str(&format!("<h2>{}</h2>\n", summary_title));

        if result.total_issues == 0 {
            html.push_str("<p>&#x2705; No issues found.</p>\n");
        } else {
            html.push_str("<ul>\n");
            html.push_str(&format!(
                "<li><strong>Total:</strong> {}</li>\n",
                result.total_issues
            ));

            // Sort by severity
            let level_order = [
                IssueLevel::Error,
                IssueLevel::Warning,
                IssueLevel::Info,
                IssueLevel::Hint,
            ];
            for level in &level_order {
                if let Some(count) = result.issues_by_level.get(level) {
                    let (class, icon) = match level {
                        IssueLevel::Error => ("error", "&#x274C;"),
                        IssueLevel::Warning => ("warning", "&#x26A0;"),
                        IssueLevel::Info => ("info", "&#x2139;"),
                        IssueLevel::Hint => ("info", "&#x1F4A1;"),
                    };
                    html.push_str(&format!(
                        "<li class=\"{}\"><strong>{} {}:</strong> {}</li>\n",
                        class, icon, level, count
                    ));
                }
            }
            html.push_str(&format!(
                "<li><strong>Categories:</strong> {}</li>\n",
                result.unique_patterns.len()
            ));
            html.push_str(&format!(
                "<li><strong>Files Affected:</strong> {}</li>\n",
                result.issues_by_file.len()
            ));
            html.push_str("</ul>\n");

            // Detailed tables
            html.push_str("<h2>Details</h2>\n");
            html.push_str("<table>\n");
            html.push_str(
                "<tr><th>Severity</th><th>File</th><th>Position</th><th>Description</th></tr>\n",
            );

            for issues in result.issues_by_file.values() {
                for issue in issues {
                    let level_class = match issue.level {
                        IssueLevel::Error => "error",
                        IssueLevel::Warning => "warning",
                        _ => "info",
                    };

                    let location = match (issue.location.line_number, issue.location.column_number)
                    {
                        (Some(line), Some(col)) => format!("line {}, col {}", line, col),
                        (Some(line), None) => format!("line {}", line),
                        _ => "-".to_string(),
                    };

                    let code_display = issue
                        .code
                        .as_ref()
                        .map(|c| format!(" [{}]", c))
                        .unwrap_or_default();

                    html.push_str(&format!(
                        "<tr><td class=\"{}\">{}{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                        level_class,
                        issue.level,
                        code_display,
                        html_escape(&issue.location.file_path),
                        location,
                        html_escape(&issue.message)
                    ));
                }
            }

            html.push_str("</table>\n");
        }

        html.push_str("</body>\n</html>");

        Ok(html)
    }

    /// Generate an HTML test report that carries the full test statistics and
    /// per-case results in addition to any compile issues. Unlike the compile
    /// report, a run with failing tests but zero compile issues must never
    /// short-circuit to "no issues found".
    fn generate_test_report_with_options(
        &self,
        result: &TestAnalysisResult,
        options: ReportOptions,
    ) -> Result<String, ReporterError> {
        let mut html = String::new();

        let title = if result.all_passed() {
            "Test Report - All Passed"
        } else {
            "Test Report - Issues Found"
        };

        html.push_str("<!DOCTYPE html>\n");
        html.push_str("<html>\n<head>\n");
        html.push_str("<meta charset=\"UTF-8\">\n");
        html.push_str(&format!("<title>{}</title>\n", html_escape(title)));
        html.push_str("<style>\n");
        html.push_str("body { font-family: Arial, sans-serif; margin: 40px; }\n");
        html.push_str("h1 { color: #333; }\n");
        html.push_str("h2 { color: #444; margin-top: 30px; }\n");
        html.push_str(".error { color: #d32f2f; }\n");
        html.push_str(".warning { color: #f57c00; }\n");
        html.push_str(".info { color: #1976d2; }\n");
        html.push_str("table { border-collapse: collapse; width: 100%; margin: 20px 0; }\n");
        html.push_str("th, td { border: 1px solid #ddd; padding: 12px; text-align: left; }\n");
        html.push_str("th { background-color: #f5f5f5; }\n");
        html.push_str("pre { background: #f8f8f8; padding: 10px; overflow-x: auto; }\n");
        html.push_str("</style>\n");
        html.push_str("</head>\n<body>\n");

        html.push_str(&format!("<h1>{}</h1>\n", html_escape(title)));

        // Test summary
        if let Some(ref summary) = result.test_summary {
            html.push_str("<h2>Summary</h2>\n");
            html.push_str("<ul>\n");
            html.push_str(&format!(
                "<li><strong>Total:</strong> {} (calculated: {})</li>\n",
                summary.total,
                result.collected_tests()
            ));
            html.push_str(&format!(
                "<li><strong>Passed:</strong> &#x2705; {}</li>\n",
                summary.passed
            ));
            html.push_str(&format!(
                "<li><strong>Failed:</strong> &#x274C; {}</li>\n",
                summary.failed
            ));
            html.push_str(&format!(
                "<li><strong>Ignored:</strong> {}</li>\n",
                summary.ignored
            ));
            if summary.execution_time().is_some() {
                html.push_str(&format!(
                    "<li><strong>Duration:</strong> {}</li>\n",
                    summary.execution_time_formatted()
                ));
            }
            html.push_str("</ul>\n");
        }

        // Failed tests
        if !result.failed_tests.is_empty() {
            html.push_str(&format!(
                "<h2>Failed Tests ({} item(s))</h2>\n",
                result.failed_tests.len()
            ));
            html.push_str("<table>\n");
            html.push_str("<tr><th>Test</th><th>Details</th></tr>\n");
            for test in &result.failed_tests {
                let details = test.failure_details.as_deref().unwrap_or("");
                html.push_str(&format!(
                    "<tr class=\"error\"><td>{}</td><td><pre>{}</pre></td></tr>\n",
                    html_escape(&test.name),
                    html_escape(details)
                ));
            }
            html.push_str("</table>\n");
        }

        // Ignored tests
        if !result.ignored_tests.is_empty() {
            html.push_str(&format!(
                "<h2>Ignored Tests ({} item(s))</h2>\n",
                result.ignored_tests.len()
            ));
            html.push_str("<ul>\n");
            for test in &result.ignored_tests {
                let reason = match &test.status {
                    crate::core::types::TestStatus::Ignored(Some(r)) => format!(" - {}", r),
                    _ => String::new(),
                };
                html.push_str(&format!(
                    "<li>&#x1F515; {}{}</li>\n",
                    html_escape(&test.name),
                    html_escape(&reason)
                ));
            }
            html.push_str("</ul>\n");
        }

        // Passed tests
        if !result.passed_tests.is_empty() {
            html.push_str(&format!(
                "<h2>Passed Tests ({} item(s))</h2>\n",
                result.passed_tests.len()
            ));
            html.push_str("<ul>\n");
            if result.passed_tests.len() <= 10 {
                for test in &result.passed_tests {
                    html.push_str(&format!(
                        "<li>&#x2705; {}</li>\n",
                        html_escape(&test.name)
                    ));
                }
            } else {
                html.push_str(&format!(
                    "<li>&#x2705; {} tests passed</li>\n",
                    result.passed_tests.len()
                ));
            }
            html.push_str("</ul>\n");
        }

        // Compile issues (always rendered without short-circuit)
        if result.compile_result.total_issues > 0 {
            html.push_str("<h2>Compile Issues</h2>\n");
            let compile_html = self.generate_with_options(
                &result.compile_result,
                ReportOptions {
                    success_short_circuit: false,
                    ..options.clone()
                },
            )?;
            // Strip the outer <html>/<head>/<body> wrapper; keep the inner content.
            let inner = compile_html
                .split_once("<body>\n")
                .map(|(_, rest)| rest.trim_end_matches("</body>\n</html>").trim_end())
                .unwrap_or(compile_html.as_str());
            html.push_str(inner);
            html.push('\n');
        }

        html.push_str("</body>\n</html>");

        Ok(html)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Issue, IssueLevel, Location};
    use crate::core::reporter::ReportOptions;

    fn sample_result() -> AnalysisResult {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Error,
            "undefined reference to `foo`",
            Location::new("src/main.rs").with_line(10).with_column(5),
        ));
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "unused variable",
            Location::new("src/lib.rs").with_line(20),
        ));
        r
    }

    #[test]
    fn test_html_generate_empty() {
        let reporter = HtmlReporter::new();
        let result = AnalysisResult::new();
        let report = reporter.generate(&result).unwrap();
        assert!(report.contains("<!DOCTYPE html>"));
        assert!(report.contains("No issues found"));
    }

    #[test]
    fn test_html_generate_with_issues() {
        let reporter = HtmlReporter::new();
        let report = reporter.generate(&sample_result()).unwrap();
        assert!(report.contains("<!DOCTYPE html>"));
        assert!(report.contains("undefined reference"));
        assert!(report.contains("unused variable"));
        assert!(report.contains("src/main.rs"));
        assert!(report.contains("src/lib.rs"));
        assert!(report.contains("Analysis Report"));
    }

    #[test]
    fn test_html_short_circuit() {
        let reporter = HtmlReporter::new();
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
