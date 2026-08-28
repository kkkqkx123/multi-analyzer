//! Log-analysis integration tests.
//!
//! Analyzes pre-existing build logs (e.g. a saved `cmake --build build`
//! output like ZLMediaKit's `/tmp/zlm_build_warn.log`) without executing
//! any command, through the public `analyze_log_file` entry point.

use std::path::Path;

use analyzer::core::{
    analyze_log_file, AnalyzeOptions, IssueLevel, ReporterFactory, ReportFormat, ReportOptions,
};
use analyzer::plugins::cpp::cmake::parser::CMakeParser;

fn log_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/raw_output/cmake_build_warn.log")
}

#[test]
fn test_analyze_cmake_build_log() {
    let options = AnalyzeOptions::default();
    let result =
        analyze_log_file(&log_path(), "cmake --build build", &CMakeParser::new(), &options)
            .expect("log analysis should succeed");

    // Log mode: no execution → no exit code, never marked as failed
    assert_eq!(result.exit_code, None);
    assert!(!result.command_failed);

    // 2 gcc warnings + 1 gcc error from the saved build log
    assert_eq!(result.error_count(), 1);
    assert_eq!(result.warning_count(), 2);
    assert_eq!(result.total_issues, 3);
}

#[test]
fn test_analyze_cmake_build_log_locations() {
    let options = AnalyzeOptions::default();
    let result =
        analyze_log_file(&log_path(), "cmake --build build", &CMakeParser::new(), &options)
            .expect("log analysis should succeed");

    let errors = result.errors();
    let error = errors.first().expect("one error expected");
    assert_eq!(error.location.file_path, "/workspace/zlm/src/Server/MediaServer.cpp");
    assert_eq!(error.location.line_number, Some(132));
    assert_eq!(error.level, IssueLevel::Error);

    let warnings = result.warnings();
    assert_eq!(warnings.len(), 2);

    // issues_by_file is a HashMap → order is not deterministic; compare as sets
    let mut warning_files: Vec<&str> = warnings
        .iter()
        .map(|w| w.location.file_path.as_str())
        .collect();
    warning_files.sort_unstable();
    assert_eq!(
        warning_files,
        vec![
            "/workspace/zlm/src/Network/socket.cpp",
            "/workspace/zlm/src/Util/util.cpp",
        ]
    );

    let socket_warning = warnings
        .iter()
        .find(|w| w.location.file_path.contains("socket.cpp"))
        .expect("socket.cpp warning expected");
    assert_eq!(socket_warning.location.line_number, Some(87));
    let util_warning = warnings
        .iter()
        .find(|w| w.location.file_path.contains("util.cpp"))
        .expect("util.cpp warning expected");
    assert_eq!(util_warning.location.line_number, Some(42));
}

#[test]
fn test_analyze_cmake_build_log_generates_report() {
    // Full pipeline: log → parse → report
    let options = AnalyzeOptions::default();
    let result =
        analyze_log_file(&log_path(), "cmake --build build", &CMakeParser::new(), &options)
            .expect("log analysis should succeed");

    let reporter = ReporterFactory::create(ReportFormat::Markdown);
    let report = reporter
        .generate_with_options(&result, ReportOptions::default())
        .expect("report generation should succeed");

    assert!(report.contains("MediaServer.cpp"), "report should list the error file");
    assert!(report.contains("unused variable"), "report should contain the warning message");
}

#[test]
fn test_analyze_cmake_build_log_missing_file() {
    let options = AnalyzeOptions::default();
    let err = analyze_log_file(
        Path::new("/nonexistent/zlm_build_warn.log"),
        "cmake --build build",
        &CMakeParser::new(),
        &options,
    )
    .expect_err("missing log file must fail");

    assert!(err.to_string().contains("IO error"), "got: {}", err);
}
