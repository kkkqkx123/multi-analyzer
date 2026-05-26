//! Shared utility functions for text processing and command execution.
//!
//! Extracted from individual parser modules to avoid duplication.
//! Inspired by RTK's utility patterns.

use regex::Regex;
use std::sync::OnceLock;

fn ansi_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap())
}

/// Strip ANSI escape codes from text using a pre-compiled regex.
pub fn strip_ansi(text: &str) -> String {
    ansi_re().replace_all(text, "").to_string()
}

/// Truncate string to `max_len` visible characters, appending "..." if truncated.
pub fn truncate(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len < 3 {
        "...".to_string()
    } else {
        format!("{}...", s.chars().take(max_len - 3).collect::<String>())
    }
}

/// Check if a tool is available in PATH.
pub fn tool_exists(tool: &str) -> bool {
    std::process::Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(tool)
        .output()
        .ok()
        .map_or(false, |o| o.status.success())
}

/// Format a duration in seconds as a human-readable string.
pub fn format_duration(secs: f64) -> String {
    if secs < 1.0 {
        format!("{:.0}ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.2}s", secs)
    } else {
        let mins = secs / 60.0;
        let rem = secs % 60.0;
        format!("{:.0}m {:.0}s", mins, rem)
    }
}

/// Count the number of lines in a string.
pub fn count_lines(s: &str) -> usize {
    if s.is_empty() { 0 } else { s.lines().count() }
}

/// Get a summary of output: first N lines + "... (+M more)"
pub fn summarize_output(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        output.to_string()
    } else {
        let head: Vec<&str> = lines[..max_lines].to_vec();
        format!("{}\n... (+{} more lines)", head.join("\n"), lines.len() - max_lines)
    }
}

/// Parse a key=value pair from a string, trimming whitespace.
pub fn parse_key_value(s: &str, delimiter: char) -> Option<(String, String)> {
    let trimmed = s.trim();
    let pos = trimmed.find(delimiter)?;
    let key = trimmed[..pos].trim().to_string();
    let value = trimmed[pos + 1..].trim().to_string();
    if key.is_empty() { None } else { Some((key, value)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        assert_eq!(strip_ansi("\x1b[31mHello\x1b[0m"), "Hello");
        assert_eq!(strip_ansi("No ansi"), "No ansi");
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello world", 5), "he...");
        assert_eq!(truncate("hi", 2), "hi");
        assert_eq!(truncate("abcde", 3), "...");
    }

    #[test]
    fn test_count_lines() {
        assert_eq!(count_lines(""), 0);
        assert_eq!(count_lines("one"), 1);
        assert_eq!(count_lines("one\ntwo\nthree"), 3);
    }

    #[test]
    fn test_summarize_output() {
        assert_eq!(summarize_output("a\nb\nc", 5), "a\nb\nc");
        let result = summarize_output("1\n2\n3\n4\n5", 2);
        assert!(result.contains("+3 more"));
    }

    #[test]
    fn test_parse_key_value() {
        assert_eq!(parse_key_value("key = value", '='), Some(("key".into(), "value".into())));
        assert_eq!(parse_key_value("key:value", ':'), Some(("key".into(), "value".into())));
        assert_eq!(parse_key_value("", '='), None);
    }

    #[test]
    fn test_format_duration() {
        assert!(format_duration(0.5).contains("ms"));
        assert!(format_duration(30.0).contains("s"));
        assert!(format_duration(125.0).contains("m"));
    }
}