//! Report Generator Module
//! Support for multiple output formats (Markdown, JSON, HTML, Raw)

use super::types::{AnalysisResult, ReportFormat, TestAnalysisResult, Verbosity};
use std::path::Path;

mod html;
mod json;
mod markdown;
mod raw;

pub use html::HtmlReporter;
pub use json::JsonReporter;
pub use markdown::MarkdownReporter;
pub use raw::RawReporter;

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
        // Default implementation ignores options for backward compatibility
        let _ = options;
        self.generate(result)
    }

    /// Generate a test-specific report
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
            ReportFormat::Raw => Box::new(RawReporter::new()),
            ReportFormat::RawJson => Box::new(RawReporter::new_json_lines()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Issue, IssueLevel, Location};

    fn sample_result() -> AnalysisResult {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Error,
            "undefined reference to `foo`",
            Location::new("src/main.rs").with_line(10).with_column(5),
        ));
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "unused variable",
            Location::new("src/lib.rs").with_line(20),
        ));
        r
    }

    #[test]
    fn test_reporter_factory_create_markdown() {
        let reporter = ReporterFactory::create(ReportFormat::Markdown);
        let result = sample_result();
        let report = reporter.generate(&result).unwrap();
        assert!(report.contains("Analysis Report"));
        assert!(report.contains("undefined reference"));
    }

    #[test]
    fn test_reporter_factory_create_json() {
        let reporter = ReporterFactory::create(ReportFormat::Json);
        let result = sample_result();
        let report = reporter.generate(&result).unwrap();
        assert!(report.contains("\"metadata\""));
        assert!(report.contains("\"total\": 2"));
    }

    #[test]
    fn test_reporter_factory_create_html() {
        let reporter = ReporterFactory::create(ReportFormat::Html);
        let result = sample_result();
        let report = reporter.generate(&result).unwrap();
        assert!(report.contains("<!DOCTYPE html>"));
        assert!(report.contains("undefined reference"));
    }

    #[test]
    fn test_reporter_factory_create_raw() {
        let reporter = ReporterFactory::create(ReportFormat::Raw);
        let result = sample_result();
        let report = reporter.generate(&result).unwrap();
        assert!(!report.is_empty());
    }

    #[test]
    fn test_reporter_factory_create_raw_json() {
        let reporter = ReporterFactory::create(ReportFormat::RawJson);
        let result = sample_result();
        let report = reporter.generate(&result).unwrap();
        assert!(!report.is_empty());
    }

    #[test]
    fn test_report_options_short_circuit_enabled() {
        let opts = ReportOptions {
            success_short_circuit: true,
            tech_stack: Some("cargo check".to_string()),
            ..Default::default()
        };
        assert_eq!(
            opts.short_circuit_message(),
            Some("cargo check: no issues found".to_string())
        );
    }

    #[test]
    fn test_report_options_short_circuit_disabled() {
        let opts = ReportOptions {
            success_short_circuit: false,
            ..Default::default()
        };
        assert!(opts.short_circuit_message().is_none());
    }

    #[test]
    fn test_report_options_short_circuit_default_tech_stack() {
        let opts = ReportOptions {
            success_short_circuit: true,
            tech_stack: None,
            ..Default::default()
        };
        assert_eq!(opts.short_circuit_message(), Some("analysis: no issues found".to_string()));
    }

    #[test]
    fn test_reporter_write_to_file() {
        let reporter = ReporterFactory::create(ReportFormat::Markdown);
        let result = sample_result();
        let report = reporter.generate(&result).unwrap();

        let tmp_path = std::env::temp_dir().join("analyzer_test_report.md");
        reporter.write_to_file(&report, &tmp_path).unwrap();
        assert!(tmp_path.exists());
        let content = std::fs::read_to_string(&tmp_path).unwrap();
        assert_eq!(content, report);
        let _ = std::fs::remove_file(&tmp_path);
    }

    #[test]
    fn test_generate_with_options_short_circuit() {
        let reporter = ReporterFactory::create(ReportFormat::Markdown);
        let empty = AnalysisResult::new();
        let opts = ReportOptions {
            success_short_circuit: true,
            tech_stack: Some("cargo check".to_string()),
            ..Default::default()
        };
        let report = reporter.generate_with_options(&empty, opts).unwrap();
        assert_eq!(report, "cargo check: no issues found");
    }
}
