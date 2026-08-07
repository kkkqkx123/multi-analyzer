//! Cargo Output Parser
//Parsing the output of cargo check/clippy/test Parsing output from cargo check/clippy/test

use crate::core::{
    BaseParser, Issue, IssueLevel, Location, OutputParser, ParseResult, ParsedTestOutput, TestCase,
    TestOutputParser, TestStatus, TestSummary,
};

/// Auto-detect whether the output is from cargo nextest.
fn is_nextest_output(lines: &[&str]) -> bool {
    for line in lines.iter().take(20) {
        let trimmed = line.trim();
        if trimmed.starts_with("PASS [") || trimmed.starts_with("FAIL [") {
            return true;
        }
    }
    false
}

pub struct CargoParser {
    base: BaseParser,
}

impl CargoParser {
    pub fn new() -> Self {
        Self {
            base: BaseParser::new(),
        }
    }

    /// Extract package name from file path
    /// Cargo workspace structure: crates/package-name/src/... or packages/package-name/src/...
    fn extract_package_from_path(&self, file_path: &str) -> Option<String> {
        // Normalize path separators
        let normalized = file_path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').collect();

        // Look for common workspace directory names
        for i in 0..parts.len() {
            if (parts[i] == "crates" || parts[i] == "packages" || parts[i] == "members")
                && i + 1 < parts.len()
            {
                return Some(parts[i + 1].to_string());
            }
        }

        None
    }

    fn parse_single_line(&self, line: &str) -> Option<Issue> {
        let parts: Vec<&str> = line.splitn(5, ':').collect();
        if parts.len() < 5 {
            return None;
        }

        let file_path = parts[0].trim();
        let line_num = parts[1].trim().parse::<u32>().ok()?;
        let col_num = parts[2].trim().parse::<u32>().ok()?;
        let error_type = parts[3].trim();
        let description = parts[4].trim();

        let level = if error_type.starts_with("error") {
            IssueLevel::Error
        } else if error_type.starts_with("warning") {
            IssueLevel::Warning
        } else {
            return None;
        };

        let location = Location::new(file_path.to_string())
            .with_line(line_num)
            .with_column(col_num);

        let mut issue = Issue::new(level, description.to_string(), location);

        // Extract package name from file path
        if let Some(package) = self.extract_package_from_path(file_path) {
            issue = issue.with_package(package);
        }

        if let Some(code) = self.base.extract_error_code(error_type) {
            issue = issue.with_code(code);
        }

        Some(issue)
    }

    /// Split a rustc diagnostic header into its level, optional code and message.
    ///
    /// Recognises both the plain and the coded form:
    ///
    /// ```text
    /// error: cannot find value `x` in this scope   -> (Error,   None,          "cannot find ...")
    /// error[E0308]: mismatched types               -> (Error,   Some("E0308"), "mismatched types")
    /// warning[unused_imports]: unused import       -> (Warning, Some("unused_imports"), "unused import")
    /// ```
    ///
    /// Returns `None` for any line that is not a diagnostic header.
    fn split_diagnostic_header(line: &str) -> Option<(IssueLevel, Option<&str>, &str)> {
        let (level, rest) = if let Some(rest) = line.strip_prefix("error") {
            (IssueLevel::Error, rest)
        } else if let Some(rest) = line.strip_prefix("warning") {
            (IssueLevel::Warning, rest)
        } else {
            return None;
        };

        // Plain form: `error: message`
        if let Some(message) = rest.strip_prefix(':') {
            return Some((level, None, message.trim()));
        }

        // Coded form: `error[E0308]: message`
        let rest = rest.strip_prefix('[')?;
        let close = rest.find(']')?;
        let code = &rest[..close];
        let message = rest[close + 1..].strip_prefix(':')?;

        if code.is_empty() {
            return None;
        }

        Some((level, Some(code), message.trim()))
    }

