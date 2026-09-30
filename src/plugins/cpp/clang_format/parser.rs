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

    // ── --dry-run + --Werror (GCC-style ERROR lines) ────────────────

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
    fn test_parse_werror_format_long_path() {
        let parser = ClangFormatParser::new();
        let line = "/a/very/long/nested/path/src/components/main.cpp:42:3: error: code should be clang-formatted [-Wclang-format-violations]";
        let issue = parser.parse_format_line(line).unwrap();
        assert_eq!(issue.location.file_path, "/a/very/long/nested/path/src/components/main.cpp");
        assert_eq!(issue.location.line_number, Some(42));
    }

    #[test]
    fn test_parse_werror_short_message() {
        let parser = ClangFormatParser::new();
        let line = "x.cpp:1:1: error: clang-formatted [-Wclang-format-violations]";
        let issue = parser.parse_format_line(line).unwrap();
        assert_eq!(issue.location.file_path, "x.cpp");
    }

    // ── --dry-run file list format ────────────────────────────────

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
    fn test_parse_file_list_c_extension() {
        let parser = ClangFormatParser::new();
        let issue = parser.parse_format_line("src/main.c").unwrap();
        assert_eq!(issue.location.file_path, "src/main.c");
    }

    #[test]
    fn test_parse_file_list_all_cpp_extensions() {
        let parser = ClangFormatParser::new();
        for ext in &["cpp", "c", "hpp", "h", "cc", "cxx", "hxx"] {
            let line = format!("/tmp/file.{}", ext);
            let issue = parser.parse_format_line(&line);
            assert!(
                issue.is_some(),
                "extension .{} should be recognized",
                ext
            );
        }
    }

    #[test]
    fn test_parse_file_list_windows_path() {
        let parser = ClangFormatParser::new();
        let issue = parser.parse_format_line("C:\\Users\\me\\proj\\src\\main.cpp").unwrap();
        assert_eq!(issue.location.file_path, "C:\\Users\\me\\proj\\src\\main.cpp");
    }

    #[test]
    fn test_parse_file_list_no_extension_ignored() {
        let parser = ClangFormatParser::new();
        assert!(parser.parse_format_line("CMakeLists.txt").is_none());
        assert!(parser.parse_format_line("Makefile").is_none());
        assert!(parser.parse_format_line("file.o").is_none());
    }

    // ── Noise filtering ───────────────────────────────────────────

    #[test]
    fn test_parse_skip_non_code() {
        let parser = ClangFormatParser::new();
        assert!(parser.parse_format_line("").is_none());
        assert!(parser.parse_format_line("some random text").is_none());
    }

    #[test]
    fn test_parse_skip_clang_format_version_banner() {
        let parser = ClangFormatParser::new();
        assert!(parser.parse_format_line("clang-format version 16.0.0").is_none());
    }

    // ── Full parse ─────────────────────────────────────────────────

    #[test]
    fn test_parse_full_output() {
        let parser = ClangFormatParser::new();
        let output = "src/main.cpp\nsrc/util.cpp\ninclude/header.h";

        let result = parser.parse(output);
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 3);
    }

    #[test]
    fn test_parse_full_werror_output() {
        let parser = ClangFormatParser::new();
        let output = "\
src/a.cpp:1:1: error: code should be clang-formatted [-Wclang-format-violations]
src/b.cpp:2:2: error: code should be clang-formatted [-Wclang-format-violations]";

        let result = parser.parse(output);
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 2);
        assert!(matches!(issues[0].level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_empty() {
        let parser = ClangFormatParser::new();
        let result = parser.parse("");
        let issues = result.data().unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_parse_whitespace_only() {
        let parser = ClangFormatParser::new();
        let result = parser.parse("\n\n  \n");
        let issues = result.data().unwrap();
        assert!(issues.is_empty());
    }
}
