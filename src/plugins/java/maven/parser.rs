//! Maven Output Parser
//Parsing the output of Maven compile/test Parsing the output of Maven compile/test

use crate::core::{
    Issue, IssueLevel, Location, OutputParser, ParseResult, ParsedTestOutput, TestCase,
    TestOutputParser, TestStatus, TestSummary,
};

pub struct MavenParser;

impl MavenParser {
    pub fn new() -> Self {
        Self
    }

    /// Extract module name from Maven error message
    /// Format: "Failed to execute goal on project my-module: ..."
    fn extract_module_from_message(&self, line: &str) -> Option<String> {
        if line.contains("on project") {
            let re = regex::Regex::new(r"on project\s+([^:\s]+)").ok()?;
            let caps = re.captures(line)?;
            return Some(caps.get(1)?.as_str().to_string());
        }
        None
    }

    /// Parsing Maven Compile Error/Warning Lines
    /// 格式: [ERROR] /path/to/File.java:[10,5] error: message
    /// 格式: [WARNING] /path/to/File.java:[20,10] warning: message
    fn parse_maven_issue_line(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();

        // Check for error/warning lines
        let level = if trimmed.starts_with("[ERROR]") {
            IssueLevel::Error
        } else if trimmed.starts_with("[WARNING]") {
            IssueLevel::Warning
        } else {
            return None;
        };

        // Remove the [ERROR] or [WARNING] prefix.
        let content = trimmed
            .strip_prefix("[ERROR]")
            .or_else(|| trimmed.strip_prefix("[WARNING]"))
            .map(|s| s.trim())
            .unwrap_or(trimmed);

        // Parsing file paths and locations
        // 格式: /path/to/File.java:[10,5] error: message
        if let Some(location_end) = content.find(']') {
            let location_part = &content[..location_end];
            let rest = &content[location_end + 1..].trim();

            // Parsing file paths and line numbers
            if let Some((file_path, line_num, col_num)) = self.parse_java_location(location_part) {
                // Parsing messages (removing the "error:" or "warning:" prefix)
                let message = self.extract_message(rest);

                let location = Location::new(file_path)
                    .with_line(line_num)
                    .with_column(col_num);

                return Some(Issue::new(level, message, location));
            }
        }

        // Trying to parse a format without line numbers
        // 格式: [ERROR] message
        if !content.contains(':') || content.starts_with("Failed to execute goal") {
            let location = Location::new("pom.xml");
            let mut issue = Issue::new(level, content.to_string(), location);

            // Try to extract module name from the message
            if let Some(module) = self.extract_module_from_message(line) {
                issue = issue.with_package(module);
            }

            return Some(issue);
        }

        None
    }

    /// Parsing Java File Locations
    /// Format: /path/to/File.java:[10,5] or /path/to/File.java:10
    fn parse_java_location(&self, location_str: &str) -> Option<(String, u32, u32)> {
        // Find the position of '[' (beginning of the row and column numbers)
        if let Some(bracket_start) = location_str.rfind('[') {
            let file_path = location_str[..bracket_start].trim();
            // Remove the colon at the end (if any)
            let file_path = file_path.trim_end_matches(':');
            let coords = &location_str[bracket_start + 1..]; // Skip "["

            // Parses row and column numbers, format: 10,5] or 10].
            let coords = coords.trim_end_matches(']');
            let parts: Vec<&str> = coords.split(',').collect();
            if !parts.is_empty() {
                let line_num = parts[0].trim().parse::<u32>().ok()?;
                let col_num = parts
                    .get(1)
                    .and_then(|p| p.trim().parse::<u32>().ok())
                    .unwrap_or(0);
                return Some((file_path.to_string(), line_num, col_num));
            }
        }

        // Try the simple format: path:line
        if let Some(colon_pos) = location_str.rfind(':') {
            let file_path = &location_str[..colon_pos];
            let line_str = &location_str[colon_pos + 1..];
            if let Ok(line_num) = line_str.parse::<u32>() {
                return Some((file_path.to_string(), line_num, 0));
            }
        }

        None
    }

