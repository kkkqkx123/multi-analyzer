//! Gradle Output Parser
//! Parsing the output of Gradle compile/test

use crate::core::{
    Issue, IssueLevel, Location, OutputParser, ParseResult, ParsedTestOutput, TestCase,
    TestOutputParser, TestStatus, TestSummary,
};

use std::collections::HashSet;
use std::sync::OnceLock;

/// Shared regex for Gradle test event lines. Supports both the Gradle 8+
/// two-part format ("com.example.MyTest > testMethod PASSED") and the legacy
/// three-part format ("com.example.MyTest > testMethod > FAILED"), with an
/// optional execution-time suffix like "PASSED (0.05s)".
fn test_event_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^(.+?) > (.+?)(?: > )?(PASSED|FAILED|SKIPPED)(?: \((\d+(?:\.\d+)?)s\))?\s*$")
            .expect("valid test event regex")
    })
}

pub struct GradleParser;

impl GradleParser {
    pub fn new() -> Self {
        Self
    }

    /// Detect Gradle test event lines like:
    ///   Gradle 8+:  "com.example.MyTest > testMethod PASSED" / "FAILED (0.05s)"
    ///   Legacy:     "com.example.MyTest > testMethod > FAILED"
    /// These are test results, not compile diagnostics, and must never be
    /// reported as issues on build.gradle.
    fn is_test_event_line(line: &str) -> bool {
        let trimmed = line.trim();
        if !trimmed.contains(" > ") {
            return false;
        }
        let re = test_event_regex();
        re.is_match(trimmed)
    }

