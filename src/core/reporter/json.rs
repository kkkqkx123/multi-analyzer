//! JSON Report Generator

use super::{Reporter, ReporterError};
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
