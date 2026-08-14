//! Output processing pipeline for command output analysis.
//!
//! Provides:
//!   - `OutputPostProcessor` (in utils.rs) — the single-source-of-truth 9-stage pipeline
//!   - `LineFilter` trait + `PostProcessLineFilter` — line-by-line adapter for streaming
//!   - `run_analyzer()` — unified entry point for all plugins, auto-selects processor

use std::sync::OnceLock;

use crate::config::filter_compiler::compile_toml_filter;
use crate::config::filter_registry::FilterRegistry;
use crate::config::tee_writer;
use crate::core::analyzer::AnalyzerError;
use crate::core::command::CommandBuilder;
use crate::core::parser::{OutputParser, ParseResult};
use crate::core::tracking::TimingGuard;
use crate::core::types::{AnalysisResult, AnalyzeOptions, Issue, IssueLevel, Location};
use crate::core::utils::OutputPostProcessor;

fn filter_registry() -> &'static FilterRegistry {
    static REGISTRY: OnceLock<FilterRegistry> = OnceLock::new();
    REGISTRY.get_or_init(FilterRegistry::load)
}

/// Result of a pipeline stage.
#[derive(Debug)]
enum StageResult<T> {
    Complete(T),
    Failed(Vec<String>),
}

/// Unified entry point for all plugins.
///
/// Automatically resolves the post-processor by merging:
/// 1. Base config from `AnalyzeOptions` (ANSI, TUI, noise/keep patterns, line limits)
/// 2. TOML filter config from `FilterRegistry` (replace, short-circuit, on-empty, etc.)
///
/// Uses streaming mode (line-by-line filtering) for memory-efficient processing.
pub fn run_analyzer(
    builder: &CommandBuilder,
    parser: &dyn OutputParser,
    options: &AnalyzeOptions,
) -> Result<AnalysisResult, AnalyzerError> {
    let command_str = builder.command_string();
    let tech = command_str.split_whitespace().next().unwrap_or("unknown");

    let mut guard = TimingGuard::start(tech, &command_str);

    // NOTE: `raw` / `raw-json` are structured *report* formats
    // (LEVEL|CODE|FILE:LINE:COL|MESSAGE and JSON lines respectively), not a
    // passthrough of the child process output. They therefore go through the
    // exact same parse pipeline as the other formats; only the reporter differs.

    let processor = resolve_processor(builder, options);
    let line_filter = PostProcessLineFilter::new(processor);
    // Suppress the "Running: ..." echo under quiet mode so machine-readable
    // output stays clean. The echo is written to stderr regardless; quiet mode
    // simply flips the `verbose` flag before execution.
    let mut exec_builder = builder.clone();
    exec_builder = exec_builder.with_verbose(!options.verbosity.is_minimal());
    let result = exec_builder.execute_streaming(FilterMode::Streaming(Box::new(line_filter)))?;

    let exit_code = result.exit_code;
    let total_output = result.filtered.len()
        + result.raw_stdout.as_ref().map_or(0, |s| s.len())
        + result.raw_stderr.as_ref().map_or(0, |s| s.len());
    guard.set_output_bytes(total_output);

    let raw_for_tee = match (&result.raw_stdout, &result.raw_stderr) {
        (Some(out), Some(err)) => format!("{}\n{}", out, err),
        (Some(out), None) => out.clone(),
        (None, Some(err)) => err.clone(),
        (None, None) => String::new(),
    };
    if !raw_for_tee.is_empty() {
        tee_writer::tee_raw(&raw_for_tee, &command_str, exit_code);
    }

    let command_success = exit_code == 0;
    match parse_and_analyze(parser, &result.filtered, options) {
        StageResult::Complete(mut r) => {
            r.exit_code = Some(exit_code);
            r.command_failed = !command_success;
            // Fallback: command failed but no issues parsed → surface the raw
            // error so the caller sees *why* it failed (e.g. a missing
            // dependency like `eslint`/`turbo` not being installed).
            if !command_success && r.total_issues == 0 {
                let raw = match (&result.raw_stdout, &result.raw_stderr) {
                    (Some(out), Some(err)) if !err.is_empty() => format!(
                        "Command failed (exit code {}). Raw output:\n{}\n{}",
                        exit_code, out, err
                    ),
                    (Some(out), _) => format!(
                        "Command failed (exit code {}). Raw output:\n{}",
                        exit_code, out
                    ),
                    (None, Some(err)) if !err.is_empty() => format!(
                        "Command failed (exit code {}). Raw output:\n{}",
                        exit_code, err
                    ),
                    _ => format!(
                        "Command failed (exit code {}). No output captured.",
                        exit_code
                    ),
                };
                r.add_issue(Issue::new(
                    IssueLevel::Error,
                    raw,
                    Location::new("unknown"),
                ));
            }
            guard.complete(Some(exit_code), r.total_issues, command_success);
            Ok(r)
        }
        StageResult::Failed(w) => {
            let issue_count = 0;
            guard.complete(Some(exit_code), issue_count, false);
            // Always set command_failed when parser fails
            Err(AnalyzerError::ParseError(w.join("; ")))
        }
    }
}

