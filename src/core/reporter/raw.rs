//! Raw format reporter for machine-readable output.
//!
//! Supports two output modes:
//! - `raw`: pipe-delimited format (LEVEL|CODE|FILE:LINE:COL|MESSAGE)
//! - `raw-json`: JSON lines format (one JSON object per line)

use super::{Reporter, ReporterError};
use crate::core::types::AnalysisResult;

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
}

impl RawReporter {
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
