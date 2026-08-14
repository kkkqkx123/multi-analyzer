//! NPM/Node.js Analyzer
//! Run the npm/pnpm/yarn command and parse the output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack, TestAnalyzer, TestOptions, TestOutputParser,
};

use super::parser::NpmParser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
}

impl PackageManager {
    fn as_str(&self) -> &str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
        }
    }

    fn build_command(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("");

        // Build command directly from the command string
        let mut builder = CommandBuilder::new(self.as_str());

        // Split the command string and add as arguments
        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // Set CI=true to disable turbo TUI mode and get plain text output
        builder.env("CI", "true")
    }
}

pub struct NpmAnalyzer {
    parser: NpmParser,
    package_manager: PackageManager,
}

impl NpmAnalyzer {
    pub fn new(package_manager: PackageManager) -> Self {
        Self {
            parser: NpmParser::new(),
            package_manager,
        }
    }

    pub fn npm() -> Self {
        Self::new(PackageManager::Npm)
    }

    pub fn pnpm() -> Self {
        Self::new(PackageManager::Pnpm)
    }

    pub fn yarn() -> Self {
        Self::new(PackageManager::Yarn)
    }
}

impl Default for NpmAnalyzer {
    fn default() -> Self {
        Self::new(PackageManager::Npm)
    }
}

impl BuildAnalyzer for NpmAnalyzer {
    fn tech_stack(&self) -> TechStack {
        match self.package_manager {
            PackageManager::Npm => TechStack::Npm,
            PackageManager::Pnpm => TechStack::Pnpm,
            PackageManager::Yarn => TechStack::Yarn,
        }
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec![self.package_manager.as_str(), "node"]
    }

    fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
        let builder = self.package_manager.build_command(options);
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

    fn as_test_analyzer(&self) -> Option<&dyn TestAnalyzer> {
        Some(self)
    }
}

impl TestAnalyzer for NpmAnalyzer {
    fn supports_test(&self) -> bool {
        true
    }

    fn build_test_command(&self, options: &TestOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new(self.package_manager.as_str());

        // Default to "test" if no command specified
        let command_str = if options.command.is_empty() {
            "test"
        } else {
            &options.command
        };

        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // Adding test filters (test name mode)
        if let Some(ref filter) = options.filter {
            builder = builder.arg(filter);
        }

        builder
    }

    fn test_parser(&self) -> Option<&dyn TestOutputParser> {
        Some(&self.parser)
    }
}
