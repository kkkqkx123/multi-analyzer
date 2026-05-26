//! Mypy Output Parser
//! Parsing the output of mypy

use crate::core::{BaseParser, Issue, OutputParser, ParseResult};

pub struct MypyParser {
    base: BaseParser,
}

impl MypyParser {
    pub fn new() -> Self {
        Self {
            base: BaseParser::new(),
        }
    }

    fn parse_single_line(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("Success")
            || trimmed.starts_with("Found")
        {
            return None;
        }

        self.base.parse_standard_format(line)
    }
}

impl Default for MypyParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for MypyParser {
    // Custom parse implementation for Mypy output
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let lines: Vec<String> = output.lines().map(|s| s.to_string()).collect();
        let mut issues = Vec::new();

        for line in &lines {
            if let Some(issue) = self.parse_single_line(line) {
                issues.push(issue);
            }
        }

        ParseResult::Full(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::IssueLevel;

    #[test]
    fn test_parse_error_with_column() {
        let parser = MypyParser::new();
        let line = "src/main.py:10:5: error: Incompatible types in assignment";
        let issue = parser.parse_single_line(line).unwrap();

        assert_eq!(issue.location.file_path, "src/main.py");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.message, "Incompatible types in assignment");
        assert!(matches!(issue.level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_warning() {
        let parser = MypyParser::new();
        let line = "src/utils.py:20: warning: Unused import";
        let issue = parser.parse_single_line(line).unwrap();

        assert_eq!(issue.location.file_path, "src/utils.py");
        assert_eq!(issue.location.line_number, Some(20));
        assert_eq!(issue.message, "Unused import");
        assert!(matches!(issue.level, IssueLevel::Warning));
    }

    #[test]
    fn test_parse_note() {
        let parser = MypyParser::new();
        let line = "src/types.py:15:3: note: Revealed type is 'int'";
        let issue = parser.parse_single_line(line).unwrap();

        assert_eq!(issue.location.file_path, "src/types.py");
        assert_eq!(issue.location.line_number, Some(15));
        assert_eq!(issue.location.column_number, Some(3));
        assert_eq!(issue.message, "Revealed type is 'int'");
        assert!(matches!(issue.level, IssueLevel::Info));
    }

    #[test]
    fn test_skip_success_message() {
        let parser = MypyParser::new();
        let line = "Success: no issues found in 1 source file";
        assert!(parser.parse_single_line(line).is_none());
    }

    #[test]
    fn test_skip_found_message() {
        let parser = MypyParser::new();
        let line = "Found 3 errors in 2 files (checked 5 source files)";
        assert!(parser.parse_single_line(line).is_none());
    }
}
