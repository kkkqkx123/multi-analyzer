//! Go Output Parser
//! Parsing the output of go build, go vet, go test, golangci-lint

use crate::core::{
    Issue, IssueLevel, Location, OutputParser, ParseResult, ParsedTestOutput, TestCase,
    TestOutputParser, TestStatus, TestSummary,
};

/// Go command type to distinguish output formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoCommandType {
    #[default]
    Unknown,
    Build,
    Vet,
    Test,
    GolangciLint,
    GoFmt,
}

pub struct GoParser {
    command_type: GoCommandType,
}

impl GoParser {
    pub fn new() -> Self {
        Self {
            command_type: GoCommandType::Unknown,
        }
    }

    /// Auto-detect command type from output content
    fn detect_command_type(&self, output: &str) -> GoCommandType {
        let lines: Vec<&str> = output.lines().collect();
        let mut fmt_file_count = 0usize;
        let mut other_count = 0usize;

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // golangci-lint has linter names in parentheses
            if trimmed.contains('(') && trimmed.contains(')') {
                if let Some(open_paren) = trimmed.rfind('(') {
                    if let Some(close_paren) = trimmed.rfind(')') {
                        let linter_name = &trimmed[open_paren + 1..close_paren];
                        let known_linters = [
                            "errcheck",
                            "gosimple",
                            "govet",
                            "ineffassign",
                            "staticcheck",
                            "unused",
                            "deadcode",
                            "gofmt",
                        ];
                        if known_linters.iter().any(|l| linter_name.contains(l)) {
                            return GoCommandType::GolangciLint;
                        }
                    }
                }
            }

            // go test has specific test output markers
            if trimmed.starts_with("=== RUN")
                || trimmed.starts_with("--- PASS")
                || trimmed.starts_with("--- FAIL")
                || trimmed.starts_with("--- SKIP")
            {
                return GoCommandType::Test;
            }

            // go vet typically has these patterns (but not compiler errors)
            if trimmed.contains("Printf format")
                || (trimmed.contains("return value") && trimmed.contains("is not checked"))
            {
                return GoCommandType::Vet;
            }

            // go build errors have specific keywords
            if trimmed.contains("undefined:")
                || trimmed.contains("cannot use")
                || trimmed.contains("not declared")
                || trimmed.contains("declared but not used")
            {
                return GoCommandType::Build;
            }

            // go fmt / gofmt output: file paths with .go extension
            if trimmed.ends_with(".go") && !trimmed.contains(':') {
                fmt_file_count += 1;
            } else {
                other_count += 1;
            }
        }

        // If most lines are .go file paths, it's go fmt output
        if fmt_file_count > 0 && fmt_file_count >= other_count {
            return GoCommandType::GoFmt;
        }

