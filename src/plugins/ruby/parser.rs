//! Ruby Output Parser
//! Parsing output from RuboCop, RSpec, Rake, Minitest, and general Ruby commands

use crate::core::{
    BaseParser, Issue, IssueLevel, Location, OutputParser, ParseResult, ParsedTestOutput, TestCase,
    TestOutputParser, TestStatus, TestSummary,
};

pub struct RubyParser {
    base: BaseParser,
}

impl RubyParser {
    pub fn new() -> Self {
        Self {
            base: BaseParser::new(),
        }
    }

    /// Extract the first top-level JSON object from mixed output.
    ///
    /// Command execution merges stdout and stderr, and tools like RuboCop write
    /// trailing advisory text to stderr (e.g. "The following cops were added...").
    /// Feeding that merged text straight to `serde_json::from_str` fails on the
    /// trailing content, so the JSON payload must be isolated first.
    fn extract_json_object<'a>(&self, output: &'a str) -> Option<&'a str> {
        let start = output.find('{')?;
        let bytes = output.as_bytes();
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (i, &b) in bytes.iter().enumerate().skip(start) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_string = false;
                }
            } else {
                match b {
                    b'"' => in_string = true,
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(&output[start..=i]);
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Detect the type of Ruby output based on content heuristics
    fn detect_output_type(&self, output: &str) -> RubyOutputType {
        let trimmed = output.trim();
        if trimmed.is_empty() {
            return RubyOutputType::Unknown;
        }

        // Prefer the isolated JSON payload when present: merged stdout+stderr
        // may carry trailing non-JSON text that would defeat `starts_with`.
        if let Some(json) = self.extract_json_object(output) {
            // RSpec JSON output starts with {"version": ...} or contains "examples" array
            if json.contains("\"version\"") && json.contains("\"examples\"") {
                return RubyOutputType::RspecJson;
            }

            // RuboCop JSON output contains "metadata"
            if json.contains("\"metadata\"") {
                return RubyOutputType::RubocopJson;
            }
        }

        // RSpec default output
        if output.contains("example")
            && output.contains("failure")
            && output.contains("Finished in")
        {
            return RubyOutputType::RspecDefault;
        }

        // Standard Ruby error format: file.rb:line:in `method': message (ErrorType)
        if output.contains("Traceback") || output.contains("in `<main>'") {
            return RubyOutputType::RuntimeError;
        }

        RubyOutputType::Unknown
    }

    /// Parse RuboCop JSON output
    fn parse_rubocop_json(&self, output: &str) -> Vec<Issue> {
        let mut issues = Vec::new();

        // Try to parse as JSON
        let Some(json) = self.extract_json_object(output) else {
            return issues;
        };
        let parsed: serde_json::Value = match serde_json::from_str(json) {
            Ok(v) => v,
            Err(_) => return issues,
        };

        // RuboCop JSON format:
        // { "files": [{ "path": "...", "offenses": [{ "severity": "...", "message": "...",
        //   "cop_name": "...", "location": { "line": N, "column": N, "length": N } }] }] }
        if let Some(files) = parsed.get("files").and_then(|v| v.as_array()) {
            for file_entry in files {
                let file_path = file_entry
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                if let Some(offenses) = file_entry.get("offenses").and_then(|v| v.as_array()) {
                    for offense in offenses {
                        let severity = offense
                            .get("severity")
                            .and_then(|v| v.as_str())
                            .unwrap_or("convention");
                        let message = offense
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let cop_name = offense
                            .get("cop_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        let location_data = offense.get("location");
                        let (line_num, col_num) = location_data
                            .map(|loc| {
                                let line =
                                    loc.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                let col =
                                    loc.get("column").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                (line, col)
                            })
                            .unwrap_or((0, 0));

                        let level = self
                            .base
                            .detect_level(severity)
                            .unwrap_or(IssueLevel::Warning);

                        let location = if line_num > 0 && col_num > 0 {
                            Location::new(file_path.to_string())
                                .with_line(line_num)
                                .with_column(col_num)
                        } else if line_num > 0 {
                            Location::new(file_path.to_string()).with_line(line_num)
                        } else {
                            Location::new(file_path.to_string())
                        };

                        let mut issue = Issue::new(level, message.to_string(), location)
                            .with_code(format!("RuboCop/{}", cop_name));

                        // Add context about the cop
                        issue = issue.with_context(format!("cop: {}", cop_name));
                        issues.push(issue);
                    }
                }
            }
        }

        issues
    }

    /// Parse RSpec JSON output
    fn parse_rspec_json(&self, output: &str) -> Vec<Issue> {
        let mut issues = Vec::new();

        let Some(json) = self.extract_json_object(output) else {
            return issues;
        };
        let parsed: serde_json::Value = match serde_json::from_str(json) {
            Ok(v) => v,
            Err(_) => return issues,
        };

        // RSpec JSON format:
        // { "examples": [{ "id": "...", "description": "...", "full_description": "...",
        //   "status": "passed|failed|pending", "file_path": "...", "line_number": N,
        //   "exception": { "class": "...", "message": "...", "backtrace": [...] } }] }
        if let Some(examples) = parsed.get("examples").and_then(|v| v.as_array()) {
            for example in examples {
                let status = example
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("passed");
                let description = example
                    .get("full_description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let file_path = example
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let line_number = example
                    .get("line_number")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;

                if status == "failed" {
                    let mut message = description.to_string();
                    let mut context = String::new();

                    if let Some(exception) = example.get("exception") {
                        let exc_message = exception
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let exc_class = exception
                            .get("class")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Error");

                        message = format!("{}: {}", exc_class, exc_message);
                        context = description.to_string();
                    }

                    let location = if line_number > 0 {
                        Location::new(file_path.to_string()).with_line(line_number)
                    } else {
                        Location::new(file_path.to_string())
                    };

                    let mut issue = Issue::new(IssueLevel::Error, message, location);
                    if !context.is_empty() {
                        issue = issue.with_context(context);
                    }
                    issues.push(issue);
                }
            }
        }

        issues
    }

    /// Parse RSpec default text output
    fn parse_rspec_default(&self, output: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^\s*#\s\./(.+?):(\d+)").ok();

        for line in output.lines() {
            if let Some(re) = &re {
                if let Some(caps) = re.captures(line) {
                    let file_path = caps.get(1).map(|m| m.as_str()).unwrap_or("unknown");
                    let line_num: u32 = caps
                        .get(2)
                        .and_then(|m| m.as_str().parse().ok())
                        .unwrap_or(0);

                    // Check if the previous line indicates a failure
                    issues.push(Issue::new(
                        IssueLevel::Error,
                        "RSpec failure reference".to_string(),
                        Location::new(file_path.to_string()).with_line(line_num),
                    ));
                }
            }
        }

        issues
    }

    /// Parse Ruby runtime error (Traceback format):
    ///   Traceback (most recent call last):
    ///     N: from file.rb:line:in `method'
    ///   file.rb:line:in `method': message (ErrorClass)
    fn parse_ruby_error(&self, line: &str) -> Option<Issue> {
        let re = regex::Regex::new(r"^(.+?):(\d+):in\s+`[^']*':\s*(.+?)\s*\((.+?)\)").ok()?;

        if let Some(caps) = re.captures(line.trim()) {
            let file_path = caps.get(1)?.as_str();
            let line_num: u32 = caps.get(2)?.as_str().parse().ok()?;
            let message = caps.get(3)?.as_str();
            let error_type = caps.get(4)?.as_str();

            let location = Location::new(file_path.to_string()).with_line(line_num);
            let mut issue = Issue::new(IssueLevel::Error, message.to_string(), location);
            issue = issue.with_code(error_type.to_string());
            return Some(issue);
        }

        None
    }

    /// Parse test output for RSpec results
    fn parse_rspec_test_results(&self, output: &str) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();
        let mut in_failure_details = false;
        let mut _current_failure_name = String::new();
        let mut current_failure_details = Vec::new();

        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;
        let failure_re = regex::Regex::new(r"^rspec\s+\./(.+?):(\d+)$").ok();

        while i < lines.len() {
            let line = lines[i];

            // Try parsing as JSON first (extract payload from merged output)
            if i == 0
                && (line.trim().starts_with('{')
                    || output.contains("\"examples\":")
                    || output.contains("\"metadata\":"))
            {
                let Some(json) = self.extract_json_object(output) else {
                    i += 1;
                    continue;
                };
                let parsed: serde_json::Value = match serde_json::from_str(json) {
                    Ok(v) => v,
                    Err(_) => {
                        i += 1;
                        continue;
                    }
                };

                if let Some(examples) = parsed.get("examples").and_then(|v| v.as_array()) {
                    for example in examples {
                        let status = example
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("passed");
                        let description = example
                            .get("full_description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let file_path = example
                            .get("file_path")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let line_number = example
                            .get("line_number")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u32;

                        let location = if line_number > 0 && !file_path.is_empty() {
                            Some(Location::new(file_path.to_string()).with_line(line_number))
                        } else {
                            None
                        };

                        let test_case = TestCase {
                            name: description.to_string(),
                            status: match status {
                                "passed" => TestStatus::Passed,
                                "failed" => TestStatus::Failed,
                                "pending" => TestStatus::Ignored(Some("pending".to_string())),
                                _ => TestStatus::Ignored(None),
                            },
                            location,
                            failure_details: if status == "failed" {
                                example.get("exception").map(|e| {
                                    let msg =
                                        e.get("message").and_then(|v| v.as_str()).unwrap_or("");
                                    let backtrace =
                                        e.get("backtrace").and_then(|v| v.as_array()).map(|arr| {
                                            arr.iter()
                                                .filter_map(|v| v.as_str())
                                                .collect::<Vec<_>>()
                                                .join("\n")
                                        });
                                    format!("{}\n{}", msg, backtrace.unwrap_or_default())
                                })
                            } else {
                                None
                            },
                            execution_time: None,
                        };

                        match test_case.status {
                            TestStatus::Passed => result.passed_tests.push(test_case),
                            TestStatus::Failed => result.failed_tests.push(test_case),
                            TestStatus::Ignored(_) => result.ignored_tests.push(test_case),
                        }
                    }
                }

                // Parse summary from JSON
                if let Some(summary) = parsed.get("summary") {
                    let passed = summary
                        .get("example_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize
                        - summary
                            .get("failure_count")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize
                        - summary
                            .get("pending_count")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                    let failed = summary
                        .get("failure_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    let pending = summary
                        .get("pending_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    let total = summary
                        .get("example_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;

                    result.test_summary = Some(TestSummary {
                        total,
                        passed,
                        failed,
                        ignored: pending,
                        measured: 0,
                        filtered: 0,
                        execution_time: summary.get("duration").and_then(|v| v.as_f64()),
                    });
                }

                break; // Done processing JSON
            }

            // Text-based test output parsing
            // Match: "N examples, M failures, P pending"
            if let Some(summary) = self.parse_rspec_text_summary(line) {
                result.test_summary = Some(summary);
            }

            // Match failure reference: "rspec ./spec/file_spec.rb:123"
            if let Some(re) = &failure_re {
                if let Some(caps) = re.captures(line.trim()) {
                    let name = format!("rspec {}", caps.get(1).map(|m| m.as_str()).unwrap_or(""));
                    let location =
                        Location::new(caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string())
                            .with_line(
                                caps.get(2)
                                    .and_then(|m| m.as_str().parse().ok())
                                    .unwrap_or(0),
                            );

                    result.failed_tests.push(TestCase {
                        name,
                        status: TestStatus::Failed,
                        location: Some(location),
                        failure_details: if in_failure_details {
                            Some(current_failure_details.join("\n"))
                        } else {
                            None
                        },
                        execution_time: None,
                    });
                }
            }

            // Track failure details block
            if line.starts_with("  Failure/Error:") {
                in_failure_details = true;
                _current_failure_name = String::new();
                current_failure_details.clear();
            } else if in_failure_details {
                if line.trim().is_empty() && !current_failure_details.is_empty() {
                    in_failure_details = false;
                } else {
                    current_failure_details.push(line.to_string());
                }
            }

            i += 1;
        }

        result
    }

    /// Parse RSpec text summary: "N examples, M failures, P pending"
    fn parse_rspec_text_summary(&self, line: &str) -> Option<TestSummary> {
        let re =
            regex::Regex::new(r"^(\d+)\s+examples?,\s+(\d+)\s+failures?(?:,\s+(\d+)\s+pending)?")
                .ok()?;

        let caps = re.captures(line.trim())?;

        let total: usize = caps.get(1)?.as_str().parse().ok()?;
        let failed: usize = caps.get(2)?.as_str().parse().ok()?;
        let pending: usize = caps
            .get(3)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);
        let passed = total.saturating_sub(failed).saturating_sub(pending);

        Some(TestSummary {
            total,
            passed,
            failed,
            ignored: pending,
            measured: 0,
            filtered: 0,
            execution_time: None,
        })
    }
}

/// Types of Ruby output that the parser can handle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RubyOutputType {
    RubocopJson,
    RspecJson,
    RspecDefault,
    RuntimeError,
    Unknown,
}

impl Default for RubyParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for RubyParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let output_type = self.detect_output_type(output);

        let issues = match output_type {
            RubyOutputType::RubocopJson => self.parse_rubocop_json(output),
            RubyOutputType::RspecJson => self.parse_rspec_json(output),
            RubyOutputType::RspecDefault => {
                let mut issues = self.parse_rspec_json(output);
                let default_issues = self.parse_rspec_default(output);
                issues.extend(default_issues);
                issues
            }
            RubyOutputType::RuntimeError | RubyOutputType::Unknown => {
                // Fallback: line-by-line parsing
                let mut issues = Vec::new();
                for line in output.lines() {
                    if let Some(issue) = self.parse_ruby_error(line) {
                        issues.push(issue);
                    }
                }
                issues
            }
        };

        ParseResult::Full(issues)
    }
}

impl TestOutputParser for RubyParser {
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput {
        self.parse_rspec_test_results(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rubocop_json() {
        let parser = RubyParser::new();
        let json = r#"{
            "metadata": { "rubocop_version": "1.50.0" },
            "files": [
                {
                    "path": "src/app.rb",
                    "offenses": [
                        {
                            "severity": "warning",
                            "message": "Unused method argument - `unused_var`",
                            "cop_name": "Lint/UnusedMethodArgument",
                            "location": { "line": 15, "column": 5, "length": 10 }
                        }
                    ]
                }
            ]
        }"#;

        let issues = parser.parse_rubocop_json(json);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location.file_path, "src/app.rb");
        assert_eq!(issues[0].location.line_number, Some(15));
        assert_eq!(issues[0].location.column_number, Some(5));
        assert_eq!(
            issues[0].code,
            Some("RuboCop/Lint/UnusedMethodArgument".to_string())
        );
        assert!(matches!(issues[0].level, IssueLevel::Warning));
    }

    #[test]
    fn test_parse_rubocop_convention() {
        let parser = RubyParser::new();
        let json = r#"{
            "metadata": {},
            "files": [
                {
                    "path": "src/app.rb",
                    "offenses": [
                        {
                            "severity": "convention",
                            "message": "Use 2 spaces for indentation",
                            "cop_name": "Layout/IndentationWidth",
                            "location": { "line": 3, "column": 1, "length": 2 }
                        }
                    ]
                }
            ]
        }"#;

        let issues = parser.parse_rubocop_json(json);
        assert_eq!(issues.len(), 1);
        assert!(matches!(issues[0].level, IssueLevel::Warning));
    }

    #[test]
    fn test_parse_rspec_json_failed() {
        let parser = RubyParser::new();
        let json = r#"{
            "version": "3.12.0",
            "examples": [
                {
                    "id": "./spec/calculator_spec.rb[1:1]",
                    "description": "adds two numbers",
                    "full_description": "Calculator#add adds two numbers",
                    "status": "failed",
                    "file_path": "./spec/calculator_spec.rb",
                    "line_number": 5,
                    "exception": {
                        "class": "RSpec::Expectations::ExpectationNotMetError",
                        "message": "expected: 5, got: 4",
                        "backtrace": ["./spec/calculator_spec.rb:5"]
                    }
                }
            ],
            "summary": {
                "duration": 0.023,
                "example_count": 1,
                "failure_count": 1,
                "pending_count": 0,
                "errors_outside_of_examples_count": 0
            }
        }"#;

        let issues = parser.parse_rspec_json(json);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location.file_path, "./spec/calculator_spec.rb");
        assert_eq!(issues[0].location.line_number, Some(5));
        assert_eq!(
            issues[0].message,
            "RSpec::Expectations::ExpectationNotMetError: expected: 5, got: 4"
        );
        assert!(matches!(issues[0].level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_rspec_json_passed() {
        let parser = RubyParser::new();
        let json = r#"{
            "version": "3.12.0",
            "examples": [
                {
                    "id": "./spec/test_spec.rb[1:1]",
                    "description": "works",
                    "full_description": "Test works",
                    "status": "passed",
                    "file_path": "./spec/test_spec.rb",
                    "line_number": 3
                }
            ],
            "summary": {
                "example_count": 1,
                "failure_count": 0,
                "pending_count": 0
            }
        }"#;

        let issues = parser.parse_rspec_json(json);
        assert_eq!(issues.len(), 0); // No issues for passed tests
    }

    #[test]
    fn test_parse_ruby_runtime_error() {
        let parser = RubyParser::new();
        let line = "src/app.rb:25:in `divide': divided by 0 (ZeroDivisionError)";
        let issue = parser.parse_ruby_error(line).unwrap();

        assert_eq!(issue.location.file_path, "src/app.rb");
        assert_eq!(issue.location.line_number, Some(25));
        assert_eq!(issue.message, "divided by 0");
        assert_eq!(issue.code, Some("ZeroDivisionError".to_string()));
    }

    #[test]
    fn test_parse_rspec_json_test_output() {
        let parser = RubyParser::new();
        let json = r#"{
            "version": "3.12.0",
            "examples": [
                {
                    "id": "./spec/test_spec.rb[1:1]",
                    "description": "passes",
                    "full_description": "Test passes",
                    "status": "passed",
                    "file_path": "./spec/test_spec.rb",
                    "line_number": 3
                },
                {
                    "id": "./spec/test_spec.rb[1:2]",
                    "description": "fails",
                    "full_description": "Test fails",
                    "status": "failed",
                    "file_path": "./spec/test_spec.rb",
                    "line_number": 10,
                    "exception": {
                        "class": "StandardError",
                        "message": "went wrong",
                        "backtrace": ["./spec/test_spec.rb:10"]
                    }
                }
            ],
            "summary": {
                "example_count": 2,
                "failure_count": 1,
                "pending_count": 0,
                "duration": 0.1
            }
        }"#;

        let result = parser.parse_rspec_test_results(json);
        assert_eq!(result.passed_tests.len(), 1);
        assert_eq!(result.failed_tests.len(), 1);
        assert_eq!(result.ignored_tests.len(), 0);
        assert!(result.test_summary.is_some());
        assert_eq!(result.test_summary.as_ref().unwrap().total, 2);
        assert_eq!(result.test_summary.as_ref().unwrap().failed, 1);
        assert_eq!(result.test_summary.as_ref().unwrap().passed, 1);
    }

    #[test]
    fn test_detect_rubocop_json() {
        let parser = RubyParser::new();
        let output = "{\"metadata\": {\"rubocop_version\": \"1.50.0\"}}";
        assert_eq!(
            parser.detect_output_type(output),
            RubyOutputType::RubocopJson
        );
    }

    #[test]
    fn test_detect_rspec_json() {
        let parser = RubyParser::new();
        let output = "{\"version\": \"3.12\", \"examples\": []}";
        assert_eq!(parser.detect_output_type(output), RubyOutputType::RspecJson);
    }
}
