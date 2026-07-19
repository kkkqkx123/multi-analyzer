//! C++ Output Parser
//! Shared parser for GCC, Clang, and MSVC compiler outputs

use crate::core::{Issue, IssueLevel, Location, OutputParser, ParseResult};
use regex::Regex;
use std::sync::OnceLock;

fn gcc_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(.*?):(\d+):(\d+):\s*(error|warning|note):\s*(.*?)(?:\s*\[(.*?)\])?$")
            .unwrap()
    })
}

fn msvc_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
        r"^(.*?)\((\d+)\s*(?:,\s*(\d+))?\)\s*:\s*(error|warning|fatal error)\s+(\w+)?\s*:\s*(.*)$"
    ).unwrap()
    })
}

/// Compiler type for C++ parsers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerType {
    Gcc,
    Clang,
    Msvc,
}

/// C++ Parser that handles GCC, Clang, and MSVC output formats
pub struct CppParser {
    compiler_type: CompilerType,
}

impl CppParser {
    pub fn new(compiler_type: CompilerType) -> Self {
        Self { compiler_type }
    }

    pub fn with_gcc() -> Self {
        Self::new(CompilerType::Gcc)
    }

    pub fn with_clang() -> Self {
        Self::new(CompilerType::Clang)
    }

    pub fn with_msvc() -> Self {
        Self::new(CompilerType::Msvc)
    }

    fn parse_gcc_style(&self, output: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let re = gcc_regex();

        for line in output.lines() {
            if let Some(caps) = re.captures(line) {
                let file_path = caps[1].to_string();
                let line_num = caps[2].parse::<u32>().ok();
                let col_num = caps[3].parse::<u32>().ok();
                let severity = &caps[4];
                let message = caps[5].to_string();
                let code = caps.get(6).map(|m| m.as_str().to_string());

                let level = match severity {
                    "error" => IssueLevel::Error,
                    "warning" => IssueLevel::Warning,
                    "note" => IssueLevel::Info,
                    _ => IssueLevel::Hint,
                };

                let mut location = Location::new(file_path);
                if let Some(ln) = line_num {
                    location = location.with_line(ln);
                }
                if let Some(cn) = col_num {
                    location = location.with_column(cn);
                }

                let mut issue = Issue::new(level, message, location);
                if let Some(c) = code {
                    issue = issue.with_code(c);
                }

                issues.push(issue);
            }
        }

        issues
    }

    fn parse_msvc_style(&self, output: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let re = msvc_regex();

        for line in output.lines() {
            if let Some(caps) = re.captures(line) {
                let file_path = caps[1].to_string();
                let line_num = caps[2].parse::<u32>().ok();
                let col_num = caps.get(3).and_then(|m| m.as_str().parse::<u32>().ok());
                let severity = &caps[4];
                let code = caps.get(5).map(|m| m.as_str().to_string());
                let message = caps[6].to_string();

                let level = match severity {
                    "error" | "fatal error" => IssueLevel::Error,
                    "warning" => IssueLevel::Warning,
                    _ => IssueLevel::Hint,
                };

                let mut location = Location::new(file_path);
                if let Some(ln) = line_num {
                    location = location.with_line(ln);
                }
                if let Some(cn) = col_num {
                    location = location.with_column(cn);
                }

                let mut issue = Issue::new(level, message, location);
                if let Some(c) = code {
                    issue = issue.with_code(c);
                }

                issues.push(issue);
            }
        }

        issues
    }

    /// Detect compiler type from output
    pub fn detect_compiler_type(output: &str) -> CompilerType {
        if output.contains("clang version") || output.contains("clang++") {
            CompilerType::Clang
        } else if output.contains("gcc version") || output.contains("g++") {
            CompilerType::Gcc
        } else if output.contains("Microsoft")
            || output.contains("cl.exe")
            || output.contains("Microsoft (R) C/C++")
        {
            CompilerType::Msvc
        } else {
            CompilerType::Gcc
        }
    }
}

impl OutputParser for CppParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        match self.compiler_type {
            CompilerType::Gcc | CompilerType::Clang => {
                ParseResult::Full(self.parse_gcc_style(output))
            }
            CompilerType::Msvc => ParseResult::Full(self.parse_msvc_style(output)),
        }
    }
}

