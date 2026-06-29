//! Ruby Analyzer
//! Run Ruby-related commands and parse the output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack, TestAnalyzer, TestOptions, TestOutputParser,
};

use super::parser::RubyParser;

pub struct RubyAnalyzer {
    parser: RubyParser,
}

impl RubyAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: RubyParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("");
        let lower = command_str.to_lowercase();

        // Detect command type and auto-inject --format json where possible
        if (lower.starts_with("rubocop") || lower.starts_with("rspec"))
            && !command_str.contains("--format")
            && !command_str.contains("-f")
        {
            let mut builder = CommandBuilder::new("bundle");
            builder = builder.arg("exec");

            for arg in command_str.split_whitespace() {
                builder = builder.arg(arg);
            }

            builder = builder.arg("--format").arg("json");
            builder
        } else {
            // For other Ruby commands (rake, minitest, plain ruby), use as-is
            CommandBuilder::from_exec_string(&format!("bundle exec {}", command_str))
        }
    }
}

impl Default for RubyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for RubyAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Rubocop
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["ruby", "rails", "rubocop", "rspec", "rake"]
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

    fn as_test_analyzer(&self) -> Option<&dyn TestAnalyzer> {
        Some(self)
    }
}

impl TestAnalyzer for RubyAnalyzer {
    fn supports_test(&self) -> bool {
        true
    }

    fn build_test_command(&self, options: &TestOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("bundle");
        builder = builder.arg("exec");

        let command_str = if options.command.is_empty() {
            "rspec"
        } else {
            &options.command
        };

        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // Auto-inject JSON format for RSpec
        if command_str.starts_with("rspec")
            && !command_str.contains("--format")
            && !command_str.contains("-f")
        {
            builder = builder.arg("--format").arg("json");
        }

        if let Some(ref filter) = options.filter {
            builder = builder.arg("--example").arg(filter);
        }

        builder
    }

    fn test_parser(&self) -> Option<&dyn TestOutputParser> {
        Some(&self.parser)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ruby_analyzer_name() {
        let analyzer = RubyAnalyzer::new();
        assert_eq!(analyzer.name(), "rubocop");
    }

    #[test]
    fn test_ruby_analyzer_supported_commands() {
        let analyzer = RubyAnalyzer::new();
        let commands = analyzer.supported_commands();
        assert!(commands.contains(&"ruby"));
        assert!(commands.contains(&"rspec"));
    }
}
