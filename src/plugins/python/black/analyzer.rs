//! Black Analyzer
//! Run black commands and parse the formatting output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::parser::BlackParser;

pub struct BlackAnalyzer {
    parser: BlackParser,
}

impl BlackAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: BlackParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("--check .");
        CommandBuilder::from_exec_string(&format!("black {}", command_str))
    }
}

impl Default for BlackAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for BlackAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Black
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["black", "python-format"]
    }

    fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
        let builder = self.create_command_builder(options);
        let result = run_analyzer(&builder, &self.parser, options)?;
        println!("Found {} issues", result.total_issues);
        Ok(result)
    }

    fn parser(&self) -> &dyn OutputParser {
        &self.parser
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
