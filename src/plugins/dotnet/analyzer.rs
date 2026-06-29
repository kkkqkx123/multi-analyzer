//! .NET Analyzer
//! Run dotnet commands and parse the output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack, TestAnalyzer, TestOptions, TestOutputParser,
};

use super::parser::DotnetParser;

pub struct DotnetAnalyzer {
    parser: DotnetParser,
}

impl DotnetAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: DotnetParser::new(),
        }
    }

    /// Create command builder based on command string
    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("build");
        let mut builder = CommandBuilder::new("dotnet");

        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // Pass project file if specified via package
        for pkg in &options.package {
            builder = builder.arg("--project").arg(pkg);
        }

        // Configuration (Debug/Release)
        if let Some(ref config) = options.target {
            builder = builder.arg("--configuration").arg(config);
        }

        builder
    }
}

impl Default for DotnetAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for DotnetAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::Dotnet
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["dotnet", "msbuild", "csharp"]
    }

    fn name(&self) -> &'static str {
        "dotnet"
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

impl TestAnalyzer for DotnetAnalyzer {
    fn supports_test(&self) -> bool {
        true
    }

    fn build_test_command(&self, options: &TestOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("dotnet");

        // Default to "test" if no command specified
        let command_str = if options.command.is_empty() {
            "test"
        } else {
            &options.command
        };

        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // Pass project file if specified
        if let Some(ref package) = options.package {
            builder = builder.arg("--project").arg(package);
        }

        // Add test filter if specified
        if let Some(ref filter) = options.filter {
            builder = builder.arg("--filter").arg(filter);
        }

        // Add settings file if specified
        if !options.extra_args.is_empty() {
            for arg in &options.extra_args {
                builder = builder.arg(arg);
            }
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
    fn test_dotnet_analyzer_name() {
        let analyzer = DotnetAnalyzer::new();
        assert_eq!(analyzer.name(), "dotnet");
    }

    #[test]
    fn test_dotnet_analyzer_supported_commands() {
        let analyzer = DotnetAnalyzer::new();
        let commands = analyzer.supported_commands();
        assert!(commands.contains(&"dotnet"));
        assert!(commands.contains(&"msbuild"));
    }
}