    /// Extract message content
    fn extract_message(&self, rest: &str) -> String {
        // Remove the "error:" or "warning:" prefix.
        let msg = rest
            .trim_start_matches("error:")
            .trim_start_matches("warning:")
            .trim_start_matches("[unchecked]")
            .trim();

        msg.to_string()
    }

    /// Parsing multi-line errors (collecting error details)
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
        if let Some(issue) = self.parse_maven_issue_line(line) {
            return (Some(issue), start_index + 1);
        }

        // Checking for mis-symbolized multi-line formatting
        // Symbol: variable x
        // 位置: 类 com.example.MyClass
        if line.trim().starts_with("Sign: (1)") || line.trim().starts_with("Position: (1)") {
            // Look up the error line
            for i in (0..start_index).rev() {
                if let Some(issue) = self.parse_maven_issue_line(&lines[i]) {
                    return (Some(issue), start_index + 1);
                }
            }
        }

        (None, start_index + 1)
    }
}

impl Default for MavenParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for MavenParser {
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

impl TestOutputParser for MavenParser {
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();
        result.compile_issues = <Self as OutputParser>::parse(self, output).data_or_default_owned();

        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;
        let mut tests_run: usize = 0;
        let mut failures: usize = 0;
        let mut errors: usize = 0;
        let mut skipped: usize = 0;

        let summary_re = regex::Regex::new(
            r"Tests run:\s*(\d+),\s*Failures:\s*(\d+),\s*Errors:\s*(\d+),\s*Skipped:\s*(\d+)",
        )
        .ok();

        while i < lines.len() {
            let line = lines[i];

            // Parse test results line: "Tests run: 5, Failures: 1, Errors: 0, Skipped: 0"
            if line.contains("Tests run:") {
                if let Some(re) = &summary_re {
                    if let Some(caps) = re.captures(line) {
                        tests_run = caps
                            .get(1)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(0);
                        failures = caps
                            .get(2)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(0);
                        errors = caps
                            .get(3)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(0);
                        skipped = caps
                            .get(4)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(0);
                    }
                }
            }

            // Parse test class running line: "Running com.example.MyTest"
            if line.trim().starts_with("Running ") {
                let class_name = line.trim().strip_prefix("Running ").unwrap_or("").trim();
                if !class_name.is_empty() {
                    // Check if this specific test failed by looking ahead for FAILURE! marker
                    let mut j = i + 1;
                    let mut class_failed = false;
                    while j < lines.len() && !lines[j].contains("Tests run:") {
                        if lines[j].contains("FAILURE!") {
                            class_failed = true;
                        }
                        j += 1;
                    }
                    if class_failed {
                        result.failed_tests.push(TestCase {
                            name: class_name.to_string(),
                            status: TestStatus::Failed,
                            location: None,
                            failure_details: None,
                            execution_time: None,
                        });
                    } else {
                        result.passed_tests.push(TestCase {
                            name: class_name.to_string(),
                            status: TestStatus::Passed,
                            location: None,
                            failure_details: None,
                            execution_time: None,
                        });
                    }
                }
            }

            i += 1;
        }

        let passed = tests_run.saturating_sub(failures + errors);
        result.test_summary = Some(TestSummary {
            total: tests_run,
            passed,
            failed: failures + errors,
            ignored: skipped,
            measured: 0,
            filtered: 0,
            execution_time: None,
        });

        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_line() {
        let parser = MavenParser::new();
        let line = "[ERROR] /path/to/File.java:[10,5] error: cannot find symbol";

        let issue = parser.parse_maven_issue_line(line).unwrap();

        assert_eq!(issue.level, IssueLevel::Error);
        assert_eq!(issue.location.file_path, "/path/to/File.java");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert!(issue.message.contains("cannot find symbol"));
    }

    #[test]
    fn test_parse_warning_line() {
        let parser = MavenParser::new();
        let line = "[WARNING] /path/to/File.java:[20,10] warning: [unchecked] unchecked conversion";

        let issue = parser.parse_maven_issue_line(line).unwrap();

        assert_eq!(issue.level, IssueLevel::Warning);
        assert_eq!(issue.location.file_path, "/path/to/File.java");
        assert_eq!(issue.location.line_number, Some(20));
        assert_eq!(issue.location.column_number, Some(10));
    }
}
