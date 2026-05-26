//! Shared utility functions for text processing and command execution.
//!
//! Extracted from individual parser modules to avoid duplication.
//! Inspired by RTK's utility patterns.

use regex::Regex;
use std::path::Path;
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

/// Filter out lines matching any of the given regex patterns (noise line stripping).
/// Returns only lines that do NOT match any pattern.
pub fn filter_noise_lines(output: &str, patterns: &[String]) -> String {
    if patterns.is_empty() {
        return output.to_string();
    }
    let re_list: Vec<Regex> = patterns
        .iter()
        .map(|p| Regex::new(p).expect("Invalid noise pattern regex"))
        .collect();
    output
        .lines()
        .filter(|line| !re_list.iter().any(|re| re.is_match(line.trim())))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Keep only lines matching any of the given regex patterns.
/// Returns only lines that match at least one pattern.
pub fn keep_matching_lines(output: &str, patterns: &[String]) -> String {
    if patterns.is_empty() {
        return output.to_string();
    }
    let re_list: Vec<Regex> = patterns
        .iter()
        .map(|p| Regex::new(p).expect("Invalid keep pattern regex"))
        .collect();
    output
        .lines()
        .filter(|line| re_list.iter().any(|re| re.is_match(line)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Truncation strategy for line count control
pub enum OutputTruncation {
    /// Keep first N lines
    Head(usize),
    /// Keep last N lines
    Tail(usize),
    /// Keep first H + last T lines with a separator
    HeadTail { head: usize, tail: usize },
    /// Keep at most N lines (head-only, same as summarize_output)
    Max(usize),
}

/// Truncate output lines according to the given strategy.
/// For HeadTail mode, returns `[head lines] ... (+skipped more) [tail lines]`.
pub fn truncate_output(output: &str, strategy: OutputTruncation) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let total = lines.len();

    match strategy {
        OutputTruncation::Head(n) | OutputTruncation::Max(n) => {
            if total <= n {
                output.to_string()
            } else {
                let head: Vec<&str> = lines[..n].to_vec();
                format!("{}\n... (+{} more lines)", head.join("\n"), total - n)
            }
        }
        OutputTruncation::Tail(n) => {
            if total <= n {
                output.to_string()
            } else {
                let tail: Vec<&str> = lines[total - n..].to_vec();
                format!("... (+{} more lines)\n{}", total - n, tail.join("\n"))
            }
        }
        OutputTruncation::HeadTail { head, tail } => {
            if total <= head + tail {
                output.to_string()
            } else {
                let head_lines: Vec<&str> = lines[..head].to_vec();
                let tail_lines: Vec<&str> = lines[total - tail..].to_vec();
                format!(
                    "{}\n... (+{} more lines)\n{}",
                    head_lines.join("\n"),
                    total - head - tail,
                    tail_lines.join("\n")
                )
            }
        }
    }
}

/// Smart line truncation: truncate a single line to max_len, preserving context around a keyword.
/// If keyword is found, keeps `context_before` chars before it and fills the rest after.
pub fn smart_truncate_line(line: &str, max_len: usize, keyword: Option<&str>) -> String {
    let char_count = line.chars().count();
    if char_count <= max_len {
        return line.to_string();
    }
    if max_len < 5 {
        return format!("{}...", line.chars().take(max_len.saturating_sub(3)).collect::<String>());
    }
    if let Some(kw) = keyword {
        if let Some(pos) = line.find(kw) {
            let char_pos = line[..pos].chars().count();
            let context_before = max_len / 3;
            let start = char_pos.saturating_sub(context_before);
            let end = (start + max_len).min(char_count);
            let truncated: String = line.chars().skip(start).take(end - start).collect();
            if start > 0 && end < char_count {
                return format!("...{}...", truncated);
            } else if start > 0 {
                return format!("...{}", truncated);
            } else if end < char_count {
                return format!("{}...", truncated);
            }
            return truncated;
        }
    }
    format!(
        "{}...",
        line.chars().take(max_len.saturating_sub(3)).collect::<String>()
    )
}

/// Shorten an absolute path to a compact relative form.
/// Strips common base prefixes and normalizes separators.
pub fn compact_path(path: &str, base_dir: Option<&str>) -> String {
    let normalized = path.replace('\\', "/");
    if let Some(base) = base_dir {
        let base_normalized = base.replace('\\', "/");
        if let Ok(relative) = Path::new(&normalized).strip_prefix(&base_normalized) {
            return relative.to_string_lossy().to_string();
        }
    }
    let markers = ["/src/", "/packages/", "/crates/", "/app/", "/lib/", "/components/"];
    for marker in &markers {
        if let Some(pos) = normalized.rfind(marker) {
            if pos + marker.len() < normalized.len() {
                return normalized[pos + 1..].to_string();
            }
        }
    }
    normalized
}

/// Estimate the number of tokens in a text (rough approximation).
/// Uses text.len() / 4 as a simple heuristic for mixed-language content.
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 4.0).ceil() as usize
}

/// Post-process output string: apply ANSI stripping, path shortening, and line truncation.
pub struct OutputPostProcessor {
    pub strip_ansi: bool,
    pub max_lines: Option<usize>,
    pub max_line_length: Option<usize>,
    pub noise_patterns: Vec<String>,
    pub keep_patterns: Vec<String>,
}

impl OutputPostProcessor {
    pub fn new() -> Self {
        Self {
            strip_ansi: true,
            max_lines: None,
            max_line_length: None,
            noise_patterns: Vec::new(),
            keep_patterns: Vec::new(),
        }
    }

    pub fn with_strip_ansi(mut self, enabled: bool) -> Self {
        self.strip_ansi = enabled;
        self
    }

    pub fn with_max_lines(mut self, n: usize) -> Self {
        self.max_lines = Some(n);
        self
    }

    pub fn with_max_line_length(mut self, n: usize) -> Self {
        self.max_line_length = Some(n);
        self
    }

    pub fn with_noise_patterns(mut self, patterns: Vec<String>) -> Self {
        self.noise_patterns = patterns;
        self
    }

    pub fn with_keep_patterns(mut self, patterns: Vec<String>) -> Self {
        self.keep_patterns = patterns;
        self
    }

    /// Process output string through all configured filters in order:
    /// 1. ANSI stripping
    /// 2. Noise line stripping (if patterns provided)
    /// 3. Keep line matching (if patterns provided)
    /// 4. Per-line length truncation
    /// 5. Total line count truncation
    pub fn process(&self, output: &str) -> String {
        let mut result = output.to_string();

        if self.strip_ansi {
            result = strip_ansi(&result);
        }

        if !self.noise_patterns.is_empty() {
            result = filter_noise_lines(&result, &self.noise_patterns);
        }

        if !self.keep_patterns.is_empty() {
            result = keep_matching_lines(&result, &self.keep_patterns);
        }

        if let Some(max_len) = self.max_line_length {
            result = result
                .lines()
                .map(|line| {
                    if line.chars().count() > max_len {
                        truncate(line, max_len)
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
        }

        if let Some(max_lines) = self.max_lines {
            result = truncate_output(&result, OutputTruncation::Max(max_lines));
        }

        result
    }
}

impl Default for OutputPostProcessor {
    fn default() -> Self {
        Self::new()
    }
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

    #[test]
    fn test_filter_noise_lines() {
        let output = "make[1]: Entering directory\ncc -O2 foo.c\nmake[1]: Leaving directory\n";
        let filtered = filter_noise_lines(output, &["^make\\[\\d+\\]:".to_string()]);
        assert!(!filtered.contains("make[1]:"));
        assert!(filtered.contains("cc -O2 foo.c"));
    }

    #[test]
    fn test_keep_matching_lines() {
        let output = "info: ok\nerror: failed\nwarning: caution\ndebug: verbose\n";
        let kept = keep_matching_lines(output, &["^error:".to_string(), "^warning:".to_string()]);
        assert!(!kept.contains("info:"));
        assert!(kept.contains("error:"));
        assert!(kept.contains("warning:"));
        assert!(!kept.contains("debug:"));
    }

    #[test]
    fn test_truncate_output_head() {
        let output = "a\nb\nc\nd\ne";
        assert_eq!(truncate_output(output, OutputTruncation::Head(3)), "a\nb\nc\n... (+2 more lines)");
        assert_eq!(truncate_output(output, OutputTruncation::Head(10)), output);
    }

    #[test]
    fn test_truncate_output_tail() {
        let output = "a\nb\nc\nd\ne";
        let result = truncate_output(output, OutputTruncation::Tail(2));
        assert!(result.starts_with("... (+3 more lines)"));
        assert!(result.contains("d"));
        assert!(result.contains("e"));
    }

    #[test]
    fn test_truncate_output_head_tail() {
        let output = "a\nb\nc\nd\ne\nf\ng";
        let result = truncate_output(output, OutputTruncation::HeadTail { head: 2, tail: 2 });
        assert!(result.starts_with("a\nb"));
        assert!(result.contains("... (+3 more lines)"));
        assert!(result.ends_with("f\ng"));
    }

    #[test]
    fn test_compact_path() {
        let path = "/home/user/project/src/main.rs";
        let compact = compact_path(path, Some("/home/user/project"));
        assert_eq!(compact, "src/main.rs");

        let win_path = "D:\\projects\\myapp\\src\\components\\Button.tsx";
        let compact_win = compact_path(win_path, None);
        assert_eq!(compact_win, "src/components/Button.tsx");
    }

    #[test]
    fn test_estimate_tokens() {
        assert!(estimate_tokens("hello world") > 0);
        assert!(estimate_tokens("a") >= 1);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_smart_truncate_line() {
        let long = "before_error_keyword_after_content_here_more_stuff";
        let result = smart_truncate_line(long, 20, Some("error"));
        assert!(result.len() < long.len());
        assert!(result.contains("error"));
    }

    #[test]
    fn test_output_post_processor() {
        let processor = OutputPostProcessor::new()
            .with_noise_patterns(vec!["^debug:".to_string()]);
        let output = "debug: verbose\nerror: failed\ninfo: ok\n";
        let result = processor.process(output);
        assert!(!result.contains("debug:"));
        assert!(result.contains("error:"));
        assert!(result.contains("info:"));
    }
}