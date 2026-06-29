//! Parser trait definition
//! defines the interface for parsing command output

#![allow(dead_code)]

use super::types::{Issue, IssueLevel, Location};

/// Parse result with degradation tier.
/// Inspired by RTK's three-tier fallback strategy:
/// - Full: successful parse with complete structured data
/// - Degraded: partial parse with warnings (e.g. some lines could not be parsed)
/// - Passthrough: parsing failed entirely, returning truncated raw text
#[derive(Debug)]
pub enum ParseResult<T> {
    Full(T),
    Degraded(T, Vec<String>),
    Passthrough(String),
}

impl<T> ParseResult<T> {
    pub fn is_full(&self) -> bool {
        matches!(self, ParseResult::Full(_))
    }

    pub fn tier(&self) -> u8 {
        match self {
            ParseResult::Full(_) => 1,
            ParseResult::Degraded(_, _) => 2,
            ParseResult::Passthrough(_) => 3,
        }
    }

    /// Extract inner data if available (Full or Degraded), fallback to default.
    pub fn data_or_default(self, default: T) -> T {
        match self {
            ParseResult::Full(data) | ParseResult::Degraded(data, _) => data,
            ParseResult::Passthrough(_) => default,
        }
    }

    /// Extract inner data, returning None for Passthrough.
    pub fn data(self) -> Option<T> {
        match self {
            ParseResult::Full(data) | ParseResult::Degraded(data, _) => Some(data),
            ParseResult::Passthrough(_) => None,
        }
    }

    pub fn map<U, F>(self, f: F) -> ParseResult<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            ParseResult::Full(data) => ParseResult::Full(f(data)),
            ParseResult::Degraded(data, warnings) => ParseResult::Degraded(f(data), warnings),
            ParseResult::Passthrough(raw) => ParseResult::Passthrough(raw),
        }
    }

    /// Get degradation warnings (empty for Full and Passthrough).
    pub fn warnings(&self) -> &[String] {
        match self {
            ParseResult::Degraded(_, warnings) => warnings,
            _ => &[],
        }
    }
}

impl<T: Default> ParseResult<T> {
    /// Extract inner data with default if Passthrough.
    pub fn data_or_default_owned(self) -> T {
        self.data_or_default(T::default())
    }
}

/// Output parser trait
/// Implement this trait to support the new technology stack output format
///
/// This trait uses the template method pattern - it provides a default
/// implementation of `parse()` that calls the abstract methods
/// `is_issue_start()` and `parse_issue()`. Implementors can override
/// `parse()` for completely custom behavior, or just implement the
/// abstract methods for standard streaming parsing.
pub trait OutputParser: Send + Sync {
    /// Parses command output to extract all problem information
    ///
    /// Default implementation uses streaming parsing via `is_issue_start`
    /// and `parse_issue`. Override this method for custom parsing logic.
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        let lines: Vec<String> = output.lines().map(String::from).collect();
        let mut issues = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            if self.is_issue_start(&lines[i]) {
                let (issue, consumed) = self.parse_issue(&lines, i);
                if let Some(issue) = issue {
                    issues.push(issue);
                }
                i += consumed;
            } else {
                // Hook method: allow subclasses to handle single lines
                if let Some(issue) = self.parse_single_line(&lines[i]) {
                    issues.push(issue);
                }
                i += 1;
            }
        }

        ParseResult::Full(issues)
    }

    /// Check if a row is the starting row of the problem
    ///
    /// Required for the default `parse()` implementation.
    /// Return false if using a custom `parse()` implementation.
    fn is_issue_start(&self, _line: &str) -> bool {
        false
    }

    /// Parsing one-line question information
    /// Returns the parsed Issue and the number of lines of text consumed.
    ///
    /// Required for the default `parse()` implementation.
    fn parse_issue(&self, _lines: &[String], _start_index: usize) -> (Option<Issue>, usize) {
        (None, 1)
    }

    /// Hook method for parsing a single line
    ///
    /// Called by the default `parse()` implementation for lines that
    /// are not identified as issue starts. Override to add single-line
    /// parsing logic. Default implementation returns None.
    fn parse_single_line(&self, _line: &str) -> Option<Issue> {
        None
    }
}

/// Base parser implementation providing generic helper methods
pub struct BaseParser;

impl BaseParser {
    pub fn new() -> Self {
        Self
    }

    /// Detection problem level
    pub fn detect_level(&self, text: &str) -> Option<IssueLevel> {
        let lower = text.to_lowercase();
        if lower.contains("error") {
            Some(IssueLevel::Error)
        } else if lower.contains("warning") || lower.contains("warn") {
            Some(IssueLevel::Warning)
        } else if lower.contains("info") {
            Some(IssueLevel::Info)
        } else if lower.contains("hint") {
            Some(IssueLevel::Hint)
        } else if lower.contains("note") {
            Some(IssueLevel::Info)
        } else {
            None
        }
    }

