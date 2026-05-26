//! Gradle Analyzer
//! Run gradle commands and parse the output

use crate::core::{
    AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder, OutputParser,
    TechStack,
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
        let command_str = options.subcommand.as_ref().map(|s| s.as_str()).unwrap_or("compileJava --quiet");
        CommandBuilder::from_exec_string(&format!("gradle {}", command_str))
    }

    fn filter_issues(&self, result: AnalysisResult, options: &AnalyzeOptions) -> AnalysisResult {
        result.filter_by_options(options)
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
        let output = builder.execute()?;

        println!("Parsing Gradle output...");
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
