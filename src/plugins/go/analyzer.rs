//! Go Analyzer
//! Run go commands and parse the output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack, TestAnalyzer, TestOptions, TestOutputParser,
};

use super::parser::GoParser;

pub struct GoAnalyzer {
    parser: GoParser,
}

impl GoAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: GoParser::new(),
        }
    }

    /// Create command builder based on command string
    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("build ./...");
        CommandBuilder::from_exec_string(&format!("go {}", command_str))
    }
}

impl Default for GoAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for GoAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::GoBuild
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["go", "golang"]
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

impl TestAnalyzer for GoAnalyzer {
    fn supports_test(&self) -> bool {
        true
    }

    fn build_test_command(&self, options: &TestOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("go");

        // Default to "test -v ./..." if no command specified
        let command_str = if options.command.is_empty() {
            "test -v ./..."
        } else {
            &options.command
        };

        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // Add package path if specified
        if let Some(ref package) = options.package {
            builder = builder.arg(package);
        }

        // Add test filter if specified
        if let Some(ref filter) = options.filter {
            builder = builder.arg("-run").arg(filter);
        }

        // Add timeout if specified
        if let Some(timeout) = options.timeout {
            builder = builder.arg("-timeout").arg(format!("{}s", timeout));
        }

        // Add race detector if enabled
        if options.race {
            builder = builder.arg("-race");
        }

        // Add cover profile if specified
        if options.coverage {
            builder = builder.arg("-cover");
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
    fn test_go_analyzer_name() {
        let analyzer = GoAnalyzer::new();
        assert_eq!(analyzer.name(), "go");
    }

    #[test]
    fn test_go_analyzer_supported_commands() {
        let analyzer = GoAnalyzer::new();
        let commands = analyzer.supported_commands();
        assert!(commands.contains(&"go"));
        assert!(commands.contains(&"golang"));
    }
}
