//! Analyzer trait definition
//! defines the interface to the build tool analyzer

use super::parser::OutputParser;
use super::test_analyzer::TestAnalyzer;
use super::types::{AnalysisResult, AnalyzeOptions, TechStack};
use std::time::Duration;

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
    pub fn check_applicable(
        &self,
        stack: TechStack,
        project_path: &std::path::Path,
    ) -> Result<(), AnalyzerError> {
        let analyzer = self.get(stack).ok_or(AnalyzerError::NotApplicable)?;

        // Check if the analyzer supports any of the commands for this project
        let supported = analyzer.supported_commands();
        if supported.is_empty() {
            return Err(AnalyzerError::NotApplicable);
        }

        // Check if project has required files (basic check)
        let has_required_files = match stack {
            TechStack::Cargo => project_path.join("Cargo.toml").exists(),
            TechStack::Maven => project_path.join("pom.xml").exists(),
            TechStack::Gradle => {
                project_path.join("build.gradle").exists()
                    || project_path.join("build.gradle.kts").exists()
            }
            TechStack::Npm | TechStack::Pnpm | TechStack::Yarn => {
                project_path.join("package.json").exists()
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::OutputParser;
    use crate::core::types::Issue;
    use crate::core::parser::ParseResult;
    use std::any::Any;

    /// A mock analyzer for testing PluginRegistry
    struct MockAnalyzer {
        stack: TechStack,
        commands: Vec<&'static str>,
    }

    impl MockAnalyzer {
        fn new(stack: TechStack) -> Self {
            let commands = match stack {
                TechStack::Cargo => vec!["check", "build", "test"],
                TechStack::Maven => vec!["compile", "test"],
                _ => vec!["run"],
            };
            Self { stack, commands }
        }
    }

    impl BuildAnalyzer for MockAnalyzer {
        fn tech_stack(&self) -> TechStack {
            self.stack
        }

        fn supported_commands(&self) -> Vec<&str> {
            self.commands.clone()
        }

        fn analyze(&self, _options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
            Ok(AnalysisResult::new())
        }

        fn parser(&self) -> &dyn OutputParser {
            panic!("not used in tests")
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[test]
    fn test_registry_new_empty() {
        let registry = PluginRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = PluginRegistry::new();
        registry.register(Box::new(MockAnalyzer::new(TechStack::Cargo)));
        let analyzer = registry.get(TechStack::Cargo);
        assert!(analyzer.is_some());
        assert_eq!(analyzer.unwrap().tech_stack(), TechStack::Cargo);
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let registry = PluginRegistry::new();
        assert!(registry.get(TechStack::Cargo).is_none());
    }

    #[test]
    fn test_registry_list() {
        let mut registry = PluginRegistry::new();
        registry.register(Box::new(MockAnalyzer::new(TechStack::Cargo)));
        registry.register(Box::new(MockAnalyzer::new(TechStack::Maven)));
        let names = registry.list();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"cargo"));
        assert!(names.contains(&"maven"));
    }

    #[test]
    fn test_check_applicable_no_file_check_stacks() {
        let mut registry = PluginRegistry::new();
        // Stacks without file checks should always return Ok
        registry.register(Box::new(MockAnalyzer::new(TechStack::Mypy)));
        registry.register(Box::new(MockAnalyzer::new(TechStack::Pytest)));
        let result = registry.check_applicable(TechStack::Mypy, std::path::Path::new("/nonexistent"));
        assert!(result.is_ok());
        let result = registry.check_applicable(TechStack::Pytest, std::path::Path::new("/nonexistent"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_registry_default() {
        let registry = PluginRegistry::default();
        assert!(registry.list().is_empty());
    }

    // ── AnalyzerError Display ──────────────────────────────────────

    #[test]
    fn test_analyzer_error_command_failed_display() {
        let err = AnalyzerError::CommandFailed("command not found".to_string());
        assert_eq!(err.to_string(), "Command failed: command not found");
    }

    #[test]
    fn test_analyzer_error_parse_error_display() {
        let err = AnalyzerError::ParseError("invalid syntax".to_string());
        assert_eq!(err.to_string(), "Parse error: invalid syntax");
    }

    #[test]
    fn test_analyzer_error_io_error_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = AnalyzerError::IoError(io_err);
        assert!(err.to_string().contains("IO error:"));
    }

    #[test]
    fn test_analyzer_error_not_applicable_display() {
        let err = AnalyzerError::NotApplicable;
        assert_eq!(
            err.to_string(),
            "Analyzer not applicable for this project"
        );
    }

    #[test]
    fn test_analyzer_error_timeout_display() {
        let err = AnalyzerError::Timeout(Duration::from_secs(30));
        assert_eq!(err.to_string(), "Command timed out after 30s");
    }

    #[test]
    fn test_analyzer_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let err: AnalyzerError = io_err.into();
        assert!(matches!(err, AnalyzerError::IoError(_)));
    }
}
