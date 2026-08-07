//! Test analyzer trait definition
//! Define a uniform interface for test execution

use super::command::CommandBuilder;
use super::types::{AnalyzeOptions, Issue, TestAnalysisResult, TestCase, TestSummary};

/// Test Analyzer Error
#[derive(Debug)]
pub enum TestAnalyzerError {
    CommandFailed(String),
    NotSupported,
}

impl std::fmt::Display for TestAnalyzerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestAnalyzerError::CommandFailed(msg) => write!(f, "Test command failed: {}", msg),
            TestAnalyzerError::NotSupported => {
                write!(f, "Test analysis not supported for this analyzer")
            }
        }
    }
}

impl std::error::Error for TestAnalyzerError {}

/// Parsed test output
#[derive(Debug, Default)]
pub struct ParsedTestOutput {
    /// Problems at the compilation stage
    pub compile_issues: Vec<Issue>,
    /// Test Summary
    pub test_summary: Option<TestSummary>,
    /// Failed Test Cases
    pub failed_tests: Vec<TestCase>,
    /// Test cases passed
    pub passed_tests: Vec<TestCase>,
    /// Neglected Test Cases
    pub ignored_tests: Vec<TestCase>,
}

impl ParsedTestOutput {
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<ParsedTestOutput> for TestAnalysisResult {
    fn from(output: ParsedTestOutput) -> Self {
        use super::types::AnalysisResult;

        let compile_result = AnalysisResult::from_issues(output.compile_issues);

        // Use from_compile_result to create the base result
        let mut result = TestAnalysisResult::from_compile_result(compile_result);

        // Add test-specific data
        result.test_summary = output.test_summary;
        result.failed_tests = output.failed_tests;
        result.passed_tests = output.passed_tests;
        result.ignored_tests = output.ignored_tests;
        result.has_test_output = true;

        result
    }
}

/// Test output parser trait
pub trait TestOutputParser: Send + Sync {
    /// Parsing Test Output
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput;
}

/// Test Options
#[derive(Debug, Default, Clone)]
pub struct TestOptions {
    /// The command string to execute (e.g., "test", "run test:unit")
    pub command: String,
    /// Test filters (e.g. test name pattern)
    pub filter: Option<String>,
    /// Run library tests only
    pub lib_only: bool,
    /// Run only the tests for the specified binary
    pub bin: Option<String>,
    /// Running Integration Tests Only
    pub test: Option<String>,
    /// Running Documentation Tests Only
    pub doc_only: bool,
    /// Package path (for Go: ./..., ./pkg/...)
    pub package: Option<String>,
    /// Test timeout in seconds
    pub timeout: Option<u64>,
    /// Enable race detector
    pub race: bool,
    /// Enable coverage reporting
    pub coverage: bool,
    /// Other parameters
    pub extra_args: Vec<String>,
}

impl From<&AnalyzeOptions> for TestOptions {
    fn from(options: &AnalyzeOptions) -> Self {
        let command = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str().to_string())
            .unwrap_or_default();
        Self {
            command,
            filter: None,
            lib_only: false,
            bin: None,
            test: None,
            doc_only: false,
            package: None,
            timeout: None,
            race: false,
            coverage: false,
            extra_args: Vec::new(),
        }
    }
}

/// Test Analyzer trait
/// Implement this trait to support test execution and analysis
pub trait TestAnalyzer: Send + Sync {
    /// Whether to support test analysis
    fn supports_test(&self) -> bool;

    /// Run the test and return the parsed output
    /// Default implementation uses build_test_command + test_parser
    fn run_tests(&self, options: &TestOptions) -> Result<ParsedTestOutput, TestAnalyzerError> {
        let builder = self.build_test_command(options);
        let output = builder
            .execute()
            .map_err(|e| TestAnalyzerError::CommandFailed(e.to_string()))?;
        let parsed = self
            .test_parser()
            .ok_or(TestAnalyzerError::NotSupported)?
            .parse_test_output(&output);
        Ok(parsed)
    }

    /// Build the test command
    fn build_test_command(&self, options: &TestOptions) -> CommandBuilder;

    /// Getting the test parser
    fn test_parser(&self) -> Option<&dyn TestOutputParser> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Issue, IssueLevel, Location, SubCommand, TestCase, TestStatus, TestSummary};

    #[test]
    fn test_test_options_from_analyze_options_with_subcommand() {
        let analyze_opts = AnalyzeOptions {
            subcommand: Some(SubCommand::new("test")),
            ..Default::default()
        };
        let test_opts = TestOptions::from(&analyze_opts);
        assert_eq!(test_opts.command, "test");
        // All other fields should be default
        assert!(test_opts.filter.is_none());
        assert!(!test_opts.lib_only);
        assert!(test_opts.bin.is_none());
        assert!(test_opts.test.is_none());
        assert!(!test_opts.doc_only);
        assert!(test_opts.package.is_none());
        assert!(test_opts.timeout.is_none());
        assert!(!test_opts.race);
        assert!(!test_opts.coverage);
        assert!(test_opts.extra_args.is_empty());
    }

    #[test]
    fn test_test_options_from_analyze_options_no_subcommand() {
        let analyze_opts = AnalyzeOptions::default();
        let test_opts = TestOptions::from(&analyze_opts);
        assert_eq!(test_opts.command, "");
    }

    #[test]
    fn test_test_options_from_analyze_options_with_complex_command() {
        let analyze_opts = AnalyzeOptions {
            subcommand: Some(SubCommand::new("test --features integration")),
            ..Default::default()
        };
        let test_opts = TestOptions::from(&analyze_opts);
        assert_eq!(test_opts.command, "test --features integration");
    }

    #[test]
    fn test_parsed_test_output_to_test_analysis_result() {
        let output = ParsedTestOutput {
            compile_issues: vec![
                Issue::new(IssueLevel::Error, "compile error", Location::new("src/main.rs")),
            ],
            test_summary: Some(TestSummary {
                total: 10,
                passed: 8,
                failed: 2,
                ignored: 0,
                measured: 0,
                filtered: 0,
                execution_time: Some(1.5),
            }),
            failed_tests: vec![
                TestCase::new("test_fail", TestStatus::Failed)
                    .with_failure_details("assertion failed"),
            ],
            passed_tests: vec![
                TestCase::new("test_pass", TestStatus::Passed),
            ],
            ignored_tests: vec![],
        };

        let result: TestAnalysisResult = output.into();
        assert_eq!(result.compile_result.total_issues, 1);
        assert!(result.has_test_output);
        assert_eq!(result.failed_tests.len(), 1);
        assert_eq!(result.passed_tests.len(), 1);
        assert!(result.ignored_tests.is_empty());
        assert_eq!(result.test_summary.as_ref().unwrap().total, 10);
        assert_eq!(result.total_tests(), 2);
        assert!(!result.all_passed());
    }

    #[test]
    fn test_parsed_test_output_empty() {
        let output = ParsedTestOutput::new();
        let result: TestAnalysisResult = output.into();
        assert_eq!(result.compile_result.total_issues, 0);
        assert!(result.has_test_output);
        assert!(result.test_summary.is_none());
        assert!(result.failed_tests.is_empty());
        assert!(result.passed_tests.is_empty());
        assert!(result.ignored_tests.is_empty());
        assert!(result.all_passed());
    }

    #[test]
    fn test_test_analyzer_error_display() {
        let err = TestAnalyzerError::CommandFailed("timeout".to_string());
        assert_eq!(err.to_string(), "Test command failed: timeout");

        let err = TestAnalyzerError::NotSupported;
        assert_eq!(err.to_string(), "Test analysis not supported for this analyzer");
    }
}
