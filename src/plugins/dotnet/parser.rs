//! .NET Output Parser
//! Parsing the output of dotnet build and dotnet test

use crate::core::{
    BaseParser, Issue, IssueLevel, Location, OutputParser, ParseResult, ParsedTestOutput, TestCase,
    TestOutputParser, TestStatus, TestSummary,
};

pub struct DotnetParser {
    base: BaseParser,
}

impl DotnetParser {
    pub fn new() -> Self {
        Self {
            base: BaseParser::new(),
        }
    }

    /// Parse MSBuild error format:
    ///   {file}({line},{col}): error|warning {code}: {message} [{project}]
    ///   {file}({line},{col},{end_line},{end_col}): error|warning {code}: {message} [{project}]
    fn parse_msbuild_error(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Pattern: file(line,col): error|warning CODE: message [project]
        // Or:      file(line,col,end_line,end_col): error|warning CODE: message [project]
        let re = regex::Regex::new(
            r"^(.+?)\((\d+),\s*(\d+)(?:,\s*\d+,\s*\d+)?\)\s*:\s*(error|warning)\s+(\S+)\s*:\s*(.+?)(?:\s+\[.+?\])?$"
        ).ok()?;

        let caps = re.captures(trimmed)?;

        let file_path = caps.get(1)?.as_str();
        let line_num: u32 = caps.get(2)?.as_str().parse().ok()?;
        let col_num: u32 = caps.get(3)?.as_str().parse().ok()?;
        let level_str = caps.get(4)?.as_str();
        let code = caps.get(5)?.as_str().to_string();
        let message = caps.get(6)?.as_str().to_string();

        let level = self.base.detect_level(level_str)?;

        let location = Location::new(file_path.to_string())
            .with_line(line_num)
            .with_column(col_num);

        let issue = Issue::new(level, message, location).with_code(code);

        Some(issue)
    }

    /// Parse MSBuild error format that may not have column numbers:
    ///   {file}({line}): error|warning {code}: {message} [{project}]
    fn parse_msbuild_error_no_col(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let re = regex::Regex::new(
            r"^(.+?)\((\d+)\)\s*:\s*(error|warning)\s+(\S+)\s*:\s*(.+?)(?:\s+\[.+?\])?$",
        )
        .ok()?;

        let caps = re.captures(trimmed)?;

        let file_path = caps.get(1)?.as_str();
        let line_num: u32 = caps.get(2)?.as_str().parse().ok()?;
        let level_str = caps.get(3)?.as_str();
        let code = caps.get(4)?.as_str().to_string();
        let message = caps.get(5)?.as_str().to_string();

        let level = self.base.detect_level(level_str)?;

        let location = Location::new(file_path.to_string()).with_line(line_num);

        let issue = Issue::new(level, message, location).with_code(code);

        Some(issue)
    }

    /// Parse build summary lines like:
    ///   Build succeeded.   0 warnings, 0 errors
    ///   Build FAILED.      2 warnings, 1 error
    #[allow(dead_code)]
    fn parse_build_summary(&self, _lines: &[&str]) -> (usize, usize) {
        // This is informational; actual issues are extracted from individual error lines
        (0, 0)
    }

    /// Parse dotnet format output: style/whitespace formatting issues.
    ///
    /// Format variants:
    ///   {file}({line},{col}): Fix {description}
    ///   {file}({line},{col}): {description}
    ///
    /// These are always treated as Warning-level issues.
    fn parse_format_issue(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let re = regex::Regex::new(
            r"^(.+?)\((\d+),\s*(\d+)\)\s*:\s*(?:Fix\s+)?(.+)$"
        ).ok()?;

        let caps = re.captures(trimmed)?;

        let message = caps.get(4)?.as_str();
        // Bail out early on MSBuild lines whose severity token (error|warning)
        // would be mistakenly captured as part of the message.
        let first_word = message.split_whitespace().next().unwrap_or("");
        if matches!(first_word.to_lowercase().as_str(), "error" | "warning") {
            return None;
        }

        let file_path = caps.get(1)?.as_str();
        let line_num: u32 = caps.get(2)?.as_str().parse().ok()?;
        let col_num: u32 = caps.get(3)?.as_str().parse().ok()?;

        let location = Location::new(file_path.to_string())
            .with_line(line_num)
            .with_column(col_num);

        let issue = Issue::new(IssueLevel::Warning, message.to_string(), location)
            .with_code("FORMAT".to_string());

        Some(issue)
    }

