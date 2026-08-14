//! Raw format reporter for machine-readable output.
//!
//! Supports two output modes:
//! - `raw`: pipe-delimited format (LEVEL|CODE|FILE:LINE:COL|MESSAGE)
//! - `raw-json`: JSON lines format (one JSON object per line)

use super::{Reporter, ReporterError};
use crate::core::types::{AnalysisResult, TestAnalysisResult};

/// Raw format reporter.
///
/// Generates pipe-delimited output by default, or JSON lines when `json_lines` is true.
pub struct RawReporter {
    json_lines: bool,
}

impl RawReporter {
    /// Create a new raw reporter (pipe-delimited mode).
    pub fn new() -> Self {
        Self { json_lines: false }
    }

    /// Create a new raw reporter in JSON lines mode.
    pub fn new_json_lines() -> Self {
        Self { json_lines: true }
    }
}

impl Default for RawReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for RawReporter {
    fn generate(&self, result: &AnalysisResult) -> Result<String, ReporterError> {
        if self.json_lines {
            self.generate_json_lines(result)
        } else {
            self.generate_pipe_delimited(result)
        }
    }

    /// Generate a raw test report carrying the full test statistics and per-case
    /// results in addition to any compile issues. A run with failing tests but
    /// zero compile issues must never collapse to an empty report.
    fn generate_test_report(
        &self,
        result: &TestAnalysisResult,
    ) -> Result<String, ReporterError> {
        self.generate_test_report_with_options(result, super::ReportOptions::default())
    }

    fn generate_test_report_with_options(
        &self,
        result: &TestAnalysisResult,
        _options: super::ReportOptions,
    ) -> Result<String, ReporterError> {
        if self.json_lines {
            self.generate_test_json_lines(result)
        } else {
            self.generate_test_pipe_delimited(result)
        }
    }
}

impl RawReporter {
    /// Generate raw pipe-delimited test output:
    ///   TEST_SUMMARY|total=<n>|passed=<n>|failed=<n>|ignored=<n>
    ///   TEST|FAILED|<name>|<details>
    ///   TEST|PASSED|<name>
    ///   TEST|SKIPPED|<name>
    /// followed by any compile issues in the standard pipe-delimited format.
    fn generate_test_pipe_delimited(
        &self,
        result: &TestAnalysisResult,
    ) -> Result<String, ReporterError> {
        let mut output = String::new();
        if let Some(ref summary) = result.test_summary {
            output.push_str(&format!(
                "TEST_SUMMARY|total={}|passed={}|failed={}|ignored={}\n",
                summary.total, summary.passed, summary.failed, summary.ignored
            ));
        }
        for test in &result.failed_tests {
            let details = test
                .failure_details
                .as_deref()
                .unwrap_or("")
                .replace('\n', "\\n");
            output.push_str(&format!(
                "TEST|FAILED|{}|{}\n",
                test.name.replace('|', "\\|"),
                details.replace('|', "\\|")
            ));
        }
        for test in &result.passed_tests {
            output.push_str(&format!("TEST|PASSED|{}\n", test.name.replace('|', "\\|")));
        }
        for test in &result.ignored_tests {
            output.push_str(&format!("TEST|SKIPPED|{}\n", test.name.replace('|', "\\|")));
        }
        output.push_str(&self.generate_pipe_delimited(&result.compile_result)?);
        Ok(output)
    }

    /// Generate raw JSON-lines test output: one JSON object per line for the
    /// summary, each per-case result, and any compile issues.
    fn generate_test_json_lines(
        &self,
        result: &TestAnalysisResult,
    ) -> Result<String, ReporterError> {
        let mut output = String::new();
        if let Some(ref summary) = result.test_summary {
            output.push_str(&format!(
                r#"{{"type":"test_summary","total":{},"passed":{},"failed":{},"ignored":{},"measured":{},"filtered":{}}}"#,
                summary.total, summary.passed, summary.failed, summary.ignored,
                summary.measured, summary.filtered
            ));
            output.push('\n');
        }
        for test in &result.failed_tests {
            let details = test
                .failure_details
                .as_deref()
                .unwrap_or("")
                .replace('"', "\\\"")
                .replace('\n', "\\n");
            output.push_str(&format!(
                r#"{{"type":"test","status":"failed","name":"{}","details":"{}"}}"#,
                test.name.replace('"', "\\\""),
                details
            ));
            output.push('\n');
        }
        for test in &result.passed_tests {
            output.push_str(&format!(
                r#"{{"type":"test","status":"passed","name":"{}"}}"#,
                test.name.replace('"', "\\\"")
            ));
            output.push('\n');
        }
        for test in &result.ignored_tests {
            output.push_str(&format!(
                r#"{{"type":"test","status":"skipped","name":"{}"}}"#,
                test.name.replace('"', "\\\"")
            ));
            output.push('\n');
        }
        output.push_str(&self.generate_json_lines(&result.compile_result)?);
        Ok(output)
    }
    /// Generate pipe-delimited output:
    /// LEVEL|CODE|FILE:LINE:COL|MESSAGE
    fn generate_pipe_delimited(&self, result: &AnalysisResult) -> Result<String, ReporterError> {
        let mut output = String::new();
        let all_issues: Vec<_> = result.issues_by_file.values().flatten().collect();
        for issue in &all_issues {
            let line = issue
                .location
                .line_number
                .map(|n| n.to_string())
                .unwrap_or_default();
            let col = issue
                .location
                .column_number
                .map(|n| n.to_string())
                .unwrap_or_default();
            let code = issue.code.as_deref().unwrap_or("-");
            output.push_str(&format!(
                "{}|{}|{}:{}:{}|{}\n",
                issue.level, code, issue.location.file_path, line, col, issue.message
            ));
        }
        Ok(output)
    }

    /// Generate JSON lines output:
    /// {"level":"error","code":"E0308","file":"src/main.rs","line":10,"column":5,"message":"..."}
    fn generate_json_lines(&self, result: &AnalysisResult) -> Result<String, ReporterError> {
        let mut output = String::new();
        let all_issues: Vec<_> = result.issues_by_file.values().flatten().collect();
        for issue in &all_issues {
            let line = issue
                .location
                .line_number
                .map(|n| n.to_string())
                .unwrap_or_else(|| "null".to_string());
            let col = issue
                .location
                .column_number
                .map(|n| n.to_string())
                .unwrap_or_else(|| "null".to_string());
            let code = issue
                .code
                .as_deref()
                .map(|c| format!("\"{}\"", c))
                .unwrap_or_else(|| "null".to_string());

            output.push_str(&format!(
                r#"{{"level":"{}","code":{},"file":"{}","line":{},"column":{},"message":"{}"}}"#,
                issue.level,
                code,
                issue.location.file_path.replace('"', "\\\""),
                line,
                col,
                issue.message.replace('"', "\\\"")
            ));
            output.push('\n');
        }
        Ok(output)
    }
}
