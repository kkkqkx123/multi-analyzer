//! C++ Output Parser
//! Shared parser for GCC, Clang, and MSVC compiler outputs

use crate::core::{Issue, IssueLevel, Location, OutputParser, ParseResult};
use regex::Regex;
use std::sync::OnceLock;

fn gcc_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(
        r"^(.*?):(\d+):(\d+):\s*(error|warning|note):\s*(.*?)(?:\s*\[(.*?)\])?$"
    ).unwrap())
}

fn msvc_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(
        r"^(.*?)\((\d+)\s*(?:,\s*(\d+))?\)\s*:\s*(error|warning|fatal error)\s+(\w+)?\s*:\s*(.*)$"
    ).unwrap())
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
                let col_num = caps.get(3)
                    .and_then(|m| m.as_str().parse::<u32>().ok());
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
        } else if output.contains("Microsoft") || output.contains("cl.exe") || output.contains("Microsoft (R) C/C++") {
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
            CompilerType::Msvc => {
                ParseResult::Full(self.parse_msvc_style(output))
            }
        }
    }
}

impl Default for CppParser {
    fn default() -> Self {
        Self::new(CompilerType::Gcc)
    }
}
