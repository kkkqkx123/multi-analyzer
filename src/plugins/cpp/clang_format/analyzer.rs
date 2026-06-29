//! ClangFormat Analyzer
//! Run clang-format commands and parse the formatting output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::parser::ClangFormatParser;

pub struct ClangFormatAnalyzer {
    parser: ClangFormatParser,
}

impl ClangFormatAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: ClangFormatParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("--dry-run --Werror .");
        CommandBuilder::from_exec_string(&format!("clang-format {}", command_str))
    }
}

impl Default for ClangFormatAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for ClangFormatAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::ClangFormat
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["clang-format", "cpp-format"]
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