/// Resolve the OutputPostProcessor by merging AnalyzeOptions with TOML filter configs.
fn resolve_processor(builder: &CommandBuilder, options: &AnalyzeOptions) -> OutputPostProcessor {
    let base = OutputPostProcessor::from_options(options);
    let command_str = builder.command_string();

    let registry = filter_registry();
    if let Some(toml_config) = registry.find_filter(&command_str) {
        let toml_processor = compile_toml_filter(toml_config);
        return OutputPostProcessor::merge(base, toml_processor);
    }

    base
}

/// Line-by-line filter for streaming output processing.
/// Implementations process individual lines and decide whether to
/// emit them (Some) or drop them (None).
pub trait LineFilter: Send {
    /// Process a single output line.
    /// Return `Some(filtered_line)` to keep, `None` to drop.
    fn feed_line(&mut self, line: &str) -> Option<String>;

    /// Called after all lines have been fed.
    /// Returns remaining lines to append (e.g., on-empty message, short-circuit result).
    fn on_complete(&mut self) -> Vec<String> {
        Vec::new()
    }
}

/// Mode for processing command output.
pub enum FilterMode<'a> {
    /// Apply a line-by-line filter during command execution (memory efficient).
    Streaming(Box<dyn LineFilter + 'a>),
    /// No filtering, passthrough stdin/stdout/stderr directly.
    Passthrough,
}

/// Result of streaming command execution.
#[derive(Debug)]
pub struct StreamResult {
    /// Process exit code
    pub exit_code: i32,
    /// Raw stdout captured during streaming (always captured, regardless of verbosity)
    pub raw_stdout: Option<String>,
    /// Raw stderr captured during streaming (always captured, regardless of verbosity)
    pub raw_stderr: Option<String>,
    /// Filtered output (result of applying LineFilter to both stdout and stderr)
    pub filtered: String,
}

/// Post-processing adapter that wraps OutputPostProcessor as a LineFilter.
///
/// Delegates each stage to OutputPostProcessor's per-line methods,
/// accumulating text for batch-only stages (short-circuit, on-empty)
/// that are evaluated in `on_complete()`.
pub struct PostProcessLineFilter {
    processor: OutputPostProcessor,
    line_count: usize,
    accumulated: String,
    capped: bool,
}

impl PostProcessLineFilter {
    /// Build a streaming filter that owns the given OutputPostProcessor.
    pub fn new(processor: OutputPostProcessor) -> Self {
        Self {
            processor,
            line_count: 0,
            accumulated: String::new(),
            capped: false,
        }
    }
}

impl LineFilter for PostProcessLineFilter {
    fn feed_line(&mut self, line: &str) -> Option<String> {
        if self.capped {
            return None;
        }

        // Stage 1: ANSI stripping (per-line)
        let mut result = self.processor.process_line_ansi(line);

        // Stage 2: Regex replace (per-line)
        result = self.processor.process_line_replace(&result);

        // Stage 4: TUI frame/border detection (per-line)
        result = self.processor.process_line_tui(&result)?;

        // Stage 5: Noise filtering (per-line)
        if self.processor.is_noise_line(&result) {
            return None;
        }

        // Stage 6: Keep filtering (per-line)
        if !self.processor.is_keep_line(&result) {
            return None;
        }

        // Stage 7: Per-line length truncation (per-line)
        result = self.processor.process_line_truncate(&result);

        // Stage 8: Max lines check
        if let Some(max_lines) = self.processor.max_lines {
            if self.line_count >= max_lines {
                self.capped = true;
                return None;
            }
        }

        self.line_count += 1;
        if !self.accumulated.is_empty() {
            self.accumulated.push('\n');
        }
        self.accumulated.push_str(&result);
        Some(result)
    }

    fn on_complete(&mut self) -> Vec<String> {
        // Stage 3: Short-circuit (batch — needs full accumulated output)
        if let Some(msg) = self.processor.check_short_circuit(&self.accumulated) {
            return vec![msg];
        }

        // Stage 9: On-empty fallback
        if self.line_count == 0 {
            if let Some(ref msg) = self.processor.on_empty_message {
                return vec![msg.clone()];
            }
        }
        Vec::new()
    }
}

