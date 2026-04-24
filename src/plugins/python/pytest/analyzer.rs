//! Pytest Analyzer
//! Run pytest commands and parse the output

use crate::core::{
    AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder, OutputParser,
    ParsedTestOutput, TechStack, TestAnalyzer, TestAnalyzerError, TestOptions,
    TestOutputParser,
};

use super::parser::PytestParser;

pub struct PytestAnalyzer {
    parser: PytestParser,
}

impl PytestAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: PytestParser::new(),
        }
    }

    /// Create command builder for pytest
    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options.subcommand.as_ref().map(|s| s.as_str()).unwrap_or("-v --color=no --tb=short");

        // Build command directly from the command string
        let mut builder = CommandBuilder::new("pytest");
        
        // Split the command string and add as arguments
        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        builder
    }

    /// Create test command builder
    fn create_test_command(&self, options: &TestOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("pytest");
        
        // Default to "-v --color=no" if no command specified
        let command_str = if options.command.is_empty() {
            "-v --color=no"
        } else {
            &options.command
        };
        
        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // Add test filter if specified
        if let Some(ref filter) = options.filter {
            builder = builder.arg("-k").arg(filter);
        }

        // Run specific test file or directory if specified
        if let Some(ref test) = options.test {
            builder = builder.arg(test);
        }

        // Add extra arguments
        for arg in &options.extra_args {
            builder = builder.arg(arg);
        }

        builder
    }

    fn filter_issues(&self, result: AnalysisResult, options: &AnalyzeOptions) -> AnalysisResult {
        if !options.filter_warnings && options.filter_paths.is_empty() {
            return result;
        }

        let mut filtered = AnalysisResult::new();

        for (file_path, issues) in result.issues_by_file {
            if !options.filter_paths.is_empty() {
                let matches = options
                    .filter_paths
                    .iter()
                    .any(|filter| file_path.contains(filter));
                if !matches {
                    continue;
                }
            }

            for issue in issues {
                if options.filter_warnings && matches!(issue.level, crate::core::IssueLevel::Warning)
                {
                    continue;
                }

                filtered.add_issue(issue);
            }
        }

        filtered
    }
}

impl Default for PytestAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for PytestAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Pytest
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["pytest", "py.test", "python-test"]
    }

    fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
        let builder = self.create_command_builder(options);
        let output = builder.execute()?;

        println!("Parsing pytest output...");
        let parsed = self.parser.parse_test_output(&output);
        println!(
            "Found {} passed, {} failed, {} skipped",
            parsed.passed_tests.len(),
            parsed.failed_tests.len(),
            parsed.ignored_tests.len()
        );

        // Convert test failures to issues for the analysis result
        let mut result = AnalysisResult::new();

        // Add failed tests as issues
        for test in &parsed.failed_tests {
            if let Some(ref location) = test.location {
                let issue = crate::core::Issue::new(
                    crate::core::IssueLevel::Error,
                    format!("Test failed: {}", test.name),
                    location.clone(),
                )
                .with_context(test.failure_details.clone().unwrap_or_default());
                result.add_issue(issue);
            }
        }

        Ok(self.filter_issues(result, options))
    }

    fn parser(&self) -> &dyn OutputParser {
        &self.parser
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TestAnalyzer for PytestAnalyzer {
    fn supports_test(&self) -> bool {
        true
    }

    fn run_tests(&self, options: &TestOptions) -> Result<ParsedTestOutput, TestAnalyzerError> {
        let builder = self.create_test_command(options);
        let output = builder
            .execute()
            .map_err(|e| TestAnalyzerError::CommandFailed(e.to_string()))?;

        // Parse test output
        let parsed = self
            .test_parser()
            .ok_or(TestAnalyzerError::NotSupported)?
            .parse_test_output(&output);

        Ok(parsed)
    }

    fn test_parser(&self) -> Option<&dyn TestOutputParser> {
        Some(&self.parser)
    }
}
