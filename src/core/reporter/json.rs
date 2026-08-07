//! JSON Report Generator

use super::{Reporter, ReporterError, ReportOptions};
use crate::core::types::AnalysisResult;

/// JSON Report Generator
pub struct JsonReporter;

impl JsonReporter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for JsonReporter {
    fn generate(&self, result: &AnalysisResult) -> Result<String, ReporterError> {
        self.generate_with_options(result, ReportOptions::default())
    }

    fn generate_with_options(
        &self,
        result: &AnalysisResult,
        options: ReportOptions,
    ) -> Result<String, ReporterError> {
        // Success short-circuit: if no issues found and short-circuit is enabled,
        // output a single-line confirmation instead of a full JSON report
        if options.success_short_circuit && result.total_issues == 0 {
            if let Some(msg) = options.short_circuit_message() {
                return Ok(msg);
            }
        }

        let mut json = String::new();
        json.push_str("{\n");

        // metadata
        json.push_str("  \"metadata\": {\n");
        json.push_str(&format!("    \"total\": {},\n", result.total_issues));
        json.push_str(&format!(
            "    \"categories\": {},\n",
            result.unique_patterns.len()
        ));
        json.push_str(&format!(
            "    \"files_affected\": {}\n",
            result.issues_by_file.len()
        ));
        json.push_str("  },\n");

        // Statistics by level
        json.push_str("  \"summary_by_level\": {\n");
        let level_order = ["error", "warning", "info", "hint"];
        let mut first = true;
        for level_str in &level_order {
            if let Some((level, count)) = result
                .issues_by_level
                .iter()
                .find(|(l, _)| l.to_string() == *level_str)
            {
                if !first {
                    json.push_str(",\n");
                }
                json.push_str(&format!("    \"{}\": {}", level, count));
                first = false;
            }
        }
        json.push_str("\n  },\n");

        // Statistics by category
        if !result.issues_by_type.is_empty() {
            json.push_str("  \"summary_by_category\": {\n");
            let mut types: Vec<_> = result.issues_by_type.iter().collect();
            types.sort_by(|a, b| b.1.cmp(a.1));
            for (i, (issue_type, count)) in types.iter().enumerate() {
                let comma = if i < types.len() - 1 { "," } else { "" };
                json.push_str(&format!(
                    "    \"{}\": {}{}\n",
                    issue_type.replace('"', "\\\""),
                    count,
                    comma
                ));
            }
            json.push_str("  },\n");
        }

        // Top error codes
        let top_codes = result.top_error_codes(5);
        if !top_codes.is_empty() {
            json.push_str("  \"top_error_codes\": [\n");
            for (i, (code, count)) in top_codes.iter().enumerate() {
                let comma = if i < top_codes.len() - 1 { "," } else { "" };
                json.push_str(&format!(
                    "    {{\"code\": \"{}\", \"count\": {}}}{}\n",
                    code.replace('"', "\\\""),
                    count,
                    comma
                ));
            }
            json.push_str("  ],\n");
        }

        // Detailed list of questions
        json.push_str("  \"items\": [\n");
        let all_issues: Vec<_> = result.issues_by_file.values().flatten().collect();
        for (i, issue) in all_issues.iter().enumerate() {
            let comma = if i < all_issues.len() - 1 { "," } else { "" };
            json.push_str("    {\n");
            json.push_str(&format!("      \"severity\": \"{}\",\n", issue.level));
            if let Some(code) = &issue.code {
                json.push_str(&format!("      \"code\": \"{}\",\n", code));
            }
            json.push_str(&format!(
                "      \"message\": \"{}\",\n",
                issue.message.replace('"', "\\\"")
            ));
            json.push_str("      \"location\": {\n");
            json.push_str(&format!(
                "        \"file\": \"{}\"",
                issue.location.file_path.replace('"', "\\\"")
            ));
            if let Some(line) = issue.location.line_number {
                json.push_str(&format!(",\n        \"line\": {}", line));
            }
            if let Some(col) = issue.location.column_number {
                json.push_str(&format!(",\n        \"column\": {}", col));
            }
            json.push_str("\n      }\n");
            json.push_str(&format!("    }}{}\n", comma));
        }
        json.push_str("  ]\n");

        json.push('}');
        Ok(json)
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
            "type mismatch",
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
    fn test_json_generate_empty() {
        let reporter = JsonReporter::new();
        let result = AnalysisResult::new();
        let report = reporter.generate(&result).unwrap();
        assert!(report.contains("\"metadata\""));
        assert!(report.contains("\"total\": 0"));
        assert!(report.contains("\"items\": ["));
    }

    #[test]
    fn test_json_generate_with_issues() {
        let reporter = JsonReporter::new();
        let report = reporter.generate(&sample_result()).unwrap();
        assert!(report.contains("\"total\": 2"));
        assert!(report.contains("\"error\": 1"));
        assert!(report.contains("\"warning\": 1"));
        assert!(report.contains("type mismatch"));
        assert!(report.contains("unused variable"));
        assert!(report.contains("src/main.rs"));
        assert!(report.contains("src/lib.rs"));
    }

    #[test]
    fn test_json_generate_valid_json() {
        let reporter = JsonReporter::new();
        let report = reporter.generate(&sample_result()).unwrap();
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(parsed["metadata"]["total"], 2);
        assert_eq!(parsed["summary_by_level"]["error"], 1);
        assert_eq!(parsed["items"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_json_short_circuit() {
        let reporter = JsonReporter::new();
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
