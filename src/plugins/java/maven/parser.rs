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

        // Only real diagnostics carry a `[line,col]` (or `[line]`) location.
        // Everything else ([ERROR] blank lines, "To see the full stack",
        // "Re-run Maven using -X", "-> [Help 1]", "COMPILATION ERROR :", etc.)
        // is Maven boilerplate and must be skipped to avoid false positives.
        if let Some(location_end) = content.find(']') {
            let location_part = &content[..location_end];
            // The char right after ']' is the location terminator; strip it and
            // any leading whitespace so the message does not start with "] ".
            let rest = content[location_end + 1..].trim_start_matches(']').trim();

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

        // Module-level build failure: "Failed to execute goal ... on project X: ..."
        // This is a genuine diagnostic (no file-level location available).
        if content.starts_with("Failed to execute goal") {
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

    #[test]
    fn test_parse_maven_issue_line_no_match() {
        let parser = MavenParser::new();
        assert!(parser.parse_maven_issue_line("Some random log line").is_none());
        assert!(parser.parse_maven_issue_line("[INFO] Building project...").is_none());
    }

    #[test]
    fn test_parse_via_trait_empty() {
        let parser = MavenParser::new();
        let result = parser.parse("");
        assert!(result.is_full());
        assert!(result.data().unwrap().is_empty());
    }

    #[test]
    fn test_parse_via_trait_with_issues() {
        let parser = MavenParser::new();
        let output = "[ERROR] /src/Main.java:[10,5] error: cannot find symbol\n\
                      [WARNING] /src/Util.java:[20,10] warning: [unchecked] unchecked conversion";
        let result = parser.parse(output);
        assert!(result.is_full());
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].level, IssueLevel::Error);
        assert_eq!(issues[1].level, IssueLevel::Warning);
    }

    #[test]
    fn test_parse_java_location() {
        let parser = MavenParser::new();
        let (file, line, col) = parser.parse_java_location("/path/to/File.java:[10,5]").unwrap();
        assert_eq!(file, "/path/to/File.java");
        assert_eq!(line, 10);
        assert_eq!(col, 5);
    }

    #[test]
    fn test_parse_java_location_invalid() {
        let parser = MavenParser::new();
        assert!(parser.parse_java_location("no brackets here").is_none());
        assert!(parser.parse_java_location("/path:[abc]").is_none());
    }

    #[test]
    fn test_parse_test_output() {
        let parser = MavenParser::new();
        let output = "\
Tests run: 5, Failures: 1, Errors: 0, Skipped: 0, Time elapsed: 1.5 sec
[ERROR] Tests run: 5, Failures: 1, Errors: 0, Skipped: 0
Running com.example.TestSuite
    testSomething: FAILURE!
    expected:<X> but was:<Y>
Running com.example.OtherTest
    testOther: FAILURE!
    NullPointerException";
        let test_result = parser.parse_test_output(output);
        assert_eq!(test_result.failed_tests.len(), 2);
        assert!(test_result.failed_tests[0].name.contains("TestSuite"));
        assert!(test_result.failed_tests[1].name.contains("OtherTest"));
        assert!(test_result.test_summary.is_some());
        let summary = test_result.test_summary.unwrap();
        assert_eq!(summary.total, 5);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn test_skips_maven_boilerplate() {
        let parser = MavenParser::new();
        // Real diagnostic must be captured...
        let good = parser
            .parse_maven_issue_line("[ERROR] /src/Main.java:[8,28] cannot find symbol")
            .unwrap();
        assert_eq!(good.level, IssueLevel::Error);
        assert_eq!(good.message, "cannot find symbol");
        assert_eq!(good.location.line_number, Some(8));
        // ...while boilerplate lines must be ignored.
        for boilerplate in [
            "[ERROR]",
            "[ERROR] COMPILATION ERROR :",
            "[ERROR] -> [Help 1]",
            "[ERROR] To see the full stack trace of the errors.",
            "[ERROR] Re-run Maven using the -X switch to enable full debug logging.",
            "[ERROR] For more information about the errors and possible solutions, please read the following articles:",
        ] {
            assert!(
                parser.parse_maven_issue_line(boilerplate).is_none(),
                "boilerplate should be skipped: {boilerplate:?}"
            );
        }
    }

    #[test]
    fn test_real_error_message_has_no_leading_bracket() {
        let parser = MavenParser::new();
        let line = "[ERROR] /src/Main.java:[8,28] cannot find symbol";
        let issue = parser.parse_maven_issue_line(line).unwrap();
        assert_eq!(issue.message, "cannot find symbol");
        assert!(!issue.message.starts_with(']'));
    }

    #[test]
    fn test_module_level_build_failure_captured() {
        let parser = MavenParser::new();
        let line = "[ERROR] Failed to execute goal org.apache.maven.plugins:maven-compiler-plugin:3.13.0:compile (default-compile) on project maven-demo: Compilation failure";
        let issue = parser.parse_maven_issue_line(line).unwrap();
        assert!(issue.message.contains("Failed to execute goal"));
    }
}
