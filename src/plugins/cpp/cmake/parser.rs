//! CMake Output Parser
//! Parses CMake configuration and build output

use crate::core::{BlockCollector, Issue, IssueLevel, Location, OutputParser, ParseResult};
use crate::plugins::cpp::parser::{CompilerType, CppParser};
use regex::Regex;

pub struct CMakeParser {
    cmake_error_regex: Regex,
    cmake_warning_regex: Regex,
}

impl CMakeParser {
    pub fn new() -> Self {
        let cmake_error_regex =
            Regex::new(r"CMake Error at\s+(.*?):(\d+)\s*\((.*?)\):\s*(.*)").unwrap();

        let cmake_warning_regex =
            Regex::new(r"CMake Warning at\s+(.*?):(\d+)\s*\((.*?)\):\s*(.*)").unwrap();

        Self {
            cmake_error_regex,
            cmake_warning_regex,
        }
    }


    fn detect_compiler_type(&self, output: &str) -> CompilerType {
        CppParser::detect_compiler_type(output)
    }

    fn make_location(file_path: &str, line_num: Option<u32>) -> Location {
        let mut loc = Location::new(file_path.to_string());
        if let Some(ln) = line_num {
            loc = loc.with_line(ln);
        }
        loc
    }
}

impl BlockCollector for CMakeParser {
    fn is_block_start(&self, line: &str) -> bool {
        self.cmake_error_regex.is_match(line) || self.cmake_warning_regex.is_match(line)
    }

    fn is_block_end(&self, line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }
        !line.starts_with("  ")
    }

    fn extract_issues(&self, block: &[String]) -> Vec<Issue> {
        if block.is_empty() {
            return vec![];
        }

        let first = &block[0];
        let is_error = self.cmake_error_regex.is_match(first);

        let caps = if is_error {
            self.cmake_error_regex.captures(first)
        } else {
            self.cmake_warning_regex.captures(first)
        };

        let caps = match caps {
            Some(c) => c,
            None => return vec![],
        };

        let file_path = caps[1].to_string();
        let line_num = caps[2].parse::<u32>().ok();
        let command = &caps[3];
        let same_line_msg = caps[4].trim();

        let mut message_parts: Vec<String> = Vec::new();
        if !same_line_msg.is_empty() {
            message_parts.push(same_line_msg.to_string());
        }

        for continuation in &block[1..] {
            let trimmed = continuation.trim();
            if !trimmed.is_empty() {
                message_parts.push(trimmed.to_string());
            }
        }

        let message = if message_parts.is_empty() {
            command.to_string()
        } else {
            message_parts.join(" ")
        };

        let level = if is_error {
            IssueLevel::Error
        } else {
            IssueLevel::Warning
        };
        let code = if is_error { "CMake Error" } else { "CMake Warning" };

        let location = Self::make_location(&file_path, line_num);

        vec![Issue::new(level, message, location).with_code(code)]
    }
}

impl Default for CMakeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for CMakeParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let mut issues = self.collect_all_blocks(output);

        // Also parse compiler errors from build output
        let compiler_type = self.detect_compiler_type(output);
        let cpp_parser = CppParser::new(compiler_type);
        let cpp_result = <CppParser as OutputParser>::parse(&cpp_parser, output);
        issues.extend(cpp_result.data_or_default_owned());

        ParseResult::Full(issues)
    }
}
