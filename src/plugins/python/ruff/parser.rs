//! Ruff Output Parser
//! Parses ruff check --output-format json output

use crate::core::{Issue, IssueLevel, Location, OutputParser, ParseResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RuffOutput {
    filename: String,
    location: RuffLocation,
    code: Option<String>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct RuffLocation {
    row: u32,
    column: u32,
}

pub struct RuffParser;

impl RuffParser {
    pub fn new() -> Self {
        Self
    }

    fn parse_json(&self, output: &str) -> Option<Vec<Issue>> {
        let items: Vec<RuffOutput> = serde_json::from_str(output).ok()?;
        let mut issues = Vec::new();

        for item in items {
            let location = Location::new(item.filename)
                .with_line(item.location.row)
                .with_column(item.location.column);

            let mut issue = Issue::new(IssueLevel::Error, item.message, location);
            if let Some(code) = item.code {
                issue = issue.with_code(code);
            }
            issues.push(issue);
        }

        Some(issues)
    }

    fn parse_text_line(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Found") {
            return None;
        }

        let (rest, level) = if let Some(rest) = trimmed.strip_prefix("error: ") {
            (rest, IssueLevel::Error)
        } else if let Some(rest) = trimmed.strip_prefix("warning: ") {
            (rest, IssueLevel::Warning)
        } else {
            // No prefix - treat as error, parse file:line:col: format directly
            (trimmed, IssueLevel::Error)
        };

        // Split file:line:col: code message
        // Format: path/file.py:10:5: F401 message...
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        if parts.len() < 2 {
            return None;
        }

        let header = parts[0].trim_end_matches(':');
        let msg = parts[1];

        let header_parts: Vec<&str> = header.rsplitn(3, ':').collect();
        if header_parts.len() < 3 {
            return None;
        }

        let col = header_parts[0].parse::<u32>().ok()?;
        let line_num = header_parts[1].parse::<u32>().ok()?;
        let file_path = header_parts[2];

        let location = Location::new(file_path.to_string())
            .with_line(line_num)
            .with_column(col);

        // Check if there's a code prefix in the message
        let msg_trimmed = msg.trim();
        let (code, message) = if let Some(code_end) = msg_trimmed.find(' ') {
            let potential_code = &msg_trimmed[..code_end];
            if potential_code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
                let rest_msg = msg_trimmed[code_end + 1..].trim();
                (Some(potential_code.to_string()), rest_msg.to_string())
            } else {
                (None, msg_trimmed.to_string())
            }
        } else {
            (None, msg_trimmed.to_string())
        };

        let mut issue = Issue::new(level, message, location);
        if let Some(c) = code {
            issue = issue.with_code(c);
        }

        Some(issue)
    }

    /// Parse ruff format --check output.
    ///
    /// Format:
    ///   Would reformat: path/to/file.py
    ///
    /// Summary lines (skip):
    ///   N file(s) reformatted, M file(s) left unchanged
    ///   All files already formatted
    fn parse_format_line(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let lower = trimmed.to_lowercase();
        if lower.contains("would reformat") {
            if let Some(path) = trimmed.split(':').nth(1) {
                let path = path.trim();
                let location = Location::new(path.to_string());
                return Some(
                    Issue::new(
                        IssueLevel::Warning,
                        "File would be reformatted".to_string(),
                        location,
                    )
                    .with_code("FORMAT".to_string()),
                );
            }
        }

        None
    }

    /// Check if the output appears to be from ruff format.
    fn is_format_output(&self, output: &str) -> bool {
        let lower = output.to_lowercase();
        lower.contains("would reformat")
            || lower.contains("reformatted")
            || lower.contains("left unchanged")
    }
}

