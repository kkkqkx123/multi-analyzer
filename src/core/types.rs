//! Core Type Definition
//! Provide types that are common to all tech stacks

use std::collections::{HashMap, HashSet};

/// Issue level
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IssueLevel {
    Error,
    Warning,
    Info,
    Hint,
}

impl std::fmt::Display for IssueLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueLevel::Error => write!(f, "error"),
            IssueLevel::Warning => write!(f, "warning"),
            IssueLevel::Info => write!(f, "info"),
            IssueLevel::Hint => write!(f, "hint"),
        }
    }
}

/// Problem location
#[derive(Debug, Clone)]
pub struct Location {
    pub file_path: String,
    pub line_number: Option<u32>,
    pub column_number: Option<u32>,
}

impl Location {
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            line_number: None,
            column_number: None,
        }
    }

    pub fn with_line(mut self, line: u32) -> Self {
        self.line_number = Some(line);
        self
    }

    pub fn with_column(mut self, column: u32) -> Self {
        self.column_number = Some(column);
        self
    }
}

/// Problem information
#[derive(Debug, Clone)]
pub struct Issue {
    pub level: IssueLevel,
    pub code: Option<String>,
    pub message: String,
    pub location: Location,
    pub context: Option<String>,
    /// Package name (for monorepo/workspace support)
    pub package: Option<String>,
}

impl Issue {
    pub fn new(level: IssueLevel, message: impl Into<String>, location: Location) -> Self {
        Self {
            level,
            code: None,
            message: message.into(),
            location,
            context: None,
            package: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_package(mut self, package: impl Into<String>) -> Self {
        self.package = Some(package.into());
        self
    }
}

/// Analysis results statistics
#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub total_issues: usize,
    pub issues_by_level: HashMap<IssueLevel, usize>,
    pub issues_by_type: HashMap<String, usize>,
    pub issues_by_file: HashMap<String, Vec<Issue>>,
    /// Issues grouped by package (for monorepo/workspace support)
    pub issues_by_package: HashMap<String, Vec<Issue>>,
    pub issues_by_code: HashMap<String, usize>,
    pub unique_patterns: HashSet<String>,
    /// Command exit code (None if not executed)
    pub exit_code: Option<i32>,
    /// Whether the command failed (non-zero exit or execution error)
    pub command_failed: bool,
}

impl AnalysisResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_issues(issues: Vec<Issue>) -> Self {
        let mut result = Self::new();
        for issue in issues {
            result.add_issue(issue);
        }
        result
    }

    pub fn add_issue(&mut self, issue: Issue) {
        self.total_issues += 1;

        // Statistics by level
        *self.issues_by_level.entry(issue.level.clone()).or_insert(0) += 1;

        // Statistics by error code
        if let Some(ref code) = issue.code {
            *self.issues_by_code.entry(code.clone()).or_insert(0) += 1;
        }

        // Statistics by type (using error codes or message patterns)
        let type_key = issue
            .code
            .clone()
            .unwrap_or_else(|| self.extract_pattern(&issue.message));
        *self.issues_by_type.entry(type_key.clone()).or_insert(0) += 1;

        // Statistics by document
        self.issues_by_file
            .entry(issue.location.file_path.clone())
            .or_default()
            .push(issue.clone());

        // Statistics by package
        let package_key = issue
            .package
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        self.issues_by_package
            .entry(package_key)
            .or_default()
            .push(issue);

        // Record uniqueness model
        self.unique_patterns.insert(type_key);
    }

    fn extract_pattern(&self, message: &str) -> String {
        // Simplify messages, extract patterns
        // Remove specific variable names, line numbers, etc.
        message
            .split_whitespace()
            .take(5)
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn errors(&self) -> Vec<&Issue> {
        self.issues_by_file
            .values()
            .flat_map(|issues| issues.iter())
            .filter(|i| i.level == IssueLevel::Error)
            .collect()
    }

    pub fn warnings(&self) -> Vec<&Issue> {
        self.issues_by_file
            .values()
            .flat_map(|issues| issues.iter())
            .filter(|i| i.level == IssueLevel::Warning)
            .collect()
    }

    /// Get total error count
    pub fn error_count(&self) -> usize {
        self.issues_by_level
            .get(&IssueLevel::Error)
            .copied()
            .unwrap_or(0)
    }

    /// Get top N most frequent error codes
    pub fn top_error_codes(&self, n: usize) -> Vec<(String, usize)> {
        let mut codes: Vec<_> = self
            .issues_by_code
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        codes.sort_by_key(|b| std::cmp::Reverse(b.1));
        codes.truncate(n);
        codes
    }

    /// Get total warning count
    pub fn warning_count(&self) -> usize {
        self.issues_by_level
            .get(&IssueLevel::Warning)
            .copied()
            .unwrap_or(0)
    }

    /// Filter self based on AnalyzeOptions (shared utility for all plugin analyzers).
    ///
    /// Applies `filter_warnings`, `filter_paths`, and `max_issues` to produce a filtered result.
    /// This replaces the identical `filter_issues()` methods previously duplicated
    /// across all 10 plugin analyzers.
    pub fn filter_by_options(self, options: &AnalyzeOptions) -> Self {
        let needs_filtering = options.filter_warnings
            || !options.filter_paths.is_empty()
            || options.max_issues.is_some();

        if !needs_filtering {
            return self;
        }

        let mut filtered = AnalysisResult::new();
        let max_issues = options.max_issues.unwrap_or(usize::MAX);

        for (file_path, issues) in self.issues_by_file {
            if !options.filter_paths.is_empty() {
                let matches = options
                    .filter_paths
                    .iter()
                    .any(|filter| file_path.contains(filter));
                if !matches {
                    continue;
                }
            }

            for issue in issues {
                if filtered.total_issues >= max_issues {
                    return filtered;
                }

                if options.filter_warnings && matches!(issue.level, IssueLevel::Warning) {
                    continue;
                }

                filtered.add_issue(issue);
            }
        }

        filtered
    }
}

/// Test Result Status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestStatus {
    Passed,
    Failed,
    Ignored(Option<String>),
}

/// Test Case Information
#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub status: TestStatus,
    pub location: Option<Location>,
    pub failure_details: Option<String>,
    pub execution_time: Option<f64>,
}

