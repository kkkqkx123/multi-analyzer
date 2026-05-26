//! CMake Analyzer
//! Runs CMake commands and parses output

use crate::core::{
    AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::parser::CMakeParser;

pub struct CMakeAnalyzer {
    parser: CMakeParser,
}

impl CMakeAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: CMakeParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options.subcommand.as_ref().map(|s| s.as_str()).unwrap_or("--build build");
        CommandBuilder::from_exec_string(&format!("cmake {}", command_str))
    }

    fn filter_issues(&self, result: AnalysisResult, options: &AnalyzeOptions) -> AnalysisResult {
        result.filter_by_options(options)
    }
}

impl Default for CMakeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for CMakeAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::CMake
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["cmake", "cmake-build"]
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
