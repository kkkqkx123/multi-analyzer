//! CMake Output Parser
//! Parses CMake configuration and build output

use regex::Regex;
use crate::core::{Issue, IssueLevel, Location, OutputParser, ParseResult};
use crate::plugins::cpp::parser::{CppParser, CompilerType};

pub struct CMakeParser {
    cmake_error_regex: Regex,
    cmake_warning_regex: Regex,
}

impl CMakeParser {
    pub fn new() -> Self {
        let cmake_error_regex = Regex::new(
            r"CMake Error at\s+(.*?):(\d+)\s*\((.*?)\):\s*(.*)"
        ).unwrap();

        let cmake_warning_regex = Regex::new(
            r"CMake Warning at\s+(.*?):(\d+)\s*\((.*?)\):\s*(.*)"
        ).unwrap();

        Self {
            cmake_error_regex,
            cmake_warning_regex,
        }
    }

    fn parse_cmake_errors(&self, output: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // Parse CMake errors
            if let Some(caps) = self.cmake_error_regex.captures(line) {
                let file_path = caps[1].to_string();
                let line_num = caps[2].parse::<u32>().ok();
                let command = &caps[3];
                // Capture any message on the same line after ":"
                let same_line_msg = caps[4].trim();

                // Collect multi-line message (indented lines following the error)
                let mut message_parts = Vec::new();
                if !same_line_msg.is_empty() {
                    message_parts.push(same_line_msg.to_string());
                }

                i += 1;
                while i < lines.len() {
                    let next_line = lines[i];
                    // Check if next line is indented (part of the message)
                    // or empty line within message block
                    if next_line.starts_with("  ") || next_line.is_empty() {
                        let trimmed = next_line.trim();
                        if !trimmed.is_empty() {
                            message_parts.push(trimmed.to_string());
                        }
                        i += 1;
                    } else {
                        break;
                    }
                }

                let message = if message_parts.is_empty() {
                    command.to_string()
                } else {
                    message_parts.join(" ")
                };

                let mut location = Location::new(file_path);
                if let Some(ln) = line_num {
                    location = location.with_line(ln);
                }

                let issue = Issue::new(IssueLevel::Error, message, location)
                    .with_code("CMake Error");
                issues.push(issue);
                continue;
            }

            // Parse CMake warnings
            if let Some(caps) = self.cmake_warning_regex.captures(line) {
                let file_path = caps[1].to_string();
                let line_num = caps[2].parse::<u32>().ok();
                let command = &caps[3];
                let same_line_msg = caps[4].trim();

                let mut message_parts = Vec::new();
                if !same_line_msg.is_empty() {
                    message_parts.push(same_line_msg.to_string());
                }

                i += 1;
                while i < lines.len() {
                    let next_line = lines[i];
                    if next_line.starts_with("  ") || next_line.is_empty() {
                        let trimmed = next_line.trim();
                        if !trimmed.is_empty() {
                            message_parts.push(trimmed.to_string());
                        }
                        i += 1;
                    } else {
                        break;
                    }
                }

                let message = if message_parts.is_empty() {
                    command.to_string()
                } else {
                    message_parts.join(" ")
                };

                let mut location = Location::new(file_path);
                if let Some(ln) = line_num {
                    location = location.with_line(ln);
                }

                let issue = Issue::new(IssueLevel::Warning, message, location)
                    .with_code("CMake Warning");
                issues.push(issue);
                continue;
            }

            i += 1;
        }

        issues
    }

    fn detect_compiler_type(&self, output: &str) -> CompilerType {
        CppParser::detect_compiler_type(output)
    }
}

impl Default for CMakeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for CMakeParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let mut issues = Vec::new();

        // Parse CMake configuration errors
        issues.extend(self.parse_cmake_errors(output));

        // Parse compiler errors from build output
        let compiler_type = self.detect_compiler_type(output);
        let cpp_parser = CppParser::new(compiler_type);
        let cpp_result = <CppParser as OutputParser>::parse(&cpp_parser, output);
        issues.extend(cpp_result.data_or_default_owned());

        ParseResult::Full(issues)
    }
}