impl TestCase {
    pub fn new(name: impl Into<String>, status: TestStatus) -> Self {
        Self {
            name: name.into(),
            status,
            location: None,
            failure_details: None,
            execution_time: None,
        }
    }

    pub fn with_location(mut self, location: Location) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_failure_details(mut self, details: impl Into<String>) -> Self {
        self.failure_details = Some(details.into());
        self
    }

    pub fn with_execution_time(mut self, time: f64) -> Self {
        self.execution_time = Some(time);
        self
    }
}

/// Test Summary
#[derive(Debug, Clone, Default)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
    /// Number of measured tests
    pub measured: usize,
    /// Number of filtered tests
    pub filtered: usize,
    /// Execution time in seconds (available for external use)
    pub execution_time: Option<f64>,
}

impl TestSummary {
    /// Get execution time in seconds if available (available for external use)
    pub fn execution_time(&self) -> Option<f64> {
        self.execution_time
    }

    /// Get execution time formatted as string (available for external use)
    pub fn execution_time_formatted(&self) -> String {
        match self.execution_time {
            Some(time) => format!("{:.2}s", time),
            None => "N/A".to_string(),
        }
    }
}

/// Extending AnalysisResult to support test information
///
/// Responsibility split: `test_summary` carries the authoritative statistics
/// as declared by the test runner (see `total_tests`), while the per-case
/// lists (`failed_tests`, `passed_tests`, `ignored_tests`) are detail records
/// only. Per-case granularity varies by runner: some emit one entry per test
/// method (Cargo, Go, Gradle), while Maven aggregates whole classes for
/// passing tests (surefire text output has no per-method PASSED lines) and
/// reports only failing methods. The detail lists therefore must never be
/// used as the source of aggregate counts.
#[derive(Debug, Default)]
pub struct TestAnalysisResult {
    /// Problems at the compilation stage
    pub compile_result: AnalysisResult,
    /// Test Summary (authoritative statistics declared by the runner)
    pub test_summary: Option<TestSummary>,
    /// Failed Test Cases (detail only; granularity varies by runner)
    pub failed_tests: Vec<TestCase>,
    /// Test cases passed (detail only; granularity varies by runner)
    pub passed_tests: Vec<TestCase>,
    /// Neglected Test Cases (detail only; granularity varies by runner)
    pub ignored_tests: Vec<TestCase>,
    /// Availability of test output
    pub has_test_output: bool,
}

impl TestAnalysisResult {
    pub fn from_compile_result(compile_result: AnalysisResult) -> Self {
        Self {
            compile_result,
            ..Default::default()
        }
    }

    /// Check if all tests passed (no failures and no compile issues).
    ///
    /// The per-case list is not authoritative on its own: several runners only
    /// print aggregate counts when their output is piped (Vitest, for example,
    /// emits `Tests  2 failed | 2 passed (4)` and no per-case lines outside a
    /// TTY). Trusting `failed_tests` alone reported such runs as fully green,
    /// so a failure count reported in the summary counts as a failure too.
    pub fn all_passed(&self) -> bool {
        let summary_reports_failure = self
            .test_summary
            .as_ref()
            .is_some_and(|summary| summary.failed > 0);

        self.failed_tests.is_empty()
            && !summary_reports_failure
            && self.compile_result.total_issues == 0
    }

    /// Number of test cases collected by the parser into the detail lists.
    ///
    /// This is a completeness metric, not a statistics source: per-case
    /// granularity varies by runner and the lists can be partially or fully
    /// empty (e.g. Maven records whole classes for passing tests, and runners
    /// like Vitest emit no per-case lines at all when piped). Use `total_tests`
    /// for authoritative counts.
    pub fn collected_tests(&self) -> usize {
        self.passed_tests.len() + self.failed_tests.len() + self.ignored_tests.len()
    }

    /// Get authoritative total test count.
    ///
    /// The runner-declared summary is the single source of truth for
    /// statistics; per-case lines are detail records that may be incomplete
    /// (see `collected_tests`). The detail count is used only as a fallback
    /// when the runner emitted no summary (e.g. a crashed run that still
    /// printed failing cases).
    pub fn total_tests(&self) -> usize {
        match self.test_summary {
            Some(ref summary) => summary.total,
            None => self.collected_tests(),
        }
    }
}

/// Technology stack type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechStack {
    Cargo,
    Nextest,
    Maven,
    Gradle,
    Npm,
    Pnpm,
    Yarn,
    Mypy,
    Pytest,
    GoBuild,
    GolangciLint,
    Dotnet,
    Rubocop,
    Rspec,
    Ruff,
    Black,
    CMake,
    Gcc,
    Clang,
    ClangFormat,
    Msvc,
}

impl TechStack {
    pub fn as_str(&self) -> &'static str {
        match self {
            TechStack::Cargo => "cargo",
            TechStack::Nextest => "cargo-nextest",
            TechStack::Maven => "maven",
            TechStack::Gradle => "gradle",
            TechStack::Npm => "npm",
            TechStack::Pnpm => "pnpm",
            TechStack::Yarn => "yarn",
            TechStack::Mypy => "mypy",
            TechStack::Pytest => "pytest",
            TechStack::GoBuild => "go",
            TechStack::GolangciLint => "golangci-lint",
            TechStack::Dotnet => "dotnet",
            TechStack::Rubocop => "rubocop",
            TechStack::Rspec => "rspec",
            TechStack::Ruff => "ruff",
            TechStack::Black => "black",
            TechStack::CMake => "cmake",
            TechStack::Gcc => "gcc",
            TechStack::Clang => "clang",
            TechStack::ClangFormat => "clang-format",
            TechStack::Msvc => "msvc",
        }
    }

    /// True when the analyzer has a sensible default command and can be driven
    /// purely by build options (no subcommand required). All C++ analyzers
    /// fall back to a default command when the subcommand is absent, so an
    /// invocation like `analyzer cmake --build-dir out` is valid.
    pub fn allows_default_command(&self) -> bool {
        matches!(
            self,
            TechStack::CMake
                | TechStack::Gcc
                | TechStack::Clang
                | TechStack::ClangFormat
                | TechStack::Msvc
        )
    }
}

