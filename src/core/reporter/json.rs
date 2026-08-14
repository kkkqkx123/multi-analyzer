//! JSON Report Generator

use super::{Reporter, ReporterError, ReportOptions};
use crate::core::types::{AnalysisResult, IssueLevel, TestAnalysisResult, TestStatus};

/// JSON Report Generator
pub struct JsonReporter;

/// Escape a string for embedding inside a JSON string literal.
///
/// JSON forbids raw control characters in strings; runner output routinely
/// contains them (surefire stack traces indent with tabs, messages may carry
/// carriage returns). Beyond quotes and backslashes this encodes every
/// control character as an escape sequence so the emitted document always
/// parses.
fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

impl JsonReporter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for JsonReporter {
    fn generate(&self, result: &AnalysisResult) -> Result<String, ReporterError> {
        self.generate_with_options(result, ReportOptions::default())
    }

    fn generate_with_options(
        &self,
        result: &AnalysisResult,
        options: ReportOptions,
    ) -> Result<String, ReporterError> {
        // Success short-circuit: if no issues found and short-circuit is enabled,
        // output a single-line confirmation instead of a full JSON report
        if options.success_short_circuit && result.total_issues == 0 {
            if let Some(msg) = options.short_circuit_message() {
                return Ok(msg);
            }
        }

        let mut json = String::new();
        json.push_str("{\n");

        // metadata
        json.push_str("  \"metadata\": {\n");
        json.push_str(&format!("    \"total\": {},\n", result.total_issues));
        json.push_str(&format!(
            "    \"categories\": {},\n",
            result.unique_patterns.len()
        ));
        json.push_str(&format!(
            "    \"files_affected\": {}\n",
            result.issues_by_file.len()
        ));
        json.push_str("  },\n");

        // Statistics by level
        json.push_str("  \"summary_by_level\": {\n");
        let level_order = ["error", "warning", "info", "hint"];
        let mut first = true;
        for level_str in &level_order {
            if let Some((level, count)) = result
                .issues_by_level
                .iter()
                .find(|(l, _)| l.to_string() == *level_str)
            {
                if !first {
                    json.push_str(",\n");
                }
                json.push_str(&format!("    \"{}\": {}", level, count));
                first = false;
            }
        }
        json.push_str("\n  },\n");

        // Statistics by category
        if !result.issues_by_type.is_empty() {
            json.push_str("  \"summary_by_category\": {\n");
            let mut types: Vec<_> = result.issues_by_type.iter().collect();
            types.sort_by(|a, b| b.1.cmp(a.1));
            for (i, (issue_type, count)) in types.iter().enumerate() {
                let comma = if i < types.len() - 1 { "," } else { "" };
                json.push_str(&format!(
                    "    \"{}\": {}{}\n",
                    escape_json_str(issue_type),
                    count,
                    comma
                ));
            }
            json.push_str("  },\n");
        }

        // Top error codes
        let top_codes = result.top_error_codes(5);
        if !top_codes.is_empty() {
            json.push_str("  \"top_error_codes\": [\n");
            for (i, (code, count)) in top_codes.iter().enumerate() {
                let comma = if i < top_codes.len() - 1 { "," } else { "" };
                json.push_str(&format!(
                    "    {{\"code\": \"{}\", \"count\": {}}}{}\n",
                    escape_json_str(code),
                    count,
                    comma
                ));
            }
            json.push_str("  ],\n");
        }

        // Detailed list of questions
        json.push_str("  \"items\": [\n");
        let all_issues: Vec<_> = result.issues_by_file.values().flatten().collect();
        for (i, issue) in all_issues.iter().enumerate() {
            let comma = if i < all_issues.len() - 1 { "," } else { "" };
            json.push_str("    {\n");
            json.push_str(&format!("      \"severity\": \"{}\",\n", issue.level));
            if let Some(code) = &issue.code {
                json.push_str(&format!("      \"code\": \"{}\",\n", code));
            }
                json.push_str(&format!(
                    "      \"message\": \"{}\",\n",
                    escape_json_str(&issue.message)
                ));
                json.push_str("      \"location\": {\n");
                json.push_str(&format!(
                    "        \"file\": \"{}\"",
                    escape_json_str(&issue.location.file_path)
                ));
            if let Some(line) = issue.location.line_number {
                json.push_str(&format!(",\n        \"line\": {}", line));
            }
            if let Some(col) = issue.location.column_number {
                json.push_str(&format!(",\n        \"column\": {}", col));
            }
            json.push_str("\n      }\n");
            json.push_str(&format!("    }}{}\n", comma));
        }
        json.push_str("  ]\n");

        json.push('}');
        Ok(json)
    }

