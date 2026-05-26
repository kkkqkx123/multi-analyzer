//! GCC Analyzer
//! Runs GCC compiler commands and parses output

use crate::core::{
    AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder, OutputParser,
    TechStack,
};

use super::parser::GccParser;

pub struct GccAnalyzer {
    parser: GccParser,
}

impl GccAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: GccParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options.subcommand.as_ref().map(|s| s.as_str()).unwrap_or("-fsyntax-only");

        // Build command directly from the command string
        let mut builder = CommandBuilder::new("g++");
        
        // Add base warning options
        builder = builder.arg("-Wall").arg("-Wextra").arg("-Wpedantic");
        
        // Split the command string and add as arguments
        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // Add C++ standard if specified
        if let Some(ref std_ver) = options.cpp_standard {
            builder = builder.arg(format!("-std={}", std_ver));
        }

        // Add include paths
        for include_path in &options.include_paths {
            builder = builder.arg("-I").arg(include_path);
        }

        // Add macro definitions
        for define in &options.defines {
            builder = builder.arg(format!("-D{}", define));
        }

        // Add source files
        for file in &options.target_files {
            builder = builder.arg(file);
        }

        builder
    }
}

impl Default for GccAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for GccAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Gcc
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["gcc", "g++"]
    }

    fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
        use crate::core::run_analysis_pipeline;
        use crate::core::stream::StageResult;

        let builder = self.create_command_builder(options);
        let output = builder.execute()?;

        println!("Parsing output...");
        match run_analysis_pipeline(&self.parser, &output, options) {
            StageResult::Complete(result) | StageResult::Degraded(result, _) => {
                println!("Found {} issues", result.total_issues);
                Ok(result)
            }
            StageResult::Failed(warnings) => {
                Err(AnalyzerError::ParseError(warnings.join("; ")))
            }
        }
    }

    fn parser(&self) -> &dyn OutputParser {
        &self.parser
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
