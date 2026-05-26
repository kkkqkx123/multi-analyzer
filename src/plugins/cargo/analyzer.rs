//! Cargo Analyzer
//! Run cargo commands and parse the output

use crate::core::{
    AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder, OutputParser,
    ParsedTestOutput, TechStack, TestAnalyzer, TestAnalyzerError, TestOptions,
    TestOutputParser,
};

use super::parser::CargoParser;

pub struct CargoAnalyzer {
    parser: CargoParser,
}

impl CargoAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: CargoParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options.subcommand.as_ref().map(|s| s.as_str()).unwrap_or("check");

        // Build command directly from the command string
        let mut builder = CommandBuilder::new("cargo");
        
        // Split the command string and add as arguments
        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // === Workspace Options ===
        if options.workspace {
            builder = builder.arg("--workspace");
        }
        for pkg in &options.package {
            builder = builder.arg("--package").arg(pkg);
        }
        for pkg in &options.exclude {
            builder = builder.arg("--exclude").arg(pkg);
        }

        // === Target Options ===
        if options.lib {
            builder = builder.arg("--lib");
        }
        for name in &options.bin {
            builder = builder.arg("--bin").arg(name);
        }
        if options.bins {
            builder = builder.arg("--bins");
        }
        for name in &options.test {
            builder = builder.arg("--test").arg(name);
        }
        if options.tests {
            builder = builder.arg("--tests");
        }
        for name in &options.example {
            builder = builder.arg("--example").arg(name);
        }
        if options.examples {
            builder = builder.arg("--examples");
        }
        for name in &options.bench {
            builder = builder.arg("--bench").arg(name);
        }
        if options.benches {
            builder = builder.arg("--benches");
        }
        if options.all_targets {
            builder = builder.arg("--all-targets");
        }

        // === Feature Options ===
        if !options.features.is_empty() {
            builder = builder.arg("--features").arg(options.features.join(","));
        }
        if options.all_features {
            builder = builder.arg("--all-features");
        }
        if options.no_default_features {
            builder = builder.arg("--no-default-features");
        }

        builder.arg("--message-format=short")
    }

    /// Creating a test command
    fn create_test_command(&self, options: &TestOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("cargo");
        
        // Default to "test" if no command specified
        let command_str = if options.command.is_empty() {
            "test"
        } else {
            &options.command
        };
        
        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        if options.lib_only {
            builder = builder.arg("--lib");
        }

        if let Some(ref bin) = options.bin {
            builder = builder.arg("--bin").arg(bin);
        }

        if let Some(ref test) = options.test {
            builder = builder.arg("--test").arg(test);
        }

        if options.doc_only {
            builder = builder.arg("--doc");
        }

        if let Some(ref filter) = options.filter {
            builder = builder.arg(filter);
        }

        // Add --nocapture to get the full output
        builder = builder.arg("--").arg("--nocapture");

        builder
    }

    fn filter_issues(&self, result: AnalysisResult, options: &AnalyzeOptions) -> AnalysisResult {
        result.filter_by_options(options)
    }
}

impl Default for CargoAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for CargoAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Cargo
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["cargo", "rust"]
    }

    fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
        let builder = self.create_command_builder(options);
        let output = builder.execute()?;

        println!("Parsing output...");
        let issues = self.parser.parse(&output).data_or_default_owned();
        println!("Found {} issues", issues.len());

        // Validate that we got valid output
        if output.contains("error: could not compile") && issues.is_empty() {
            return Err(AnalyzerError::ParseError(
                "Failed to parse cargo output: compilation failed but no issues were extracted".to_string()
            ));
        }

        let result = AnalysisResult::from_issues(issues);
        Ok(self.filter_issues(result, options))
    }

    fn parser(&self) -> &dyn OutputParser {
        &self.parser
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TestAnalyzer for CargoAnalyzer {
    fn supports_test(&self) -> bool {
        true
    }

    fn run_tests(&self, options: &TestOptions) -> Result<ParsedTestOutput, TestAnalyzerError> {
        let builder = self.create_test_command(options);
        let output = builder
            .execute()
            .map_err(|e| TestAnalyzerError::CommandFailed(e.to_string()))?;

        // Parsing Output with TestOutputParser
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