    /// Parse dotnet format --verify-no-changes output: analyzer diagnostics.
    ///
    /// Format:
    ///   {file}({line},{col}): {analyzer_id}: {message}
    ///
    /// Example:
    ///   src/Program.cs(10,5): CA1822: Mark members as static
    fn parse_format_analyzer(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let re = regex::Regex::new(
            r"^(.+?)\((\d+),\s*(\d+)\)\s*:\s*(\S+)\s*:\s*(.+)$"
        ).ok()?;

        let caps = re.captures(trimmed)?;

        let kind = caps.get(4)?.as_str();
        // Bail out early on MSBuild lines whose severity token (error|warning)
        // would be mistakenly captured as the code group.
        if matches!(kind.to_lowercase().as_str(), "error" | "warning") {
            return None;
        }

        let file_path = caps.get(1)?.as_str();
        let line_num: u32 = caps.get(2)?.as_str().parse().ok()?;
        let col_num: u32 = caps.get(3)?.as_str().parse().ok()?;
        let code = kind.to_string();
        let message = caps.get(5)?.as_str().to_string();

        let location = Location::new(file_path.to_string())
            .with_line(line_num)
            .with_column(col_num);

        let issue = Issue::new(IssueLevel::Warning, message, location)
            .with_code(code);

        Some(issue)
    }

    /// Check if the output looks like `dotnet format` style output.
    #[allow(dead_code)]
    fn is_format_output(trimmed: &str) -> bool {
        trimmed.contains(": Fix ") || trimmed.contains("Formatting ")
    }

    /// Parse a single test result line from dotnet test output.
    ///   Passed  TestNamespace.TestClass.TestMethod
    ///   Failed  TestNamespace.TestClass.TestMethod
    ///   Skipped TestNamespace.TestClass.TestMethod
    fn parse_test_result_line(&self, line: &str) -> Option<TestCase> {
        let trimmed = line.trim();

        let (status_str, name) = if let Some(name) = trimmed.strip_prefix("Passed ") {
            ("Passed", name.trim())
        } else if let Some(name) = trimmed.strip_prefix("Failed ") {
            ("Failed", name.trim())
        } else if let Some(name) = trimmed.strip_prefix("Skipped ") {
            ("Skipped", name.trim())
        } else {
            return None;
        };

        let status = match status_str {
            "Passed" => TestStatus::Passed,
            "Failed" => TestStatus::Failed,
            "Skipped" => TestStatus::Ignored(None),
            _ => return None,
        };

        Some(TestCase {
            name: name.to_string(),
            status,
            location: None,
            failure_details: None,
            execution_time: None,
        })
    }

    /// Parse the final test summary line:
    ///   Passed! - Failed: 0, Passed: 42, Skipped: 3, Total: 45
    fn parse_test_summary_line(&self, line: &str) -> Option<TestSummary> {
        let re = regex::Regex::new(
            r"(?:Passed|Failed)!?\s*-\s*Failed:\s*(\d+),\s*Passed:\s*(\d+),\s*Skipped:\s*(\d+),\s*Total:\s*(\d+)"
        ).ok()?;

        let caps = re.captures(line)?;

        let failed: usize = caps.get(1)?.as_str().parse().ok()?;
        let passed: usize = caps.get(2)?.as_str().parse().ok()?;
        let skipped: usize = caps.get(3)?.as_str().parse().ok()?;

        Some(TestSummary {
            total: passed + failed + skipped,
            passed,
            failed,
            ignored: skipped,
            measured: 0,
            filtered: 0,
            execution_time: None,
        })
    }
}

impl Default for DotnetParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for DotnetParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let mut issues = Vec::new();

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("Formatted") {
                continue;
            }

            if let Some(issue) = self.parse_format_analyzer(line) {
                issues.push(issue);
                continue;
            }

            if let Some(issue) = self.parse_format_issue(line) {
                issues.push(issue);
                continue;
            }

            // Try MSBuild format with column first, then without column
            if let Some(issue) = self.parse_msbuild_error(line) {
                issues.push(issue);
            } else if let Some(issue) = self.parse_msbuild_error_no_col(line) {
                issues.push(issue);
            }
        }

        ParseResult::Full(issues)
    }
}

impl TestOutputParser for DotnetParser {
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();

        // 1. Extract compile issues from build output
        result.compile_issues = OutputParser::parse(self, output).data_or_default_owned();

        // 2. Parse test results
        let lines: Vec<&str> = output.lines().collect();

