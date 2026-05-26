//! Processing pipeline for command output analysis
//! Provides a stage-based pipeline abstraction for chaining analysis steps.
//!
//! # Integration Status
//! This module offers an optional processing pipeline that individual plugins
//! can use instead of directly calling parsers + filters. It supports:
//!   - Stage-based processing (Parse → Filter → Analyze)
//!   - Degradation: `StageResult::Complete / Degraded / Failed`
//!   - `ProcessingPipeline::run()` for a complete parse-filter-analyze flow
//!
//! TODO: Integrate into plugin `analyze()` methods via delegation. Currently
//!       every plugin manually calls parser + filters + analyzer in sequence.
//!       The pipeline can reduce boilerplate but requires:
//!         1. Plugins to accept a `ProcessingPipeline` (or build one from config)
//!         2. Consistent error/degradation handling across all plugins
//!       This is deferred because the current per-plugin approach is simpler
//!       and more explicit during active development.

#![allow(dead_code)]

use crate::core::parser::{OutputParser, ParseResult};
use crate::core::types::{AnalysisResult, AnalyzeOptions, Issue};
use crate::core::utils::OutputPostProcessor;

/// Pipeline error with degradation support
#[derive(Debug)]
pub enum PipelineError {
    StageFailed(String),
    EmptyOutput,
    IoError(std::io::Error),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::StageFailed(msg) => write!(f, "Pipeline stage failed: {}", msg),
            PipelineError::EmptyOutput => write!(f, "No output produced by pipeline"),
            PipelineError::IoError(e) => write!(f, "I/O error in pipeline: {}", e),
        }
    }
}

impl std::error::Error for PipelineError {}

impl From<std::io::Error> for PipelineError {
    fn from(e: std::io::Error) -> Self {
        PipelineError::IoError(e)
    }
}

/// Result of a pipeline stage
#[derive(Debug)]
pub enum StageResult<T> {
    Complete(T),
    Degraded(T, Vec<String>),
    Failed(Vec<String>),
}

impl<T> StageResult<T> {
    pub fn data(self) -> Option<T> {
        match self {
            StageResult::Complete(data) => Some(data),
            StageResult::Degraded(data, _) => Some(data),
            StageResult::Failed(_) => None,
        }
    }

    pub fn warnings(&self) -> &[String] {
        match self {
            StageResult::Complete(_) => &[],
            StageResult::Degraded(_, warnings) => warnings,
            StageResult::Failed(warnings) => warnings,
        }
    }
}

/// A single stage in the processing pipeline
pub trait PipelineStage<Input, Output>: Send {
    /// Process input and produce output
    fn process(&mut self, input: Input) -> StageResult<Output>;

    /// Name of this stage for diagnostics
    fn name(&self) -> &str;
}

/// Parsing stage: raw text -> issues
pub struct ParseStage<P> {
    parser: P,
    warnings: Vec<String>,
}

impl<P> ParseStage<P> {
    pub fn new(parser: P) -> Self {
        Self {
            parser,
            warnings: Vec::new(),
        }
    }
}

impl<P: crate::core::parser::OutputParser> PipelineStage<String, Vec<Issue>> for ParseStage<P> {
    fn process(&mut self, input: String) -> StageResult<Vec<Issue>> {
        let result = self.parser.parse(&input);
        match result {
            ParseResult::Full(issues) => StageResult::Complete(issues),
            ParseResult::Degraded(issues, warnings) => {
                self.warnings.extend(warnings.clone());
                StageResult::Degraded(issues, warnings)
            }
            ParseResult::Passthrough(raw) => {
                let warning = format!("Parser fell back to passthrough ({} chars)", raw.len());
                self.warnings.push(warning.clone());
                StageResult::Failed(vec![warning])
            }
        }
    }

    fn name(&self) -> &str {
        "parse"
    }
}

/// Filtering stage: filter issues by criteria
pub struct FilterStage;

impl FilterStage {
    pub fn new() -> Self {
        Self
    }

    /// Filter issues by file path patterns (keep only matching paths)
    pub fn include_paths(paths: Vec<String>) -> IncludePathsFilter {
        IncludePathsFilter { paths }
    }

    /// Suppress warning-level issues
    pub fn errors_only() -> LevelFilter {
        LevelFilter { errors_only: true }
    }
}

/// Filter issues to only include those matching specified paths
pub struct IncludePathsFilter {
    paths: Vec<String>,
}

impl PipelineStage<Vec<Issue>, Vec<Issue>> for IncludePathsFilter {
    fn process(&mut self, input: Vec<Issue>) -> StageResult<Vec<Issue>> {
        if self.paths.is_empty() {
            return StageResult::Complete(input);
        }
        let filtered: Vec<Issue> = input
            .into_iter()
            .filter(|issue| {
                self.paths
                    .iter()
                    .any(|p| issue.location.file_path.contains(p))
            })
            .collect();
        StageResult::Complete(filtered)
    }

