//! Analyze existing build logs without executing any command.
//!
//! Reuses the post-execution half of the pipeline from `stream`:
//! output post-processing (`OutputPostProcessor`) → parsing (`OutputParser`)
//! → option filtering (`parse_and_analyze`), so log-mode reports are
//! identical to executing-mode reports for the same text.

use std::fs;
use std::io::Read;
use std::path::Path;

use crate::core::analyzer::AnalyzerError;
use crate::core::parser::OutputParser;
use crate::core::stream::{parse_and_analyze, resolve_processor};
use crate::core::types::{AnalysisResult, AnalyzeOptions};

/// Maximum log file size accepted by `read_log_file` (64 MiB).
/// Larger files are truncated to this size before analysis.
const MAX_LOG_BYTES: u64 = 64 * 1024 * 1024;

/// Analyze build-log text captured in memory.
///
/// Mirrors the post-execution half of `stream::run_analyzer`:
/// 1. Resolve the output post-processor (options + TOML filter keyed by `command_str`);
/// 2. Batch-process the full text (ANSI strip, TUI frame removal, noise/keep, truncation);
/// 3. Parse with the stack's `OutputParser` and apply option filters.
///
/// No command is executed. `AnalysisResult.exit_code` stays `None` and
/// `command_failed` stays `false` (the defaults from `AnalysisResult::from_issues`).
pub fn analyze_log_text(
    raw: &str,
    command_str: &str,
    parser: &dyn OutputParser,
    options: &AnalyzeOptions,
) -> Result<AnalysisResult, AnalyzerError> {
    let processor = resolve_processor(command_str, options);
    let processed = processor.process(raw);
    parse_and_analyze(parser, &processed, options)
}

/// Read a build-log file and analyze it.
///
/// The file is read as UTF-8 (with lossy fallback for invalid bytes) and
/// capped at `MAX_LOG_BYTES` to bound memory usage on huge CI logs.
pub fn analyze_log_file(
    path: &Path,
    command_str: &str,
    parser: &dyn OutputParser,
    options: &AnalyzeOptions,
) -> Result<AnalysisResult, AnalyzerError> {
    let raw = read_log_file(path)?;
    analyze_log_text(&raw, command_str, parser, options)
}