        GoCommandType::Unknown
    }

    /// Parse a single line based on provided command type
    fn parse_line_with_type(&self, line: &str, cmd_type: GoCommandType) -> Option<Issue> {
        match cmd_type {
            GoCommandType::Build => self.parse_go_build_error(line),
            GoCommandType::Vet => self.parse_go_vet_error(line),
            GoCommandType::GolangciLint => self.parse_golangci_lint_error(line),
            GoCommandType::Test => self.parse_go_build_error(line),
            GoCommandType::GoFmt => self.parse_gofmt_line(line),
            GoCommandType::Unknown => {
                if let Some(issue) = self.parse_golangci_lint_error(line) {
                    return Some(issue);
                }
                if let Some(issue) = self.parse_go_vet_error(line) {
                    return Some(issue);
                }
                if let Some(issue) = self.parse_gofmt_line(line) {
                    return Some(issue);
                }
                self.parse_go_build_error(line)
            }
        }
    }

    /// Parse go build error format:
    /// # example.com/myproject
    /// ./main.go:10:5: undefined: someVariable
    /// ./main.go:15:2: cannot use x (type int) as type string
    fn parse_go_build_error(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }

        // Try standard format: file:line:col: message
        let parts: Vec<&str> = trimmed.splitn(4, ':').collect();
        if parts.len() < 3 {
            return None;
        }

        let file_path = parts[0].trim();
        let line_num = parts[1].trim().parse::<u32>().ok()?;

        // Check if column number is present
        let (col_num, message) = if parts.len() >= 4 {
            let col = parts[2].trim().parse::<u32>().ok()?;
            (Some(col), parts[3].trim().to_string())
        } else {
            (None, parts[2].trim().to_string())
        };

        // For go build, all issues are errors (compilation failures)
        let message = message.trim_start_matches("error:").trim().to_string();

        let location = Location::new(file_path.to_string()).with_line(line_num);
        let location = if let Some(col) = col_num {
            location.with_column(col)
        } else {
            location
        };

        Some(Issue::new(IssueLevel::Error, message, location))
    }

    /// Parse go vet output format:
    /// # command-line-arguments
    /// ./main.go:10:5: Printf format %s has arg x of wrong type int
    fn parse_go_vet_error(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }

        // Skip lines that look like go build errors (contain specific keywords)
        if trimmed.contains("undefined:")
            || trimmed.contains("cannot use")
            || trimmed.contains("not declared")
            || trimmed.contains("declared but not used")
        {
            return None;
        }

        // Format: file:line:col: message
        let parts: Vec<&str> = trimmed.splitn(4, ':').collect();
        if parts.len() < 4 {
            return None;
        }

        let file_path = parts[0].trim();
        let line_num = parts[1].trim().parse::<u32>().ok()?;
        let col_num = parts[2].trim().parse::<u32>().ok()?;
        let message = parts[3].trim().to_string();

        let location = Location::new(file_path.to_string())
            .with_line(line_num)
            .with_column(col_num);

        Some(Issue::new(IssueLevel::Warning, message, location))
    }

    /// Parse golangci-lint output format:
    /// main.go:10:5: Error return value of `fmt.Println` is not checked (errcheck)
    /// main.go:15:2: SA1000: invalid regular expression (staticcheck)
    fn parse_golangci_lint_error(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Must have parentheses at the end for linter name
        if !trimmed.contains('(') || !trimmed.contains(')') {
            return None;
        }

        // Format: file:line:col: message (linter_name)
        // or: file:line:col: linter_code: message (linter_name)
        let parts: Vec<&str> = trimmed.splitn(4, ':').collect();
        if parts.len() < 4 {
            return None;
        }

        let file_path = parts[0].trim();
        let line_num = parts[1].trim().parse::<u32>().ok()?;
        let col_num = parts[2].trim().parse::<u32>().ok()?;
        let rest = parts[3].trim();

        // Extract linter name from parentheses at the end
        let (message, linter_name) = if let Some(start) = rest.rfind('(') {
            if let Some(end) = rest.rfind(')') {
                let linter = rest[start + 1..end].trim().to_string();
                let msg = rest[..start].trim().to_string();
                (msg, Some(linter))
            } else {
                (rest.to_string(), None)
            }
        } else {
            (rest.to_string(), None)
        };

        // Try to extract error code (e.g., SA1000, ST1005)
        let (code, final_message) = if let Some(first_colon) = message.find(':') {
            let potential_code = message[..first_colon].trim();
            if potential_code
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !potential_code.is_empty()
                && potential_code.chars().next().unwrap().is_ascii_uppercase()
            {
                let code = format!("[{}]", potential_code);
                let msg = message[first_colon + 1..].trim().to_string();
                (Some(code), msg)
            } else {
                (None, message)
            }
        } else {
            (None, message)
        };

        let location = Location::new(file_path.to_string())
            .with_line(line_num)
            .with_column(col_num);

        let mut issue = Issue::new(IssueLevel::Warning, final_message, location);
        if let Some(c) = code {
            issue = issue.with_code(c);
        }
        if let Some(linter) = linter_name {
            issue = issue.with_context(format!("linter: {}", linter));
        }

        Some(issue)
    }

    /// Parse gofmt / go fmt output.
    ///
    /// `gofmt -l .` outputs file paths that differ from the standard format,
    /// one per line. `go fmt ./...` outputs modified file paths.
    fn parse_gofmt_line(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }

        // go fmt output is just file paths (e.g., "./main.go" or "main.go")
        if trimmed.ends_with(".go") && !trimmed.contains(':') {
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

    /// Parse go test output format
    fn parse_go_test_output(&self, output: &str) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();
        let lines: Vec<&str> = output.lines().collect();

        // First, parse compilation errors using build parser
        for line in &lines {
            // Skip package lines
            if line.starts_with("# ") || line.starts_with("? ") || line.starts_with("ok ") {
                continue;
            }

            // Check for FAIL line indicating test failures
            if line.starts_with("FAIL") && line.contains('\t') {
                continue;
            }

            // Parse test case results
            if let Some(test_case) = self.parse_test_case_line(line) {
                match test_case.status {
                    TestStatus::Passed => result.passed_tests.push(test_case),
                    TestStatus::Failed => result.failed_tests.push(test_case),
                    TestStatus::Ignored(_) => result.ignored_tests.push(test_case),
                }
                continue;
            }

            // Parse compilation errors using build error parser.
            // Only consider non-indented lines: real Go compile errors are
            // emitted at column 0, whereas indented `file:line: msg` lines are
            // assertion-detail output (e.g. `  main_test.go:14: add(2,3) = 5`)
            // that must not be mistaken for compile issues.
            if !line.starts_with(char::is_whitespace) {
                if let Some(issue) = self.parse_go_build_error(line) {
                    result.compile_issues.push(issue);
                }
            }
        }

        // Parse test summary
        result.test_summary = self.parse_test_summary(&lines);

        result
    }

    /// Parse individual test case line
    /// Format: --- PASS: TestName (0.00s)
    ///         --- FAIL: TestName (0.00s)
    ///         --- SKIP: TestName (0.00s)
    fn parse_test_case_line(&self, line: &str) -> Option<TestCase> {
        let trimmed = line.trim();

        // Match: --- PASS: TestName (0.00s)
        if let Some(rest) = trimmed.strip_prefix("--- PASS: ") {
            let name = rest.split_whitespace().next()?.to_string();
            let execution_time = self.parse_execution_time(rest);
            return Some(TestCase {
                name,
                status: TestStatus::Passed,
                location: None,
                failure_details: None,
                execution_time,
            });
        }

        // Match: --- FAIL: TestName (0.00s)
        if let Some(rest) = trimmed.strip_prefix("--- FAIL: ") {
            let name = rest.split_whitespace().next()?.to_string();
            let execution_time = self.parse_execution_time(rest);
            return Some(TestCase {
                name,
                status: TestStatus::Failed,
                location: None,
                failure_details: None,
                execution_time,
            });
        }

        // Match: --- SKIP: TestName (0.00s)
        if let Some(rest) = trimmed.strip_prefix("--- SKIP: ") {
            let name = rest.split_whitespace().next()?.to_string();
            let execution_time = self.parse_execution_time(rest);
            let reason = self.parse_skip_reason(rest);
            return Some(TestCase {
                name,
                status: TestStatus::Ignored(reason),
                location: None,
                failure_details: None,
                execution_time,
            });
        }

        None
    }

    fn parse_execution_time(&self, line: &str) -> Option<f64> {
        // Extract time from (0.00s)
        if let Some(start) = line.find('(') {
            if let Some(end) = line.find("s)") {
                let time_str = &line[start + 1..end];
                return time_str.parse().ok();
            }
        }
        None
    }

    fn parse_skip_reason(&self, line: &str) -> Option<String> {
        // Try to find reason after time, e.g., "(0.00s) reason"
        if let Some(end) = line.find("s)") {
            let rest = line[end + 2..].trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
        None
    }

    /// Parse test summary from lines
    fn parse_test_summary(&self, lines: &[&str]) -> Option<TestSummary> {
        let mut passed = 0;
        let mut failed = 0;
        let mut ignored = 0;

        for line in lines {
            let trimmed = line.trim();

            // Count test results
            if trimmed.starts_with("--- PASS:") {
                passed += 1;
            } else if trimmed.starts_with("--- FAIL:") {
                failed += 1;
            } else if trimmed.starts_with("--- SKIP:") {
                ignored += 1;
            }
        }

        if passed > 0 || failed > 0 || ignored > 0 {
            Some(TestSummary {
                total: passed + failed + ignored,
                passed,
                failed,
                ignored,
                measured: 0,
                filtered: 0,
                execution_time: None,
            })
        } else {
            None
        }
    }
}

