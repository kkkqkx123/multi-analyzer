//! ClangFormat Output Parser
//! Parses clang-format output for formatting issues

use crate::core::{Issue, IssueLevel, Location, OutputParser, ParseResult};

pub struct ClangFormatParser;

impl ClangFormatParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse clang-format output.
    ///
    /// With --dry-run --Werror, clang-format exits non-zero and outputs
    /// warnings like:
    ///
    ///   /path/to/file.cpp:123:5: error: code should be clang-formatted [-Wclang-format-violations]
    ///
    /// In --dry-run mode without --Werror:
    ///   /path/to/file.cpp
    ///
    /// Each line is a file that needs formatting.
    fn parse_format_line(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Handle --dry-run + --Werror output format:
        // file:line:col: error: message [-W...]
        if trimmed.contains("clang-formatted") || trimmed.contains("clang-format-violations") {
            let parts: Vec<&str> = trimmed.splitn(5, ':').collect();
            if parts.len() >= 5 {
                let file_path = parts[0].trim();
                let line_num = parts[1].trim().parse::<u32>().ok()?;
                let col_num = parts[2].trim().parse::<u32>().ok()?;
                let message = parts[4].trim().to_string();

                let location = Location::new(file_path.to_string())
                    .with_line(line_num)
                    .with_column(col_num);

                return Some(
                    Issue::new(IssueLevel::Error, message, location)
                        .with_code("FORMAT".to_string()),
                );
            }
        }

        // Handle --dry-run (file list) format:
        // Each line is a file path
        if trimmed.contains('.')
            && (trimmed.ends_with(".cpp")
                || trimmed.ends_with(".c")
                || trimmed.ends_with(".hpp")
                || trimmed.ends_with(".h")
                || trimmed.ends_with(".cc")
                || trimmed.ends_with(".cxx")
                || trimmed.ends_with(".hxx"))
        {
            let location = Location::new(trimmed.to_string());
            return Some(
                Issue::new(
                    IssueLevel::Warning,
                    "File requires formatting".to_string(),
                    location,
                )
                .with_code("FORMAT".to_string()),
            );
        }

        None
    }
}

impl Default for ClangFormatParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for ClangFormatParser {
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
    fn test_parse_werror_format() {
        let parser = ClangFormatParser::new();
        let line = "/home/user/project/src/main.cpp:123:5: error: code should be clang-formatted [-Wclang-format-violations]";
        let issue = parser.parse_format_line(line).unwrap();

        assert_eq!(issue.location.file_path, "/home/user/project/src/main.cpp");
        assert_eq!(issue.location.line_number, Some(123));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.code, Some("FORMAT".to_string()));
        assert!(matches!(issue.level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_file_list_format() {
        let parser = ClangFormatParser::new();
        let line = "/path/to/file.cpp";
        let issue = parser.parse_format_line(line).unwrap();

        assert_eq!(issue.location.file_path, "/path/to/file.cpp");
        assert_eq!(issue.code, Some("FORMAT".to_string()));
        assert!(matches!(issue.level, IssueLevel::Warning));
    }

    #[test]
    fn test_parse_skip_non_code() {
        let parser = ClangFormatParser::new();
        assert!(parser.parse_format_line("").is_none());
        assert!(parser.parse_format_line("some random text").is_none());
    }

    #[test]
    fn test_parse_full_output() {
        let parser = ClangFormatParser::new();
        let output = "src/main.cpp\nsrc/util.cpp\ninclude/header.h";

        let result = parser.parse(output);
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 3);
    }
}
