//! Pytest Output Parser
//! Parsing the output of pytest

use crate::core::{
    Issue, Location, OutputParser, ParseResult, ParsedTestOutput, TestCase,
    TestOutputParser, TestStatus, TestSummary,
};

pub struct PytestParser;

impl PytestParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse pytest failure output format
    /// Example:
    /// test_file.py::test_function - AssertionError: assert 1 == 2
    fn parse_failure_line(&self, line: &str) -> Option<(String, String, Option<Location>)> {
        // Match format: file.py::test_name - ExceptionType: message
        let re = regex::Regex::new(
            r"^(\S+\.py)::(\S+)\s+-\s+(.+)$"
        ).ok()?;

        let caps = re.captures(line)?;
        let file_path = caps.get(1)?.as_str().to_string();
        let test_name = caps.get(2)?.as_str().to_string();
        let error_msg = caps.get(3)?.as_str().to_string();

        // Try to extract location from file_path
        let location = Some(Location::new(file_path.clone()));

        Some((test_name, error_msg, location))
    }

    /// Parse test case line in verbose mode
    /// Example:
    /// test_file.py::test_function PASSED [0.01s]
    /// test_file.py::test_function FAILED [0.01s]
    /// test_file.py::test_function SKIPPED [reason]
    pub fn parse_test_case_line(&self, line: &str) -> Option<TestCase> {
        // Match format: file.py::test_name STATUS [extra]
        let re = regex::Regex::new(
            r"^(\S+\.py)::(\S+)\s+(PASSED|FAILED|SKIPPED|ERROR|XFAIL|XPASS)(?:\s+\[(.+)\])?$"
        ).ok()?;

        let caps = re.captures(line)?;
        let file_path = caps.get(1)?.as_str().to_string();
        let test_name = caps.get(2)?.as_str().to_string();
        let status_str = caps.get(3)?.as_str();
        let extra = caps.get(4).map(|m| m.as_str());

        let status = match status_str {
            "PASSED" => TestStatus::Passed,
            "FAILED" => TestStatus::Failed,
            "ERROR" => TestStatus::Failed,
            "SKIPPED" => TestStatus::Ignored(extra.map(|s| s.to_string())),
            "XFAIL" => TestStatus::Ignored(Some("expected failure".to_string())),
            "XPASS" => TestStatus::Passed,
            _ => return None,
        };

        // Parse execution time if available
        let execution_time = extra.and_then(|e| {
            // Match format: 0.01s
            if e.ends_with('s') && !e.contains(' ') {
                e[..e.len() - 1].parse().ok()
            } else {
                None
            }
        });

        let location = Location::new(file_path);

        let mut test_case = TestCase::new(test_name, status)
            .with_location(location);

        if let Some(time) = execution_time {
            test_case = test_case.with_execution_time(time);
        }

        Some(test_case)
    }

    /// Parse test summary line
    /// Example:
    /// ===================== 5 passed, 2 failed, 1 skipped in 0.05s ======================
    /// ===================== short test summary info ======================
    pub fn parse_test_summary(&self, line: &str) -> Option<TestSummary> {
        // Match summary line with various counts including xfailed/xpassed
        // Format: "8 passed, 1 skipped, 1 xfailed, 1 failed in 0.15s"
        let re = regex::Regex::new(
            r"(\d+)\s+passed.*?(\d+)\s+skipped.*?(\d+)\s+xfailed.*?(\d+)\s+failed.*?in\s+([\d.]+)s"
        ).ok()?;

        if let Some(caps) = re.captures(line) {
            let passed: usize = caps.get(1)?.as_str().parse().ok()?;
            let skipped: usize = caps.get(2)?.as_str().parse().ok()?;
            let xfailed: usize = caps.get(3)?.as_str().parse().ok()?;
            let failed: usize = caps.get(4)?.as_str().parse().ok()?;
            let execution_time: f64 = caps.get(5)?.as_str().parse().ok()?;

            return Some(TestSummary {
                total: passed + failed + skipped + xfailed,
                passed: passed + xfailed,  // xfailed counts as passed
                failed,
                ignored: skipped,
                measured: 0,
                filtered: 0,
                execution_time: Some(execution_time),
            });
        }

        // Match summary line with standard counts
        // Format: "5 passed, 2 failed, 1 skipped in 0.05s"
        let re2 = regex::Regex::new(
            r"(\d+)\s+passed.*?(\d+)\s+failed.*?(\d+)\s+skipped.*?in\s+([\d.]+)s"
        ).ok()?;

        if let Some(caps) = re2.captures(line) {
            let passed: usize = caps.get(1)?.as_str().parse().ok()?;
            let failed: usize = caps.get(2)?.as_str().parse().ok()?;
            let skipped: usize = caps.get(3)?.as_str().parse().ok()?;
            let execution_time: f64 = caps.get(4)?.as_str().parse().ok()?;

            return Some(TestSummary {
                total: passed + failed + skipped,
                passed,
                failed,
                ignored: skipped,
                measured: 0,
                filtered: 0,
                execution_time: Some(execution_time),
            });
        }

        // Alternative format without skipped
        let re3 = regex::Regex::new(
            r"(\d+)\s+passed.*?(\d+)\s+failed.*?in\s+([\d.]+)s"
        ).ok()?;

        if let Some(caps) = re3.captures(line) {
            let passed: usize = caps.get(1)?.as_str().parse().ok()?;
            let failed: usize = caps.get(2)?.as_str().parse().ok()?;
            let execution_time: f64 = caps.get(3)?.as_str().parse().ok()?;

            return Some(TestSummary {
                total: passed + failed,
                passed,
                failed,
                ignored: 0,
                measured: 0,
                filtered: 0,
                execution_time: Some(execution_time),
            });
        }

        // Format with only passed: "5 passed in 0.08s"
        let re4 = regex::Regex::new(
            r"(\d+)\s+passed\s+in\s+([\d.]+)s"
        ).ok()?;

        if let Some(caps) = re4.captures(line) {
            let passed: usize = caps.get(1)?.as_str().parse().ok()?;
            let execution_time: f64 = caps.get(2)?.as_str().parse().ok()?;

            return Some(TestSummary {
                total: passed,
                passed,
                failed: 0,
                ignored: 0,
                measured: 0,
                filtered: 0,
                execution_time: Some(execution_time),
            });
        }

        None
    }

    /// Parse Python traceback to extract error location
    fn parse_traceback(&self, lines: &[&str], start_index: usize) -> Option<(String, Location)> {
        // Look for pattern:
        // File "path/to/file.py", line 10, in function_name
        //   some_code
        // SomeException: message

        let file_re = regex::Regex::new(
            r#"^\s*File\s+"([^"]+)"\s*,\s*line\s+(\d+)\s*,\s*in\s+(.+)$"#
        ).ok()?;

        let mut last_file_location: Option<(String, u32, String)> = None;
        let mut error_message: Option<String> = None;

        for (_i, line) in lines.iter().enumerate().skip(start_index) {
            if let Some(caps) = file_re.captures(line) {
                let file_path = caps.get(1)?.as_str().to_string();
                let line_num: u32 = caps.get(2)?.as_str().parse().ok()?;
                let func_name = caps.get(3)?.as_str().to_string();
                last_file_location = Some((file_path, line_num, func_name));
            } else if line.trim().starts_with("File") {
                // Continue searching
                continue;
            } else if !line.starts_with(' ') && !line.is_empty() && error_message.is_none() {
                // This might be the exception line
                if line.contains(':') {
                    error_message = Some(line.to_string());
                    break;
                }
            }
        }

        if let Some((file_path, line_num, _)) = last_file_location {
            let location = Location::new(file_path).with_line(line_num);
            let message = error_message.unwrap_or_default();
            return Some((message, location));
        }

        None
    }

    /// Parse short test summary info section
    fn parse_short_test_summary(&self, lines: &[&str], start_index: usize) -> Vec<(String, String)> {
        let mut failures = Vec::new();
        let re = regex::Regex::new(r"^(FAILED|ERROR)\s+(\S+::\S+)\s+-\s+(.+)$").ok();

        if re.is_none() {
            return failures;
        }

        let re = re.unwrap();

        for line in lines.iter().skip(start_index + 1) {
            // Stop at separator or empty line
            if line.starts_with("=") || line.trim().is_empty() {
                break;
            }

            if let Some(caps) = re.captures(line) {
                if let (Some(test_name_match), Some(error_msg_match)) = (caps.get(2), caps.get(3)) {
                    let test_name = test_name_match.as_str().to_string();
                    let error_msg = error_msg_match.as_str().to_string();
                    failures.push((test_name, error_msg));
                }
            }
        }

        failures
    }
}

