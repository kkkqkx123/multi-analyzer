//! Clang-Tidy Output Parser
//! Parses clang-tidy output in JSON (preferred) or GCC-style text format,
//! filtering out code-context lines that clang-tidy prints with `^` markers.

use crate::core::{Issue, IssueLevel, Location, OutputParser, ParseResult};
use regex::Regex;
use std::sync::OnceLock;

/// clang-tidy text format: file:line:col: level: message [check-name1,check-name2,...]
fn clang_tidy_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^(.*?):(\d+):(\d+):\s*(error|warning|note):\s*(.*?)(?:\s*\[([^\]]+)\])?\s*$",
        )
        .unwrap()
    })
}

pub struct ClangTidyParser;

impl ClangTidyParser {
    pub fn new() -> Self {
        Self
    }

    /// Returns true when the line is a clang-tidy code-context display line
    /// that should NOT be parsed as an Issue. Context lines look like:
    ///
    ///     10 |     int foo = 42;
    ///        |         ^~~~~~~~~~~
    fn is_context_line(line: &str) -> bool {
        let trimmed = line.trim();
        // Empty lines are not context lines (they separate blocks).
        if trimmed.is_empty() {
            return false;
        }
        // clang-tidy's "context bar" lines: a bare "|" (vertical bar column
        // indicator) or caret blocks like "   ^~~". They never contain
        // "file:line:col:" style prefixes.
        let has_indicator = trimmed.starts_with('|')
            || trimmed.contains('|') && !trimmed.contains(':')
            || trimmed.contains('^');
        if has_indicator {
            // Avoid triggering on legitimate file paths that happen to contain
            // "|" or "^": issue lines always start with "<file>:<line>:<col>:".
            !trimmed.splitn(4, ':').last().map_or(false, |rest| {
                rest.starts_with(" error:")
                    || rest.starts_with(" warning:")
                    || rest.starts_with(" note:")
            })
        } else {
            false
        }
    }

    /// Detect whether output is JSON (clang-tidy --format=json).
    fn is_json_output(output: &str) -> bool {
        let trimmed = output.trim_start();
        trimmed.starts_with('{') || trimmed.starts_with('[')
    }

    /// Map clang-tidy severity string to IssueLevel.
    fn map_severity(severity: &str) -> IssueLevel {
        match severity.to_lowercase().as_str() {
            "error" => IssueLevel::Error,
            "warning" => IssueLevel::Warning,
            "note" | "info" => IssueLevel::Info,
            _ => IssueLevel::Hint,
        }
    }

    /// Parse JSON output (--format=json). Returns None if the JSON is invalid.
    fn parse_json(&self, output: &str) -> Option<Vec<Issue>> {
        #[derive(serde::Deserialize)]
        struct DiagLocation {
            #[serde(rename = "File")]
            file: String,
            #[serde(rename = "Line")]
            line: u32,
            #[serde(rename = "Column")]
            column: u32,
        }

        #[derive(serde::Deserialize)]
        struct Diag {
            #[serde(rename = "DiagnosticName")]
            diagnostic_name: String,
            #[serde(rename = "Message")]
            message: String,
            #[serde(rename = "Location")]
            location: DiagLocation,
            #[serde(rename = "Severity")]
            severity: String,
        }

        #[derive(serde::Deserialize)]
        struct TidyOutput {
            #[serde(rename = "Diagnostics")]
            diagnostics: Vec<Diag>,
        }

        let tidy: TidyOutput = serde_json::from_str(output).ok()?;
        let mut issues = Vec::with_capacity(tidy.diagnostics.len());

        for diag in tidy.diagnostics {
            let location = Location::new(diag.location.file)
                .with_line(diag.location.line)
                .with_column(diag.location.column);

            issues.push(
                Issue::new(Self::map_severity(&diag.severity), diag.message, location)
                    .with_code(diag.diagnostic_name),
            );
        }

        Some(issues)
    }