impl std::str::FromStr for TechStack {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cargo" | "rust" => Ok(TechStack::Cargo),
            "cargo-nextest" | "nextest" => Ok(TechStack::Nextest),
            "maven" | "mvn" => Ok(TechStack::Maven),
            "gradle" | "gradlew" => Ok(TechStack::Gradle),
            "npm" | "node" => Ok(TechStack::Npm),
            "pnpm" => Ok(TechStack::Pnpm),
            "yarn" => Ok(TechStack::Yarn),
            "mypy" => Ok(TechStack::Mypy),
            "pytest" | "py.test" => Ok(TechStack::Pytest),
            "go" | "golang" => Ok(TechStack::GoBuild),
            "golangci-lint" => Ok(TechStack::GolangciLint),
            "dotnet" | "msbuild" | "csharp" => Ok(TechStack::Dotnet),
            "rubocop" | "ruby" | "rails" => Ok(TechStack::Rubocop),
            "rspec" => Ok(TechStack::Rspec),
            "ruff" | "python-lint" => Ok(TechStack::Ruff),
            "black" => Ok(TechStack::Black),
            "cmake" | "cmake-build" => Ok(TechStack::CMake),
            "gcc" | "g++" => Ok(TechStack::Gcc),
            "clang" | "clang++" => Ok(TechStack::Clang),
            "clang-format" => Ok(TechStack::ClangFormat),
            "msvc" | "cl" => Ok(TechStack::Msvc),
            _ => Err(format!("Unknown tech stack: {}", s)),
        }
    }
}

/// Command category for grouping and organization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Check,  // Syntax and type checking
    Lint,   // Code linting
    Test,   // Test execution
    Audit,  // Security audit
    Build,  // Build compilation
    Format, // Code formatting
    Custom, // User-defined
}

/// Subcommand is now a simple string wrapper for full command flexibility
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubCommand(pub String);

impl SubCommand {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the category of this subcommand based on common patterns
    pub fn category(&self) -> CommandCategory {
        let s = self.0.to_lowercase();
        if s.contains("check") || s.contains("type") {
            CommandCategory::Check
        } else if s.contains("lint") || s.contains("clippy") {
            CommandCategory::Lint
        } else if s.contains("test") {
            CommandCategory::Test
        } else if s.contains("audit") {
            CommandCategory::Audit
        } else if s.contains("build") || s.contains("compile") {
            CommandCategory::Build
        } else if s.contains("fmt") || s.contains("format") {
            CommandCategory::Format
        } else {
            CommandCategory::Custom
        }
    }

    /// Returns true if the command category is `Custom` (i.e. does not match
    /// any known pattern like `check`, `test`, `build`, etc.).
    pub fn is_custom(&self) -> bool {
        matches!(self.category(), CommandCategory::Custom)
    }
}

impl std::str::FromStr for SubCommand {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Accept any non-empty string as a valid subcommand
        if s.trim().is_empty() {
            Err("Subcommand cannot be empty".to_string())
        } else {
            Ok(SubCommand(s.to_string()))
        }
    }
}

/// Verbosity level for report output
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    /// Minimal output: summary only, no details
    Minimal,
    /// Normal output: summary + top issues (default)
    #[default]
    Normal,
    /// Verbose output: full details, no truncation
    Verbose,
}

impl Verbosity {
    pub fn is_minimal(&self) -> bool {
        matches!(self, Verbosity::Minimal)
    }

    pub fn is_verbose(&self) -> bool {
        matches!(self, Verbosity::Verbose)
    }
}

/// Analyze options
#[derive(Debug, Default, Clone)]
pub struct AnalyzeOptions {
    pub subcommand: Option<SubCommand>,
    /// The raw tech stack string as typed by the user (e.g. "ruby", "rails",
    /// "rubocop"). Kept separate from the resolved `TechStack` enum so plugins
    /// can recover the original alias for command construction.
    pub raw_tech_stack: Option<String>,
    pub filter_warnings: bool,
    pub filter_paths: Vec<String>,
    pub noise_patterns: Vec<String>,
    pub keep_patterns: Vec<String>,
    pub max_output_lines: usize,
    pub max_line_length: usize,
    pub strip_ansi: bool,
    pub strip_tui_frames: bool,
    pub output_file: Option<String>,
    /// Output to stdout only, do not write to file
    pub stdout_only: bool,
    /// Verbosity level
    pub verbosity: Verbosity,
    // C++ related options
    pub source_dir: Option<String>,
    pub build_dir: Option<String>,
    pub cmake_generator: Option<String>,
    pub target: Option<String>,
    pub target_files: Vec<String>,
    pub include_paths: Vec<String>,
    pub defines: Vec<String>,
    pub cpp_standard: Option<String>,
    /// Report output format: markdown, json, or html
    pub report_format: ReportFormat,
    /// Enable success short-circuit: when no issues found, output a single-line confirmation
    pub success_short_circuit: bool,

    // === Result Limits ===
    /// Maximum number of issues to keep in the analysis result.
    /// 0 or None = unlimited. Applied after all filters.
    pub max_issues: Option<usize>,

    // === Cargo Workspace Support ===
    /// --workspace
    pub workspace: bool,
    /// --package <SPEC>
    pub package: Vec<String>,
    /// --exclude <SPEC>
    pub exclude: Vec<String>,

    // === Cargo Target Support ===
    /// --lib
    pub lib: bool,
    /// --bin <NAME>
    pub bin: Vec<String>,
    /// --bins
    pub bins: bool,
    /// --test <NAME>
    pub test: Vec<String>,
    /// --tests
    pub tests: bool,
    /// --example <NAME>
    pub example: Vec<String>,
    /// --examples
    pub examples: bool,
    /// --bench <NAME>
    pub bench: Vec<String>,
    /// --benches
    pub benches: bool,
    /// --all-targets
    pub all_targets: bool,

    // === Cargo Feature Support ===
    /// --features <FEATURES>
    pub features: Vec<String>,
    /// --all-features
    pub all_features: bool,
    /// --no-default-features
    pub no_default_features: bool,

    }