        for line in &lines {
            // Parse individual test case results
            if let Some(test_case) = self.parse_test_result_line(line) {
                match test_case.status {
                    TestStatus::Passed => result.passed_tests.push(test_case),
                    TestStatus::Failed => result.failed_tests.push(test_case),
                    TestStatus::Ignored(_) => result.ignored_tests.push(test_case),
                }
            }

            // Parse summary line
            if let Some(summary) = self.parse_test_summary_line(line) {
                result.test_summary = Some(summary);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_msbuild_error_with_col() {
        let parser = DotnetParser::new();
        let line = "src/Program.cs(10,5): error CS1001: Identifier expected [MyApp.csproj]";
        let issue = parser.parse_msbuild_error(line).unwrap();

        assert_eq!(issue.location.file_path, "src/Program.cs");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.code, Some("CS1001".to_string()));
        assert_eq!(issue.message, "Identifier expected");
        assert!(matches!(issue.level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_msbuild_warning() {
        let parser = DotnetParser::new();
        let line = "src/Util.cs(25,10): warning CA1050: Declare types in namespaces [MyApp.csproj]";
        let issue = parser.parse_msbuild_error(line).unwrap();

        assert_eq!(issue.location.file_path, "src/Util.cs");
        assert_eq!(issue.location.line_number, Some(25));
        assert_eq!(issue.location.column_number, Some(10));
        assert_eq!(issue.code, Some("CA1050".to_string()));
        assert!(matches!(issue.level, IssueLevel::Warning));
    }

    #[test]
    fn test_parse_msbuild_error_no_col() {
        let parser = DotnetParser::new();
        let line = "src/MyClass.cs(42): error CS0103: The name 'foo' does not exist in the current context [MyApp.csproj]";
        let issue = parser.parse_msbuild_error_no_col(line).unwrap();

        assert_eq!(issue.location.file_path, "src/MyClass.cs");
        assert_eq!(issue.location.line_number, Some(42));
        assert_eq!(issue.location.column_number, None);
        assert_eq!(issue.code, Some("CS0103".to_string()));
        assert!(matches!(issue.level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_msbuild_four_part_location() {
        let parser = DotnetParser::new();
        // MSBuild can emit file(line,col,end_line,end_col) format
        let line =
            "src/Util.cs(25,10,30,15): warning CA1050: Declare types in namespaces [MyApp.csproj]";
        let issue = parser.parse_msbuild_error(line).unwrap();

        assert_eq!(issue.location.file_path, "src/Util.cs");
        assert_eq!(issue.location.line_number, Some(25));
        assert_eq!(issue.location.column_number, Some(10));
    }

    #[test]
    fn test_parse_test_result_passed() {
        let parser = DotnetParser::new();
        let line = "Passed  Tests.UnitTest1.TestMethod1";
        let test_case = parser.parse_test_result_line(line).unwrap();

        assert_eq!(test_case.name, "Tests.UnitTest1.TestMethod1");
        assert!(matches!(test_case.status, TestStatus::Passed));
    }

    #[test]
    fn test_parse_test_result_failed() {
        let parser = DotnetParser::new();
        let line = "Failed  Tests.UnitTest1.TestMethod2";
        let test_case = parser.parse_test_result_line(line).unwrap();

        assert_eq!(test_case.name, "Tests.UnitTest1.TestMethod2");
        assert!(matches!(test_case.status, TestStatus::Failed));
    }

    #[test]
    fn test_parse_test_summary() {
        let parser = DotnetParser::new();
        let line = "Passed! - Failed: 0, Passed: 42, Skipped: 3, Total: 45";
        let summary = parser.parse_test_summary_line(line).unwrap();

        assert_eq!(summary.total, 45);
        assert_eq!(summary.passed, 42);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.ignored, 3);
    }

    #[test]
    fn test_parse_failed_test_summary() {
        let parser = DotnetParser::new();
        let line = "Failed! - Failed: 2, Passed: 40, Skipped: 3, Total: 45";
        let summary = parser.parse_test_summary_line(line).unwrap();

        assert_eq!(summary.total, 45);
        assert_eq!(summary.passed, 40);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.ignored, 3);
    }

    #[test]
    fn test_parse_full_output() {
        let parser = DotnetParser::new();
        let output = "\
Build FAILED.
src/Program.cs(10,5): error CS1001: Identifier expected [MyApp.csproj]
src/Util.cs(25,10): warning CA1050: Declare types in namespaces [MyApp.csproj]
    0 Warning(s)
    1 Error(s)
";

        let result = parser.parse(output);
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 2);

        let error_count = issues
            .iter()
            .filter(|i| matches!(i.level, IssueLevel::Error))
            .count();
        let warning_count = issues
            .iter()
            .filter(|i| matches!(i.level, IssueLevel::Warning))
            .count();
        assert_eq!(error_count, 1);
        assert_eq!(warning_count, 1);
    }
}