    /// Extract the error code (e.g. E0308 or TS1234)
    pub fn extract_error_code(&self, text: &str) -> Option<String> {
        if let Some(start) = text.find('[') {
            if let Some(end) = text.find(']') {
                if start < end {
                    let code = &text[start..=end];
                    if code.starts_with('[') && code.ends_with(']') {
                        let inner = &code[1..code.len() - 1];
                        if inner.chars().all(|c| c.is_ascii_alphanumeric()) {
                            return Some(code.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// Parsing standard format: file:line:col: level: message
    pub fn parse_standard_format(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let parts: Vec<&str> = trimmed.splitn(5, ':').collect();
        if parts.len() < 4 {
            return None;
        }

        let file_path = parts[0].trim();
        let line_num = parts[1].trim().parse::<u32>().ok()?;

        let (col_num, level_str, message) = if parts.len() >= 5 {
            let col = parts[2].trim().parse::<u32>().ok()?;
            (Some(col), parts[3].trim(), parts[4].trim())
        } else {
            (None, parts[2].trim(), parts[3].trim())
        };

        let level = self.detect_level(level_str)?;

        let location = if let Some(col) = col_num {
            Location::new(file_path.to_string())
                .with_line(line_num)
                .with_column(col)
        } else {
            Location::new(file_path.to_string()).with_line(line_num)
        };

        let mut issue = Issue::new(level, message.to_string(), location);

        if let Some(code) = self.extract_error_code(message) {
            issue = issue.with_code(code);
        }

        Some(issue)
    }

    /// 解析带括号的格式：file(line,col): level: message
    pub fn parse_parentheses_format(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();

        if let Some(open_paren) = trimmed.find('(') {
            if let Some(close_paren) = trimmed.find(')') {
                // Ensure close_paren is after open_paren
                if close_paren <= open_paren {
                    return None;
                }
                // Ensure we don't go out of bounds
                if close_paren + 1 >= trimmed.len() {
                    return None;
                }
                let file_path = &trimmed[..open_paren].trim();
                let location_part = &trimmed[open_paren + 1..close_paren];
                let after_paren = &trimmed[close_paren + 1..].trim();

                let loc_parts: Vec<&str> = location_part.split(',').collect();
                if loc_parts.len() == 2 {
                    let line_num = loc_parts[0].trim().parse::<u32>().ok()?;
                    let col_num = loc_parts[1].trim().parse::<u32>().ok()?;

                    if after_paren.starts_with(':') {
                        let rest = after_paren
                            .strip_prefix(':')
                            .map(|s| s.trim())
                            .unwrap_or(after_paren);
                        let level = self.detect_level(rest)?;

                        let (code, message) = if let Some(colon_pos) = rest.find(':') {
                            // Ensure we don't go out of bounds
                            if colon_pos + 1 >= rest.len() {
                                return None;
                            }
                            let before_colon = rest[..colon_pos].trim();
                            let msg_part = rest[colon_pos + 1..].trim();

                            let parts: Vec<&str> = before_colon.split_whitespace().collect();
                            let code_part = parts.last().unwrap_or(&before_colon);

                            let formatted_code =
                                if code_part.starts_with('[') && code_part.ends_with(']') {
                                    Some(code_part.to_string())
                                } else if code_part.chars().all(|c| c.is_alphanumeric())
                                    && code_part.len() > 1
                                {
                                    Some(format!("[{}]", code_part))
                                } else {
                                    None
                                };

                            (formatted_code, msg_part.to_string())
                        } else {
                            (None, rest.to_string())
                        };

                        let location = Location::new(file_path.to_string())
                            .with_line(line_num)
                            .with_column(col_num);

                        let mut issue = Issue::new(level, message, location);

                        if let Some(c) = code {
                            issue = issue.with_code(c);
                        }

                        return Some(issue);
                    }
                }
            }
        }

        None
    }

    /// Extracting messages from text (removing suffixes such as rule names)
    pub fn extract_message(&self, text: &str) -> String {
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() > 1 {
            if let Some(last) = parts.last() {
                if last.contains('/') || last.contains('-') {
                    return parts[..parts.len() - 1].join(" ");
                }
            }
        }
        text.to_string()
    }
}

impl Default for BaseParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Block collector for multi-line error blocks
///
/// Some build tools (cargo, gcc, clang, MSBuild) output errors as
/// multi-line blocks that require full context for proper extraction.
/// This trait provides a lightweight block accumulation pattern
/// that parsers can optionally implement alongside `OutputParser`.
///
/// # Example — Cargo error block
///
/// ```ignore
/// error[E0308]: mismatched types
///   --> src/main.rs:10:5
///    |
/// 10 |     let x: String = 42;
///    |     ^^^^^^^^^^^^^^^^^^^^ expected `String`, found integer
/// ```
///
/// # Usage
///
/// A parser that implements `BlockCollector` can use the
/// [`collect_blocks`] helper to iterate blocks, or call
/// [`collect_all_blocks`] to process the full output.
///
/// [`collect_blocks`]: BlockCollector::collect_blocks
/// [`collect_all_blocks`]: BlockCollector::collect_all_blocks
pub trait BlockCollector: Send + Sync {
    /// Whether the given line marks the start of a new block.
    fn is_block_start(&self, line: &str) -> bool;

    /// Whether the given line marks the end of the current block.
    /// Called for each line after the block start.
    /// Default: empty lines terminate blocks.
    fn is_block_end(&self, line: &str) -> bool {
        line.trim().is_empty()
    }

    /// Extract issues from a fully collected block.
    fn extract_issues(&self, block: &[String]) -> Vec<Issue>;

    /// Iterate over lines, yielding each accumulated block.
    ///
    /// Lines before the first block start are ignored.
    fn collect_blocks<'a>(&'a self, lines: &'a [String]) -> BlockIter<'a, Self>
    where
        Self: Sized,
    {
        BlockIter {
            collector: self,
            lines,
            index: 0,
            in_block: false,
        }
    }

    /// Collect all blocks from the output and extract issues.
    fn collect_all_blocks(&self, output: &str) -> Vec<Issue> {
        let lines: Vec<String> = output.lines().map(String::from).collect();
        let mut issues = Vec::new();
        let mut block: Vec<String> = Vec::new();
        let mut in_block = false;

        for line in &lines {
            if !in_block {
                if self.is_block_start(line) {
                    in_block = true;
                    block.clear();
                    block.push(line.clone());
                }
            } else if self.is_block_end(line) {
                issues.extend(self.extract_issues(&block));
                in_block = false;
                block.clear();
            } else {
                block.push(line.clone());
            }
        }

        // Flush remaining block
        if in_block && !block.is_empty() {
            issues.extend(self.extract_issues(&block));
        }

        issues
    }
}

/// Iterator that yields accumulated blocks from lines.
pub struct BlockIter<'a, C: ?Sized> {
    collector: &'a C,
    lines: &'a [String],
    index: usize,
    in_block: bool,
}

impl<'a, C: BlockCollector + ?Sized> Iterator for BlockIter<'a, C> {
    type Item = Vec<String>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.lines.len() {
            let line = &self.lines[self.index];

            if !self.in_block {
                if self.collector.is_block_start(line) {
                    self.in_block = true;
                    let mut block = Vec::new();
                    block.push(line.clone());
                    self.index += 1;

                    // Accumulate remaining lines until block end
                    while self.index < self.lines.len() {
                        let next_line = &self.lines[self.index];
                        if self.collector.is_block_end(next_line) {
                            self.in_block = false;
                            return Some(block);
                        }
                        block.push(next_line.clone());
                        self.index += 1;
                    }

                    // End of input while in block
                    self.in_block = false;
                    return Some(block);
                }
            } else {
                // Should not reach here due to above logic
                self.in_block = false;
            }

            self.index += 1;
        }

        None
    }
}

impl<C: BlockCollector + ?Sized> BlockIter<'_, C> {
    /// Collect all remaining blocks into a Vec.
    pub fn collect_remaining(&mut self) -> Vec<Vec<String>> {
        let mut blocks = Vec::new();
        for block in self.by_ref() {
            blocks.push(block);
        }
        blocks
    }
}

#[cfg(test)]
mod block_collector_tests {
    use super::*;

    /// A mock BlockCollector that treats lines starting with "error:" as block starts
    /// and empty lines as block ends. Extracts the first line as the issue message.
    struct MockErrorCollector;

    impl BlockCollector for MockErrorCollector {
        fn is_block_start(&self, line: &str) -> bool {
            let trimmed = line.trim();
            trimmed.starts_with("error:")
                || trimmed.starts_with("error[")
                || trimmed.starts_with("warning:")
        }

        fn is_block_end(&self, line: &str) -> bool {
            line.trim().is_empty() || line.trim() == "---"
        }

        fn extract_issues(&self, block: &[String]) -> Vec<Issue> {
            if block.is_empty() {
                return vec![];
            }

            let first = block[0].trim();
            let level = if first.starts_with("error:") || first.starts_with("error[") {
                IssueLevel::Error
            } else {
                IssueLevel::Warning
            };

            let message = if let Some(colon) = first.find(':') {
                first[colon + 1..].trim().to_string()
            } else {
                first.to_string()
            };

            // Try to extract file:line from block
            // Format: "  --> file.rs:line:col"  or  "  --> file.rs:line"
            let mut location = Location::new("unknown");
            for line in block {
                let trimmed = line.trim();
                if let Some(arrow) = trimmed.find("-->") {
                    let path_part = arrow + 3;
                    let path_trimmed = trimmed[path_part..].trim();

                    // Split on ':' — format is "file.rs:line:col" or "file.rs:line"
                    // Use rsplitn to find last colon, then second-to-last for line number
                    let parts: Vec<&str> = path_trimmed.splitn(3, ':').collect();
                    if parts.len() >= 2 {
                        let file_path = parts[0].trim();
                        if let Ok(line_num) = parts[1].trim().parse::<u32>() {
                            location = Location::new(file_path.to_string()).with_line(line_num);
                        }
                    }
                }
            }

            vec![Issue::new(level, message, location)]
        }
    }

    #[test]
    fn test_collect_single_block() {
        let collector = MockErrorCollector;
        let output = "\
error[E0308]: mismatched types
  --> src/main.rs:10:5
   |
10 |     let x: String = 42;
   |     ^^^^^^^^^^^^^^^^^^^^ expected `String`, found integer
";
        let issues = collector.collect_all_blocks(output);
        assert_eq!(issues.len(), 1, "Expected 1 issue, got {}", issues.len());

        assert_eq!(
            issues[0].location.file_path, "src/main.rs",
            "Expected file_path = src/main.rs, got {}",
            issues[0].location.file_path
        );
        assert_eq!(
            issues[0].message, "mismatched types",
            "Expected message = mismatched types, got {}",
            issues[0].message
        );
        assert_eq!(
            issues[0].location.line_number,
            Some(10),
            "Expected line_number = 10, got {:?}",
            issues[0].location.line_number
        );
        assert!(
            matches!(issues[0].level, IssueLevel::Error),
            "Expected Error level, got {:?}",
            issues[0].level
        );
    }

    #[test]
    fn test_collect_multiple_blocks() {
        let collector = MockErrorCollector;
        let output = "\
error: first error
  --> src/a.rs:1:1

warning: second issue
  --> src/b.rs:5:3
";
        let issues = collector.collect_all_blocks(output);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].message, "first error");
        assert_eq!(issues[1].message, "second issue");
    }

    #[test]
    fn test_block_iterator() {
        let collector = MockErrorCollector;
        let lines: Vec<String> = vec![
            "error: first".to_string(),
            "  --> a.rs:1".to_string(),
            "".to_string(),
            "warning: second".to_string(),
            "  --> b.rs:5".to_string(),
            "".to_string(),
        ];

        let blocks: Vec<_> = collector.collect_blocks(&lines).collect();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].len(), 2);
        assert_eq!(blocks[1].len(), 2);
    }

