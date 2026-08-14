//! MSVC Analyzer
//! Runs Microsoft Visual C++ compiler commands and parses output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::parser::MsvcParser;

pub struct MsvcAnalyzer {
    parser: MsvcParser,
}

impl MsvcAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: MsvcParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("/Zs");

        // Build command directly from the command string
        let mut builder = CommandBuilder::new("cl");

        // Add base warning options
        builder = builder.arg("/W4").arg("/EHsc").arg("/nologo");

        // Split the command string and add as arguments
        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // Add C++ standard if specified
        if let Some(ref std_ver) = options.cpp_standard {
            let std_flag = match std_ver.as_str() {
                "c++11" => "/std:c++11",
                "c++14" => "/std:c++14",
                "c++17" => "/std:c++17",
                "c++20" => "/std:c++20",
                "c++latest" => "/std:c++latest",
                _ => "/std:c++17",
            };
            builder = builder.arg(std_flag);
        }

        // Add include paths
        for include_path in &options.include_paths {
            builder = builder.arg("/I").arg(include_path);
        }

        // Add macro definitions
        for define in &options.defines {
            builder = builder.arg(format!("/D{}", define));
        }

        // Add source files
        for file in &options.target_files {
            builder = builder.arg(file);
        }

        builder
    }
}

impl Default for MsvcAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for MsvcAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Msvc
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["msvc", "cl"]
    }

    fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
        let builder = self.create_command_builder(options);
        let result = run_analyzer(&builder, &self.parser, options)?;
        eprintln!("Found {} issues", result.total_issues);
        Ok(result)
    }

    fn parser(&self) -> &dyn OutputParser {
        &self.parser
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
