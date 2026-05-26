//! Maven Analyzer
//! Run mvn commands and parse the output

use crate::core::{
    AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder, OutputParser,
    TechStack, TestAnalyzer, TestOptions, TestOutputParser,
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
        use crate::core::run_analysis_pipeline;
        use crate::core::stream::StageResult;

        let builder = self.create_command_builder(options);
        let output = builder.execute()?;

        println!("Parsing Maven output...");
        match run_analysis_pipeline(&self.parser, &output, options) {
            StageResult::Complete(result) | StageResult::Degraded(result, _) => {
                println!("Found {} issues", result.total_issues);
                Ok(result)
            }
            StageResult::Failed(warnings) => {
                Err(AnalyzerError::ParseError(warnings.join("; ")))
            }
        }
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

impl TestAnalyzer for MavenAnalyzer {
    fn supports_test(&self) -> bool {
        true
    }

    fn build_test_command(&self, options: &TestOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("mvn");

        let command_str = if options.command.is_empty() {
            "test -q"
        } else {
            &options.command
        };

        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        if let Some(ref filter) = options.filter {
            builder = builder.arg("-Dtest").arg(filter);
        }

        if let Some(ref test_file) = options.test {
            builder = builder.arg("-Dtest").arg(test_file);
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
