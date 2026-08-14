//! Ruff Analyzer
//! Run ruff check commands and parse the JSON output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::parser::RuffParser;

pub struct RuffAnalyzer {
    parser: RuffParser,
}

impl RuffAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: RuffParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("check --output-format json .");
        CommandBuilder::from_exec_string(&format!("ruff {}", command_str))
    }
}

impl Default for RuffAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for RuffAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Ruff
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["ruff", "python-lint"]
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