    /// Parse GCC-style text output, skipping context lines.
    fn parse_text(&self, output: &str) -> Vec<Issue> {
        let re = clang_tidy_line_regex();
        let mut issues = Vec::new();

        for line in output.lines() {
            if Self::is_context_line(line) {
                continue;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(caps) = re.captures(trimmed) {
                let file_path = caps[1].to_string();
                let line_num = caps[2].parse::<u32>().ok();
                let col_num = caps[3].parse::<u32>().ok();
                let severity = &caps[4];
                let message = caps[5].to_string();
                let code = caps.get(6).map(|m| m.as_str().to_string());

                let mut location = Location::new(file_path);
                if let Some(ln) = line_num {
                    location = location.with_line(ln);
                }
                if let Some(cn) = col_num {
                    location = location.with_column(cn);
                }

                let mut issue = Issue::new(Self::map_severity(severity), message, location);
                if let Some(c) = code {
                    issue = issue.with_code(c);
                }
                issues.push(issue);
            }
        }
        issues
    }
}

impl Default for ClangTidyParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for ClangTidyParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        if Self::is_json_output(output) {
            if let Some(issues) = self.parse_json(output) {
                return ParseResult::Full(issues);
            }
            // JSON parse failure: fall through to text path.
        }
        ParseResult::Full(self.parse_text(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_context_line ────────────────────────────────────────────

    #[test]
    fn test_context_line_bar() {
        assert!(ClangTidyParser::is_context_line(
            "   10 |     int foo = 42;"
        ));
        assert!(ClangTidyParser::is_context_line("      |"));
        assert!(ClangTidyParser::is_context_line("      |         ^~~~~~~~~~~"));
    }

    #[test]
    fn test_context_line_caret_only() {
        assert!(ClangTidyParser::is_context_line("         ^~~~~~~~~~~"));
        assert!(ClangTidyParser::is_context_line("              ^"));
    }

    #[test]
    fn test_context_line_empty() {
        assert!(!ClangTidyParser::is_context_line(""));
    }

    #[test]
    fn test_context_line_not_issue() {
        // A normal file:line:col: error: line must NOT be treated as context.
        assert!(!ClangTidyParser::is_context_line(
            "a.cpp:10:5: warning: unused variable [bugprone-unused]"
        ));
        assert!(!ClangTidyParser::is_context_line(
            "/abs/path/to/file.cpp:3:1: error: null deref [bugprone-nullpointeraccess]"
        ));
    }

    // ── JSON parse ─────────────────────────────────────────────────

    #[test]
    fn test_json_single_diagnostic() {
        let p = ClangTidyParser::new();
        let out = r#"{
            "Diagnostics": [
                {
                    "DiagnosticName": "cppcoreguidelines-avoid-magic-numbers",
                    "Message": "unused variable 'foo'",
                    "Location": {"File": "main.cpp", "Line": 10, "Column": 5},
                    "Severity": "warning"
                }
            ]
        }"#;
        let issues = p.parse_json(out).unwrap();
        assert_eq!(issues.len(), 1);
        let i = &issues[0];
        assert_eq!(i.code.as_deref(), Some("cppcoreguidelines-avoid-magic-numbers"));
        assert_eq!(i.message, "unused variable 'foo'");
        assert_eq!(i.location.file_path, "main.cpp");
        assert_eq!(i.location.line_number, Some(10));
        assert_eq!(i.location.column_number, Some(5));
        assert!(matches!(i.level, IssueLevel::Warning));
    }

    #[test]
    fn test_json_multiple_diagnostics() {
        let p = ClangTidyParser::new();
        let out = r#"{
            "Diagnostics": [
                {"DiagnosticName": "a","Message": "m1","Location":{"File":"x","Line":1,"Column":1},"Severity":"error"},
                {"DiagnosticName": "b","Message": "m2","Location":{"File":"y","Line":2,"Column":2},"Severity":"warning"},
                {"DiagnosticName": "c","Message": "m3","Location":{"File":"z","Line":3,"Column":3},"Severity":"note"}
            ]
        }"#;
        let issues = p.parse_json(out).unwrap();
        assert_eq!(issues.len(), 3);
        assert!(matches!(issues[0].level, IssueLevel::Error));
        assert!(matches!(issues[1].level, IssueLevel::Warning));
        assert!(matches!(issues[2].level, IssueLevel::Info));
    }

    #[test]
    fn test_json_empty_array() {
        let p = ClangTidyParser::new();
        let issues = p.parse_json(r#"{"Diagnostics":[]}"#).unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_json_severity_case_insensitive() {
        let p = ClangTidyParser::new();
        let out = r#"{
            "Diagnostics": [
                {"DiagnosticName":"a","Message":"m","Location":{"File":"f","Line":1,"Column":1},"Severity":"Warning"}
            ]
        }"#;
        let issues = p.parse_json(out).unwrap();
        assert!(matches!(issues[0].level, IssueLevel::Warning));
    }

    #[test]
    fn test_json_invalid_falls_through() {
        let p = ClangTidyParser::new();
        let out = "{not valid json";
        assert!(p.parse_json(out).is_none());
        // And parse() falls through to text, which returns no issues.
        let result = p.parse(out);
        assert!(result.is_full());
        assert!(result.data().unwrap().is_empty());
    }