impl Default for PytestParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for PytestParser {
    // Pytest doesn't typically produce compilation-style issues
    // It produces test failures which are handled by TestOutputParser
    fn parse(&self, _output: &str) -> ParseResult<Vec<Issue>> {
        ParseResult::Full(Vec::new())
    }
}

impl TestOutputParser for PytestParser {
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();
        let lines: Vec<&str> = output.lines().collect();

        let mut _in_short_summary = false;
        let mut in_failures_section = false;
        let mut current_failure: Option<(String, Vec<String>)> = None;
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // Detect short test summary section
            if line.contains("short test summary info") {
                _in_short_summary = true;
                let failures = self.parse_short_test_summary(&lines, i);
                for (test_name, error_msg) in failures {
                    // Find and update the failed test with error message
                    if let Some(test) = result.failed_tests.iter_mut().find(|t| {
                        let full_name = format!("{}::{}" , t.location.as_ref().map(|l| l.file_path.clone()).unwrap_or_default(), t.name);
                        full_name == test_name || t.name == test_name.split("::").last().unwrap_or(&test_name)
                    }) {
                        test.failure_details = Some(error_msg);
                    }
                }
                i += 1;
                continue;
            }

            // Detect failures section (detailed traceback)
            if line.contains("FAILURES") || line.contains("Failures") {
                in_failures_section = true;
                i += 1;
                continue;
            }

