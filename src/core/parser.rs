//! Parser trait definition
//! defines the interface for parsing command output

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
            Location::new(file_path.to_string())
                .with_line(line_num)
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
                        let rest = after_paren.strip_prefix(':').map(|s| s.trim()).unwrap_or(after_paren);
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
                            
                            let formatted_code = if code_part.starts_with('[') && code_part.ends_with(']') {
                                Some(code_part.to_string())
                            } else if code_part.chars().all(|c| c.is_alphanumeric()) && code_part.len() > 1 {
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