    #[test]
    fn test_json_file_path_with_spaces() {
        let p = ClangTidyParser::new();
        let out = r#"{
            "Diagnostics": [
                {"DiagnosticName":"a","Message":"m","Location":{"File":"/path/with spaces/file.cpp","Line":1,"Column":1},"Severity":"warning"}
            ]
        }"#;
        let issues = p.parse_json(out).unwrap();
        assert_eq!(issues[0].location.file_path, "/path/with spaces/file.cpp");
    }

    // ── Text parse ─────────────────────────────────────────────────

    #[test]
    fn test_text_single_warning() {
        let p = ClangTidyParser::new();
        let out = "main.cpp:10:5: warning: unused variable 'foo' [bugprone-unused]";
        let issues = p.parse_text(out);
        assert_eq!(issues.len(), 1);
        assert!(matches!(issues[0].level, IssueLevel::Warning));
        assert_eq!(issues[0].code.as_deref(), Some("bugprone-unused"));
        assert_eq!(issues[0].location.file_path, "main.cpp");
        assert_eq!(issues[0].location.line_number, Some(10));
        assert_eq!(issues[0].location.column_number, Some(5));
    }

    #[test]
    fn test_text_error() {
        let p = ClangTidyParser::new();
        let out = "a.cpp:3:1: error: null pointer dereference [bugprone-nullpointeraccess]";
        let issues = p.parse_text(out);
        assert_eq!(issues.len(), 1);
        assert!(matches!(issues[0].level, IssueLevel::Error));
    }

    #[test]
    fn test_text_note_level() {
        let p = ClangTidyParser::new();
        let out = "main.cpp:10:5: note: expanded from macro 'FOO'";
        let issues = p.parse_text(out);
        assert_eq!(issues.len(), 1);
        assert!(matches!(issues[0].level, IssueLevel::Info));
        assert!(issues[0].code.is_none());
    }

    #[test]
    fn test_text_no_code() {
        let p = ClangTidyParser::new();
        let out = "f.cpp:1:1: warning: generic warning";
        let issues = p.parse_text(out);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].code.is_none());
    }

    #[test]
    fn test_text_multiple_rules_in_code() {
        let p = ClangTidyParser::new();
        let out = "main.cpp:1:1: warning: msg [bugprone-*,readability-*,-warnings-as-errors]";
        let issues = p.parse_text(out);
        assert_eq!(issues.len(), 1);
        assert_eq!(
            issues[0].code.as_deref(),
            Some("bugprone-*,readability-*,-warnings-as-errors")
        );
    }

    #[test]
    fn test_text_filters_context_lines() {
        let p = ClangTidyParser::new();
        let out = "\
main.cpp:10:5: warning: unused variable 'foo' [bugprone-unused]
   8 | void bar(int x) {
   9 |     int y = x;
> 10 |     int foo = 42;
     |         ^~~~~~~~~~~
  11 |     std::cout << \"hello\";
  12 | }";
        let issues = p.parse_text(out);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].message, "unused variable 'foo'");
    }

    #[test]
    fn test_text_multiple_issues() {
        let p = ClangTidyParser::new();
        let out = "\
a.cpp:1:1: warning: w1 [c1]
b.cpp:2:2: warning: w2 [c2]
c.cpp:3:3: error: e1 [c3]";
        let issues = p.parse_text(out);
        assert_eq!(issues.len(), 3);
    }

    // ── OutputParser trait ──────────────────────────────────────────

    #[test]
    fn test_parse_prefers_json() {
        let p = ClangTidyParser::new();
        let out = r#"{
            "Diagnostics": [
                {"DiagnosticName":"x","Message":"m","Location":{"File":"f","Line":1,"Column":1},"Severity":"warning"}
            ]
        }"#;
        let result = p.parse(out);
        assert!(result.is_full());
        assert_eq!(result.data().unwrap().len(), 1);
    }

    #[test]
    fn test_parse_falls_back_to_text_on_bad_json() {
        let p = ClangTidyParser::new();
        let out = "{\nnot json\n\na.cpp:1:1: warning: w [c]";
        let result = p.parse(out);
        assert!(result.is_full());
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code.as_deref(), Some("c"));
    }

    #[test]
    fn test_parse_empty_output() {
        let p = ClangTidyParser::new();
        let result = p.parse("");
        assert!(result.is_full());
        assert!(result.data().unwrap().is_empty());
    }
}
