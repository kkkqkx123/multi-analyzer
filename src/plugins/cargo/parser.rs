//! Cargo Output Parser
//Parsing the output of cargo check/clippy/test Parsing output from cargo check/clippy/test

use crate::core::{
    BaseParser, Issue, IssueLevel, Location, OutputParser, ParseResult, ParsedTestOutput,
    TestCase, TestOutputParser, TestStatus, TestSummary,
};

pub struct CargoParser {
    base: BaseParser,
}

impl CargoParser {
    pub fn new() -> Self {
        Self {
            base: BaseParser::new(),
        }
    }

    /// Extract package name from file path
    /// Cargo workspace structure: crates/package-name/src/... or packages/package-name/src/...
    fn extract_package_from_path(&self, file_path: &str) -> Option<String> {
        // Normalize path separators
        let normalized = file_path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').collect();
        
        // Look for common workspace directory names
        for i in 0..parts.len() {
            if (parts[i] == "crates" || parts[i] == "packages" || parts[i] == "members") 
                && i + 1 < parts.len() {
                return Some(parts[i + 1].to_string());
            }
        }
        
        None
    }

    fn parse_single_line(&self, line: &str) -> Option<Issue> {
        let parts: Vec<&str> = line.splitn(5, ':').collect();
        if parts.len() < 5 {
            return None;
        }

        let file_path = parts[0].trim();
        let line_num = parts[1].trim().parse::<u32>().ok()?;
        let col_num = parts[2].trim().parse::<u32>().ok()?;
        let error_type = parts[3].trim();
        let description = parts[4].trim();

        let level = if error_type.starts_with("error") {
            IssueLevel::Error
        } else if error_type.starts_with("warning") {
            IssueLevel::Warning
        } else {
            return None;
        };

        let location = Location::new(file_path.to_string())
            .with_line(line_num)
            .with_column(col_num);

        let mut issue = Issue::new(level, description.to_string(), location);
        
        // Extract package name from file path
        if let Some(package) = self.extract_package_from_path(file_path) {
            issue = issue.with_package(package);
        }
        
        if let Some(code) = self.base.extract_error_code(error_type) {
            issue = issue.with_code(code);
        }

        Some(issue)
    }

    fn parse_multiline_error(
        &self,
        lines: &[String],
        start_index: usize,
    ) -> (Option<Issue>, usize) {
        if start_index >= lines.len() {
            return (None, start_index);
        }

        let line = &lines[start_index];

        let (level, desc) = if let Some(rest) = line.strip_prefix("error:") {
            (IssueLevel::Error, rest.trim())
        } else if let Some(rest) = line.strip_prefix("warning:") {
            (IssueLevel::Warning, rest.trim())
        } else {
            return (None, start_index + 1);
        };

        if start_index + 1 < lines.len() {
            let next_line = &lines[start_index + 1];
            let trimmed = next_line.trim();

            if let Some(location_part) = trimmed.strip_prefix("-->") {
                let location_part = location_part.trim();

                if let Some(location) = self.parse_cargo_location(location_part) {
                    // Extract package name from file path before moving location
                    let package = self.extract_package_from_path(&location.file_path);
                    
                    let mut issue = Issue::new(level, desc.to_string(), location);

                    // Add package information
                    if let Some(pkg) = package {
                        issue = issue.with_package(pkg);
                    }

                    if let Some(code) = self.base.extract_error_code(desc) {
                        issue = issue.with_code(code);
                    }

                    return (Some(issue), start_index + 2);
                }
            }
        }

        (None, start_index + 1)
    }

    fn parse_cargo_location(&self, location_str: &str) -> Option<Location> {
        let mut colon_positions = Vec::new();
        for (i, c) in location_str.char_indices() {
            if c == ':' {
                colon_positions.push(i);
            }
        }

        if colon_positions.len() >= 2 {
            let last_colon = colon_positions[colon_positions.len() - 1];
            let second_last_colon = colon_positions[colon_positions.len() - 2];

            let col_part = &location_str[last_colon + 1..];
            let line_part = &location_str[second_last_colon + 1..last_colon];
            let file_part = &location_str[..second_last_colon];

            let line_num = line_part.parse::<u32>().ok()?;
            let col_num = col_part.parse::<u32>().ok()?;

            Some(
                Location::new(file_part.trim().to_string())
                    .with_line(line_num)
                    .with_column(col_num),
            )
        } else {
            None
        }
    }
}

impl Default for CargoParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for CargoParser {
    // Custom parse implementation for Cargo output
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let lines: Vec<String> = output.lines().map(|s| s.to_string()).collect();
        let mut issues = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            let (issue, new_index) = self.parse_multiline_error(&lines, i);

            if let Some(issue) = issue {
                issues.push(issue);
                i = new_index;
            } else {
                if let Some(issue) = self.parse_single_line(&lines[i]) {
                    issues.push(issue);
                }
                i += 1;
            }
        }

        ParseResult::Full(issues)
    }
}

impl TestOutputParser for CargoParser {
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();

        // 1. Reuse of existing logic to resolve compilation issues
        result.compile_issues = <Self as OutputParser>::parse(self, output).data_or_default_owned();

        // 2. Parsing test execution results
        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;
        let mut in_failures_section = false;
        let mut current_failure: Option<(String, Vec<String>)> = None;