    fn name(&self) -> &str {
        "include_paths"
    }
}

/// Filter issues by severity level
pub struct LevelFilter {
    errors_only: bool,
}

impl PipelineStage<Vec<Issue>, Vec<Issue>> for LevelFilter {
    fn process(&mut self, input: Vec<Issue>) -> StageResult<Vec<Issue>> {
        if !self.errors_only {
            return StageResult::Complete(input);
        }
        let filtered: Vec<Issue> = input
            .into_iter()
            .filter(|issue| matches!(issue.level, crate::core::types::IssueLevel::Error))
            .collect();
        StageResult::Complete(filtered)
    }

    fn name(&self) -> &str {
        "level_filter"
    }
}

/// Analysis stage: issues -> AnalysisResult
pub struct AnalyzeStage;

impl PipelineStage<Vec<Issue>, AnalysisResult> for AnalyzeStage {
    fn process(&mut self, input: Vec<Issue>) -> StageResult<AnalysisResult> {
        let result = AnalysisResult::from_issues(input);
        StageResult::Complete(result)
    }

    fn name(&self) -> &str {
        "analyze"
    }
}

/// Chained processing pipeline
pub struct ProcessingPipeline {
    warnings: Vec<String>,
}

impl ProcessingPipeline {
    pub fn new() -> Self {
        Self {
            warnings: Vec::new(),
        }
    }

    /// Run a complete analysis pipeline: parse -> filter -> analyze
    pub fn run<P: crate::core::parser::OutputParser>(
        &mut self,
        parser: P,
        output: &str,
        filter_level: Option<crate::core::types::IssueLevel>,
    ) -> StageResult<AnalysisResult> {
        // Stage 1: Parse
        let mut parse_stage = ParseStage::new(parser);
        let issues = match parse_stage.process(output.to_string()) {
            StageResult::Complete(issues) => issues,
            StageResult::Degraded(issues, warnings) => {
                self.warnings.extend(warnings);
                issues
            }
            StageResult::Failed(warnings) => {
                self.warnings.extend(warnings);
                return StageResult::Failed(std::mem::take(&mut self.warnings));
            }
        };

        // Stage 2: Filter
        let filtered = if let Some(level) = filter_level {
            match level {
                crate::core::types::IssueLevel::Error => {
                    let mut stage = LevelFilter { errors_only: true };
                    stage.process(issues).data().unwrap_or_default()
                }
                _ => issues,
            }
        } else {
            issues
        };

        // Stage 3: Analyze
        let mut analyze_stage = AnalyzeStage;
        match analyze_stage.process(filtered) {
            StageResult::Complete(result) => {
                if self.warnings.is_empty() {
                    StageResult::Complete(result)
                } else {
                    StageResult::Degraded(result, std::mem::take(&mut self.warnings))
                }
            }
            StageResult::Degraded(result, warnings) => {
                self.warnings.extend(warnings);
                StageResult::Degraded(result, std::mem::take(&mut self.warnings))
            }
            StageResult::Failed(warnings) => {
                self.warnings.extend(warnings);
                StageResult::Failed(std::mem::take(&mut self.warnings))
            }
        }
    }

    /// Collect all warnings accumulated during pipeline execution
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

/// Process output through a single stage
pub fn process_stage<I, O>(
    mut stage: impl PipelineStage<I, O>,
    input: I,
) -> StageResult<O> {
    stage.process(input)
}

/// Simple line-based output stream filter
pub struct LineFilter {
    /// Lines to exclude (prefix match)
    exclude_prefixes: Vec<String>,
    /// Maximum number of lines to keep
    max_lines: usize,
    kept_lines: Vec<String>,
}

impl LineFilter {
    pub fn new(max_lines: usize) -> Self {
        Self {
            exclude_prefixes: Vec::new(),
            max_lines,
            kept_lines: Vec::new(),
        }
    }

    /// Add a prefix to exclude from output
    pub fn exclude_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.exclude_prefixes.push(prefix.into());
        self
    }

    pub fn into_output(self) -> String {
        self.kept_lines.join("\n")
    }
}

impl PipelineStage<String, String> for LineFilter {
    fn process(&mut self, input: String) -> StageResult<String> {
        self.kept_lines.clear();
        for (i, line) in input.lines().enumerate() {
            if i >= self.max_lines {
                break;
            }
            let should_exclude = self
                .exclude_prefixes
                .iter()
                .any(|p| line.starts_with(p));
            if !should_exclude {
                self.kept_lines.push(line.to_string());
            }
        }
        StageResult::Complete(self.kept_lines.join("\n"))
    }

    fn name(&self) -> &str {
        "line_filter"
    }
}

