//! Cargo Analyzer
//! Run cargo commands and parse the output

use crate::core::{
    AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder, OutputParser,
    TechStack, TestAnalyzer, TestOptions, TestOutputParser,
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
        use crate::core::run_analysis_pipeline;
        use crate::core::stream::StageResult;

        let builder = self.create_command_builder(options);
        let output = builder.execute()?;

        println!("Parsing output...");
        let result = match run_analysis_pipeline(&self.parser, &output, options) {
            StageResult::Complete(r) | StageResult::Degraded(r, _) => r,
            StageResult::Failed(warnings) => {
                return Err(AnalyzerError::ParseError(warnings.join("; ")));
            }
        };

        // Validate that we got valid output
        if output.contains("error: could not compile") && result.total_issues == 0 {
            return Err(AnalyzerError::ParseError(
                "Failed to parse cargo output: compilation failed but no issues were extracted".to_string()
            ));
        }

        println!("Found {} issues", result.total_issues);
        Ok(result)
    }

    fn parser(&self) -> &dyn OutputParser {
        &self.parser
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_test_analyzer(&self) -> Option<&dyn TestAnalyzer> {
        Some(self)
    }
}

impl TestAnalyzer for CargoAnalyzer {
    fn supports_test(&self) -> bool {
        true
    }

    fn build_test_command(&self, options: &TestOptions) -> CommandBuilder {
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

    fn test_parser(&self) -> Option<&dyn TestOutputParser> {
        Some(&self.parser)
    }
}
