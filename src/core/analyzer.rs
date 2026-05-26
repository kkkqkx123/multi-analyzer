//! Analyzer trait definition
//! defines the interface to the build tool analyzer

use std::time::Duration;
use super::types::{AnalysisResult, AnalyzeOptions, TechStack};
use super::parser::OutputParser;
use super::test_analyzer::TestAnalyzer;

/// Analyzer Error Type
#[derive(Debug)]
pub enum AnalyzerError {
    CommandFailed(String),
    ParseError(String),
    IoError(std::io::Error),
    NotApplicable,
    Timeout(Duration),
}

impl std::fmt::Display for AnalyzerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalyzerError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
            AnalyzerError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            AnalyzerError::IoError(e) => write!(f, "IO error: {}", e),
            AnalyzerError::NotApplicable => write!(f, "Analyzer not applicable for this project"),
            AnalyzerError::Timeout(d) => write!(f, "Command timed out after {:?}", d),
        }
    }
}

impl std::error::Error for AnalyzerError {}

impl From<std::io::Error> for AnalyzerError {
    fn from(e: std::io::Error) -> Self {
        AnalyzerError::IoError(e)
    }
}

/// Build tool analyzer trait
/// Implement this trait to support new build tools
pub trait BuildAnalyzer: Send + Sync {
    /// Get the technology stack
    fn tech_stack(&self) -> TechStack;

    /// Get the name of the technology stack
    fn name(&self) -> &str {
        self.tech_stack().as_str()
    }

    /// Get supported command aliases
    fn supported_commands(&self) -> Vec<&str>;

    /// Run Analysis Command
    fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError>;

    /// Get parser
    fn parser(&self) -> &dyn OutputParser;

    /// Convert to Any for downcasting
    #[allow(dead_code)]
    fn as_any(&self) -> &dyn std::any::Any;

    /// Get the test analyzer implementation if supported
    fn as_test_analyzer(&self) -> Option<&dyn TestAnalyzer> {
        None
    }
}

/// Plugin Registry
pub struct PluginRegistry {
    analyzers: Vec<Box<dyn BuildAnalyzer>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            analyzers: Vec::new(),
        }
    }

    /// Registration Analyzer
    pub fn register(&mut self, analyzer: Box<dyn BuildAnalyzer>) {
        self.analyzers.push(analyzer);
    }

    /// Get analyzer by TechStack
    pub fn get(&self, stack: TechStack) -> Option<&dyn BuildAnalyzer> {
        self.analyzers
            .iter()
            .find(|a| a.tech_stack() == stack)
            .map(|b| b.as_ref())
    }

    /// List all registered analyzers
    pub fn list(&self) -> Vec<&str> {
        self.analyzers.iter().map(|a| a.name()).collect()
    }

    /// Check if an analyzer is applicable for the given project path
    pub fn check_applicable(&self, stack: TechStack, project_path: &std::path::Path) -> Result<(), AnalyzerError> {
        let analyzer = self.get(stack)
            .ok_or(AnalyzerError::NotApplicable)?;

        // Check if the analyzer supports any of the commands for this project
        let supported = analyzer.supported_commands();
        if supported.is_empty() {
            return Err(AnalyzerError::NotApplicable);
        }

        // Check if project has required files (basic check)
        let has_required_files = match stack {
            TechStack::Cargo => project_path.join("Cargo.toml").exists(),
            TechStack::Maven => project_path.join("pom.xml").exists(),
            TechStack::Gradle => project_path.join("build.gradle").exists() || project_path.join("build.gradle.kts").exists(),
            TechStack::Npm | TechStack::Pnpm | TechStack::Yarn => project_path.join("package.json").exists(),
            _ => true, // For other stacks, assume applicable
        };

        if !has_required_files {
            return Err(AnalyzerError::NotApplicable);
        }

        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
