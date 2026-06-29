//! Gradle Analyzer
//! Run gradle commands and parse the output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack, TestAnalyzer, TestOptions, TestOutputParser,
};

use super::parser::GradleParser;

pub struct GradleAnalyzer {
    parser: GradleParser,
}

impl GradleAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: GradleParser::new(),
        }
    }

    /// Creating a command builder
    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("compileJava --quiet");
        CommandBuilder::from_exec_string(&format!("gradle {}", command_str))
    }
}

impl Default for GradleAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for GradleAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Gradle
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["gradle", "gradlew", "java"]
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

impl TestAnalyzer for GradleAnalyzer {
    fn supports_test(&self) -> bool {
        true
    }

    fn build_test_command(&self, options: &TestOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("gradle");

        let command_str = if options.command.is_empty() {
            "test --quiet"
        } else {
            &options.command
        };

        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        if let Some(ref filter) = options.filter {
            builder = builder.arg("--tests").arg(filter);
        }

        if let Some(ref test_file) = options.test {
            builder = builder.arg("--tests").arg(test_file);
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
    fn test_gradle_analyzer_name() {
        let analyzer = GradleAnalyzer::new();
        assert_eq!(analyzer.name(), "gradle");
    }

    #[test]
    fn test_supported_commands() {
        let analyzer = GradleAnalyzer::new();
        let commands = analyzer.supported_commands();
        assert!(commands.contains(&"gradle"));
        assert!(commands.contains(&"gradlew"));
        assert!(commands.contains(&"java"));
    }
}
