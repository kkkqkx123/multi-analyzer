//! Clang Analyzer
//! Runs Clang compiler commands and parses output

use crate::core::{
    AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::parser::ClangParser;

pub struct ClangAnalyzer {
    parser: ClangParser,
}

impl ClangAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: ClangParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options.subcommand.as_ref().map(|s| s.as_str()).unwrap_or("-fsyntax-only");

        // Build command directly from the command string
        let mut builder = CommandBuilder::new("clang++");
        
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

        // Add JSON output option if requested
        if options.json_output {
            builder = builder.arg("-fdiagnostics-format=json");
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

    fn filter_issues(&self, result: AnalysisResult, options: &AnalyzeOptions) -> AnalysisResult {
        result.filter_by_options(options)
    }
}

impl Default for ClangAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for ClangAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Clang
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["clang", "clang++"]
    }

    fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
        let builder = self.create_command_builder(options);
        let output = builder.execute()?;

        println!("Parsing output...");
        let issues = self.parser.parse(&output).data_or_default_owned();
        println!("Found {} issues", issues.len());

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