/// Read a log file into a string, truncating at `MAX_LOG_BYTES`.
fn read_log_file(path: &Path) -> Result<String, AnalyzerError> {
    let file = fs::File::open(path)?;
    let meta = file.metadata()?;
    let limit = meta.len().min(MAX_LOG_BYTES) as usize;

    let mut bytes = Vec::with_capacity(limit);
    file.take(MAX_LOG_BYTES).read_to_end(&mut bytes)?;

    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::BaseParser;
    use crate::core::stream::resolve_processor;
    use crate::core::types::IssueLevel;
    use crate::plugins::cpp::cmake::parser::CMakeParser;
    use crate::plugins::cpp::parser::CppParser;

    /// Minimal parser for `file:line:col: level: message` lines.
    struct StdFormatParser;

    impl OutputParser for StdFormatParser {
        fn parse_single_line(&self, line: &str) -> Option<crate::core::Issue> {
            BaseParser::new().parse_standard_format(line)
        }
    }

    fn options() -> AnalyzeOptions {
        AnalyzeOptions::default()
    }

    fn gcc_fixture_text() -> String {
        std::fs::read_to_string("tests/data/raw_output/gcc_warnings.txt")
            .expect("gcc_warnings.txt fixture should exist")
    }

    #[test]
    fn test_analyze_log_text_gcc_warning() {
        let result =
            analyze_log_text(&gcc_fixture_text(), "cmake --build build", &CppParser::with_gcc(), &options())
                .unwrap();

        assert_eq!(result.total_issues, 1);
        // Log mode: no execution → no exit code, never marked as failed
        assert_eq!(result.exit_code, None);
        assert!(!result.command_failed);

        let issues = &result.issues_by_file["/tmp/analyzer_test_cpp/test.cpp"];
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].level, IssueLevel::Warning);
        assert_eq!(issues[0].location.line_number, Some(3));
        assert_eq!(issues[0].location.column_number, Some(9));
    }

    #[test]
    fn test_analyze_log_text_clang_errors() {
        let text = std::fs::read_to_string("tests/data/raw_output/clang_compile.txt")
            .expect("clang_compile.txt fixture should exist");
        let result = analyze_log_text(&text, "clang -c src/main.cpp", &CppParser::with_clang(), &options())
            .unwrap();

        // 2 errors + 3 note lines (parsed as Info-level issues)
        assert_eq!(result.total_issues, 5);
        assert_eq!(result.error_count(), 2);
    }

    #[test]
    fn test_analyze_log_text_cmake_error() {
        let text = "CMake Error at CMakeLists.txt:15 (add_library):\n\
                     Cannot find source file: missing.cpp\n";
        let result = analyze_log_text(text, "cmake --build build", &CMakeParser::new(), &options()).unwrap();

        assert_eq!(result.total_issues, 1);
        let issue = &result.issues_by_file["CMakeLists.txt"][0];
        assert_eq!(issue.level, IssueLevel::Error);
        assert_eq!(issue.code.as_deref(), Some("CMake Error"));
        assert_eq!(issue.location.line_number, Some(15));
    }

    #[test]
    fn test_analyze_log_text_empty() {
        let result = analyze_log_text("", "cargo check", &StdFormatParser, &options()).unwrap();
        assert_eq!(result.total_issues, 0);
    }

    #[test]
    fn test_analyze_log_text_strips_ansi() {
        let text = format!("\x1b[31m{}\x1b[0m", gcc_fixture_text());
        let mut opts = options();
        opts.strip_ansi = true;
        let result = analyze_log_text(&text, "cmake --build build", &CppParser::with_gcc(), &opts).unwrap();
        assert_eq!(result.total_issues, 1);
    }

    #[test]
    fn test_analyze_log_text_filter_warnings() {
        let mut opts = options();
        opts.filter_warnings = true;
        let result =
            analyze_log_text(&gcc_fixture_text(), "cmake --build build", &CppParser::with_gcc(), &opts)
                .unwrap();
        assert_eq!(result.total_issues, 0);
    }

    #[test]
    fn test_analyze_log_text_turbo_filter_applied() {
        // The turbo.toml command filter is keyed by the synthesized command
        // string, so the same text parses through the merged TOML filter.
        let text = "2 packages in scope\nsrc/a.ts:3:1: error: 'x' is declared but its value is never read\n";
        let result = analyze_log_text(text, "turbo run build", &StdFormatParser, &options()).unwrap();

        assert_eq!(result.total_issues, 1);
        let issue = &result.issues_by_file["src/a.ts"][0];
        assert_eq!(issue.level, IssueLevel::Error);
        assert_eq!(issue.location.line_number, Some(3));
    }

    #[test]
    fn test_resolve_processor_command_lookup() {
        // "turbo run build" must resolve the turbo.toml filter (noise patterns + max_lines)
        let turbo_proc = resolve_processor("turbo run build", &options());
        assert!(!turbo_proc.noise_patterns.is_empty());
        assert_eq!(turbo_proc.max_lines, Some(100));

        // An unrelated command must NOT pick up the turbo filter
        let plain_proc = resolve_processor("cargo check", &options());
        assert!(plain_proc.noise_patterns.is_empty());
        assert_eq!(plain_proc.max_lines, None);
    }

    #[test]
    fn test_analyze_log_file_reads_fixture() {
        let result = analyze_log_file(
            Path::new("tests/data/raw_output/gcc_warnings.txt"),
            "cmake --build build",
            &CppParser::with_gcc(),
            &options(),
        )
        .unwrap();
        assert_eq!(result.total_issues, 1);
    }

    #[test]
    fn test_analyze_log_file_missing() {
        let err = analyze_log_file(
            Path::new("/nonexistent/build.log"),
            "cmake --build build",
            &CppParser::with_gcc(),
            &options(),
        )
        .unwrap_err();
        assert!(matches!(err, AnalyzerError::IoError(_)));
    }
}