        while i < lines.len() {
            let line = lines[i];

            // Parsing test case line: "test <name> ... <result>"
            if let Some(test_case) = self.parse_test_case_line(line) {
                match test_case.status {
                    TestStatus::Passed => result.passed_tests.push(test_case),
                    TestStatus::Failed => {
                        // Failure to fill in later details
                        result.failed_tests.push(test_case);
                    }
                    TestStatus::Ignored(_) => result.ignored_tests.push(test_case),
                }
                i += 1;
                continue;
            }

            // Identify failures block start
            if line == "failures:" {
                in_failures_section = true;
                i += 1;
                continue;
            }

            // Parsing failure details in the failures block
            if in_failures_section {
                if line.starts_with("---- ") && line.contains("stdout ----") {
                    // Starting a new failure details
                    let test_name = line[5..line.find(" stdout ----").unwrap_or(line.len())]
                        .to_string();
                    current_failure = Some((test_name, Vec::new()));
                } else if line.trim().is_empty() && current_failure.is_some() {
                    // A blank line indicates the end of the current failure details
                    if let Some((name, details)) = current_failure.take() {
                        // Find the corresponding test case and populate the details
                        if let Some(test) = result.failed_tests.iter_mut().find(|t| t.name == name) {
                            test.failure_details = Some(details.join("\n"));
                            // Try to parse the location from the details
                            test.location = self.parse_panic_location(&details.join("\n"));
                        }
                    }
                } else if let Some((_, ref mut details)) = current_failure {
                    details.push(line.to_string());
                }

                // failures Block end marker
                if line.starts_with("test result:") {
                    in_failures_section = false;
                }
            }

            // Summary of Parsing Test Results
            if line.starts_with("test result:") {
                result.test_summary = self.parse_test_summary(line);
            }

            i += 1;
        }

        result
    }
}

impl CargoParser {
    /// Parsing individual test case lines
    fn parse_test_case_line(&self, line: &str) -> Option<TestCase> {
        // 匹配: "test <name> ... ok/FAILED/ignored"
        let re = regex::Regex::new(
            r"^test\s+(\S+)\s+\.\.\.\s+(ok|FAILED|ignored)(?:\s*\(([^)]+)\))?",
        )
        .ok()?;

        let caps = re.captures(line)?;

        let name = caps.get(1)?.as_str().to_string();
        let result_str = caps.get(2)?.as_str();
        let extra = caps.get(3).map(|m| m.as_str());

        let status = match result_str {
            "ok" => TestStatus::Passed,
            "FAILED" => TestStatus::Failed,
            "ignored" => TestStatus::Ignored(extra.map(|s| s.to_string())),
            _ => return None,
        };

        // Try to parse the execution time from extra
        let execution_time = extra.and_then(|e| {
            e.strip_suffix('s').unwrap_or(e).parse().ok()
        });

        Some(TestCase {
            name,
            status,
            location: None,
            failure_details: None,
            execution_time,
        })
    }

    /// Parsing the location from a panic message
    fn parse_panic_location(&self, detail: &str) -> Option<Location> {
        let re = regex::Regex::new(r"panicked at\s+(\S+):(\d+):(\d+)").ok()?;
        let caps = re.captures(detail)?;

        Some(
            Location::new(caps.get(1)?.as_str().to_string())
                .with_line(caps.get(2)?.as_str().parse().ok()?)
                .with_column(caps.get(3)?.as_str().parse().ok()?),
        )
    }

    /// Summary of Parsing Test Results
    fn parse_test_summary(&self, line: &str) -> Option<TestSummary> {
        // 匹配: "test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
        let re = regex::Regex::new(
            r"test result:\s+(ok|FAILED)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored;\s+(\d+)\s+measured;\s+(\d+)\s+filtered out",
        )
        .ok()?;

        let caps = re.captures(line)?;

        let passed: usize = caps.get(2)?.as_str().parse().ok()?;
        let failed: usize = caps.get(3)?.as_str().parse().ok()?;
        let ignored: usize = caps.get(4)?.as_str().parse().ok()?;
        let measured: usize = caps.get(5)?.as_str().parse().ok()?;
        let filtered: usize = caps.get(6)?.as_str().parse().ok()?;

        Some(TestSummary {
            total: passed + failed + ignored,
            passed,
            failed,
            ignored,
            measured,
            filtered,
            execution_time: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_line() {
        let parser = CargoParser::new();
        let line = "src/main.rs:10:5: error[E0308]: mismatched types";
        let issue = parser.parse_single_line(line).unwrap();

        assert_eq!(issue.location.file_path, "src/main.rs");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.code, Some("[E0308]".to_string()));
        assert_eq!(issue.message, "mismatched types");
        assert!(matches!(issue.level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_multiline_error() {
        let parser = CargoParser::new();
        let lines = vec![
            "error: mismatched types".to_string(),
            " --> src/main.rs:10:5".to_string(),
        ];

        let (issue, next_index) = parser.parse_multiline_error(&lines, 0);
        let issue = issue.unwrap();

        assert_eq!(issue.location.file_path, "src/main.rs");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.message, "mismatched types");
        assert_eq!(next_index, 2);
    }

    #[test]
    fn test_parse_windows_path() {
        let parser = CargoParser::new();
        let lines = vec![
            "error: some error".to_string(),
            " --> D:\\project\\src\\main.rs:10:5".to_string(),
        ];

        let (issue, _) = parser.parse_multiline_error(&lines, 0);
        let issue = issue.unwrap();

        assert_eq!(issue.location.file_path, "D:\\project\\src\\main.rs");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
    }
}
