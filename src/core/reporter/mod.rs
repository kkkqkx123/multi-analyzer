//! Report Generator Module
//! Support for multiple output formats (Markdown, JSON, HTML)

use std::path::Path;
use super::types::{AnalysisResult, ReportFormat, TestAnalysisResult, Verbosity};

mod markdown;
mod json;
mod html;

pub use markdown::MarkdownReporter;
pub use json::JsonReporter;
pub use html::HtmlReporter;

/// Report Generation Error
#[derive(Debug)]
pub enum ReporterError {
    IoError(std::io::Error),
}

impl std::fmt::Display for ReporterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReporterError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ReporterError {}

impl From<std::io::Error> for ReporterError {
    fn from(e: std::io::Error) -> Self {
        ReporterError::IoError(e)
    }
}

/// Report generation options
#[derive(Debug, Default, Clone)]
pub struct ReportOptions {
    /// Show all issues without truncation
    pub verbose: Verbosity,
    /// Enable success short-circuit: when no issues found, output a single-line confirmation
    pub success_short_circuit: bool,
    /// Tech stack name for short-circuit message (e.g. "cargo check")
    pub tech_stack: Option<String>,
}

impl ReportOptions {
    /// Create new report options with verbose mode
    pub fn verbose() -> Self {
        Self { verbose: Verbosity::Verbose, success_short_circuit: false, tech_stack: None }
    }

    /// Create new report options with success short-circuit enabled
    pub fn with_short_circuit(mut self, tech_stack: impl Into<String>) -> Self {
        self.success_short_circuit = true;
        self.tech_stack = Some(tech_stack.into());
        self
    }

    /// Get the short-circuit message when no issues found
    pub fn short_circuit_message(&self) -> Option<String> {
        if self.success_short_circuit {
            let name = self.tech_stack.as_deref().unwrap_or("analysis");
            Some(format!("{}: no issues found", name))
        } else {
            None
        }
    }
}

/// Report generator trait
pub trait Reporter: Send + Sync {
    /// Generate report content
    fn generate(&self, result: &AnalysisResult) -> Result<String, ReporterError>;

    /// Generate report content with options
    fn generate_with_options(
        &self,
        result: &AnalysisResult,
        options: ReportOptions,
    ) -> Result<String, ReporterError> {
        // Success short-circuit: if no issues found and short-circuit is enabled,
        // output a single-line confirmation instead of full report
        if options.success_short_circuit && result.total_issues == 0 {
            if let Some(msg) = options.short_circuit_message() {
                return Ok(msg);
            }
        }
        // Default implementation ignores options for backward compatibility
        let _ = options;
        self.generate(result)
    }

    /// Generate test report content
    fn generate_test_report(&self, result: &TestAnalysisResult) -> Result<String, ReporterError> {
        // Default implementation: call General Report Generation
        self.generate(&result.compile_result)
    }

    /// Generate test report content with options
    fn generate_test_report_with_options(
        &self,
        result: &TestAnalysisResult,
        options: ReportOptions,
    ) -> Result<String, ReporterError> {
        // Default implementation: call General Report Generation with options
        self.generate_with_options(&result.compile_result, options)
    }

    /// Write report to file
    fn write_to_file(&self, content: &str, path: &Path) -> Result<(), ReporterError> {
        std::fs::write(path, content)?;
        Ok(())
    }
}

/// Report Generator Factory
pub struct ReporterFactory;

impl ReporterFactory {
    /// Create a report generator based on the format
    pub fn create(format: ReportFormat) -> Box<dyn Reporter> {
        match format {
            ReportFormat::Markdown => Box::new(MarkdownReporter::new()),
            ReportFormat::Json => Box::new(JsonReporter::new()),
            ReportFormat::Html => Box::new(HtmlReporter::new()),
        }
    }
}