    /// Generate a structured JSON test report containing the test summary and
    /// per-case results in addition to any compile issues.
    fn generate_test_report(
        &self,
        result: &TestAnalysisResult,
    ) -> Result<String, ReporterError> {
        self.generate_test_report_with_options(result, ReportOptions::default())
    }

    fn generate_test_report_with_options(
        &self,
        result: &TestAnalysisResult,
        _options: ReportOptions,
    ) -> Result<String, ReporterError> {
        // Unlike the compile report, the test report must always carry the
        // full test statistics (a run with failing tests but zero compile
        // issues must not short-circuit to "no issues found").
        let mut json = String::new();
        json.push_str("{\n");

        // Compile metadata
        json.push_str("  \"metadata\": {\n");
        json.push_str(&format!(
            "    \"total_issues\": {},\n",
            result.compile_result.total_issues
        ));
        let errors = result
            .compile_result
            .issues_by_level
            .get(&IssueLevel::Error)
            .copied()
            .unwrap_or(0);
        let warnings = result
            .compile_result
            .issues_by_level
            .get(&IssueLevel::Warning)
            .copied()
            .unwrap_or(0);
        json.push_str(&format!("    \"compile_errors\": {},\n", errors));
        json.push_str(&format!(
            "    \"compile_warnings\": {},\n",
            warnings
        ));
        json.push_str(&format!(
            "    \"total_tests\": {},\n",
            result.total_tests()
        ));
        // Detail completeness metric: number of per-case entries the parser
        // collected. Always <= total_tests for runners that aggregate passing
        // tests at class granularity (e.g. Maven) or emit no per-case lines
        // when piped (e.g. Vitest).
        json.push_str(&format!(
            "    \"collected_tests\": {}\n",
            result.collected_tests()
        ));
        json.push_str("  },\n");

        // Test summary
        if let Some(ref summary) = result.test_summary {
            json.push_str("  \"test_summary\": {\n");
            json.push_str(&format!("    \"total\": {},\n", summary.total));
            json.push_str(&format!("    \"passed\": {},\n", summary.passed));
            json.push_str(&format!("    \"failed\": {},\n", summary.failed));
            json.push_str(&format!("    \"ignored\": {},\n", summary.ignored));
            json.push_str(&format!("    \"measured\": {},\n", summary.measured));
            json.push_str(&format!("    \"filtered\": {}\n", summary.filtered));
            json.push_str("  },\n");
        } else {
            json.push_str("  \"test_summary\": null,\n");
        }

        // Failed test cases
        json.push_str("  \"failed_tests\": [\n");
        for (i, test) in result.failed_tests.iter().enumerate() {
            let comma = if i < result.failed_tests.len() - 1 { "," } else { "" };
            json.push_str("    {\n");
            json.push_str(&format!(
                "      \"name\": \"{}\",\n",
                escape_json_str(&test.name)
            ));
            if let Some(ref details) = test.failure_details {
                json.push_str(&format!(
                    "      \"failure_details\": \"{}\",\n",
                    escape_json_str(details)
                ));
            }
            json.push_str(&format!(
                "      \"execution_time\": {}\n",
                test.execution_time
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "null".to_string())
            ));
            json.push_str(&format!("    }}{}\n", comma));
        }
        json.push_str("  ],\n");