impl Default for GoParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for GoParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let mut issues = Vec::new();
        let lines: Vec<&str> = output.lines().collect();

        let command_type = if self.command_type == GoCommandType::Unknown {
            self.detect_command_type(output)
        } else {
            self.command_type
        };

        for line in &lines {
            if let Some(issue) = self.parse_line_with_type(line, command_type) {
                issues.push(issue);
            }
        }

        ParseResult::Full(issues)
    }
}

impl TestOutputParser for GoParser {
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput {
        self.parse_go_test_output(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_go_build_error() {
        let parser = GoParser::new();
        let line = "./main.go:10:5: undefined: someVariable";
        let issue = parser.parse_go_build_error(line).unwrap();

        assert_eq!(issue.location.file_path, "./main.go");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.message, "undefined: someVariable");
        assert!(matches!(issue.level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_go_vet_error() {
        let parser = GoParser::new();
        let line = "./main.go:15:10: Printf format %s has arg x of wrong type int";
        let issue = parser.parse_go_vet_error(line).unwrap();

        assert_eq!(issue.location.file_path, "./main.go");
        assert_eq!(issue.location.line_number, Some(15));
        assert_eq!(issue.location.column_number, Some(10));
        assert_eq!(
            issue.message,
            "Printf format %s has arg x of wrong type int"
        );
        assert!(matches!(issue.level, IssueLevel::Warning));
    }

    #[test]
    fn test_parse_golangci_lint_error() {
        let parser = GoParser::new();
        let line = "main.go:20:3: Error return value of `fmt.Println` is not checked (errcheck)";
        let issue = parser.parse_golangci_lint_error(line).unwrap();

        assert_eq!(issue.location.file_path, "main.go");
        assert_eq!(issue.location.line_number, Some(20));
        assert_eq!(issue.location.column_number, Some(3));
        assert_eq!(
            issue.message,
            "Error return value of `fmt.Println` is not checked"
        );
        assert_eq!(issue.context, Some("linter: errcheck".to_string()));
        assert!(matches!(issue.level, IssueLevel::Warning));
    }

    #[test]
    fn test_parse_golangci_lint_with_code() {
        let parser = GoParser::new();
        let line = "main.go:25:5: SA1000: invalid regular expression (staticcheck)";
        let issue = parser.parse_golangci_lint_error(line).unwrap();

        assert_eq!(issue.location.file_path, "main.go");
        assert_eq!(issue.location.line_number, Some(25));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.code, Some("[SA1000]".to_string()));
        assert_eq!(issue.message, "invalid regular expression");
        assert_eq!(issue.context, Some("linter: staticcheck".to_string()));
    }

    #[test]
    fn test_parse_test_pass() {
        let parser = GoParser::new();
        let line = "--- PASS: TestAddition (0.01s)";
        let test_case = parser.parse_test_case_line(line).unwrap();

        assert_eq!(test_case.name, "TestAddition");
        assert!(matches!(test_case.status, TestStatus::Passed));
        assert_eq!(test_case.execution_time, Some(0.01));
    }

    #[test]
    fn test_parse_test_fail() {
        let parser = GoParser::new();
        let line = "--- FAIL: TestDivision (0.02s)";
        let test_case = parser.parse_test_case_line(line).unwrap();

        assert_eq!(test_case.name, "TestDivision");
        assert!(matches!(test_case.status, TestStatus::Failed));
        assert_eq!(test_case.execution_time, Some(0.02));
    }

    #[test]
    fn test_parse_test_skip() {
        let parser = GoParser::new();
        let line = "--- SKIP: TestIntegration (0.00s) skipped on Windows";
        let test_case = parser.parse_test_case_line(line).unwrap();

        assert_eq!(test_case.name, "TestIntegration");
        assert!(matches!(test_case.status, TestStatus::Ignored(Some(_))));
        assert_eq!(test_case.execution_time, Some(0.00));
    }

    #[test]
    fn test_parse_full_test_output() {
        let parser = GoParser::new();
        let output = r#"
# example.com/myproject
--- PASS: TestAddition (0.01s)
--- PASS: TestSubtraction (0.02s)
--- FAIL: TestDivision (0.03s)
--- SKIP: TestIntegration (0.00s) skipped on Windows
PASS
ok  	example.com/myproject	0.061s
"#;

        let result = parser.parse_test_output(output);

        assert_eq!(result.passed_tests.len(), 2);
        assert_eq!(result.failed_tests.len(), 1);
        assert_eq!(result.ignored_tests.len(), 1);

        let summary = result.test_summary.unwrap();
        assert_eq!(summary.total, 4);
        assert_eq!(summary.passed, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.ignored, 1);
    }

    #[test]
    fn test_detect_command_type_golangci_lint() {
        let parser = GoParser::new();
        let output = "main.go:10:5: error (errcheck)";
        assert_eq!(
            parser.detect_command_type(output),
            GoCommandType::GolangciLint
        );
    }

    #[test]
    fn test_detect_command_type_test() {
        let parser = GoParser::new();
        let output = "=== RUN   TestFoo\n--- PASS: TestFoo (0.01s)";
        assert_eq!(parser.detect_command_type(output), GoCommandType::Test);
    }

    #[test]
    fn test_detect_command_type_build() {
        let parser = GoParser::new();
        let output = "./main.go:10:5: undefined: foo";
        assert_eq!(parser.detect_command_type(output), GoCommandType::Build);
    }

    #[test]
    fn test_indented_failure_detail_not_compile_issue() {
        let parser = GoParser::new();
        let output = "\
=== RUN   TestSubtract
    main_test.go:14: add(2,3) = 5, want 0
--- FAIL: TestSubtract (0.00s)
FAIL";
        let result = parser.parse_go_test_output(output);
        // The indented `main_test.go:14: ...` line is assertion detail, not a
        // compile error, so compile_issues must stay empty.
        assert!(
            result.compile_issues.is_empty(),
            "indented failure detail must not be a compile issue"
        );
    }

    #[test]
    fn test_build_failure_in_test_output_captured() {
        let parser = GoParser::new();
        // A real (column-0) compile error during `go test ./...` must be captured.
        let output = "\
# go-demo/cmd/bad
cmd/bad/main.go:6:10: undefined: undefinedSymbol";
        let result = parser.parse_go_test_output(output);
        assert_eq!(result.compile_issues.len(), 1);
    }
}