impl AnalyzeOptions {
    /// Seed AnalyzeOptions from configuration file.
    /// CLI args should override these values after calling this.
    pub fn from_config(config: &crate::config::AppConfig) -> Self {
        let mut opts = AnalyzeOptions {
            report_format: match config.report.format.as_str() {
                "json" => ReportFormat::Json,
                "html" => ReportFormat::Html,
                "raw" => ReportFormat::Raw,
                "raw-json" | "raw_json" => ReportFormat::RawJson,
                _ => ReportFormat::Markdown,
            },
            verbosity: match config.report.verbosity.as_str() {
                "minimal" => Verbosity::Minimal,
                "verbose" => Verbosity::Verbose,
                _ => Verbosity::Normal,
            },
            strip_ansi: config.filter.strip_ansi,
            strip_tui_frames: config.filter.strip_tui_frames,
            max_output_lines: config.filter.max_lines,
            max_line_length: config.filter.max_line_length,
            noise_patterns: config.filter.noise_patterns.clone(),
            keep_patterns: config.filter.keep_patterns.clone(),
            success_short_circuit: config.report.success_short_circuit,
            ..AnalyzeOptions::default()
        };
        // Default: output to stdout (like typical CLI tools).
        // Use -o/--output <file> to write to a file instead.
        opts.stdout_only = true;
        opts
    }
}

/// Report format
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ReportFormat {
    #[default]
    Markdown,
    Json,
    Html,
    /// Pipe-delimited raw text output (machine-readable)
    Raw,
    /// JSON lines output (one JSON object per line)
    RawJson,
}

impl ReportFormat {
    /// Return the file extension for this format (without dot)
    pub fn extension(&self) -> &'static str {
        match self {
            ReportFormat::Markdown => "md",
            ReportFormat::Json => "json",
            ReportFormat::Html => "html",
            ReportFormat::Raw => "txt",
            ReportFormat::RawJson => "jsonl",
        }
    }

    pub fn is_raw(&self) -> bool {
        matches!(self, ReportFormat::Raw | ReportFormat::RawJson)
    }
}

impl std::str::FromStr for ReportFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "markdown" | "md" => Ok(ReportFormat::Markdown),
            "json" => Ok(ReportFormat::Json),
            "html" => Ok(ReportFormat::Html),
            "raw" => Ok(ReportFormat::Raw),
            "raw-json" | "raw_json" => Ok(ReportFormat::RawJson),
            _ => Err(format!("Unknown report format: {}", s)),
        }
    }
}

#[cfg(test)]
mod types_tests {
    use super::*;

    // ── AnalysisResult ──────────────────────────────────────────────

    #[test]
    fn test_empty_analysis_result() {
        let r = AnalysisResult::new();
        assert_eq!(r.total_issues, 0);
        assert!(r.unique_patterns.is_empty());
        assert!(r.issues_by_code.is_empty());
        assert!(r.issues_by_file.is_empty());
        assert!(r.issues_by_level.is_empty());
        assert!(r.issues_by_package.is_empty());
        assert!(r.issues_by_type.is_empty());
    }

    #[test]
    fn test_analysis_result_from_issues() {
        let issues = vec![
            Issue::new(IssueLevel::Error, "type mismatch", Location::new("a.rs")),
            Issue::new(IssueLevel::Warning, "unused var", Location::new("b.rs")),
        ];
        let r = AnalysisResult::from_issues(issues);
        assert_eq!(r.total_issues, 2);
        assert_eq!(r.error_count(), 1);
        assert_eq!(r.warning_count(), 1);
    }