    #[test]
    fn test_collector_ignores_before_first_block() {
        let collector = MockErrorCollector;
        let lines: Vec<String> = vec![
            "some header line".to_string(),
            "another header".to_string(),
            "".to_string(),
            "error: real issue".to_string(),
            "  --> file.rs:1".to_string(),
        ];

        let blocks: Vec<_> = collector.collect_blocks(&lines).collect();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0][0], "error: real issue");
    }

    #[test]
    fn test_custom_block_end() {
        struct DashEndCollector;
        impl BlockCollector for DashEndCollector {
            fn is_block_start(&self, line: &str) -> bool {
                line.starts_with("error:")
            }
            fn is_block_end(&self, line: &str) -> bool {
                line.trim() == "---"
            }
            fn extract_issues(&self, block: &[String]) -> Vec<Issue> {
                let msg = block
                    .first()
                    .map(|l| l.trim().to_string())
                    .unwrap_or_default();
                vec![Issue::new(IssueLevel::Error, msg, Location::new("unknown"))]
            }
        }

        let collector = DashEndCollector;
        let output = "\
error: block one
line in block one
---
error: block two
line in block two
---
";
        let issues = collector.collect_all_blocks(output);
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn test_collect_remaining() {
        let collector = MockErrorCollector;
        let lines: Vec<String> = vec![
            "error: first".to_string(),
            "  --> a.rs:1".to_string(),
            "".to_string(),
            "error: second".to_string(),
            "  --> b.rs:2".to_string(),
        ];

        let mut iter = collector.collect_blocks(&lines);
        // Skip first block
        let first = iter.next();
        assert!(first.is_some());

        let remaining = iter.collect_remaining();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0][0], "error: second");
    }
}