    /// Parsing Gradle Compile Error/Warning Lines
    /// Format: /path/to/File.java:10: error: message
    /// Format: /path/to/File.java:20: warning: message
    fn parse_gradle_issue_line(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();

        // Test event lines ("AppTest > testFailingCase FAILED") are test
        // results, not compile diagnostics — do not report them as issues.
        if Self::is_test_event_line(trimmed) {
            return None;
        }

        // Check for error/warning lines with file path
        // Format: /path/to/File.java:10: error: message
        if let Some((file_path, line_num, level, message)) = self.parse_file_location(trimmed) {
            let location = Location::new(file_path).with_line(line_num).with_column(0);

            return Some(Issue::new(level, message, location));
        }

        // Task/build summary lines are not diagnostics — skip them so they are
        // not reported as spurious issues.
        //   > Task :compileJava FAILED
        //   BUILD FAILED in 519ms
        if trimmed.starts_with("> Task") {
            return None;
        }
        if trimmed.contains("FAILED") {
            if trimmed.contains("BUILD FAILED") || trimmed.contains("BUILD FAILURE") {
                return None;
            }
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

        // Continuation lines (`symbol:`, `location:`) belong to the preceding
        // diagnostic. Returning the previous issue here would emit the same
        // issue a second/third time, so just skip them.
        // Format:  symbol: variable x
        // Format:  location: class com.example.MyClass
        if line.trim().starts_with("symbol:") || line.trim().starts_with("location:") {
            return (None, start_index + 1);
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

        // Gradle prints the same compile errors twice (once in the compiler
        // output, once in the "What went wrong" section). Deduplicate exact
        // duplicates so each real diagnostic is reported once.
        let mut seen: HashSet<(IssueLevel, String, Option<u32>, Option<u32>, String)> =
            HashSet::new();
        issues.retain(|i| {
            seen.insert((
                i.level.clone(),
                i.location.file_path.clone(),
                i.location.line_number,
                i.location.column_number,
                i.message.clone(),
            ))
        });

        ParseResult::Full(issues)
    }
}

impl TestOutputParser for GradleParser {
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();
        result.compile_issues = <Self as OutputParser>::parse(self, output).data_or_default_owned();

        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;
        let mut passed: usize = 0;
        let mut failed: usize = 0;
        let mut skipped: usize = 0;

        let summary_re = regex::Regex::new(
            r"PASSED:\s*(\d+),\s*FAILED:\s*(\d+),\s*SKIPPED:\s*(\d+)",
        )
        .ok();
        let event_re = test_event_regex();

        while i < lines.len() {
            let line = lines[i];

            // Parse test event lines. Real Gradle output uses the two-part
            // format ("com.example.MyTest > testMethod PASSED"), while the
            // legacy three-part format ("... > testMethod > FAILED") is also
            // supported for compatibility.
            if let Some(caps) = event_re.captures(line) {
                let class_name = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
                let method_name = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                let status = caps.get(3).map(|m| m.as_str()).unwrap_or("");
                let exec_time: Option<f64> = caps
                    .get(4)
                    .and_then(|m| m.as_str().parse().ok());
                let full_name = format!("{}::{}", class_name, method_name);

                match status {
                    "PASSED" => {
                        result.passed_tests.push(TestCase {
                            name: full_name,
                            status: TestStatus::Passed,
                            location: None,
                            failure_details: None,
                            execution_time: exec_time,
                        });
                        passed += 1;
                    }
                    "FAILED" => {
                        // Collect failure details (following lines up to the
                        // next test event or an empty line).
                        let mut details = Vec::new();
                        let mut j = i + 1;
                        while j < lines.len() {
                            let next_line = lines[j];
                            if event_re.is_match(next_line) {
                                break;
                            }
                            if next_line.trim().is_empty() {
                                j += 1;
                                continue;
                            }
                            details.push(next_line.to_string());
                            j += 1;
                        }

                        result.failed_tests.push(TestCase {
                            name: full_name.clone(),
                            status: TestStatus::Failed,
                            location: None,
                            failure_details: if details.is_empty() {
                                None
                            } else {
                                Some(details.join("\n"))
                            },
                            execution_time: exec_time,
                        });
                        failed += 1;
                    }
                    _ => {
                        // SKIPPED
                        result.ignored_tests.push(TestCase {
                            name: full_name,
                            status: TestStatus::Ignored(None),
                            location: None,
                            failure_details: None,
                            execution_time: exec_time,
                        });
                        skipped += 1;
                    }
                }

                i += 1;
                continue;
            }

            // Parse summary line: "PASSED: 100, FAILED: 1, SKIPPED: 2"
            if line.contains("PASSED:") && line.contains("FAILED:") {
                if let Some(re) = &summary_re {
                    if let Some(caps) = re.captures(line) {
                        passed = caps
                            .get(1)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(passed);
                        failed = caps
                            .get(2)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(failed);
                        skipped = caps
                            .get(3)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(skipped);
                    }
                }
            }

            i += 1;
        }

        result.test_summary = Some(TestSummary {
            total: passed + failed + skipped,
            passed,
            failed,
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

    #[test]
    fn test_parse_gradle_issue_line_no_match() {
        let parser = GradleParser::new();
        assert!(parser.parse_gradle_issue_line("some random log").is_none());
        assert!(parser.parse_gradle_issue_line("BUILD SUCCESSFUL").is_none());
    }

    #[test]
    fn test_parse_via_trait_empty() {
        let parser = GradleParser::new();
        let result = parser.parse("");
        assert!(result.is_full());
        assert!(result.data().unwrap().is_empty());
    }

    #[test]
    fn test_parse_via_trait_with_issues() {
        let parser = GradleParser::new();
        let output = "/src/Main.java:10: error: cannot find symbol\n\
                      /src/Util.java:20: warning: unchecked conversion";
        let result = parser.parse(output);
        assert!(result.is_full());
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].level, IssueLevel::Error);
        assert_eq!(issues[1].level, IssueLevel::Warning);
    }

    #[test]
    fn test_parse_file_location() {
        let parser = GradleParser::new();
        let (file, line, _level, msg) = parser.parse_file_location("/path/to/File.java:10: error: cannot find symbol").unwrap();
        assert_eq!(file, "/path/to/File.java");
        assert_eq!(line, 10);
        assert!(msg.contains("cannot find symbol"));
    }

    #[test]
    fn test_parse_file_location_no_match() {
        let parser = GradleParser::new();
        assert!(parser.parse_file_location("BUILD SUCCESSFUL in 1s").is_none());
    }

    #[test]
    fn test_parse_test_output() {
        let parser = GradleParser::new();
        let output = "\
com.example.TestSuite > testMethod > FAILED
    org.junit.ComparisonFailure: expected:<X> but was:<Y>
com.example.TestSuite > testOtherMethod > FAILED
    java.lang.NullPointerException
PASSED: 3, FAILED: 2, SKIPPED: 0";
        let test_result = parser.parse_test_output(output);
        assert_eq!(test_result.failed_tests.len(), 2);
        assert!(test_result.failed_tests[0].name.contains("testMethod"));
        assert!(test_result.failed_tests[1].name.contains("testOtherMethod"));
    }

    #[test]
    fn test_no_duplicate_from_continuation_lines() {
        let parser = GradleParser::new();
        // The symbol:/location: continuation lines must NOT create extra issues.
        let output = "\
/workspace/proj/src/Main.java:8: error: cannot find symbol
        System.out.println(multiply(total, 4));
                           ^
  symbol:   method multiply(int,int)
  location: class Main
BUILD FAILED in 519ms";
        let result = parser.parse(output);
        let issues = result.data().unwrap();
        // 1 real compile error only — no duplicates, no BUILD FAILED noise.
        assert_eq!(issues.len(), 1, "continuation/summary lines must not duplicate");
        assert_eq!(issues[0].message, "cannot find symbol");
    }

    #[test]
    fn test_skips_task_and_build_failed_summaries() {
        let parser = GradleParser::new();
        assert!(parser.parse_gradle_issue_line("> Task :compileJava FAILED").is_none());
        assert!(parser
            .parse_gradle_issue_line("BUILD FAILED in 519ms")
            .is_none());
    }

    #[test]
    fn test_parse_test_output_gradle8_two_part_format() {
        // Real Gradle 8 testLogging output uses the two-part format:
        //   "com.example.AppTest > testMethod PASSED|FAILED|SKIPPED"
        let parser = GradleParser::new();
        let output = "\
AppTest > testGreet PASSED
AppTest > testFailingCase FAILED
    org.junit.ComparisonFailure: expected:<Hello[ World]> but was:<Hello[]>
        at com.example.AppTest.testFailingCase(AppTest.java:21)
AppTest > testSkipped SKIPPED
2 tests completed, 1 failed";
        let result = parser.parse_test_output(output);

        assert_eq!(result.passed_tests.len(), 1);
        assert_eq!(result.failed_tests.len(), 1);
        assert_eq!(result.ignored_tests.len(), 1);

        assert_eq!(result.passed_tests[0].name, "AppTest::testGreet");
        assert_eq!(result.failed_tests[0].name, "AppTest::testFailingCase");
        let details = result.failed_tests[0]
            .failure_details
            .as_ref()
            .expect("failure details should be captured");
        assert!(details.contains("ComparisonFailure"));
        assert!(details.contains("AppTest.java:21"));

        let summary = result.test_summary.unwrap();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.ignored, 1);
    }

    #[test]
    fn test_parse_test_output_with_time_suffix() {
        // Gradle emits execution times as "PASSED (0.05s)" when enabled.
        let parser = GradleParser::new();
        let output = "\
com.example.AppTest > testFast PASSED (0.05s)
com.example.AppTest > testSlow FAILED (1.234s)";
        let result = parser.parse_test_output(output);

        assert_eq!(result.passed_tests.len(), 1);
        assert_eq!(result.failed_tests.len(), 1);
        assert_eq!(result.passed_tests[0].execution_time, Some(0.05));
        assert_eq!(result.failed_tests[0].execution_time, Some(1.234));
        assert_eq!(result.passed_tests[0].name, "com.example.AppTest::testFast");
        assert_eq!(result.failed_tests[0].name, "com.example.AppTest::testSlow");
    }

    #[test]
    fn test_parse_test_output_legacy_three_part_format() {
        // Legacy format keeps working: "Class > method > FAILED"
        let parser = GradleParser::new();
        let output = "\
com.example.TestSuite > testMethod > FAILED
    org.junit.ComparisonFailure: expected:<X> but was:<Y>
com.example.TestSuite > testOtherMethod > PASSED
PASSED: 1, FAILED: 1, SKIPPED: 0";
        let result = parser.parse_test_output(output);

        assert_eq!(result.failed_tests.len(), 1);
        assert_eq!(result.failed_tests[0].name, "com.example.TestSuite::testMethod");
        assert_eq!(result.passed_tests.len(), 1);
        assert_eq!(result.passed_tests[0].name, "com.example.TestSuite::testOtherMethod");

        let summary = result.test_summary.unwrap();
        assert_eq!(summary.total, 2);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn test_test_event_lines_not_reported_as_compile_issues() {
        // Test event lines ("AppTest > testFailingCase FAILED") must never be
        // reported as compile issues on build.gradle.
        let parser = GradleParser::new();
        let output = "\
AppTest > testGreet PASSED
AppTest > testFailingCase FAILED
    org.junit.ComparisonFailure: expected:<Hello[ World]> but was:<Hello[]>
2 tests completed, 1 failed
FAILURE: Build failed with an exception.

* What went wrong:
Execution failed for task ':test'.";
        let issues = parser.parse(output).data_or_default_owned();
        assert!(
            issues.is_empty(),
            "test event lines must not be compile issues, got: {:?}",
            issues
        );
    }

    #[test]
    fn test_deduplicates_duplicate_compile_errors() {
        // Gradle prints the same compile errors twice (compiler output and the
        // "What went wrong" section) — only one issue per diagnostic expected.
        let parser = GradleParser::new();
        let output = "\
/tmp/proj/src/main/java/com/example/App.java:9: error: cannot find symbol
        System.out.println(undefinedVar);
                           ^
  symbol:   variable undefinedVar
  location: class App
/tmp/proj/src/main/java/com/example/App.java:12: error: cannot find symbol
        int result = Math.add(1, 2);
                         ^
  symbol:   method add(int,int)
  location: class Math
2 errors

FAILURE: Build failed with an exception.

* What went wrong:
Execution failed for task ':compileJava'.
> Compilation failed; see the compiler output below.
  /tmp/proj/src/main/java/com/example/App.java:9: error: cannot find symbol
          System.out.println(undefinedVar);
                             ^
    symbol:   variable undefinedVar
    location: class App
  /tmp/proj/src/main/java/com/example/App.java:12: error: cannot find symbol
          int result = Math.add(1, 2);
                           ^
    symbol:   method add(int,int)
    location: class Math
  2 errors";
        let issues = parser.parse(output).data_or_default_owned();
        assert_eq!(issues.len(), 2, "duplicate errors must be deduplicated");
        assert_eq!(issues[0].location.line_number, Some(9));
        assert_eq!(issues[1].location.line_number, Some(12));
    }
}
