//! Maven Analyzer
//! Run mvn commands and parse the output

use crate::core::{
    AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder, OutputParser,
    TechStack,
};

use super::parser::MavenParser;

pub struct MavenAnalyzer {
    parser: MavenParser,
}

impl MavenAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: MavenParser::new(),
        }
    }

    /// Creating a command builder
    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options.subcommand.as_ref().map(|s| s.as_str()).unwrap_or("compile -q");
        CommandBuilder::from_exec_string(&format!("mvn {}", command_str))
    }

    fn filter_issues(&self, result: AnalysisResult, options: &AnalyzeOptions) -> AnalysisResult {
        result.filter_by_options(options)
    }
}

impl Default for MavenAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for MavenAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Maven
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["maven", "mvn", "java"]
    }

    fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
        let builder = self.create_command_builder(options);
        let output = builder.execute()?;

        println!("Parsing Maven output...");
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
    fn test_maven_analyzer_name() {
        let analyzer = MavenAnalyzer::new();
        assert_eq!(analyzer.name(), "maven");
    }

    #[test]
    fn test_supported_commands() {
        let analyzer = MavenAnalyzer::new();
        let commands = analyzer.supported_commands();
        assert!(commands.contains(&"maven"));
        assert!(commands.contains(&"mvn"));
        assert!(commands.contains(&"java"));
    }
}