impl Default for CppParser {
    fn default() -> Self {
        Self::new(CompilerType::Gcc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── GCC style ───────────────────────────────────────────────────

    #[test]
    fn test_gcc_parse_error() {
        let parser = CppParser::with_gcc();
        let output = "src/main.cpp:10:5: error: 'x' was not declared in this scope [-Wdeclaration]";
        let issues = parser.parse_gcc_style(output);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location.file_path, "src/main.cpp");
        assert_eq!(issues[0].location.line_number, Some(10));
        assert_eq!(issues[0].location.column_number, Some(5));
        assert_eq!(issues[0].level, IssueLevel::Error);
        assert_eq!(issues[0].code, Some("-Wdeclaration".to_string()));
    }

    #[test]
    fn test_gcc_parse_warning() {
        let parser = CppParser::with_gcc();
        let output = "src/main.cpp:42:10: warning: unused parameter [-Wunused-param]";
        let issues = parser.parse_gcc_style(output);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].level, IssueLevel::Warning);
        assert_eq!(issues[0].code, Some("-Wunused-param".to_string()));
    }

    #[test]
    fn test_gcc_parse_note() {
        let parser = CppParser::with_gcc();
        let output = "src/main.cpp:50:3: note: in expansion of macro 'FOO'";
        let issues = parser.parse_gcc_style(output);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].level, IssueLevel::Info);
        assert!(issues[0].code.is_none());
    }

    #[test]
    fn test_gcc_parse_no_match() {
        let parser = CppParser::with_gcc();
        let output = "Some random build output line";
        let issues = parser.parse_gcc_style(output);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_gcc_parse_multiple_issues() {
        let parser = CppParser::with_gcc();
        let output = "\
src/main.cpp:10:5: error: undefined reference to 'foo'
src/main.cpp:20:3: warning: unused variable 'bar' [-Wunused-variable]
src/lib.cpp:1:1: warning: no newline at end of file [-Wnewline-eof]";
        let issues = parser.parse_gcc_style(output);
        assert_eq!(issues.len(), 3);
        assert_eq!(issues[0].level, IssueLevel::Error);
        assert_eq!(issues[1].level, IssueLevel::Warning);
        assert_eq!(issues[2].level, IssueLevel::Warning);
    }

    // ── MSVC style ──────────────────────────────────────────────────

    #[test]
    fn test_msvc_parse_error() {
        let parser = CppParser::with_msvc();
        let output = "src\\main.cpp(10,5): error C2065: 'x' : undeclared identifier";
        let issues = parser.parse_msvc_style(output);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location.file_path, "src\\main.cpp");
        assert_eq!(issues[0].location.line_number, Some(10));
        assert_eq!(issues[0].location.column_number, Some(5));
        assert_eq!(issues[0].level, IssueLevel::Error);
        assert_eq!(issues[0].code, Some("C2065".to_string()));
    }

    #[test]
    fn test_msvc_parse_warning() {
        let parser = CppParser::with_msvc();
        let output = "src\\main.cpp(42): warning C4100: 'x' : unreferenced formal parameter";
        let issues = parser.parse_msvc_style(output);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].level, IssueLevel::Warning);
        assert_eq!(issues[0].location.line_number, Some(42));
        assert!(issues[0].location.column_number.is_none());
    }

    #[test]
    fn test_msvc_parse_fatal_error() {
        let parser = CppParser::with_msvc();
        let output = "src\\main.cpp(1): fatal error C1083: Cannot open include file: 'missing.h'";
        let issues = parser.parse_msvc_style(output);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].level, IssueLevel::Error);
        assert_eq!(issues[0].code, Some("C1083".to_string()));
    }

    #[test]
    fn test_msvc_parse_no_match() {
        let parser = CppParser::with_msvc();
        let output = "Microsoft (R) Build Engine version 16.0";
        let issues = parser.parse_msvc_style(output);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_msvc_parse_multiple_issues() {
        let parser = CppParser::with_msvc();
        let output = "\
src\\main.cpp(10,5): error C2065: 'x' : undeclared
src\\main.cpp(20,1): warning C4100: 'y' : unused";
        let issues = parser.parse_msvc_style(output);
        assert_eq!(issues.len(), 2);
    }

    // ── detect_compiler_type ────────────────────────────────────────

    #[test]
    fn test_detect_compiler_type_clang() {
        assert_eq!(
            CppParser::detect_compiler_type("clang version 15.0.0"),
            CompilerType::Clang
        );
        assert_eq!(
            CppParser::detect_compiler_type("clang++ (LLVM)"),
            CompilerType::Clang
        );
    }

    #[test]
    fn test_detect_compiler_type_gcc() {
        assert_eq!(
            CppParser::detect_compiler_type("gcc version 12.0.0"),
            CompilerType::Gcc
        );
        assert_eq!(
            CppParser::detect_compiler_type("g++ (GCC)"),
            CompilerType::Gcc
        );
    }

    #[test]
    fn test_detect_compiler_type_msvc() {
        assert_eq!(
            CppParser::detect_compiler_type("Microsoft (R) C/C++ Optimizing Compiler"),
            CompilerType::Msvc
        );
        assert_eq!(
            CppParser::detect_compiler_type("cl.exe"),
            CompilerType::Msvc
        );
        assert_eq!(
            CppParser::detect_compiler_type("Microsoft (R) Build Engine"),
            CompilerType::Msvc
        );
    }

    #[test]
    fn test_detect_compiler_type_unknown_defaults_to_gcc() {
        assert_eq!(
            CppParser::detect_compiler_type("some unknown compiler"),
            CompilerType::Gcc
        );
    }

    // ── OutputParser trait ──────────────────────────────────────────

    #[test]
    fn test_parse_gcc_via_trait() {
        let parser = CppParser::with_gcc();
        let output = "src/main.cpp:10:5: error: undefined reference";
        let result = parser.parse(output);
        assert!(result.is_full());
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_parse_clang_via_trait() {
        let parser = CppParser::with_clang();
        let output = "src/main.cpp:10:5: error: use of undeclared identifier";
        let result = parser.parse(output);
        assert!(result.is_full());
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_parse_msvc_via_trait() {
        let parser = CppParser::with_msvc();
        let output = "src\\main.cpp(10,5): error C2065: 'x' : undeclared identifier";
        let result = parser.parse(output);
        assert!(result.is_full());
        let issues = result.data().unwrap();
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_parse_empty_output() {
        let parser = CppParser::with_gcc();
        let result = parser.parse("");
        assert!(result.is_full());
        assert!(result.data().unwrap().is_empty());
    }
}