    fn parse_multiline_error(
        &self,
        lines: &[String],
        start_index: usize,
    ) -> (Option<Issue>, usize) {
        if start_index >= lines.len() {
            return (None, start_index);
        }

        let line = &lines[start_index];

        // rustc emits either `error: msg` or, when the lint/diagnostic carries an
        // error code, `error[E0308]: msg`. The same holds for warnings
        // (`warning[unused_imports]: msg`). Both shapes must be recognised,
        // otherwise every coded diagnostic — which is the majority of real
        // compile errors — is silently dropped.
        let (level, code_from_header, desc) = match Self::split_diagnostic_header(line) {
            Some(parsed) => parsed,
            None => return (None, start_index + 1),
        };

        if start_index + 1 < lines.len() {
            let next_line = &lines[start_index + 1];
            let trimmed = next_line.trim();

            if let Some(location_part) = trimmed.strip_prefix("-->") {
                let location_part = location_part.trim();

                if let Some(location) = self.parse_cargo_location(location_part) {
                    // Extract package name from file path before moving location
                    let package = self.extract_package_from_path(&location.file_path);

                    let mut issue = Issue::new(level, desc.to_string(), location);

                    // Add package information
                    if let Some(pkg) = package {
                        issue = issue.with_package(pkg);
                    }

                    // Prefer the code carried by the header (`error[E0308]`),
                    // falling back to whatever can be recovered from the message.
                    if let Some(code) = code_from_header
                        .map(|c| c.to_string())
                        .or_else(|| self.base.extract_error_code(desc))
                    {
                        issue = issue.with_code(code);
                    }

                    return (Some(issue), start_index + 2);
                }
            }
        }

        (None, start_index + 1)
    }

    fn parse_cargo_location(&self, location_str: &str) -> Option<Location> {
        let mut colon_positions = Vec::new();
        for (i, c) in location_str.char_indices() {
            if c == ':' {
                colon_positions.push(i);
            }
        }

        if colon_positions.len() >= 2 {
            let last_colon = colon_positions[colon_positions.len() - 1];
            let second_last_colon = colon_positions[colon_positions.len() - 2];

            let col_part = &location_str[last_colon + 1..];
            let line_part = &location_str[second_last_colon + 1..last_colon];
            let file_part = &location_str[..second_last_colon];

            let line_num = line_part.parse::<u32>().ok()?;
            let col_num = col_part.parse::<u32>().ok()?;

            Some(
                Location::new(file_part.trim().to_string())
                    .with_line(line_num)
                    .with_column(col_num),
            )
        } else {
            None
        }
    }

    /// Parse cargo fmt output.
    ///
    /// `cargo fmt --check` outputs file paths that differ from the expected
    /// format, one per line. Without `--check`, the output is
    /// `Reformatting <path>`.
    fn parse_cargo_fmt_line(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let file_path = if let Some(rest) = trimmed.strip_prefix("Reformatting ") {
            rest.trim().to_string()
        } else if Self::is_bare_rust_file_path(trimmed) {
            trimmed.to_string()
        } else {
            return None;
        };

        let location = Location::new(file_path);
        let issue = Issue::new(
            IssueLevel::Warning,
            "Environment is not satisfied".to_string(),
            location,
        )
        .with_code("FORMAT".to_string());

        Some(issue)
    }

    /// Report whether a line is a bare `*.rs` path emitted by `cargo fmt --check`.
    ///
    /// This guard exists because `parse_cargo_fmt_line` is the last-resort branch
    /// of the parser and therefore sees every unmatched line of *any* cargo
    /// command. Accepting anything that merely contains a dot made cargo progress
    /// lines such as `Checking rust-demo v0.1.0 (/path)`, rustc note lines such as
    /// `--> src/main.rs:24:13` and diagnostic snippets be reported as bogus
    /// FORMAT issues.
    fn is_bare_rust_file_path(trimmed: &str) -> bool {
        // `cargo fmt --check` prints one plain path per line, nothing else.
        if !trimmed.ends_with(".rs") {
            return false;
        }
        // Progress/diagnostic lines are multi-token; a path never is.
        if trimmed.split_whitespace().count() != 1 {
            return false;
        }
        // rustc location lines (`--> src/main.rs:24:13`) and list markers are
        // not paths on their own.
        !trimmed.starts_with('-') && !trimmed.starts_with('=') && !trimmed.contains("-->")
    }
}