            // Parse test case lines (verbose mode)
            if let Some(test_case) = self.parse_test_case_line(line) {
                match test_case.status {
                    TestStatus::Passed => result.passed_tests.push(test_case),
                    TestStatus::Failed => result.failed_tests.push(test_case),
                    TestStatus::Ignored(_) => result.ignored_tests.push(test_case),
                }
                i += 1;
                continue;
            }

            // Parse failure lines from short summary (e.g., "test_file.py::test_func - AssertionError: msg")
            if line.contains("::") && line.contains(" - ") {
                if let Some((test_name, error_msg, location)) = self.parse_failure_line(line) {
                    // Check if we already have this test, if not add it
                    if !result.failed_tests.iter().any(|t| t.name == test_name) {
                        let mut test_case = TestCase::new(&test_name, TestStatus::Failed)
                            .with_failure_details(&error_msg);
                        if let Some(loc) = location {
                            test_case = test_case.with_location(loc);
                        }
                        result.failed_tests.push(test_case);
                    }
                }
                i += 1;
                continue;
            }

            // Parse test summary
            if line.contains("passed") && line.contains("in") && line.contains("s") {
                result.test_summary = self.parse_test_summary(line);
            }

            // Collect failure details in failures section
            if in_failures_section {
                if line.starts_with("_") && line.contains("_") {
                    // Start of a new failure detail block
                    if let Some((name, details)) = current_failure.take() {
                        // Save previous failure
                        if let Some(test) = result.failed_tests.iter_mut().find(|t| t.name == name) {
                            test.failure_details = Some(details.join("\n"));
                        }
                    }
                    // Extract test name from header like "____ test_name ____"
                    let test_name = line.trim_matches('_').trim().to_string();
                    current_failure = Some((test_name, Vec::new()));
                } else if let Some((_, ref mut details)) = current_failure {
                    details.push(line.to_string());
                }

                // End of failures section
                if line.starts_with("=") && line.contains("=") {
                    in_failures_section = false;
                    if let Some((name, details)) = current_failure.take() {
                        if let Some(test) = result.failed_tests.iter_mut().find(|t| t.name == name) {
                            test.failure_details = Some(details.join("\n"));
                            // Try to parse traceback for location
                            if let Some((msg, loc)) = self.parse_traceback(&lines, i.saturating_sub(details.len())) {
                                test.location = Some(loc);
                                if test.failure_details.is_none() {
                                    test.failure_details = Some(msg);
                                }
                            }
                        }
                    }
                }
            }

            i += 1;
        }

        // Handle any remaining failure
        if let Some((name, details)) = current_failure {
            if let Some(test) = result.failed_tests.iter_mut().find(|t| t.name == name) {
                test.failure_details = Some(details.join("\n"));
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_test_case_line_passed() {
        let parser = PytestParser::new();
        let line = "test_example.py::test_addition PASSED [0.01s]";
        let test_case = parser.parse_test_case_line(line).unwrap();

        assert_eq!(test_case.name, "test_addition");
        assert!(matches!(test_case.status, TestStatus::Passed));
        assert_eq!(test_case.execution_time, Some(0.01));
    }

    #[test]
    fn test_parse_test_case_line_failed() {
        let parser = PytestParser::new();
        let line = "test_example.py::test_division FAILED [0.02s]";
        let test_case = parser.parse_test_case_line(line).unwrap();

        assert_eq!(test_case.name, "test_division");
        assert!(matches!(test_case.status, TestStatus::Failed));
    }

    #[test]
    fn test_parse_test_case_line_skipped() {
        let parser = PytestParser::new();
        let line = "test_example.py::test_feature SKIPPED [not implemented]";
        let test_case = parser.parse_test_case_line(line).unwrap();

        assert_eq!(test_case.name, "test_feature");
        assert!(matches!(test_case.status, TestStatus::Ignored(Some(_))));
    }

    #[test]
    fn test_parse_test_summary() {
        let parser = PytestParser::new();
        let line = "===================== 5 passed, 2 failed, 1 skipped in 0.05s ======================";
        let summary = parser.parse_test_summary(line).unwrap();

        assert_eq!(summary.total, 8);
        assert_eq!(summary.passed, 5);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.ignored, 1);
        assert_eq!(summary.execution_time, Some(0.05));
    }

    #[test]
    fn test_parse_test_output() {
        let parser = PytestParser::new();
        let output = r#"
test_example.py::test_addition PASSED [0.01s]
test_example.py::test_subtraction PASSED [0.01s]
test_example.py::test_division FAILED [0.02s]
test_example.py::test_multiplication SKIPPED [not ready]

===================== short test summary info ======================
FAILED test_example.py::test_division - AssertionError: assert 1 == 0
================== 2 passed, 1 failed, 1 skipped in 0.05s ==================
"#;

        let result = parser.parse_test_output(output);

        assert_eq!(result.passed_tests.len(), 2);
        assert_eq!(result.failed_tests.len(), 1);
        assert_eq!(result.ignored_tests.len(), 1);
        assert!(result.test_summary.is_some());
    }
}
