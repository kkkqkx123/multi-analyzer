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
    pub unique_patterns: HashSet<String>,
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
        let package_key = issue.package.clone().unwrap_or_else(|| "unknown".to_string());
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
        self.issues_by_level.get(&IssueLevel::Error).copied().unwrap_or(0)
    }

    /// Get total warning count
    pub fn warning_count(&self) -> usize {
        self.issues_by_level.get(&IssueLevel::Warning).copied().unwrap_or(0)
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
    pub measured: usize,
    pub filtered: usize,
    /// Execution time in seconds (available for external use)
    #[allow(dead_code)]
    pub execution_time: Option<f64>,
}

impl TestSummary {
    /// Get execution time in seconds if available (available for external use)
    #[allow(dead_code)]
    pub fn execution_time(&self) -> Option<f64> {
        self.execution_time
    }

    /// Get execution time formatted as string (available for external use)
    #[allow(dead_code)]
    pub fn execution_time_formatted(&self) -> String {
        match self.execution_time {
            Some(time) => format!("{:.2}s", time),
            None => "N/A".to_string(),
        }
    }
}

/// Extending AnalysisResult to support test information
#[derive(Debug, Default)]
pub struct TestAnalysisResult {
    /// Problems at the compilation stage
    pub compile_result: AnalysisResult,
    /// Test Summary
    pub test_summary: Option<TestSummary>,
    /// Failed Test Cases
    pub failed_tests: Vec<TestCase>,
    /// Test cases passed
    pub passed_tests: Vec<TestCase>,
    /// Neglected Test Cases
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

    /// Check if all tests passed (no failures and no compile issues)
    pub fn all_passed(&self) -> bool {
        self.failed_tests.is_empty() && self.compile_result.total_issues == 0
    }

    /// Get total test count
    pub fn total_tests(&self) -> usize {
        self.passed_tests.len() + self.failed_tests.len() + self.ignored_tests.len()
    }
}

/// Technology stack type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechStack {
    Cargo,
    Maven,
    Gradle,
    Npm,
    Pnpm,
    Yarn,
    Mypy,
    Pytest,
    GoBuild,
    GolangciLint,
    CMake,
    Gcc,
    Clang,
    Msvc,
}

impl TechStack {
    pub fn as_str(&self) -> &'static str {
        match self {
            TechStack::Cargo => "cargo",
            TechStack::Maven => "maven",
            TechStack::Gradle => "gradle",
            TechStack::Npm => "npm",
            TechStack::Pnpm => "pnpm",
            TechStack::Yarn => "yarn",
            TechStack::Mypy => "mypy",
            TechStack::Pytest => "pytest",
            TechStack::GoBuild => "go",
            TechStack::GolangciLint => "golangci-lint",
            TechStack::CMake => "cmake",
            TechStack::Gcc => "gcc",
            TechStack::Clang => "clang",
            TechStack::Msvc => "msvc",
        }
    }
}

impl std::str::FromStr for TechStack {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cargo" | "rust" => Ok(TechStack::Cargo),
            "maven" | "mvn" => Ok(TechStack::Maven),
            "gradle" | "gradlew" => Ok(TechStack::Gradle),
            "npm" | "node" => Ok(TechStack::Npm),
            "pnpm" => Ok(TechStack::Pnpm),
            "yarn" => Ok(TechStack::Yarn),
            "mypy" => Ok(TechStack::Mypy),
            "pytest" | "py.test" => Ok(TechStack::Pytest),
            "go" | "golang" => Ok(TechStack::GoBuild),
            "golangci-lint" => Ok(TechStack::GolangciLint),
            "cmake" | "cmake-build" => Ok(TechStack::CMake),
            "gcc" | "g++" => Ok(TechStack::Gcc),
            "clang" | "clang++" => Ok(TechStack::Clang),
            "msvc" | "cl" => Ok(TechStack::Msvc),
            _ => Err(format!("Unknown tech stack: {}", s)),
        }
    }
}

/// Command category for grouping and organization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Check,      // Syntax and type checking
    Lint,       // Code linting
    Test,       // Test execution
    Audit,      // Security audit
    Build,      // Build compilation
    Format,     // Code formatting
    Custom,     // User-defined
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

    /// All commands are treated as custom since we use free-form strings
    pub fn is_custom(&self) -> bool {
        true
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

/// Analyzing Options
#[derive(Debug, Default, Clone)]
pub struct AnalyzeOptions {
    pub subcommand: Option<SubCommand>,
    pub filter_warnings: bool,
    pub filter_paths: Vec<String>,
    pub output_file: Option<String>,
    /// Show all issues without truncation
    pub verbose: bool,
    // C++ related options
    pub source_dir: Option<String>,
    pub build_dir: Option<String>,
    pub cmake_generator: Option<String>,
    pub target: Option<String>,
    pub target_files: Vec<String>,
    pub include_paths: Vec<String>,
    pub defines: Vec<String>,
    pub cpp_standard: Option<String>,
    pub json_output: bool,

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

/// Report format
#[derive(Debug, Clone, Copy, Default)]
pub enum ReportFormat {
    #[default]
    Markdown,
    Json,
    Html,
}

impl std::str::FromStr for ReportFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "markdown" | "md" => Ok(ReportFormat::Markdown),
            "json" => Ok(ReportFormat::Json),
            "html" => Ok(ReportFormat::Html),
            _ => Err(format!("Unknown report format: {}", s)),
        }
    }
}