impl Default for CargoParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for CargoParser {
    // Custom parse implementation for Cargo output
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let lines: Vec<String> = output.lines().map(|s| s.to_string()).collect();
        let mut issues = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            let (issue, new_index) = self.parse_multiline_error(&lines, i);

            if let Some(issue) = issue {
                issues.push(issue);
                i = new_index;
            } else {
                if let Some(issue) = self.parse_single_line(&lines[i]) {
                    issues.push(issue);
                } else if let Some(issue) = self.parse_cargo_fmt_line(&lines[i]) {
                    issues.push(issue);
                }
                i += 1;
            }
        }

        ParseResult::Full(issues)
    }
}

impl TestOutputParser for CargoParser {
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput {
        let lines: Vec<&str> = output.lines().collect();

        if is_nextest_output(&lines) {
            return self.parse_nextest_output(&lines);
        }

        self.parse_cargo_test_output(&lines)
    }
}

impl CargoParser {
    fn parse_cargo_test_output(&self, lines: &[&str]) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();

        let combined_output = lines.join("\n");
        result.compile_issues =
            <Self as OutputParser>::parse(self, &combined_output).data_or_default_owned();

        let mut i = 0;
        let mut in_failures_section = false;
        let mut current_failure: Option<(String, Vec<String>)> = None;

        while i < lines.len() {
            let line = lines[i];

            if let Some(test_case) = self.parse_test_case_line(line) {
                match test_case.status {
                    TestStatus::Passed => result.passed_tests.push(test_case),
                    TestStatus::Failed => {
                        result.failed_tests.push(test_case);
                    }
                    TestStatus::Ignored(_) => result.ignored_tests.push(test_case),
                }
                i += 1;
                continue;
            }

            if line == "failures:" {
                in_failures_section = true;
                i += 1;
                continue;
            }

            if in_failures_section {
                if line.starts_with("---- ") && line.contains("stdout ----") {
                    let test_name =
                        line[5..line.find(" stdout ----").unwrap_or(line.len())].to_string();
                    current_failure = Some((test_name, Vec::new()));
                } else if line.trim().is_empty() && current_failure.is_some() {
                    if let Some((name, details)) = current_failure.take() {
                        if let Some(test) = result.failed_tests.iter_mut().find(|t| t.name == name)
                        {
                            test.failure_details = Some(details.join("\n"));
                            test.location = self.parse_panic_location(&details.join("\n"));
                        }
                    }
                } else if let Some((_, ref mut details)) = current_failure {
                    details.push(line.to_string());
                }

                if line.starts_with("test result:") {
                    in_failures_section = false;
                }
            }

            if line.starts_with("test result:") {
                result.test_summary = self.parse_test_summary(line);
            }

            i += 1;
        }

