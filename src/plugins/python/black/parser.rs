//! Black Output Parser
//! Parses black --check output for formatting issues

use crate::core::{Issue, IssueLevel, Location, OutputParser, ParseResult};

pub struct BlackParser;

impl BlackParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse black --check output.
    ///
    /// Format:
    ///   would reformat /path/to/file.py
    ///   would reformat src/main.py
    ///
    /// Summary lines (skip):
    ///   Oh no! 💥 💔 💥
    ///   All done! ✨ 🍰 ✨
    ///   N files would be reformatted, M files would be left unchanged.
    ///   M files left unchanged.
    fn parse_format_line(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let lower = trimmed.to_lowercase();

        if lower.starts_with("would reformat") {
            if let Some(path) = trimmed.split(':').nth(1) {
                let path = path.trim();
                let location = Location::new(path.to_string());
                return Some(
                    Issue::new(
                        IssueLevel::Warning,
                        "File requires formatting".to_string(),
                        location,
                    )
                    .with_code("FORMAT".to_string()),
                );
            }
            if let Some(path) = trimmed.strip_prefix("would reformat ") {
                let path = path.trim();
                let location = Location::new(path.to_string());
                return Some(
                    Issue::new(
                        IssueLevel::Warning,
                        "File requires formatting".to_string(),
                        location,
                    )
                    .with_code("FORMAT".to_string()),
                );
            }
        }

        None
    }
}

impl Default for BlackParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for BlackParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let mut issues = Vec::new();

        for line in output.lines() {
            if let Some(issue) = self.parse_format_line(line) {
                issues.push(issue);
            }
        }

        ParseResult::Full(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_would_reformat() {
        let parser = BlackParser::new();
        let line = "would reformat src/main.py";
        let issue = parser.parse_format_line(line).unwrap();

        assert_eq!(issue.location.file_path, "src/main.py");
        assert_eq!(issue.code, Some("FORMAT".to_string()));
        assert!(matches!(issue.level, IssueLevel::Warning));
    }

    #[test]
    fn test_parse_would_reformat_colon() {
        let parser = BlackParser::new();
        let line = "would reformat: src/main.py";
        let issue = parser.parse_format_line(line).unwrap();

        assert_eq!(issue.location.file_path, "src/main.py");
    }

    #[test]
    fn test_parse_skip_summary() {
        let parser = BlackParser::new();
        assert!(parser.parse_format_line("All done! 5 files left unchanged.").is_none());
        assert!(parser.parse_format_line("Oh no! 2 files would be reformatted.").is_none());
    }

    #[test]
    fn test_parse_full_output() {
        let parser = BlackParser::new();
        let output = "would reformat src/main.py\nwould reformat tests/test_utils.py\nOh no!\n2 files would be reformatted, 3 files would be left unchanged.";

        let result = parser.parse(output);
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].location.file_path, "src/main.py");
        assert_eq!(issues[1].location.file_path, "tests/test_utils.py");
    }

    #[test]
    fn test_parse_no_issues() {
        let parser = BlackParser::new();
        let output = "All done!\n5 files left unchanged.";
        let result = parser.parse(output);
        assert!(result.data().unwrap().is_empty());
    }
}