    #[test]
    fn test_add_issue_error_level() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(IssueLevel::Error, "err msg", Location::new("f.rs")));
        assert_eq!(r.total_issues, 1);
        assert_eq!(r.error_count(), 1);
        assert_eq!(r.warning_count(), 0);
    }

    #[test]
    fn test_add_issue_warning_level() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "warn msg",
            Location::new("f.rs"),
        ));
        assert_eq!(r.total_issues, 1);
        assert_eq!(r.error_count(), 0);
        assert_eq!(r.warning_count(), 1);
    }

    #[test]
    fn test_add_issue_info_hint_levels() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(IssueLevel::Info, "info msg", Location::new("f.rs")));
        r.add_issue(Issue::new(IssueLevel::Hint, "hint msg", Location::new("f.rs")));
        assert_eq!(r.total_issues, 2);
        assert_eq!(r.error_count(), 0);
        assert_eq!(r.warning_count(), 0);
        assert_eq!(*r.issues_by_level.get(&IssueLevel::Info).unwrap(), 1);
        assert_eq!(*r.issues_by_level.get(&IssueLevel::Hint).unwrap(), 1);
    }

    #[test]
    fn test_add_issue_with_code() {
        let mut r = AnalysisResult::new();
        let issue = Issue::new(IssueLevel::Error, "msg", Location::new("f.rs"))
            .with_code("E0308");
        r.add_issue(issue);
        assert_eq!(*r.issues_by_code.get("E0308").unwrap(), 1);
        // type_key should use the code
        assert_eq!(*r.issues_by_type.get("E0308").unwrap(), 1);
    }

    #[test]
    fn test_add_issue_without_code_uses_pattern() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Error,
            "a quick brown fox jumps over",
            Location::new("f.rs"),
        ));
        let pattern = "a quick brown fox jumps";
        assert_eq!(*r.issues_by_type.get(pattern).unwrap(), 1);
        assert!(r.unique_patterns.contains(pattern));
    }

    #[test]
    fn test_issues_by_file_grouping() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(IssueLevel::Error, "e1", Location::new("a.rs")));
        r.add_issue(Issue::new(IssueLevel::Error, "e2", Location::new("a.rs")));
        r.add_issue(Issue::new(IssueLevel::Error, "e3", Location::new("b.rs")));
        assert_eq!(r.issues_by_file.get("a.rs").unwrap().len(), 2);
        assert_eq!(r.issues_by_file.get("b.rs").unwrap().len(), 1);
    }

    #[test]
    fn test_issues_by_package_grouping() {
        let mut r = AnalysisResult::new();
        r.add_issue(
            Issue::new(IssueLevel::Error, "e1", Location::new("a.rs"))
                .with_package("pkg-a"),
        );
        r.add_issue(
            Issue::new(IssueLevel::Error, "e2", Location::new("b.rs"))
                .with_package("pkg-b"),
        );
        r.add_issue(Issue::new(IssueLevel::Error, "e3", Location::new("c.rs")));
        assert_eq!(r.issues_by_package.get("pkg-a").unwrap().len(), 1);
        assert_eq!(r.issues_by_package.get("pkg-b").unwrap().len(), 1);
        // No package → "unknown"
        assert_eq!(r.issues_by_package.get("unknown").unwrap().len(), 1);
    }

    #[test]
    fn test_errors_method() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(IssueLevel::Error, "e1", Location::new("a.rs")));
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "w1",
            Location::new("a.rs"),
        ));
        r.add_issue(Issue::new(IssueLevel::Error, "e2", Location::new("b.rs")));
        let errors = r.errors();
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|i| i.level == IssueLevel::Error));
    }

    #[test]
    fn test_warnings_method() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "w1",
            Location::new("a.rs"),
        ));
        r.add_issue(Issue::new(IssueLevel::Error, "e1", Location::new("a.rs")));
        let warnings = r.warnings();
        assert_eq!(warnings.len(), 1);
        assert!(warnings.iter().all(|i| i.level == IssueLevel::Warning));
    }

    #[test]
    fn test_errors_warnings_empty() {
        let r = AnalysisResult::new();
        assert!(r.errors().is_empty());
        assert!(r.warnings().is_empty());
    }

    #[test]
    fn test_error_count() {
        let mut r = AnalysisResult::new();
        assert_eq!(r.error_count(), 0);
        r.add_issue(Issue::new(IssueLevel::Error, "e1", Location::new("a.rs")));
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "w1",
            Location::new("a.rs"),
        ));
        r.add_issue(Issue::new(IssueLevel::Error, "e2", Location::new("b.rs")));
        assert_eq!(r.error_count(), 2);
    }

    #[test]
    fn test_warning_count() {
        let mut r = AnalysisResult::new();
        assert_eq!(r.warning_count(), 0);
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "w1",
            Location::new("a.rs"),
        ));
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "w2",
            Location::new("b.rs"),
        ));
        assert_eq!(r.warning_count(), 2);
    }

    #[test]
    fn test_top_error_codes_empty() {
        let r = AnalysisResult::new();
        assert!(r.top_error_codes(5).is_empty());
    }

    #[test]
    fn test_top_error_codes_sorted() {
        let mut r = AnalysisResult::new();
        r.add_issue(
            Issue::new(IssueLevel::Error, "msg", Location::new("a.rs"))
                .with_code("C001"),
        );
        r.add_issue(
            Issue::new(IssueLevel::Error, "msg", Location::new("a.rs"))
                .with_code("C001"),
        );
        r.add_issue(
            Issue::new(IssueLevel::Error, "msg", Location::new("a.rs"))
                .with_code("C002"),
        );
        let top = r.top_error_codes(5);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0], ("C001".to_string(), 2));
        assert_eq!(top[1], ("C002".to_string(), 1));
    }

    #[test]
    fn test_top_error_codes_truncate() {
        let mut r = AnalysisResult::new();
        for i in 0..10 {
            r.add_issue(
                Issue::new(IssueLevel::Error, "msg", Location::new("a.rs"))
                    .with_code(format!("C{:03}", i)),
            );
        }
        assert_eq!(r.top_error_codes(3).len(), 3);
    }

    #[test]
    fn test_extract_pattern_short_message() {
        let r = AnalysisResult::new();
        assert_eq!(r.extract_pattern("hi"), "hi".to_string());
    }

    #[test]
    fn test_extract_pattern_long_message() {
        let r = AnalysisResult::new();
        assert_eq!(
            r.extract_pattern("a b c d e f g h"),
            "a b c d e".to_string()
        );
    }

    #[test]
    fn test_filter_by_options_no_filter() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(IssueLevel::Error, "e1", Location::new("a.rs")));
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "w1",
            Location::new("a.rs"),
        ));
        let opts = AnalyzeOptions::default();
        let filtered = r.filter_by_options(&opts);
        assert_eq!(filtered.total_issues, 2);
    }

    #[test]
    fn test_filter_by_options_filter_warnings() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(IssueLevel::Error, "e1", Location::new("a.rs")));
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "w1",
            Location::new("a.rs"),
        ));
        let opts = AnalyzeOptions {
            filter_warnings: true,
            ..Default::default()
        };
        let filtered = r.filter_by_options(&opts);
        assert_eq!(filtered.total_issues, 1);
        assert_eq!(filtered.error_count(), 1);
    }

    #[test]
    fn test_filter_by_options_filter_paths() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(IssueLevel::Error, "e1", Location::new("src/a.rs")));
        r.add_issue(Issue::new(
            IssueLevel::Error,
            "e2",
            Location::new("tests/b.rs"),
        ));
        let opts = AnalyzeOptions {
            filter_paths: vec!["src".to_string()],
            ..Default::default()
        };
        let filtered = r.filter_by_options(&opts);
        assert_eq!(filtered.total_issues, 1);
    }

    #[test]
    fn test_filter_by_options_max_issues() {
        let mut r = AnalysisResult::new();
        for i in 0..10 {
            r.add_issue(Issue::new(
                IssueLevel::Error,
                format!("e{}", i),
                Location::new("a.rs"),
            ));
        }
        let opts = AnalyzeOptions {
            max_issues: Some(3),
            ..Default::default()
        };
        let filtered = r.filter_by_options(&opts);
        assert_eq!(filtered.total_issues, 3);
    }

    #[test]
    fn test_filter_by_options_combined() {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Warning,
            "w1",
            Location::new("src/a.rs"),
        ));
        r.add_issue(Issue::new(
            IssueLevel::Error,
            "e1",
            Location::new("src/a.rs"),
        ));
        r.add_issue(Issue::new(
            IssueLevel::Error,
            "e2",
            Location::new("tests/b.rs"),
        ));
        let opts = AnalyzeOptions {
            filter_warnings: true,
            filter_paths: vec!["src".to_string()],
            max_issues: Some(10),
            ..Default::default()
        };
        let filtered = r.filter_by_options(&opts);
        assert_eq!(filtered.total_issues, 1);
    }

    // ── Issue builder ───────────────────────────────────────────────

    #[test]
    fn test_issue_builder_basic() {
        let issue = Issue::new(IssueLevel::Error, "test msg", Location::new("f.rs"));
        assert_eq!(issue.level, IssueLevel::Error);
        assert_eq!(issue.message, "test msg");
        assert_eq!(issue.location.file_path, "f.rs");
        assert!(issue.code.is_none());
        assert!(issue.context.is_none());
        assert!(issue.package.is_none());
    }

    #[test]
    fn test_issue_with_code() {
        let issue = Issue::new(IssueLevel::Error, "msg", Location::new("f.rs"))
            .with_code("E0308");
        assert_eq!(issue.code.unwrap(), "E0308");
    }

    #[test]
    fn test_issue_with_context() {
        let issue = Issue::new(IssueLevel::Error, "msg", Location::new("f.rs"))
            .with_context("expected type X");
        assert_eq!(issue.context.unwrap(), "expected type X");
    }

    #[test]
    fn test_issue_with_package() {
        let issue = Issue::new(IssueLevel::Error, "msg", Location::new("f.rs"))
            .with_package("my-crate");
        assert_eq!(issue.package.unwrap(), "my-crate");
    }

    #[test]
    fn test_issue_chained_builders() {
        let issue = Issue::new(IssueLevel::Warning, "unused", Location::new("lib.rs"))
            .with_code("W0001")
            .with_context("consider removing")
            .with_package("core");
        assert_eq!(issue.code.unwrap(), "W0001");
        assert_eq!(issue.context.unwrap(), "consider removing");
        assert_eq!(issue.package.unwrap(), "core");
    }

    // ── Location builder ────────────────────────────────────────────

    #[test]
    fn test_location_basic() {
        let loc = Location::new("src/main.rs");
        assert_eq!(loc.file_path, "src/main.rs");
        assert!(loc.line_number.is_none());
        assert!(loc.column_number.is_none());
    }

    #[test]
    fn test_location_with_line() {
        let loc = Location::new("f.rs").with_line(42);
        assert_eq!(loc.line_number, Some(42));
        assert!(loc.column_number.is_none());
    }

    #[test]
    fn test_location_with_column() {
        let loc = Location::new("f.rs").with_line(10).with_column(5);
        assert_eq!(loc.line_number, Some(10));
        assert_eq!(loc.column_number, Some(5));
    }

    // ── TestCase builder ────────────────────────────────────────────

    #[test]
    fn test_test_case_basic() {
        let tc = TestCase::new("test_foo", TestStatus::Passed);
        assert_eq!(tc.name, "test_foo");
        assert_eq!(tc.status, TestStatus::Passed);
        assert!(tc.location.is_none());
        assert!(tc.failure_details.is_none());
    }

    #[test]
    fn test_test_case_with_location() {
        let tc = TestCase::new("test_bar", TestStatus::Failed)
            .with_location(Location::new("tests/test.rs").with_line(15));
        assert!(tc.location.is_some());
        assert_eq!(tc.location.unwrap().line_number, Some(15));
    }

    #[test]
    fn test_test_case_with_failure_details() {
        let tc = TestCase::new("test_baz", TestStatus::Failed)
            .with_failure_details("assertion failed: 1 != 2");
        assert_eq!(
            tc.failure_details.unwrap(),
            "assertion failed: 1 != 2"
        );
    }

    #[test]
    fn test_test_case_with_execution_time() {
        let tc = TestCase::new("test_qux", TestStatus::Passed).with_execution_time(0.123);
        assert!((tc.execution_time.unwrap() - 0.123).abs() < 1e-9);
    }

    // ── TestSummary ─────────────────────────────────────────────────

    #[test]
    fn test_test_summary_default() {
        let s = TestSummary::default();
        assert_eq!(s.total, 0);
        assert_eq!(s.passed, 0);
        assert_eq!(s.failed, 0);
        assert_eq!(s.ignored, 0);
    }

    #[test]
    fn test_test_summary_execution_time() {
        let mut s = TestSummary::default();
        assert!(s.execution_time().is_none());
        s.execution_time = Some(1.5);
        assert!((s.execution_time().unwrap() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn test_test_summary_execution_time_formatted() {
        let mut s = TestSummary::default();
        assert_eq!(s.execution_time_formatted(), "N/A");
        s.execution_time = Some(2.5);
        assert_eq!(s.execution_time_formatted(), "2.50s");
    }

    // ── TestAnalysisResult ──────────────────────────────────────────

    #[test]
    fn test_test_analysis_result_from_compile_result() {
        let r = AnalysisResult::from_issues(vec![Issue::new(
            IssueLevel::Error,
            "compile err",
            Location::new("src/main.rs"),
        )]);
        let tar = TestAnalysisResult::from_compile_result(r);
        assert_eq!(tar.compile_result.total_issues, 1);
        assert!(!tar.all_passed());
        assert_eq!(tar.total_tests(), 0);
    }

    #[test]
    fn test_test_analysis_result_all_passed() {
        let r = AnalysisResult::new();
        let mut tar = TestAnalysisResult::from_compile_result(r);
        assert!(tar.all_passed());
        tar.failed_tests
            .push(TestCase::new("test_fail", TestStatus::Failed));
        assert!(!tar.all_passed());
    }

    /// Regression test: runners that only emit aggregate counts (Vitest outside
    /// a TTY) must not be reported as green when the summary shows failures.
    #[test]
    fn test_all_passed_honours_summary_failures() {
        let mut tar = TestAnalysisResult::from_compile_result(AnalysisResult::new());
        tar.test_summary = Some(TestSummary {
            total: 4,
            passed: 2,
            failed: 2,
            ignored: 0,
            measured: 0,
            filtered: 0,
            execution_time: None,
        });

        assert!(
            tar.failed_tests.is_empty(),
            "precondition: no per-case data available"
        );
        assert!(!tar.all_passed(), "summary failures must mark the run as failed");
        assert_eq!(tar.total_tests(), 4, "the runner summary is authoritative");
    }

    #[test]
    fn test_all_passed_with_green_summary() {
        let mut tar = TestAnalysisResult::from_compile_result(AnalysisResult::new());
        tar.test_summary = Some(TestSummary {
            total: 5,
            passed: 5,
            failed: 0,
            ignored: 0,
            measured: 0,
            filtered: 0,
            execution_time: None,
        });

        assert!(tar.all_passed());
        assert_eq!(tar.total_tests(), 5);
    }

    #[test]
    fn test_test_analysis_result_total_tests() {
        let mut tar = TestAnalysisResult::from_compile_result(AnalysisResult::new());
        tar.passed_tests
            .push(TestCase::new("t1", TestStatus::Passed));
        tar.failed_tests
            .push(TestCase::new("t2", TestStatus::Failed));
        tar.ignored_tests
            .push(TestCase::new("t3", TestStatus::Ignored(None)));
        // Without a summary, the detail count is used as a fallback.
        assert_eq!(tar.total_tests(), 3);
        assert_eq!(tar.collected_tests(), 3);
    }

    /// Regression test: a partially collected detail list (Maven records
    /// passing classes, not passing methods) must not shadow the
    /// runner-declared total.
    #[test]
    fn test_total_tests_prefers_summary_over_partial_details() {
        let mut tar = TestAnalysisResult::from_compile_result(AnalysisResult::new());
        tar.test_summary = Some(TestSummary {
            total: 2,
            passed: 1,
            failed: 1,
            ignored: 0,
            measured: 0,
            filtered: 0,
            execution_time: None,
        });
        // Only the failing method was collected; the passing class is absent.
        tar.failed_tests
            .push(TestCase::new("com.example.AppTest::testFailingCase", TestStatus::Failed));
        assert_eq!(tar.total_tests(), 2, "summary total is authoritative");
        assert_eq!(tar.collected_tests(), 1, "detail lists stay as collected");
    }

    // ── TechStack FromStr ───────────────────────────────────────────

    #[test]
    fn test_tech_stack_from_str_primary_names() {
        assert_eq!("cargo".parse::<TechStack>().unwrap(), TechStack::Cargo);
        assert_eq!("maven".parse::<TechStack>().unwrap(), TechStack::Maven);
        assert_eq!("gradle".parse::<TechStack>().unwrap(), TechStack::Gradle);
        assert_eq!("npm".parse::<TechStack>().unwrap(), TechStack::Npm);
        assert_eq!("pnpm".parse::<TechStack>().unwrap(), TechStack::Pnpm);
        assert_eq!("yarn".parse::<TechStack>().unwrap(), TechStack::Yarn);
        assert_eq!("mypy".parse::<TechStack>().unwrap(), TechStack::Mypy);
        assert_eq!("pytest".parse::<TechStack>().unwrap(), TechStack::Pytest);
        assert_eq!("go".parse::<TechStack>().unwrap(), TechStack::GoBuild);
        assert_eq!("golangci-lint".parse::<TechStack>().unwrap(), TechStack::GolangciLint);
        assert_eq!("dotnet".parse::<TechStack>().unwrap(), TechStack::Dotnet);
        assert_eq!("rubocop".parse::<TechStack>().unwrap(), TechStack::Rubocop);
        assert_eq!("rspec".parse::<TechStack>().unwrap(), TechStack::Rspec);
        assert_eq!("ruff".parse::<TechStack>().unwrap(), TechStack::Ruff);
        assert_eq!("black".parse::<TechStack>().unwrap(), TechStack::Black);
        assert_eq!("cmake".parse::<TechStack>().unwrap(), TechStack::CMake);
        assert_eq!("gcc".parse::<TechStack>().unwrap(), TechStack::Gcc);
        assert_eq!("clang".parse::<TechStack>().unwrap(), TechStack::Clang);
        assert_eq!("clang-format".parse::<TechStack>().unwrap(), TechStack::ClangFormat);
        assert_eq!("msvc".parse::<TechStack>().unwrap(), TechStack::Msvc);
        assert_eq!("cargo-nextest".parse::<TechStack>().unwrap(), TechStack::Nextest);
    }

    #[test]
    fn test_tech_stack_from_str_aliases() {
        assert_eq!("rust".parse::<TechStack>().unwrap(), TechStack::Cargo);
        assert_eq!("mvn".parse::<TechStack>().unwrap(), TechStack::Maven);
        assert_eq!("gradlew".parse::<TechStack>().unwrap(), TechStack::Gradle);
        assert_eq!("node".parse::<TechStack>().unwrap(), TechStack::Npm);
        assert_eq!("nextest".parse::<TechStack>().unwrap(), TechStack::Nextest);
        assert_eq!("py.test".parse::<TechStack>().unwrap(), TechStack::Pytest);
        assert_eq!("golang".parse::<TechStack>().unwrap(), TechStack::GoBuild);
        assert_eq!("msbuild".parse::<TechStack>().unwrap(), TechStack::Dotnet);
        assert_eq!("csharp".parse::<TechStack>().unwrap(), TechStack::Dotnet);
        assert_eq!("ruby".parse::<TechStack>().unwrap(), TechStack::Rubocop);
        assert_eq!("rails".parse::<TechStack>().unwrap(), TechStack::Rubocop);
        assert_eq!("python-lint".parse::<TechStack>().unwrap(), TechStack::Ruff);
        assert_eq!("cmake-build".parse::<TechStack>().unwrap(), TechStack::CMake);
        assert_eq!("g++".parse::<TechStack>().unwrap(), TechStack::Gcc);
        assert_eq!("clang++".parse::<TechStack>().unwrap(), TechStack::Clang);
        assert_eq!("cl".parse::<TechStack>().unwrap(), TechStack::Msvc);
    }

    #[test]
    fn test_tech_stack_from_str_case_insensitive() {
        assert_eq!("Cargo".parse::<TechStack>().unwrap(), TechStack::Cargo);
        assert_eq!("MAVEN".parse::<TechStack>().unwrap(), TechStack::Maven);
        assert_eq!("Npm".parse::<TechStack>().unwrap(), TechStack::Npm);
    }

    #[test]
    fn test_tech_stack_from_str_unknown() {
        let result = "unknown-tool".parse::<TechStack>();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown-tool"));
    }

    // ── SubCommand ──────────────────────────────────────────────────

    #[test]
    fn test_sub_command_new_and_as_str() {
        let cmd = SubCommand::new("check");
        assert_eq!(cmd.as_str(), "check");
    }

    #[test]
    fn test_sub_command_category_check() {
        assert_eq!(
            SubCommand::new("check").category(),
            CommandCategory::Check
        );
        assert_eq!(
            SubCommand::new("typecheck").category(),
            CommandCategory::Check
        );
    }

    #[test]
    fn test_sub_command_category_lint() {
        assert_eq!(
            SubCommand::new("clippy").category(),
            CommandCategory::Lint
        );
        assert_eq!(
            SubCommand::new("lint").category(),
            CommandCategory::Lint
        );
    }

    #[test]
    fn test_sub_command_category_test() {
        assert_eq!(
            SubCommand::new("test").category(),
            CommandCategory::Test
        );
    }

    #[test]
    fn test_sub_command_category_audit() {
        assert_eq!(
            SubCommand::new("audit").category(),
            CommandCategory::Audit
        );
    }

    #[test]
    fn test_sub_command_category_build() {
        assert_eq!(
            SubCommand::new("build").category(),
            CommandCategory::Build
        );
        assert_eq!(
            SubCommand::new("compile").category(),
            CommandCategory::Build
        );
    }

    #[test]
    fn test_sub_command_category_format() {
        assert_eq!(
            SubCommand::new("fmt").category(),
            CommandCategory::Format
        );
        assert_eq!(
            SubCommand::new("format").category(),
            CommandCategory::Format
        );
    }

    #[test]
    fn test_sub_command_category_custom() {
        assert_eq!(
            SubCommand::new("run").category(),
            CommandCategory::Custom
        );
        assert_eq!(
            SubCommand::new("clean").category(),
            CommandCategory::Custom
        );
    }

    #[test]
    fn test_sub_command_is_custom() {
        // Fixed: is_custom now returns true only for CommandCategory::Custom
        assert!(SubCommand::new("run").is_custom());
        assert!(SubCommand::new("clean").is_custom());
        assert!(!SubCommand::new("check").is_custom());
        assert!(!SubCommand::new("test").is_custom());
        assert!(!SubCommand::new("build").is_custom());
        assert!(!SubCommand::new("clippy").is_custom());
        assert!(!SubCommand::new("audit").is_custom());
        assert!(!SubCommand::new("fmt").is_custom());
    }

    #[test]
    fn test_sub_command_from_str_valid() {
        let cmd: SubCommand = "check".parse().unwrap();
        assert_eq!(cmd.as_str(), "check");
    }

    #[test]
    fn test_sub_command_from_str_empty() {
        let result: Result<SubCommand, String> = "  ".parse();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    // ── ReportFormat ────────────────────────────────────────────────

    #[test]
    fn test_report_format_extension() {
        assert_eq!(ReportFormat::Markdown.extension(), "md");
        assert_eq!(ReportFormat::Json.extension(), "json");
        assert_eq!(ReportFormat::Html.extension(), "html");
        assert_eq!(ReportFormat::Raw.extension(), "txt");
        assert_eq!(ReportFormat::RawJson.extension(), "jsonl");
    }

    #[test]
    fn test_report_format_is_raw() {
        assert!(ReportFormat::Raw.is_raw());
        assert!(ReportFormat::RawJson.is_raw());
        assert!(!ReportFormat::Markdown.is_raw());
        assert!(!ReportFormat::Json.is_raw());
        assert!(!ReportFormat::Html.is_raw());
    }

    #[test]
    fn test_report_format_from_str_valid() {
        assert_eq!("markdown".parse::<ReportFormat>().unwrap(), ReportFormat::Markdown);
        assert_eq!("md".parse::<ReportFormat>().unwrap(), ReportFormat::Markdown);
        assert_eq!("json".parse::<ReportFormat>().unwrap(), ReportFormat::Json);
        assert_eq!("html".parse::<ReportFormat>().unwrap(), ReportFormat::Html);
        assert_eq!("raw".parse::<ReportFormat>().unwrap(), ReportFormat::Raw);
        assert_eq!("raw-json".parse::<ReportFormat>().unwrap(), ReportFormat::RawJson);
        assert_eq!("raw_json".parse::<ReportFormat>().unwrap(), ReportFormat::RawJson);
    }

    #[test]
    fn test_report_format_from_str_invalid() {
        let result: Result<ReportFormat, String> = "unknown".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_report_format_default() {
        let fmt: ReportFormat = Default::default();
        assert_eq!(fmt, ReportFormat::Markdown);
    }

    // ── Verbosity ───────────────────────────────────────────────────

    #[test]
    fn test_verbosity_is_minimal() {
        assert!(Verbosity::Minimal.is_minimal());
        assert!(!Verbosity::Normal.is_minimal());
        assert!(!Verbosity::Verbose.is_minimal());
    }

    #[test]
    fn test_verbosity_is_verbose() {
        assert!(!Verbosity::Minimal.is_verbose());
        assert!(!Verbosity::Normal.is_verbose());
        assert!(Verbosity::Verbose.is_verbose());
    }

    #[test]
    fn test_verbosity_default() {
        let v: Verbosity = Default::default();
        assert_eq!(v, Verbosity::Normal);
    }

    // ── IssueLevel Display ──────────────────────────────────────────

    #[test]
    fn test_issue_level_display() {
        assert_eq!(IssueLevel::Error.to_string(), "error");
        assert_eq!(IssueLevel::Warning.to_string(), "warning");
        assert_eq!(IssueLevel::Info.to_string(), "info");
        assert_eq!(IssueLevel::Hint.to_string(), "hint");
    }

    // ── AnalyzeOptions from_config ───────────────────────────────────

    #[test]
    fn test_analyze_options_from_config_default_format() {
        let config = crate::config::AppConfig::default();
        let opts = AnalyzeOptions::from_config(&config);
        assert_eq!(opts.report_format, ReportFormat::Markdown);
        assert_eq!(opts.verbosity, Verbosity::Normal);
        assert!(opts.strip_ansi);
    }

    #[test]
    fn test_analyze_options_from_config_json_format() {
        let mut config = crate::config::AppConfig::default();
        config.report.format = "json".to_string();
        let opts = AnalyzeOptions::from_config(&config);
        assert_eq!(opts.report_format, ReportFormat::Json);
    }

    #[test]
    fn test_analyze_options_from_config_verbose() {
        let mut config = crate::config::AppConfig::default();
        config.report.verbosity = "verbose".to_string();
        let opts = AnalyzeOptions::from_config(&config);
        assert_eq!(opts.verbosity, Verbosity::Verbose);
    }

    #[test]
    fn test_analyze_options_from_config_minimal() {
        let mut config = crate::config::AppConfig::default();
        config.report.verbosity = "minimal".to_string();
        let opts = AnalyzeOptions::from_config(&config);
        assert_eq!(opts.verbosity, Verbosity::Minimal);
    }
}