/// Post-processing stage: apply ANSI stripping, noise filtering, line truncation.
pub struct PostProcessStage {
    processor: OutputPostProcessor,
}

impl PostProcessStage {
    pub fn new(processor: OutputPostProcessor) -> Self {
        Self { processor }
    }

    /// Create a default post-processor with sensible defaults for build output.
    pub fn with_defaults() -> Self {
        Self {
            processor: OutputPostProcessor::new(),
        }
    }
}

impl PipelineStage<String, String> for PostProcessStage {
    fn process(&mut self, input: String) -> StageResult<String> {
        let result = self.processor.process(&input);
        StageResult::Complete(result)
    }

    fn name(&self) -> &str {
        "post_process"
    }
}

/// Convenience function: run a complete analysis pipeline from raw output.
///
/// This is an optional helper that wraps the common flow:
///   `parser.parse() + AnalysisResult::from_issues() + result.filter_by_options()`
///
/// Any plugin's `analyze()` method can use this as a drop-in replacement for:
/// ```ignore
/// let issues = self.parser.parse(&output).data_or_default_owned();
/// let result = AnalysisResult::from_issues(issues);
/// Ok(self.filter_issues(result, options))
/// ```
///
/// # Example
/// ```ignore
/// use crate::core::stream::run_analysis_pipeline;
///
/// fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
///     let output = self.create_command_builder(options).execute()?;
///     match run_analysis_pipeline(&self.parser, &output, options) {
///         StageResult::Complete(r) | StageResult::Degraded(r, _) => Ok(r),
///         StageResult::Failed(warnings) => {
///             Err(AnalyzerError::ParseError(warnings.join("; ")))
///         }
///     }
/// }
/// ```
pub fn run_analysis_pipeline(
    parser: &dyn OutputParser,
    output: &str,
    options: &AnalyzeOptions,
) -> StageResult<AnalysisResult> {
    let result = parser.parse(output);
    match result {
        ParseResult::Full(issues) | ParseResult::Degraded(issues, _) => {
            let result = AnalysisResult::from_issues(issues);
            StageResult::Complete(result.filter_by_options(options))
        }
        ParseResult::Passthrough(raw) => {
            StageResult::Failed(vec![
                format!("Parser fell back to passthrough ({} chars)", raw.len())
            ])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{IssueLevel, Location};

    struct MockParser;

    impl OutputParser for MockParser {
        fn parse(&self, _output: &str) -> ParseResult<Vec<Issue>> {
            ParseResult::Full(vec![
                Issue::new(IssueLevel::Error, "test error", Location::new("src/main.rs")),
            ])
        }
    }

    #[test]
    fn test_parse_stage_complete() {
        let mut stage = ParseStage::new(MockParser);
        let result = stage.process("some output".to_string());
        let data = result.data();
        assert!(data.is_some());
        assert_eq!(data.unwrap().len(), 1);
    }

    #[test]
    fn test_analyze_stage() {
        let mut stage = AnalyzeStage;
        let issues = vec![
            Issue::new(IssueLevel::Error, "error 1", Location::new("a.rs")),
            Issue::new(IssueLevel::Warning, "warning 1", Location::new("b.rs")),
        ];
        let result = stage.process(issues);
        let data = result.data().unwrap();
        assert_eq!(data.total_issues, 2);
    }

    #[test]
    fn test_full_pipeline() {
        let mut pipeline = ProcessingPipeline::new();
        let result = pipeline.run(MockParser, "test output", None);
        match result {
            StageResult::Complete(analysis) => {
                assert_eq!(analysis.total_issues, 1);
                assert_eq!(analysis.error_count(), 1);
            }
            _ => panic!("Expected complete result"),
        }
    }

    #[test]
    fn test_line_filter() {
        let mut filter = LineFilter::new(10).exclude_prefix("DEBUG");
        let result = filter.process("INFO: ok\nDEBUG: skip\nWARN: maybe".to_string());
        let output = result.data().unwrap();
        assert!(!output.contains("DEBUG"));
        assert!(output.contains("INFO"));
        assert!(output.contains("WARN"));
    }

    #[test]
    fn test_stage_result_degraded() {
        let degraded: StageResult<Vec<i32>> = StageResult::Degraded(vec![1, 2], vec!["partial".to_string()]);
        let data = degraded.data();
        assert_eq!(data, Some(vec![1, 2]));
    }

    #[test]
    fn test_post_process_stage() {
        let processor = OutputPostProcessor::new()
            .with_noise_patterns(vec!["^debug:".to_string()]);
        let mut stage = PostProcessStage::new(processor);
        let result = stage.process("debug: verbose\nerror: failed".to_string());
        let output = result.data().unwrap();
        assert!(!output.contains("debug:"));
        assert!(output.contains("error:"));
    }
}