impl Default for RuffParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for RuffParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        // Detect format output first
        if self.is_format_output(output) {
            let mut issues = Vec::new();
            for line in output.lines() {
                if let Some(issue) = self.parse_format_line(line) {
                    issues.push(issue);
                }
            }
            return ParseResult::Full(issues);
        }

        // First try JSON format
        if let Some(issues) = self.parse_json(output) {
            return ParseResult::Full(issues);
        }

        // Fallback to text format
        let mut issues = Vec::new();
        let mut unknown_count = 0;

        for line in output.lines() {
            if let Some(issue) = self.parse_text_line(line) {
                issues.push(issue);
            } else if !line.trim().is_empty()
                && !line.starts_with("Found")
                && !line.starts_with("All checks")
            {
                unknown_count += 1;
            }
        }

        if unknown_count > 0 && issues.is_empty() {
            ParseResult::Degraded(
                issues,
                vec![format!(
                    "Ruff: {} lines could not be parsed (not JSON format)",
                    unknown_count
                )],
            )
        } else {
            ParseResult::Full(issues)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_single_issue() {
        let parser = RuffParser::new();
        let json = r#"[
            {
                "filename": "src/main.py",
                "location": {"row": 10, "column": 5},
                "code": "F401",
                "message": "`os` imported but unused"
            }
        ]"#;

        let issues = parser.parse_json(json).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location.file_path, "src/main.py");
        assert_eq!(issues[0].location.line_number, Some(10));
        assert_eq!(issues[0].location.column_number, Some(5));
        assert_eq!(issues[0].code, Some("F401".to_string()));
        assert!(matches!(issues[0].level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_json_multiple_issues() {
        let parser = RuffParser::new();
        let json = r#"[
            {
                "filename": "a.py",
                "location": {"row": 1, "column": 1},
                "code": "F401",
                "message": "unused import"
            },
            {
                "filename": "b.py",
                "location": {"row": 5, "column": 10},
                "code": "E501",
                "message": "line too long"
            }
        ]"#;

        let issues = parser.parse_json(json).unwrap();
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn test_parse_json_empty() {
        let parser = RuffParser::new();
        let json = r#"[]"#;

        let issues = parser.parse_json(json).unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_parse_json_no_code() {
        let parser = RuffParser::new();
        let json = r#"[
            {
                "filename": "src/app.py",
                "location": {"row": 3, "column": 8},
                "code": null,
                "message": "invalid syntax"
            }
        ]"#;

        let issues = parser.parse_json(json).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, None);
    }

    #[test]
    fn test_parse_text_format() {
        let parser = RuffParser::new();
        let text = "src/main.py:10:5: F401 `os` imported but unused";

        let issue = parser.parse_text_line(text).unwrap();
        assert_eq!(issue.location.file_path, "src/main.py");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert!(issue.code.is_some());
        assert!(matches!(issue.level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_text_format_warning() {
        let parser = RuffParser::new();
        let text = "warning: src/main.py:15:3: F841 assigned but never used";

        let issue = parser.parse_text_line(text).unwrap();
        assert_eq!(issue.location.line_number, Some(15));
        assert!(matches!(issue.level, IssueLevel::Warning));
    }

    #[test]
    fn test_parse_text_skip_summary() {
        let parser = RuffParser::new();
        assert!(parser
            .parse_text_line("Found 3 errors (1 fixed)")
            .is_none());
    }

    #[test]
    fn test_parse_full_output_json() {
        let parser = RuffParser::new();
        let output = r#"[{"filename": "test.py", "location": {"row": 1, "column": 1}, "code": "F401", "message": "unused"}]"#;

        let result = parser.parse(output);
        match result {
            ParseResult::Full(issues) => assert_eq!(issues.len(), 1),
            _ => panic!("Expected Full result"),
        }
    }

    #[test]
    fn test_parse_full_output_text() {
        let parser = RuffParser::new();
        let output = "src/test.py:1:5: F401 unused import\nFound 1 error";

        let result = parser.parse(output);
        match result {
            ParseResult::Full(issues) => assert_eq!(issues.len(), 1),
            _ => panic!("Expected Full result"),
        }
    }
}