        result
    }

    fn parse_nextest_output(&self, lines: &[&str]) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();

        let combined_output = lines.join("\n");
        result.compile_issues =
            <Self as OutputParser>::parse(self, &combined_output).data_or_default_owned();

        let mut i = 0;
        let mut in_failure_detail = false;
        let mut current_failure_name: Option<String> = None;
        let mut current_failure_details: Vec<String> = Vec::new();

        while i < lines.len() {
            let line = lines[i].trim();

            // Parse nextest test case lines: "PASS [   0.002s] crate::test_name"
            if let Some(test_case) = self.parse_nextest_test_case_line(line) {
                match test_case.status {
                    TestStatus::Passed => result.passed_tests.push(test_case),
                    TestStatus::Failed => result.failed_tests.push(test_case),
                    TestStatus::Ignored(_) => result.ignored_tests.push(test_case),
                }
                i += 1;
                continue;
            }

            // Detect STDOUT/STDERR failure detail blocks
            if let Some(name) = line.strip_prefix("--- STDOUT:") {
                let name = name.strip_suffix("---").unwrap_or(name).trim().to_string();
                current_failure_name = Some(name);
                current_failure_details.clear();
                in_failure_detail = true;
                i += 1;
                continue;
            }

            if let Some(name) = line.strip_prefix("--- STDERR:") {
                // Flush previous STDOUT block if any
                if let Some(ref name) = current_failure_name.take() {
                    if let Some(test) = result.failed_tests.iter_mut().find(|t| t.name == *name) {
                        if test.failure_details.is_none() {
                            test.failure_details = Some(current_failure_details.join("\n"));
                            test.location =
                                self.parse_panic_location(&current_failure_details.join("\n"));
                        }
                    }
                }
                let name = name.strip_suffix("---").unwrap_or(name).trim().to_string();
                current_failure_name = Some(name);
                current_failure_details.clear();
                in_failure_detail = true;
                i += 1;
                continue;
            }

            // End of failure detail (nextest separator or next test line)
            if in_failure_detail
                && (line.starts_with("---") || line.starts_with("PASS [") || line.starts_with("FAIL ["))
            {
                if let Some(ref name) = current_failure_name.take() {
                    if let Some(test) = result.failed_tests.iter_mut().find(|t| t.name == *name) {
                        if test.failure_details.is_none() {
                            test.failure_details = Some(current_failure_details.join("\n"));
                            test.location =
                                self.parse_panic_location(&current_failure_details.join("\n"));
                        }
                    }
                }
                current_failure_details.clear();
                in_failure_detail = false;
                // Don't advance i here, reprocess current line
                continue;
            }

            if in_failure_detail {
                current_failure_details.push(line.to_string());
            }

            // Nextest summary line
            if line.starts_with("Summary [") {
                result.test_summary = self.parse_nextest_summary(line);
            }

            i += 1;
        }

        // Flush any remaining failure detail
        if let Some(ref name) = current_failure_name.take() {
            if !current_failure_details.is_empty() {
                if let Some(test) = result.failed_tests.iter_mut().find(|t| t.name == *name) {
                    if test.failure_details.is_none() {
                        test.failure_details = Some(current_failure_details.join("\n"));
                        test.location =
                            self.parse_panic_location(&current_failure_details.join("\n"));
                    }
                }
            }
        }

        result
    }

    /// Parse nextest test case line: "PASS [   0.002s] crate::test_name"
    fn parse_nextest_test_case_line(&self, line: &str) -> Option<TestCase> {
        let re = regex::Regex::new(
            r"^\s*(PASS|FAIL|SKIP)\s*\[[^\]]*\]\s+(.+)",
        )
        .ok()?;

        let caps = re.captures(line)?;

        let status_str = caps.get(1)?.as_str();
        let name = caps.get(2)?.as_str().trim().to_string();

        let status = match status_str {
            "PASS" => TestStatus::Passed,
            "FAIL" => TestStatus::Failed,
            "SKIP" => TestStatus::Ignored(None),
            _ => return None,
        };

        Some(TestCase {
            name,
            status,
            location: None,
            failure_details: None,
            execution_time: None,
        })
    }

    /// Parse nextest summary line: "     Summary [   0.003s] 2 tests run: 1 passed, 1 failed, 0 skipped"
    fn parse_nextest_summary(&self, line: &str) -> Option<TestSummary> {
        let re = regex::Regex::new(
            r"Summary\s*\[[^\]]*\]\s+(\d+)\s+tests?\s+run:\s+(\d+)\s+passed,\s+(\d+)\s+failed,\s+(\d+)\s+skipped",
        )
        .ok()?;

        let caps = re.captures(line)?;

        let total: usize = caps.get(1)?.as_str().parse().ok()?;
        let passed: usize = caps.get(2)?.as_str().parse().ok()?;
        let failed: usize = caps.get(3)?.as_str().parse().ok()?;
        let skipped: usize = caps.get(4)?.as_str().parse().ok()?;

        Some(TestSummary {
            total,
            passed,
            failed,
            ignored: skipped,
            measured: 0,
            filtered: 0,
            execution_time: None,
        })
    }
}

