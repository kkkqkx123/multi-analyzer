//! Golangci-lint Analyzer
//! Run golangci-lint commands and parse the output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::parser::GoParser;

pub struct GolangciLintAnalyzer {
    parser: GoParser,
}

impl GolangciLintAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: GoParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("run ./...");
        CommandBuilder::from_exec_string(&format!("golangci-lint {}", command_str))
    }
}

impl Default for GolangciLintAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for GolangciLintAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::GolangciLint
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["golangci-lint"]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_golangci_lint_analyzer_name() {
        let analyzer = GolangciLintAnalyzer::new();
        assert_eq!(analyzer.name(), "golangci-lint");
    }

    #[test]
    fn test_golangci_lint_analyzer_supported_commands() {
        let analyzer = GolangciLintAnalyzer::new();
        let commands = analyzer.supported_commands();
        assert!(commands.contains(&"golangci-lint"));
    }
}