        // Passed test cases
        json.push_str("  \"passed_tests\": [\n");
        for (i, test) in result.passed_tests.iter().enumerate() {
            let comma = if i < result.passed_tests.len() - 1 { "," } else { "" };
            json.push_str("    {\n");
            json.push_str(&format!(
                "      \"name\": \"{}\",\n",
                escape_json_str(&test.name)
            ));
            json.push_str(&format!(
                "      \"execution_time\": {}\n",
                test.execution_time
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "null".to_string())
            ));
            json.push_str(&format!("    }}{}\n", comma));
        }
        json.push_str("  ],\n");

        // Ignored test cases
        json.push_str("  \"ignored_tests\": [\n");
        for (i, test) in result.ignored_tests.iter().enumerate() {
            let comma = if i < result.ignored_tests.len() - 1 { "," } else { "" };
            json.push_str(&format!(
                "      \"name\": \"{}\",\n",
                escape_json_str(&test.name)
            ));
            let reason = match &test.status {
                TestStatus::Ignored(Some(r)) => r.clone(),
                _ => String::new(),
            };
            if !reason.is_empty() {
                json.push_str(&format!(
                    "      \"reason\": \"{}\",\n",
                    escape_json_str(&reason)
                ));
            }
            json.push_str(&format!(
                "      \"execution_time\": {}\n",
                test.execution_time
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "null".to_string())
            ));
            json.push_str(&format!("    }}{}\n", comma));
        }
        json.push_str("  ],\n");

        // Compile issues (reuse the compile-report JSON generation)
        let compile_json = self.generate_with_options(
            &result.compile_result,
            ReportOptions {
                success_short_circuit: false,
                ..Default::default()
            },
        )?;
        json.push_str("  \"compile_issues\": ");
        json.push_str(&compile_json);
        json.push('\n');
        json.push('}');
        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Issue, IssueLevel, Location, TestSummary};
    use crate::core::reporter::ReportOptions;

    fn sample_result() -> AnalysisResult {
        let mut r = AnalysisResult::new();
        r.add_issue(Issue::new(
            IssueLevel::Error,
            "type mismatch",
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
    fn test_json_generate_empty() {
        let reporter = JsonReporter::new();
        let result = AnalysisResult::new();
        let report = reporter.generate(&result).unwrap();
        assert!(report.contains("\"metadata\""));
        assert!(report.contains("\"total\": 0"));
        assert!(report.contains("\"items\": ["));
    }

    #[test]
    fn test_json_generate_with_issues() {
        let reporter = JsonReporter::new();
        let report = reporter.generate(&sample_result()).unwrap();
        assert!(report.contains("\"total\": 2"));
        assert!(report.contains("\"error\": 1"));
        assert!(report.contains("\"warning\": 1"));
        assert!(report.contains("type mismatch"));
        assert!(report.contains("unused variable"));
        assert!(report.contains("src/main.rs"));
        assert!(report.contains("src/lib.rs"));
    }

    #[test]
    fn test_json_generate_valid_json() {
        let reporter = JsonReporter::new();
        let report = reporter.generate(&sample_result()).unwrap();
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(parsed["metadata"]["total"], 2);
        assert_eq!(parsed["summary_by_level"]["error"], 1);
        assert_eq!(parsed["items"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_json_short_circuit() {
        let reporter = JsonReporter::new();
        let empty = AnalysisResult::new();
        let opts = ReportOptions {
            success_short_circuit: true,
            tech_stack: Some("cargo check".to_string()),
            ..Default::default()
        };
        let report = reporter.generate_with_options(&empty, opts).unwrap();
        assert_eq!(report, "cargo check: no issues found");
    }

    fn sample_test_result() -> TestAnalysisResult {
        use crate::core::types::TestCase;
        let mut result = TestAnalysisResult::from_compile_result(AnalysisResult::new());
        result.test_summary = Some(TestSummary {
            total: 2,
            passed: 1,
            failed: 1,
            ignored: 0,
            measured: 0,
            filtered: 0,
            execution_time: Some(0.5),
        });
        result.passed_tests.push(TestCase::new("AppTest::testGreet", TestStatus::Passed));
        result.failed_tests.push(
            TestCase::new("AppTest::testFailingCase", TestStatus::Failed)
                .with_failure_details("expected:<Hello[ World]> but was:<Hello[]>")
                .with_execution_time(0.008),
        );
        result
    }

    #[test]
    fn test_json_test_report_includes_test_summary() {
        let reporter = JsonReporter::new();
        let result = sample_test_result();
        let report = reporter.generate_test_report(&result).unwrap();

        assert!(report.contains("\"test_summary\""));
        assert!(report.contains("\"total\": 2"));
        assert!(report.contains("\"passed\": 1"));
        assert!(report.contains("\"failed\": 1"));
        assert!(report.contains("\"failed_tests\""));
        assert!(report.contains("AppTest::testFailingCase"));
        assert!(report.contains("\"passed_tests\""));
        assert!(report.contains("AppTest::testGreet"));
    }

    #[test]
    fn test_json_test_report_not_short_circuited_on_failures() {
        // A run with failing tests but zero compile issues must still emit a
        // full JSON report (not the "no issues found" short-circuit).
        let reporter = JsonReporter::new();
        let result = sample_test_result();
        assert_eq!(result.compile_result.total_issues, 0);
        assert!(!result.all_passed());

        let opts = ReportOptions {
            success_short_circuit: true,
            tech_stack: Some("gradle test".to_string()),
            ..Default::default()
        };
        let report = reporter
            .generate_test_report_with_options(&result, opts)
            .unwrap();
        assert!(!report.contains("no issues found"));
        assert!(report.contains("\"failed\": 1"));
    }

    #[test]
    fn test_json_test_report_valid_json() {
        let reporter = JsonReporter::new();
        let result = sample_test_result();
        let report = reporter.generate_test_report(&result).unwrap();

        // The report must be a single valid JSON document.
        let parsed: serde_json::Value = serde_json::from_str(&report)
            .unwrap_or_else(|e| panic!("invalid JSON: {} in {:?}", e, report));
        assert_eq!(parsed["test_summary"]["total"], 2);
        assert_eq!(parsed["failed_tests"].as_array().unwrap().len(), 1);
        assert_eq!(parsed["passed_tests"].as_array().unwrap().len(), 1);
        assert_eq!(parsed["metadata"]["total_tests"], 2);
        assert_eq!(parsed["metadata"]["collected_tests"], 2);
    }

    #[test]
    fn test_json_test_report_all_passed_no_compile_issues() {
        use crate::core::types::TestCase;
        let reporter = JsonReporter::new();
        let mut result = TestAnalysisResult::from_compile_result(AnalysisResult::new());
        result.test_summary = Some(TestSummary {
            total: 1,
            passed: 1,
            failed: 0,
            ignored: 0,
            measured: 0,
            filtered: 0,
            execution_time: None,
        });
        result.passed_tests.push(TestCase::new("AppTest::testGreet", TestStatus::Passed));

        let report = reporter.generate_test_report(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(parsed["test_summary"]["passed"], 1);
        assert_eq!(parsed["failed_tests"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["metadata"]["compile_errors"], 0);
    }

    /// Regression test: runner output embeds control characters (surefire
    /// stack traces indent with tabs) which JSON forbids raw in strings.
    #[test]
    fn test_json_escapes_control_characters() {
        use crate::core::types::TestCase;
        let reporter = JsonReporter::new();
        let mut result = TestAnalysisResult::from_compile_result(AnalysisResult::new());
        result.failed_tests.push(
            TestCase::new("AppTest::testFailingCase", TestStatus::Failed)
                .with_failure_details(
                    "org.junit.ComparisonFailure: expected:<[x]> but was:<[]>\n\tat com.example.AppTest.testFailingCase(AppTest.java:21)",
                ),
        );

        let report = reporter.generate_test_report(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&report)
            .unwrap_or_else(|e| panic!("invalid JSON: {} in {:?}", e, report));
        let details = parsed["failed_tests"][0]["failure_details"]
            .as_str()
            .expect("failure_details must be a string");
        assert!(details.contains('\t'), "tab must survive round-trip");
        assert!(details.contains("AppTest.java:21"));
    }
}