impl CargoParser {
    /// Parsing individual test case lines
    fn parse_test_case_line(&self, line: &str) -> Option<TestCase> {
        // 匹配: "test <name> ... ok/FAILED/ignored"
        let re =
            regex::Regex::new(r"^test\s+(\S+)\s+\.\.\.\s+(ok|FAILED|ignored)(?:\s*\(([^)]+)\))?")
                .ok()?;

        let caps = re.captures(line)?;

        let name = caps.get(1)?.as_str().to_string();
        let result_str = caps.get(2)?.as_str();
        let extra = caps.get(3).map(|m| m.as_str());

        let status = match result_str {
            "ok" => TestStatus::Passed,
            "FAILED" => TestStatus::Failed,
            "ignored" => TestStatus::Ignored(extra.map(|s| s.to_string())),
            _ => return None,
        };

        // Try to parse the execution time from extra
        let execution_time = extra.and_then(|e| e.strip_suffix('s').unwrap_or(e).parse().ok());

        Some(TestCase {
            name,
            status,
            location: None,
            failure_details: None,
            execution_time,
        })
    }

    /// Parsing the location from a panic message
    fn parse_panic_location(&self, detail: &str) -> Option<Location> {
        let re = regex::Regex::new(r"panicked at\s+(\S+):(\d+):(\d+)").ok()?;
        let caps = re.captures(detail)?;

        Some(
            Location::new(caps.get(1)?.as_str().to_string())
                .with_line(caps.get(2)?.as_str().parse().ok()?)
                .with_column(caps.get(3)?.as_str().parse().ok()?),
        )
    }