/// Run parse + analyze on already-preprocessed output.
/// Used internally by run_analyzer after streaming execution.
fn parse_and_analyze(
    parser: &dyn OutputParser,
    processed_output: &str,
    options: &AnalyzeOptions,
) -> StageResult<AnalysisResult> {
    let result = parser.parse(processed_output);
    match result {
        ParseResult::Full(issues) | ParseResult::Degraded(issues, _) => {
            let result = AnalysisResult::from_issues(issues);
            StageResult::Complete(result.filter_by_options(options))
        }
        ParseResult::Passthrough(raw) => StageResult::Failed(vec![format!(
            "Parser fell back to passthrough ({} chars)",
            raw.len()
        )]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_process_line_filter_basic() {
        let processor = OutputPostProcessor::new()
            .with_strip_ansi(true)
            .with_noise_patterns(vec!["^debug:".to_string()]);
        let mut filter = PostProcessLineFilter::new(processor);

        assert!(filter.feed_line("debug: skip").is_none());
        assert_eq!(
            filter.feed_line("error: fail"),
            Some("error: fail".to_string())
        );

        let tail = filter.on_complete();
        assert!(tail.is_empty());
    }

    #[test]
    fn test_post_process_line_filter_ansi_strip() {
        let processor = OutputPostProcessor::new().with_strip_ansi(true);
        let mut filter = PostProcessLineFilter::new(processor);
        let result = filter.feed_line("\x1b[31merror\x1b[0m: something");
        assert_eq!(result, Some("error: something".to_string()));
    }

    #[test]
    fn test_post_process_line_filter_tui_border() {
        let processor = OutputPostProcessor::new().with_strip_tui_frames(true);
        let mut filter = PostProcessLineFilter::new(processor);
        assert!(filter
            .feed_line("\u{250c}\u{2500}\u{2500}\u{2500}\u{2510}")
            .is_none());
        let result = filter.feed_line("\u{2502} ./main.go:10:5: error");
        assert_eq!(result, Some("./main.go:10:5: error".to_string()));
    }

    #[test]
    fn test_post_process_line_filter_preserves_indentation() {
        // Regression: TUI stripping must not remove leading whitespace from
        // regular output lines, otherwise the CMake block collector loses
        // continuation lines (message becomes just the command name).
        let processor = OutputPostProcessor::new().with_strip_tui_frames(true);
        let mut filter = PostProcessLineFilter::new(processor);
        let result = filter.feed_line("  Cannot find source file:");
        assert_eq!(result, Some("  Cannot find source file:".to_string()));
        let result = filter.feed_line("    src/main.cpp");
        assert_eq!(result, Some("    src/main.cpp".to_string()));
    }

    #[test]
    fn test_post_process_line_filter_keep_only() {
        let processor =
            OutputPostProcessor::new().with_keep_patterns(vec!["error|warning".to_string()]);
        let mut filter = PostProcessLineFilter::new(processor);
        assert!(filter.feed_line("cache hit").is_none());
        assert_eq!(
            filter.feed_line("error: failed"),
            Some("error: failed".to_string())
        );
    }

    #[test]
    fn test_post_process_line_filter_max_lines() {
        let processor = OutputPostProcessor::new().with_max_lines(2);
        let mut filter = PostProcessLineFilter::new(processor);
        assert!(filter.feed_line("line1").is_some());
        assert!(filter.feed_line("line2").is_some());
        assert!(filter.feed_line("line3").is_none());
    }

    #[test]
    fn test_post_process_line_filter_on_empty() {
        let processor = OutputPostProcessor::new()
            .with_keep_patterns(vec!["$^".to_string()])
            .with_on_empty("all good");
        let mut filter = PostProcessLineFilter::new(processor);
        assert!(filter.feed_line("should be dropped").is_none());
        let tail = filter.on_complete();
        assert_eq!(tail, vec!["all good"]);
    }

    #[test]
    fn test_post_process_line_filter_short_circuit() {
        let processor = OutputPostProcessor::new().with_short_circuits(vec![
            crate::core::utils::ShortCircuitRule {
                pattern: "SUCCESS".to_string(),
                message: "Build OK".to_string(),
                unless: None,
            },
        ]);
        let mut filter = PostProcessLineFilter::new(processor);
        filter.feed_line("BUILD SUCCESS");
        let tail = filter.on_complete();
        assert_eq!(tail, vec!["Build OK"]);
    }
}
