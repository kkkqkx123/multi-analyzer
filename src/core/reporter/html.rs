//! HTML Report Generator

use super::{Reporter, ReporterError, ReportOptions};
use crate::core::types::{AnalysisResult, IssueLevel};

/// HTML Report Generator
pub struct HtmlReporter;

impl HtmlReporter {
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
        let (title, summary_title) = self.detect_report_type(result);

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
                        issue.location.file_path,
                        location,
                        issue.message
                    ));
                }
            }

            html.push_str("</table>\n");
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