    /// Summary of Parsing Test Results
    fn parse_test_summary(&self, line: &str) -> Option<TestSummary> {
        // 匹配: "test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
        let re = regex::Regex::new(
            r"test result:\s+(ok|FAILED)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored;\s+(\d+)\s+measured;\s+(\d+)\s+filtered out",
        )
        .ok()?;

        let caps = re.captures(line)?;

        let passed: usize = caps.get(2)?.as_str().parse().ok()?;
        let failed: usize = caps.get(3)?.as_str().parse().ok()?;
        let ignored: usize = caps.get(4)?.as_str().parse().ok()?;
        let measured: usize = caps.get(5)?.as_str().parse().ok()?;
        let filtered: usize = caps.get(6)?.as_str().parse().ok()?;

        Some(TestSummary {
            total: passed + failed + ignored,
            passed,
            failed,
            ignored,
            measured,
            filtered,
            execution_time: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_line() {
        let parser = CargoParser::new();
        let line = "src/main.rs:10:5: error[E0308]: mismatched types";
        let issue = parser.parse_single_line(line).unwrap();

        assert_eq!(issue.location.file_path, "src/main.rs");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.code, Some("E0308".to_string()));
        assert_eq!(issue.message, "mismatched types");
        assert!(matches!(issue.level, IssueLevel::Error));
    }

    #[test]
    fn test_parse_multiline_error() {
        let parser = CargoParser::new();
        let lines = vec![
            "error: mismatched types".to_string(),
            " --> src/main.rs:10:5".to_string(),
        ];

        let (issue, next_index) = parser.parse_multiline_error(&lines, 0);
        let issue = issue.unwrap();

        assert_eq!(issue.location.file_path, "src/main.rs");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.message, "mismatched types");
        assert_eq!(next_index, 2);
    }

    #[test]
    fn test_parse_windows_path() {
        let parser = CargoParser::new();
        let lines = vec![
            "error: some error".to_string(),
            " --> D:\\project\\src\\main.rs:10:5".to_string(),
        ];

        let (issue, _) = parser.parse_multiline_error(&lines, 0);
        let issue = issue.unwrap();

        assert_eq!(issue.location.file_path, "D:\\project\\src\\main.rs");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
    }

    // ── coded multi-line diagnostics ────────────────────────────────

    #[test]
    fn test_split_diagnostic_header_plain() {
        let (level, code, msg) =
            CargoParser::split_diagnostic_header("error: cannot find value `x`").unwrap();
        assert!(matches!(level, IssueLevel::Error));
        assert_eq!(code, None);
        assert_eq!(msg, "cannot find value `x`");
    }

    #[test]
    fn test_split_diagnostic_header_coded() {
        let (level, code, msg) =
            CargoParser::split_diagnostic_header("error[E0308]: mismatched types").unwrap();
        assert!(matches!(level, IssueLevel::Error));
        assert_eq!(code, Some("E0308"));
        assert_eq!(msg, "mismatched types");
    }

    #[test]
    fn test_split_diagnostic_header_warning_coded() {
        let (level, code, msg) =
            CargoParser::split_diagnostic_header("warning[unused_imports]: unused import").unwrap();
        assert!(matches!(level, IssueLevel::Warning));
        assert_eq!(code, Some("unused_imports"));
        assert_eq!(msg, "unused import");
    }

    #[test]
    fn test_split_diagnostic_header_rejects_non_diagnostics() {
        assert!(CargoParser::split_diagnostic_header("   Compiling foo v0.1.0").is_none());
        assert!(CargoParser::split_diagnostic_header("--> src/main.rs:1:1").is_none());
        assert!(CargoParser::split_diagnostic_header("errors happened").is_none());
    }

    /// Coded errors must survive the full multi-line parse; regression test for
    /// `error[E0308]:` headers being dropped entirely.
    #[test]
    fn test_parse_coded_multiline_error() {
        let parser = CargoParser::new();
        let output = "error[E0308]: mismatched types\n --> src/calc.rs:9:5\n  |\n";
        let issues = match parser.parse(output) {
            ParseResult::Full(issues) => issues,
            _ => panic!("expected a full parse"),
        };

        assert_eq!(issues.len(), 1);
        assert!(matches!(issues[0].level, IssueLevel::Error));
        assert_eq!(issues[0].code, Some("E0308".to_string()));
        assert_eq!(issues[0].location.file_path, "src/calc.rs");
        assert_eq!(issues[0].location.line_number, Some(9));
    }

    // ── cargo fmt fallback must not swallow ordinary output ─────────

    #[test]
    fn test_is_bare_rust_file_path_accepts_plain_paths() {
        assert!(CargoParser::is_bare_rust_file_path("src/main.rs"));
        assert!(CargoParser::is_bare_rust_file_path("crates/foo/src/lib.rs"));
    }

    #[test]
    fn test_is_bare_rust_file_path_rejects_progress_and_notes() {
        // Cargo progress lines contain dots but are not paths.
        assert!(!CargoParser::is_bare_rust_file_path(
            "Checking rust-demo v0.1.0 (/workspace/examples/rust-demo)"
        ));
        // rustc location notes.
        assert!(!CargoParser::is_bare_rust_file_path("--> src/main.rs:24:13"));
        // Diagnostic snippets and summaries.
        assert!(!CargoParser::is_bare_rust_file_path(
            "24 |     numbers.push_all(&[4, 5]);"
        ));
        assert!(!CargoParser::is_bare_rust_file_path(
            "Some errors have detailed explanations: E0308, E0425."
        ));
    }

    /// Regression test: a `cargo check` transcript must not yield bogus FORMAT
    /// issues for progress lines.
    #[test]
    fn test_parse_ignores_cargo_progress_lines() {
        let parser = CargoParser::new();
        let output = "   Compiling rust-demo v0.1.0 (/workspace/examples/rust-demo)\n\
                      Checking rust-demo v0.1.0 (/workspace/examples/rust-demo)\n\
                      For more information about an error, try `rustc --explain E0308`.\n";
        let issues = match parser.parse(output) {
            ParseResult::Full(issues) => issues,
            _ => panic!("expected a full parse"),
        };

        assert!(
            issues.is_empty(),
            "progress lines must not be reported as issues, got: {:?}",
            issues
        );
    }

    // ── extract_package_from_path ───────────────────────────────────

    #[test]
    fn test_extract_package_from_path_crates() {
        let parser = CargoParser::new();
        assert_eq!(
            parser.extract_package_from_path("crates/my-crate/src/main.rs"),
            Some("my-crate".to_string())
        );
    }

    #[test]
    fn test_extract_package_from_path_packages() {
        let parser = CargoParser::new();
        assert_eq!(
            parser.extract_package_from_path("packages/foo/src/lib.rs"),
            Some("foo".to_string())
        );
    }

    #[test]
    fn test_extract_package_from_path_members() {
        let parser = CargoParser::new();
        assert_eq!(
            parser.extract_package_from_path("members/bar/src/main.rs"),
            Some("bar".to_string())
        );
    }

    #[test]
    fn test_extract_package_from_path_no_match() {
        let parser = CargoParser::new();
        assert_eq!(
            parser.extract_package_from_path("src/main.rs"),
            None
        );
    }

    #[test]
    fn test_extract_package_from_path_unknown_prefix() {
        let parser = CargoParser::new();
        assert_eq!(
            parser.extract_package_from_path("other/test/src/main.rs"),
            None
        );
    }

    // ── parse_cargo_location ────────────────────────────────────────

    #[test]
    fn test_parse_cargo_location_standard() {
        let parser = CargoParser::new();
        let loc = parser.parse_cargo_location("src/main.rs:10:5").unwrap();
        assert_eq!(loc.file_path, "src/main.rs");
        assert_eq!(loc.line_number, Some(10));
        assert_eq!(loc.column_number, Some(5));
    }

    #[test]
    fn test_parse_cargo_location_invalid() {
        let parser = CargoParser::new();
        assert!(parser.parse_cargo_location("no colons").is_none());
    }

    // ── parse_cargo_fmt_line ────────────────────────────────────────

    #[test]
    fn test_parse_cargo_fmt_reformatting() {
        let parser = CargoParser::new();
        let issue = parser.parse_cargo_fmt_line("Reformatting src/main.rs").unwrap();
        assert_eq!(issue.location.file_path, "src/main.rs");
        assert_eq!(issue.code, Some("FORMAT".to_string()));
    }

    #[test]
    fn test_parse_cargo_fmt_direct_path() {
        let parser = CargoParser::new();
        let issue = parser.parse_cargo_fmt_line("src/main.rs").unwrap();
        assert_eq!(issue.location.file_path, "src/main.rs");
    }

    #[test]
    fn test_parse_cargo_fmt_no_match() {
        let parser = CargoParser::new();
        assert!(parser.parse_cargo_fmt_line("").is_none());
        assert!(parser.parse_cargo_fmt_line("  ").is_none());
    }

    // ── OutputParser trait ──────────────────────────────────────────

    #[test]
    fn test_parse_via_trait_empty() {
        let parser = CargoParser::new();
        let result = parser.parse("");
        assert!(result.is_full());
        assert!(result.data().unwrap().is_empty());
    }

    #[test]
    fn test_parse_via_trait_mixed() {
        let parser = CargoParser::new();
        let output = "error: mismatched types\n --> src/main.rs:10:5\n";
        let result = parser.parse(output);
        assert!(result.is_full());
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].message, "mismatched types");
    }

    // ── parse_test_case_line ────────────────────────────────────────

    #[test]
    fn test_parse_test_case_passed() {
        let parser = CargoParser::new();
        let tc = parser.parse_test_case_line("test test_foo ... ok").unwrap();
        assert_eq!(tc.name, "test_foo");
        assert_eq!(tc.status, TestStatus::Passed);
    }

    #[test]
    fn test_parse_test_case_failed() {
        let parser = CargoParser::new();
        let tc = parser.parse_test_case_line("test test_bar ... FAILED").unwrap();
        assert_eq!(tc.name, "test_bar");
        assert_eq!(tc.status, TestStatus::Failed);
    }

    #[test]
    fn test_parse_test_case_ignored() {
        let parser = CargoParser::new();
        let tc = parser.parse_test_case_line("test test_baz ... ignored").unwrap();
        assert_eq!(tc.name, "test_baz");
        assert_eq!(tc.status, TestStatus::Ignored(None));
    }

    #[test]
    fn test_parse_test_case_no_match() {
        let parser = CargoParser::new();
        assert!(parser.parse_test_case_line("running 1 test").is_none());
    }

    // ── parse_panic_location ────────────────────────────────────────

    #[test]
    fn test_parse_panic_location() {
        let parser = CargoParser::new();
        let detail = "panicked at src/main.rs:42:5: assertion failed";
        let loc = parser.parse_panic_location(detail).unwrap();
        assert_eq!(loc.file_path, "src/main.rs");
        assert_eq!(loc.line_number, Some(42));
        assert_eq!(loc.column_number, Some(5));
    }

    #[test]
    fn test_parse_panic_location_no_match() {
        let parser = CargoParser::new();
        assert!(parser.parse_panic_location("no panic here").is_none());
    }

    // ── parse_test_summary ──────────────────────────────────────────

    #[test]
    fn test_parse_test_summary_ok() {
        let parser = CargoParser::new();
        let summary = parser
            .parse_test_summary("test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out")
            .unwrap();
        assert_eq!(summary.total, 5);
        assert_eq!(summary.passed, 5);
        assert_eq!(summary.failed, 0);
    }

    #[test]
    fn test_parse_test_summary_failed() {
        let parser = CargoParser::new();
        let summary = parser
            .parse_test_summary("test result: FAILED. 3 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out")
            .unwrap();
        assert_eq!(summary.total, 5);
        assert_eq!(summary.passed, 3);
        assert_eq!(summary.failed, 2);
    }

    #[test]
    fn test_parse_test_summary_no_match() {
        let parser = CargoParser::new();
        assert!(parser.parse_test_summary("random line").is_none());
    }

    // ── is_nextest_output ───────────────────────────────────────────

    #[test]
    fn test_is_nextest_output_true() {
        let lines = vec!["PASS [   0.002s] crate::test_foo", "FAIL [   0.001s] crate::test_bar"];
        assert!(is_nextest_output(&lines));
    }

    #[test]
    fn test_is_nextest_output_false() {
        let lines = vec!["running 1 test", "test test_foo ... ok"];
        assert!(!is_nextest_output(&lines));
    }

    #[test]
    fn test_is_nextest_output_empty() {
        assert!(!is_nextest_output(&[]));
    }

    // ── parse_nextest_test_case_line ────────────────────────────────

    #[test]
    fn test_parse_nextest_test_case_passed() {
        let parser = CargoParser::new();
        let tc = parser.parse_nextest_test_case_line("PASS [   0.002s] crate::test_foo").unwrap();
        assert_eq!(tc.name, "crate::test_foo");
        assert_eq!(tc.status, TestStatus::Passed);
    }

    #[test]
    fn test_parse_nextest_test_case_failed() {
        let parser = CargoParser::new();
        let tc = parser.parse_nextest_test_case_line("FAIL [   0.001s] crate::test_bar").unwrap();
        assert_eq!(tc.name, "crate::test_bar");
        assert_eq!(tc.status, TestStatus::Failed);
    }

    #[test]
    fn test_parse_nextest_test_case_skipped() {
        let parser = CargoParser::new();
        let tc = parser.parse_nextest_test_case_line("SKIP [   0.000s] crate::test_baz").unwrap();
        assert_eq!(tc.name, "crate::test_baz");
        assert_eq!(tc.status, TestStatus::Ignored(None));
    }

    #[test]
    fn test_parse_nextest_test_case_no_match() {
        let parser = CargoParser::new();
        assert!(parser.parse_nextest_test_case_line("random line").is_none());
    }

    // ── parse_nextest_summary ───────────────────────────────────────

    #[test]
    fn test_parse_nextest_summary() {
        let parser = CargoParser::new();
        let summary = parser
            .parse_nextest_summary("     Summary [   0.003s] 5 tests run: 3 passed, 2 failed, 0 skipped")
            .unwrap();
        assert_eq!(summary.total, 5);
        assert_eq!(summary.passed, 3);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.ignored, 0);
    }

    #[test]
    fn test_parse_nextest_summary_no_match() {
        let parser = CargoParser::new();
        assert!(parser.parse_nextest_summary("random line").is_none());
    }
}
