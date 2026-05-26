//! Gradle Output Parser
//! Parsing the output of Gradle compile/test

use crate::core::{Issue, IssueLevel, Location, OutputParser, ParseResult};

pub struct GradleParser;

impl GradleParser {
    pub fn new() -> Self {
        Self
    }

    /// Parsing Gradle Compile Error/Warning Lines
    /// Format: /path/to/File.java:10: error: message
    /// Format: /path/to/File.java:20: warning: message
    fn parse_gradle_issue_line(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();

        // Check for error/warning lines with file path
        // Format: /path/to/File.java:10: error: message
        if let Some((file_path, line_num, level, message)) = self.parse_file_location(trimmed) {
            let location = Location::new(file_path)
                .with_line(line_num)
                .with_column(0);

            return Some(Issue::new(level, message, location));
        }

        // Check for general error messages without file path
        // Format: > Task :compileJava FAILED
        if trimmed.contains("FAILED") {
            let location = Location::new("build.gradle");
            let message = trimmed.to_string();
            return Some(Issue::new(IssueLevel::Error, message, location));
        }

        // Check for stack trace errors
        // Format: ERROR: message
        if trimmed.starts_with("ERROR:") || trimmed.starts_with("error:") {
            let message = trimmed
                .trim_start_matches("ERROR:")
                .trim_start_matches("error:")
                .trim()
                .to_string();
            let location = Location::new("build.gradle");
            return Some(Issue::new(IssueLevel::Error, message, location));
        }

        None
    }

    /// Parse file location with line number
    /// Format: /path/to/File.java:10: error: message
    /// Format: /path/to/File.java:20: warning: message
    fn parse_file_location(&self, line: &str) -> Option<(String, u32, IssueLevel, String)> {
        // Look for pattern: path:line: level: message
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() < 3 {
            return None;
        }

        let potential_path = parts[0];

        // Check if it looks like a file path (contains .java, .kt, .groovy, etc.)
        if !self.is_source_file(potential_path) {
            return None;
        }

        // Parse line number
        let line_num = parts[1].trim().parse::<u32>().ok()?;

        // Determine level and message
        let level_str = parts[2].trim().to_lowercase();
        let level = if level_str.contains("error") {
            IssueLevel::Error
        } else if level_str.contains("warning") || level_str.contains("warn") {
            IssueLevel::Warning
        } else {
            IssueLevel::Error // Default to error
        };

        // Extract message
        let message = if parts.len() >= 4 {
            parts[3].trim().to_string()
        } else {
            level_str
        };

        Some((potential_path.to_string(), line_num, level, message))
    }

    /// Check if path is a source file
    fn is_source_file(&self, path: &str) -> bool {
        path.ends_with(".java")
            || path.ends_with(".kt")
            || path.ends_with(".groovy")
            || path.ends_with(".scala")
            || path.contains("/src/")
            || path.contains("\\src\\")
    }

    /// Parse multi-line errors (collecting error details)
    fn parse_multiline_issue(
        &self,
        lines: &[String],
        start_index: usize,
    ) -> (Option<Issue>, usize) {
        if start_index >= lines.len() {
            return (None, start_index);
        }

        let line = &lines[start_index];

        // First try parsing the one-line format
        if let Some(issue) = self.parse_gradle_issue_line(line) {
            return (Some(issue), start_index + 1);
        }

        // Check for symbol error continuation lines
        // Format:  symbol: variable x
        // Format:  location: class com.example.MyClass
        if line.trim().starts_with("symbol:") || line.trim().starts_with("location:") {
            // Look up for the error line
            for i in (0..start_index).rev() {
                if let Some(issue) = self.parse_gradle_issue_line(&lines[i]) {
                    return (Some(issue), start_index + 1);
                }
            }
        }

        (None, start_index + 1)
    }
}

impl Default for GradleParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for GradleParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let lines: Vec<String> = output.lines().map(|s| s.to_string()).collect();
        let mut issues = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            let (issue, new_index) = self.parse_multiline_issue(&lines, i);

            if let Some(issue) = issue {
                issues.push(issue);
                i = new_index;
            } else {
                i += 1;
            }
        }

        ParseResult::Full(issues)
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_line() {
        let parser = GradleParser::new();
        let line = "/path/to/File.java:10: error: cannot find symbol";

        let issue = parser.parse_gradle_issue_line(line).unwrap();

        assert_eq!(issue.level, IssueLevel::Error);
        assert_eq!(issue.location.file_path, "/path/to/File.java");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(0));
        assert!(issue.message.contains("cannot find symbol"));
    }

    #[test]
    fn test_parse_warning_line() {
        let parser = GradleParser::new();
        let line = "/path/to/File.java:20: warning: unchecked conversion";

        let issue = parser.parse_gradle_issue_line(line).unwrap();

        assert_eq!(issue.level, IssueLevel::Warning);
        assert_eq!(issue.location.file_path, "/path/to/File.java");
        assert_eq!(issue.location.line_number, Some(20));
        assert!(issue.message.contains("unchecked conversion"));
    }

    #[test]
    fn test_parse_kt_file() {
        let parser = GradleParser::new();
        let line = "/path/to/File.kt:15: error: unresolved reference";

        let issue = parser.parse_gradle_issue_line(line).unwrap();

        assert_eq!(issue.level, IssueLevel::Error);
        assert_eq!(issue.location.file_path, "/path/to/File.kt");
        assert_eq!(issue.location.line_number, Some(15));
    }

}
