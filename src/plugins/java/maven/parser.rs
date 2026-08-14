//! Maven Output Parser
//Parsing the output of Maven compile/test Parsing the output of Maven compile/test

use crate::core::{
    Issue, IssueLevel, Location, OutputParser, ParseResult, ParsedTestOutput, TestCase,
    TestOutputParser, TestStatus, TestSummary,
};

use std::collections::HashSet;

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

        // Try parsing the one-line format
        if let Some(issue) = self.parse_maven_issue_line(line) {
            return (Some(issue), start_index + 1);
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

        // Maven prints the same compile errors twice (once in the
        // "COMPILATION ERROR" block, once in the error list after "Failed to
        // execute goal"). Deduplicate exact duplicates so each real
        // diagnostic is reported once.
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

/// True when the message is the surefire "There are test failures." goal
/// failure line. That is a test-stage outcome, not a compile problem, so the
/// test report must not surface it as a compile issue on pom.xml.
fn is_test_failure_goal(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("failed to execute goal") && lower.contains("test failure")
}

/// Normalize a surefire failure name to `Class::method`:
/// - JUnit5 event line:   "testFormatDate(com.example.UtilsTest)" -> "com.example.UtilsTest::testFormatDate"
/// - JUnit4 event line:   "com.example.AppTest.testFailingCase"  -> "com.example.AppTest::testFailingCase"
/// - "Failures:" block:   "UtilsTest.testFormatDate"             -> "UtilsTest::testFormatDate"
fn normalize_test_name(raw: &str) -> String {
    if let Some(open) = raw.find('(') {
        if raw.ends_with(')') {
            let method = raw[..open].trim();
            let class = raw[open + 1..raw.len() - 1].trim();
            if !method.is_empty() && !class.is_empty() {
                return format!("{}::{}", class, method);
            }
        }
    }
    if let Some(dot) = raw.rfind('.') {
        let class = &raw[..dot];
        let method = &raw[dot + 1..];
        if !class.is_empty() && !method.is_empty() {
            return format!("{}::{}", class, method);
        }
    }
    raw.to_string()
}

/// True when two normalized test names refer to the same test, comparing the
/// short class name (e.g. "UtilsTest" == "com.example.UtilsTest") and method.
fn same_test(a: &str, b: &str) -> bool {
    let a_parts: Vec<&str> = a.split("::").collect();
    let b_parts: Vec<&str> = b.split("::").collect();
    if a_parts.len() < 2 || b_parts.len() < 2 {
        return a == b;
    }
    let a_class = a_parts[..a_parts.len() - 1].join("::");
    let b_class = b_parts[..b_parts.len() - 1].join("::");
    let a_short = a_class.rsplit('.').next().unwrap_or(&a_class);
    let b_short = b_class.rsplit('.').next().unwrap_or(&b_class);
    let a_method = a_parts.last().unwrap_or(&"");
    let b_method = b_parts.last().unwrap_or(&"");
    a_short == b_short && a_method == b_method
}

impl TestOutputParser for MavenParser {
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();
        result.compile_issues = <Self as OutputParser>::parse(self, output)
            .data_or_default_owned()
            .into_iter()
            .filter(|i| !is_test_failure_goal(&i.message))
            .collect();

        let lines: Vec<&str> = output.lines().collect();
        let mut tests_run: usize = 0;
        let mut failures: usize = 0;
        let mut errors: usize = 0;
        let mut skipped: usize = 0;
        let mut exec_time: Option<f64> = None;
        let mut final_summary_seen = false;

        // Per-class summary lines carry a " - in Class" / "-- in Class" suffix;
        // the final aggregate line after "Results:" does not. Prefer the final
        // line, falling back to the last per-class line seen so far.
        let summary_re = regex::Regex::new(
            r"Tests run:\s*(\d+),\s*Failures:\s*(\d+),\s*Errors:\s*(\d+),\s*Skipped:\s*(\d+)",
        )
        .ok();
        let time_re = regex::Regex::new(r"Time elapsed:\s*([\d.]+)\s*s").ok();
        let running_re = regex::Regex::new(r"^\[INFO\]\s*Running\s+([\w.$]+)").ok();
        let method_re = regex::Regex::new(
            r"^\[ERROR\]\s*([\w.$]+(?:\([\w.$]+\))?)\s+(?:--\s+)?Time elapsed:\s*([\d.]+)\s*s?\s*<<< FAILURE!",
        )
        .ok();
        let failure_entry_re =
            regex::Regex::new(r"^\[ERROR\]\s+([\w.$]+):(\d+)\s+(.+)").ok();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];

            // Test class start: "[INFO] Running com.example.AppTest"
            if let Some(caps) = running_re.as_ref().and_then(|re| re.captures(line)) {
                let class_name = caps
                    .get(1)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default();
                // Look ahead for FAILURE! markers up to the next class, the
                // "Results:" block, or the build outcome to decide whether the
                // whole class failed.
                let mut j = i + 1;
                let mut class_failed = false;
                let mut has_method_detail = false;
                while j < lines.len() {
                    let nl = lines[j];
                    if running_re
                        .as_ref()
                        .map(|re| re.is_match(nl))
                        .unwrap_or(false)
                        || nl.contains("Results:")
                        || nl.contains("BUILD FAILURE")
                        || nl.contains("BUILD SUCCESS")
                    {
                        break;
                    }
                    if nl.contains("<<< FAILURE!") {
                        class_failed = true;
                    }
                    if method_re
                        .as_ref()
                        .map(|re| re.is_match(nl))
                        .unwrap_or(false)
                    {
                        has_method_detail = true;
                    }
                    j += 1;
                }
                if !class_failed {
                    result.passed_tests.push(TestCase {
                        name: class_name,
                        status: TestStatus::Passed,
                        location: None,
                        failure_details: None,
                        execution_time: None,
                    });
                } else if !has_method_detail {
                    // No per-method failure line available; record the class.
                    result.failed_tests.push(TestCase {
                        name: class_name,
                        status: TestStatus::Failed,
                        location: None,
                        failure_details: None,
                        execution_time: None,
                    });
                }
                i += 1;
                continue;
            }

            // Failed method line:
            //   JUnit5: "[ERROR] testFormatDate(com.example.UtilsTest)  Time elapsed: 0.012 s  <<< FAILURE!"
            //   JUnit4: "[ERROR] com.example.AppTest.testFailingCase -- Time elapsed: 0.011 s <<< FAILURE!"
            if let Some(caps) = method_re.as_ref().and_then(|re| re.captures(line)) {
                let raw = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
                let time: Option<f64> = caps.get(2).and_then(|m| m.as_str().parse().ok());
                let name = normalize_test_name(raw);

                // Collect failure details: un-prefixed lines following the
                // failure line (exception message + stack trace), stopping at
                // the next Maven-logged line, event, or empty line.
                let mut details = Vec::new();
                let mut j = i + 1;
                while j < lines.len() {
                    let nl = lines[j];
                    let t = nl.trim();
                    if t.is_empty() {
                        break;
                    }
                    if t.starts_with("[ERROR]")
                        || t.starts_with("[INFO]")
                        || t.starts_with("[WARNING]")
                    {
                        break;
                    }
                    if method_re
                        .as_ref()
                        .map(|re| re.is_match(nl))
                        .unwrap_or(false)
                        || nl.contains("Tests run:") {
                        break;
                    }
                    details.push(nl.to_string());
                    j += 1;
                }

                // Merge with an existing entry (e.g. from the "Failures:"
                // block) matching the same test.
                let merged_details = details.join("\n");
                let existing = result
                    .failed_tests
                    .iter_mut()
                    .find(|t| same_test(&t.name, &name));
                match existing {
                    Some(tc) => {
                        let mut merged = merged_details;
                        if let Some(ref prev) = tc.failure_details {
                            merged = format!("{}\n{}", prev, merged);
                        }
                        tc.failure_details = if merged.is_empty() { None } else { Some(merged) };
                        tc.execution_time = tc.execution_time.or(time);
                    }
                    None => {
                        result.failed_tests.push(TestCase {
                            name,
                            status: TestStatus::Failed,
                            location: None,
                            failure_details: if merged_details.is_empty() {
                                None
                            } else {
                                Some(merged_details)
                            },
                            execution_time: time,
                        });
                    }
                }
                i += 1;
                continue;
            }

            // "Failures:" block entry:
            //   "[ERROR]   UtilsTest.testFormatDate:17 expected: <true> but was: <false>"
            if let Some(caps) = failure_entry_re
                .as_ref()
                .and_then(|re| re.captures(line.trim()))
            {
                let raw_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let line_no = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let msg = caps.get(3).map(|m| m.as_str().trim()).unwrap_or("");
                // Keep the full "Class.method:line message" entry so the
                // merged details stay self-describing.
                let full_entry = if line_no.is_empty() {
                    msg.to_string()
                } else {
                    format!("{}:{} {}", raw_name, line_no, msg)
                };
                let name = normalize_test_name(raw_name);
                let existing = result
                    .failed_tests
                    .iter_mut()
                    .find(|t| same_test(&t.name, &name));
                match existing {
                    Some(tc) => {
                        let mut merged = full_entry.clone();
                        if let Some(ref prev) = tc.failure_details {
                            merged = format!("{}\n{}", prev, merged);
                        }
                        tc.failure_details = Some(merged);
                    }
                    None => {
                        result.failed_tests.push(TestCase {
                            name,
                            status: TestStatus::Failed,
                            location: None,
                            failure_details: if full_entry.is_empty() {
                                None
                            } else {
                                Some(full_entry)
                            },
                            execution_time: None,
                        });
                    }
                }
                i += 1;
                continue;
            }

            // Summary lines: "Tests run: 9, Failures: 1, Errors: 0, Skipped: 0"
            if line.contains("Tests run:") {
                if let Some(re) = &summary_re {
                    if let Some(caps) = re.captures(line) {
                        let total = caps
                            .get(1)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(0);
                        let fail = caps
                            .get(2)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(0);
                        let err = caps
                            .get(3)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(0);
                        let skip = caps
                            .get(4)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(0);
                        let time = time_re
                            .as_ref()
                            .and_then(|re| re.captures(line))
                            .and_then(|c| c.get(1))
                            .and_then(|m| m.as_str().parse().ok());
                        let is_final = !line.contains(" in ");
                        if is_final || !final_summary_seen {
                            tests_run = total;
                            failures = fail;
                            errors = err;
                            skipped = skip;
                            // The final aggregate line usually omits the
                            // "Time elapsed:" part; keep the last value seen.
                            if let Some(t) = time {
                                exec_time = Some(t);
                            }
                            if is_final {
                                final_summary_seen = true;
                            }
                        }
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
            execution_time: exec_time,
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
        // Real surefire (JUnit 5) output: per-class "Tests run:" lines carry
        // a " - in Class" suffix, failed methods carry "<<< FAILURE!", and the
        // final aggregate line follows the "Results:" block.
        let parser = MavenParser::new();
        let output = "\
[INFO] Running com.example.AppTest
[INFO] Tests run: 5, Failures: 0, Errors: 0, Skipped: 0, Time elapsed: 0.045 s - in com.example.AppTest
[INFO] Running com.example.UtilsTest
[ERROR] Tests run: 4, Failures: 1, Errors: 0, Skipped: 0, Time elapsed: 0.023 s <<< FAILURE! - in com.example.UtilsTest
[ERROR] testFormatDate(com.example.UtilsTest)  Time elapsed: 0.012 s  <<< FAILURE!
org.opentest4j.AssertionFailedError: expected: <true> but was: <false>
\tat com.example.UtilsTest.testFormatDate(UtilsTest.java:17)
[ERROR] 
[INFO] 
[INFO] Results:
[INFO] 
[ERROR] Failures: 
[ERROR]   UtilsTest.testFormatDate:17 expected: <true> but was: <false>
[INFO] 
[ERROR] Tests run: 9, Failures: 1, Errors: 0, Skipped: 0";
        let test_result = parser.parse_test_output(output);

        // Method-level failed test with normalized name and details merged.
        assert_eq!(test_result.failed_tests.len(), 1);
        let failed = &test_result.failed_tests[0];
        assert_eq!(failed.name, "com.example.UtilsTest::testFormatDate");
        assert!(failed.failure_details.is_some());
        let details = failed.failure_details.as_ref().unwrap();
        assert!(details.contains("AssertionFailedError"));
        assert!(details.contains("UtilsTest.testFormatDate:17 expected: <true> but was: <false>"));
        assert_eq!(failed.execution_time, Some(0.012));

        // Passing class recorded at class level.
        assert_eq!(test_result.passed_tests.len(), 1);
        assert_eq!(test_result.passed_tests[0].name, "com.example.AppTest");

        let summary = test_result.test_summary.unwrap();
        assert_eq!(summary.total, 9);
        assert_eq!(summary.passed, 8);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.ignored, 0);
        // The final aggregate line omits "Time elapsed:"; the value from the
        // last per-class summary line is kept.
        assert_eq!(summary.execution_time, Some(0.023));
    }

    #[test]
    fn test_parse_test_output_junit4() {
        // JUnit 4 surefire output: fully-qualified "Class.method -- Time
        // elapsed" failure lines and no "Failures:" block.
        let parser = MavenParser::new();
        let output = "\
[INFO] Running com.example.AppTest
[ERROR] Tests run: 2, Failures: 1, Errors: 0, Skipped: 0, Time elapsed: 0.121 s <<< FAILURE! -- in com.example.AppTest
[ERROR] com.example.AppTest.testFailingCase -- Time elapsed: 0.011 s <<< FAILURE!
org.junit.ComparisonFailure: expected:<Hello[ World]> but was:<Hello[]>
\tat org.junit.Assert.assertEquals(Assert.java:117)
\tat com.example.AppTest.testFailingCase(AppTest.java:21)
[ERROR] Tests run: 2, Failures: 1, Errors: 0, Skipped: 0";
        let test_result = parser.parse_test_output(output);

        assert_eq!(test_result.failed_tests.len(), 1);
        let failed = &test_result.failed_tests[0];
        assert_eq!(failed.name, "com.example.AppTest::testFailingCase");
        assert_eq!(failed.execution_time, Some(0.011));
        let details = failed.failure_details.as_ref().unwrap();
        assert!(details.contains("ComparisonFailure"));
        assert!(details.contains("at com.example.AppTest.testFailingCase"));

        let summary = test_result.test_summary.unwrap();
        assert_eq!(summary.total, 2);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.execution_time, Some(0.121));
    }

    #[test]
    fn test_parse_test_output_filters_test_failure_goal() {
        // "Failed to execute goal ... There are test failures." is a test
        // outcome, not a compile issue; it must not surface in compile_issues.
        let parser = MavenParser::new();
        let output = "\
[INFO] Running com.example.AppTest
[ERROR] Tests run: 2, Failures: 1, Errors: 0, Skipped: 0 <<< FAILURE! - in com.example.AppTest
[ERROR] com.example.AppTest.testFailingCase -- Time elapsed: 0.011 s <<< FAILURE!
[ERROR] Tests run: 2, Failures: 1, Errors: 0, Skipped: 0
[ERROR] Failed to execute goal org.apache.maven.plugins:maven-surefire-plugin:3.2.5:test (default-test) on project maven-tests: There are test failures.
[ERROR] Please refer to target/surefire-reports for the individual test results.
[ERROR] -> [Help 1]";
        let test_result = parser.parse_test_output(output);
        assert!(
            test_result.compile_issues.is_empty(),
            "test-failure goal must not be reported as a compile issue: {:?}",
            test_result.compile_issues
        );
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

    #[test]
    fn test_deduplicates_duplicate_compile_errors() {
        // Maven prints the same compile errors twice (once in the
        // "COMPILATION ERROR" block, once in the error list after "Failed to
        // execute goal") — only one issue per diagnostic expected.
        let parser = MavenParser::new();
        let output = "\
[ERROR] COMPILATION ERROR :
[ERROR] /tmp/proj/src/main/java/com/example/App.java:[9,28] cannot find symbol
  symbol:   variable undefinedVar
  location: class com.example.App
[ERROR] /tmp/proj/src/main/java/com/example/App.java:[12,26] cannot find symbol
  symbol:   method add(int,int)
  location: class java.lang.Math
[INFO] 2 errors
[ERROR] Failed to execute goal org.apache.maven.plugins:maven-compiler-plugin:3.13.0:compile (default-compile) on project maven-demo: Compilation failure: Compilation failure:
[ERROR] /tmp/proj/src/main/java/com/example/App.java:[9,28] cannot find symbol
[ERROR]   symbol:   variable undefinedVar
[ERROR]   location: class com.example.App
[ERROR] /tmp/proj/src/main/java/com/example/App.java:[12,26] cannot find symbol
[ERROR]   symbol:   method add(int,int)
[ERROR]   location: class java.lang.Math
[ERROR] -> [Help 1]";
        let issues = parser.parse(output).data_or_default_owned();

        // 2 real compile errors + 1 module-level "Failed to execute goal".
        assert_eq!(issues.len(), 3, "duplicate errors must be deduplicated");
        let file_errors: Vec<_> = issues
            .iter()
            .filter(|i| i.location.file_path.contains("App.java"))
            .collect();
        assert_eq!(file_errors.len(), 2);
        assert_eq!(file_errors[0].location.line_number, Some(9));
        assert_eq!(file_errors[0].location.column_number, Some(28));
        assert_eq!(file_errors[1].location.line_number, Some(12));
        assert_eq!(file_errors[1].location.column_number, Some(26));
        assert!(issues
            .iter()
            .any(|i| i.location.file_path == "pom.xml" && i.message.contains("Failed to execute goal")));
    }
}
