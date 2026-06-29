//! Mypy Analyzer
//! Run mypy commands and parse the output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::parser::MypyParser;

pub struct MypyAnalyzer {
    parser: MypyParser,
}

impl MypyAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: MypyParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("--show-column-numbers .");
        CommandBuilder::from_exec_string(&format!("mypy {}", command_str))
    }
}

impl Default for MypyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for MypyAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Mypy
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["mypy", "python"]
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
