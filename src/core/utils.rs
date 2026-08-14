//! Shared utility functions for text processing and command execution.

use regex::Regex;
use std::sync::OnceLock;

// ── Public free functions ─────────────────────────────────────────────

/// Unicode TUI frame/box-drawing characters used by tools like
/// Turbo (turborepo) in TUI mode, Nx, and other monorepo runners.
pub const TUI_BORDER_CHARS: &[char] = &[
    '\u{256d}', '\u{2570}', '\u{256e}', '\u{256f}', // rounded corner box
    '\u{250c}', '\u{2514}', '\u{2510}', '\u{2518}', // regular corner box
    '\u{2502}', '\u{2500}', '\u{251c}', '\u{2524}', // straight lines & T-junctions
    '\u{250f}', '\u{2517}', '\u{2513}', '\u{2516}', // double-line corner box
    '\u{2503}', '\u{2501}', '\u{2523}', '\u{252b}', // thick lines
    '\u{254b}', '\u{2533}', '\u{253b}', '\u{2527}', '\u{2528}', // cross / side junctions
    '\u{2022}', '\u{2716}', '\u{25c6}', '\u{25b8}', '\u{25b9}', // bullet / mark symbols
];

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

/// Filter out lines matching any of the given regex patterns (noise line stripping).
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

/// Check if a trimmed line is purely TUI decoration (border/box-drawing characters or whitespace).
pub fn is_tui_border_line(line: &str) -> bool {
    if line.is_empty() {
        return true;
    }
    line.chars()
        .all(|c| c.is_whitespace() || TUI_BORDER_CHARS.contains(&c))
}

/// Filter out TUI frame/border lines from output.
/// Removes lines that consist solely of box-drawing characters (e.g. `╭───╮`, `│   │`, `├───┤`).
/// Also strips leading TUI frame prefixes (e.g. `│ `, `┃ `) from content lines
/// so that `│ ./main.go:10:5: error` becomes `./main.go:10:5: error`.
pub fn filter_tui_frame_lines(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            // Remove pure border/decoration lines entirely
            if is_tui_border_line(trimmed) {
                return String::new();
            }
            // Strip leading TUI border prefix from content lines (e.g. "│ ./main.go:...");
            // keep the original indentation when the line has no border character so
            // indentation-sensitive parsers (e.g. CMake block errors) still work.
            match strip_tui_prefix(line) {
                Some(cleaned) => cleaned.to_string(),
                None => line.to_string(),
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip a leading TUI border prefix (border characters plus following spaces)
/// from a line. Returns `None` when the line does not start with a TUI border
/// character, in which case the caller must keep the line as-is.
fn strip_tui_prefix(line: &str) -> Option<&str> {
    let first = line.chars().next()?;
    if !TUI_BORDER_CHARS.contains(&first) {
        return None;
    }
    let mut cleaned = line;
    while let Some(c) = cleaned.chars().next() {
        if TUI_BORDER_CHARS.contains(&c) || c == ' ' {
            cleaned = &cleaned[c.len_utf8()..];
        } else {
            break;
        }
    }
    Some(cleaned)
}

/// Truncate output to at most `max_lines` lines, showing "… (+N more)" if truncated.
pub fn truncate_output(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        output.to_string()
    } else {
        let head: Vec<&str> = lines[..max_lines].to_vec();
        format!(
            "{}\n... (+{} more lines)",
            head.join("\n"),
            lines.len() - max_lines
        )
    }
}

/// Apply regex replace patterns to each line of output.
pub fn apply_replace_patterns(output: &str, patterns: &[(String, String)]) -> String {
    if patterns.is_empty() {
        return output.to_string();
    }
    let compiled: Vec<(Regex, &str)> = patterns
        .iter()
        .map(|(pat, repl)| {
            (
                Regex::new(pat).expect("Invalid replace pattern regex"),
                repl.as_str(),
            )
        })
        .collect();
    output
        .lines()
        .map(|line| {
            let mut result = line.to_string();
            for (re, repl) in &compiled {
                result = re.replace_all(&result, *repl).to_string();
            }
            result
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Short-circuit rule for early exit from the filter pipeline.
/// If the output matches `pattern` and does NOT match `unless`, return `message` immediately.
#[derive(Debug, Clone)]
pub struct ShortCircuitRule {
    pub pattern: String,
    pub message: String,
    pub unless: Option<String>,
}

/// Post-process output string: apply ANSI stripping, TUI frame filtering,
/// replace patterns, short-circuit matching, noise/keep patterns, line truncation,
/// and on-empty fallback.
pub struct OutputPostProcessor {
    pub strip_ansi: bool,
    pub strip_tui_frames: bool,
    pub replace_patterns: Vec<(String, String)>,
    pub short_circuits: Vec<ShortCircuitRule>,
    pub max_lines: Option<usize>,
    pub max_line_length: Option<usize>,
    pub noise_patterns: Vec<String>,
    pub keep_patterns: Vec<String>,
    pub on_empty_message: Option<String>,
    compiled_replace: OnceLock<Vec<(Regex, String)>>,
    compiled_noise: OnceLock<Vec<Regex>>,
    compiled_keep: OnceLock<Vec<Regex>>,
}

impl OutputPostProcessor {
    pub fn new() -> Self {
        Self {
            strip_ansi: true,
            strip_tui_frames: true,
            replace_patterns: Vec::new(),
            short_circuits: Vec::new(),
            max_lines: None,
            max_line_length: None,
            noise_patterns: Vec::new(),
            keep_patterns: Vec::new(),
            on_empty_message: None,
            compiled_replace: OnceLock::new(),
            compiled_noise: OnceLock::new(),
            compiled_keep: OnceLock::new(),
        }
    }

    pub fn with_strip_ansi(mut self, enabled: bool) -> Self {
        self.strip_ansi = enabled;
        self
    }

    pub fn with_strip_tui_frames(mut self, enabled: bool) -> Self {
        self.strip_tui_frames = enabled;
        self
    }

    pub fn with_replace_patterns(mut self, patterns: Vec<(String, String)>) -> Self {
        self.replace_patterns = patterns;
        self
    }

    pub fn with_short_circuits(mut self, rules: Vec<ShortCircuitRule>) -> Self {
        self.short_circuits = rules;
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

    pub fn with_on_empty(mut self, message: impl Into<String>) -> Self {
        self.on_empty_message = Some(message.into());
        self
    }

    /// Build from AnalyzeOptions, without short-circuit / on-empty rules
    /// (those come from the filter config layer).
    pub fn from_options(options: &crate::core::types::AnalyzeOptions) -> Self {
        Self {
            strip_ansi: options.strip_ansi,
            strip_tui_frames: options.strip_tui_frames,
            replace_patterns: Vec::new(),
            short_circuits: Vec::new(),
            max_lines: if options.max_output_lines > 0 {
                Some(options.max_output_lines)
            } else {
                None
            },
            max_line_length: if options.max_line_length > 0 {
                Some(options.max_line_length)
            } else {
                None
            },
            noise_patterns: options.noise_patterns.clone(),
            keep_patterns: options.keep_patterns.clone(),
            on_empty_message: None,
            compiled_replace: OnceLock::new(),
            compiled_noise: OnceLock::new(),
            compiled_keep: OnceLock::new(),
        }
    }

    /// Merge two processors: base provides fundamental options, override enriches them.
    /// TOML-sourced values take precedence except for noise patterns which combine.
    pub fn merge(base: Self, override_proc: Self) -> Self {
        Self {
            strip_ansi: base.strip_ansi || override_proc.strip_ansi,
            strip_tui_frames: base.strip_tui_frames || override_proc.strip_tui_frames,
            replace_patterns: if override_proc.replace_patterns.is_empty() {
                base.replace_patterns
            } else {
                override_proc.replace_patterns
            },
            short_circuits: if override_proc.short_circuits.is_empty() {
                base.short_circuits
            } else {
                override_proc.short_circuits
            },
            max_lines: override_proc.max_lines.or(base.max_lines),
            max_line_length: override_proc.max_line_length.or(base.max_line_length),
            noise_patterns: if !override_proc.noise_patterns.is_empty() {
                let mut combined = base.noise_patterns;
                combined.extend(override_proc.noise_patterns);
                combined
            } else {
                base.noise_patterns
            },
            keep_patterns: if override_proc.keep_patterns.is_empty() {
                base.keep_patterns
            } else {
                override_proc.keep_patterns
            },
            on_empty_message: override_proc.on_empty_message.or(base.on_empty_message),
            compiled_replace: OnceLock::new(),
            compiled_noise: OnceLock::new(),
            compiled_keep: OnceLock::new(),
        }
    }

    // ── lazy-compiled regex accessors ──────────────────────────────

    fn replace_re(&self) -> &[(Regex, String)] {
        self.compiled_replace.get_or_init(|| {
            self.replace_patterns
                .iter()
                .map(|(pat, repl)| {
                    (
                        Regex::new(pat).unwrap_or_else(|_| Regex::new("$^").unwrap()),
                        repl.clone(),
                    )
                })
                .collect()
        })
    }

    fn noise_re(&self) -> &[Regex] {
        self.compiled_noise.get_or_init(|| {
            self.noise_patterns
                .iter()
                .filter_map(|p| Regex::new(p).ok())
                .collect()
        })
    }

    fn keep_re(&self) -> &[Regex] {
        self.compiled_keep.get_or_init(|| {
            self.keep_patterns
                .iter()
                .filter_map(|p| Regex::new(p).ok())
                .collect()
        })
    }

    // ── per-line methods (used by streaming PostProcessLineFilter) ─

    /// Stage 1: strip ANSI escape codes from a single line.
    pub fn process_line_ansi(&self, line: &str) -> String {
        if self.strip_ansi {
            strip_ansi(line)
        } else {
            line.to_string()
        }
    }

    /// Stage 2: apply regex replace patterns to a single line.
    pub fn process_line_replace(&self, line: &str) -> String {
        let mut result = line.to_string();
        for (re, repl) in self.replace_re() {
            result = re.replace_all(&result, repl.as_str()).to_string();
        }
        result
    }

    /// Stage 4: strip TUI border prefix from a single line.
    /// Returns None if the line is a pure TUI decoration line.
    pub fn process_line_tui(&self, line: &str) -> Option<String> {
        if !self.strip_tui_frames {
            return Some(line.to_string());
        }
        let trimmed = line.trim();
        if is_tui_border_line(trimmed) {
            return None;
        }
        // Only strip leading whitespace when the line actually starts with a
        // TUI border character; regular output lines keep their indentation so
        // that indentation-sensitive parsers (e.g. CMake block errors) still
        // work.
        let cleaned = match strip_tui_prefix(line) {
            Some(c) => c.to_string(),
            None => line.to_string(),
        };
        if cleaned.trim().is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }

    /// Stage 5: check if a line matches any noise pattern.
    pub fn is_noise_line(&self, line: &str) -> bool {
        self.noise_re().iter().any(|re| re.is_match(line.trim()))
    }

    /// Stage 6: check if a line matches any keep pattern.
    /// Returns true when keep_patterns is empty (keep all).
    pub fn is_keep_line(&self, line: &str) -> bool {
        if self.keep_patterns.is_empty() {
            return true;
        }
        self.keep_re().iter().any(|re| re.is_match(line))
    }

    /// Stage 7: truncate a single line to max_line_length.
    pub fn process_line_truncate(&self, line: &str) -> String {
        match self.max_line_length {
            Some(max_len) if line.chars().count() > max_len => truncate(line, max_len),
            _ => line.to_string(),
        }
    }

    /// Check short-circuit rules against accumulated text (batch operation).
    pub fn check_short_circuit(&self, text: &str) -> Option<String> {
        for rule in &self.short_circuits {
            let pattern = Regex::new(&rule.pattern).expect("Invalid short-circuit pattern");
            if pattern.is_match(text) {
                let blocked = rule
                    .unless
                    .as_ref()
                    .and_then(|u| Regex::new(u).ok())
                    .map(|re| re.is_match(text))
                    .unwrap_or(false);
                if !blocked {
                    return Some(rule.message.clone());
                }
            }
        }
        None
    }

    /// Create a pre-configured processor for Turborepo output.
    /// Strips ANSI, TUI frames, cache noise, stats, and update notifications.
    pub fn for_turbo() -> Self {
        Self::new()
            .with_strip_ansi(true)
            .with_strip_tui_frames(true)
            .with_noise_patterns(vec![
                r"^\s*cache (hit|miss|bypass)".to_string(),
                r"^\s*replaying logs".to_string(),
                r"^\s*\d+ packages in scope".to_string(),
                r"^\s*Running \w+ in \d+ packages".to_string(),
                r"^\s*Remote caching".to_string(),
                r"^\s*Tasks:\s*\d+".to_string(),
                r"^\s*Cached:\s*".to_string(),
                r"^\s*Time:\s*".to_string(),
                r"^\s*Duration:\s*".to_string(),
                r"^\s*Failed:\s*".to_string(),
                r"^\s*ERROR\s+run failed:".to_string(),
                r"^\s*\d+ problems?\s*\(".to_string(),
                r"^\s*Update available".to_string(),
                r"^\s*Changelog:".to_string(),
                r"^\s*Follow @turborepo".to_string(),
                r"^\s*> .+@\d+\.\d+\.\d+ \w+".to_string(),
                r"cache (hit|miss|bypass|output)".to_string(),
                r"replaying logs".to_string(),
            ])
            .with_max_line_length(200)
            .with_max_lines(100)
            .with_on_empty("turbo: all tasks completed successfully")
    }

    /// Process output string through all configured filters in order:
    /// 1. ANSI stripping
    /// 2. Regex replace patterns
    /// 3. TUI frame/border line removal
    /// 4. Noise line stripping (if patterns provided)
    /// 5. Keep line matching (if patterns provided)
    /// 6. Per-line length truncation
    /// 7. Total line count truncation
    /// 8. Short-circuit matching
    /// 9. On-empty fallback message
    pub fn process(&self, output: &str) -> String {
        let mut result = output.to_string();

        // Stage 1: ANSI stripping
        if self.strip_ansi {
            result = strip_ansi(&result);
        }

        // Stage 2: Regex replace
        if !self.replace_patterns.is_empty() {
            result = apply_replace_patterns(&result, &self.replace_patterns);
        }

        // Stage 3: TUI frame/border line removal
        if self.strip_tui_frames {
            result = filter_tui_frame_lines(&result);
        }

        // Stage 4: Noise line stripping
        if !self.noise_patterns.is_empty() {
            result = filter_noise_lines(&result, &self.noise_patterns);
        }

        // Stage 5: Keep line matching
        if !self.keep_patterns.is_empty() {
            result = keep_matching_lines(&result, &self.keep_patterns);
        }

        // Stage 6: Per-line length truncation
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

        // Stage 7: Total line count truncation
        if let Some(max_lines) = self.max_lines {
            result = truncate_output(&result, max_lines);
        }

        // Stage 8: Short-circuit matching (after all filtering, consistent with streaming)
        if let Some(msg) = self.check_short_circuit(&result) {
            return msg;
        }

        // Stage 9: On-empty fallback
        if result.trim().is_empty() {
            if let Some(ref msg) = self.on_empty_message {
                return msg.clone();
            }
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
    fn test_strip_ansi_basic() {
        let input = "\x1b[31merror\x1b[0m: something failed";
        let result = strip_ansi(input);
        assert_eq!(result, "error: something failed");
    }

    #[test]
    fn test_strip_ansi_complex() {
        let input = "\x1b[1;32mSUCCESS\x1b[0m \x1b[33mWARNING\x1b[0m";
        let result = strip_ansi(input);
        assert_eq!(result, "SUCCESS WARNING");
    }

    #[test]
    fn test_truncate_shorter() {
        assert_eq!(truncate("hi", 10), "hi");
    }

    #[test]
    fn test_truncate_exact() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_longer() {
        let result = truncate("hello world", 8);
        assert!(result.ends_with("..."));
        assert!(result.starts_with("hello"));
    }

    #[test]
    fn test_truncate_too_short_for_ellipsis() {
        assert_eq!(truncate("abc", 2), "...");
    }

    #[test]
    fn test_filter_noise_lines() {
        let output = "error: foo\ncache hit\nwarning: bar\n";
        let patterns = vec!["cache (hit|miss)".to_string()];
        let result = filter_noise_lines(output, &patterns);
        assert!(result.contains("error: foo"));
        assert!(result.contains("warning: bar"));
        assert!(!result.contains("cache hit"));
    }

    #[test]
    fn test_keep_matching_lines() {
        let output = "info: start\ncache hit\nerror: fail\ncache miss\n";
        let patterns = vec!["error|warning".to_string()];
        let result = keep_matching_lines(output, &patterns);
        assert!(!result.contains("info: start"));
        assert!(!result.contains("cache"));
        assert!(result.contains("error: fail"));
    }

    #[test]
    fn test_is_tui_border_line() {
        assert!(is_tui_border_line(
            "\u{250c}\u{2500}\u{2500}\u{2500}\u{2510}"
        ));
        assert!(is_tui_border_line(" \u{2502}  \u{2502} "));
        assert!(!is_tui_border_line("error in file.rs"));
    }

    #[test]
    fn test_filter_tui_frame_lines_removes_borders() {
        let output = "\u{250c}\u{2500}\u{2500}\u{2500}\u{2510}\nreal content\n\u{2514}\u{2500}\u{2500}\u{2500}\u{2518}";
        let result = filter_tui_frame_lines(output);
        assert_eq!(result, "real content");
    }

    #[test]
    fn test_filter_tui_frame_lines_strips_prefix() {
        let output = "\u{2502} ./main.go:10:5: error: unused variable";
        let result = filter_tui_frame_lines(output);
        assert_eq!(result, "./main.go:10:5: error: unused variable");
    }

    #[test]
    fn test_filter_tui_frame_lines_preserves_indentation() {
        // Regular output lines (no TUI border char) must keep their leading
        // whitespace; indentation-sensitive parsers (e.g. CMake block errors)
        // depend on it.
        let output = "  Cannot find source file:\n    src/main.cpp";
        let result = filter_tui_frame_lines(output);
        assert_eq!(result, "  Cannot find source file:\n    src/main.cpp");
    }

    #[test]
    fn test_replace_patterns() {
        let output = "web:lint:   4:7   error   message";
        let patterns = vec![(r"^\S+:\S+:\s*".to_string(), "".to_string())];
        let result = apply_replace_patterns(output, &patterns);
        assert_eq!(result, "4:7   error   message");
    }

    #[test]
    fn test_short_circuit_match() {
        let processor = OutputPostProcessor::new().with_short_circuits(vec![ShortCircuitRule {
            pattern: "success".to_string(),
            message: "All good".to_string(),
            unless: None,
        }]);
        let result = processor.process("build success!");
        assert_eq!(result, "All good");
    }

    #[test]
    fn test_short_circuit_blocked_by_unless() {
        let processor = OutputPostProcessor::new().with_short_circuits(vec![ShortCircuitRule {
            pattern: ".*".to_string(),
            message: "all good".to_string(),
            unless: Some("(?i)error|fail".to_string()),
        }]);
        let result = processor.process("build failed with error");
        assert_ne!(result, "all good");
    }

    #[test]
    fn test_on_empty_fallback() {
        let processor = OutputPostProcessor::new()
            .with_noise_patterns(vec![r".*".to_string()])
            .with_on_empty("no issues found");
        let result = processor.process("all noise removed\n");
        assert_eq!(result, "no issues found");
    }

    #[test]
    fn test_for_turbo_strips_noise() {
        let processor = OutputPostProcessor::for_turbo();
        let output = "Tasks:    1 successful, 1 total\nCached:    0 cached, 1 total\n  Time:    1.234s\nsrc/main.ts\n  4:7  error  message";
        let result = processor.process(output);
        assert!(!result.contains("Tasks:"));
        assert!(!result.contains("Cached:"));
        assert!(!result.contains("Time:"));
        assert!(result.contains("src/main.ts"));
        assert!(result.contains("error"));
    }

    #[test]
    fn test_for_turbo_strips_ansi_and_tui() {
        let processor = OutputPostProcessor::for_turbo();
        let output = "\u{250c}\u{2500} @scope/pkg#lint > cache hit\n\u{2502} \x1b[31m  16:3  warning  msg\x1b[0m\n\u{2514}\u{2500} @scope/pkg#lint \u{2500}\u{2500}";
        let result = processor.process(output);
        assert!(result.contains("16:3  warning  msg"));
        assert!(!result.contains("cache hit"));
        assert!(!result.contains("\u{250c}"));
        assert!(!result.contains("\x1b[31m"));
    }

    #[test]
    fn test_for_turbo_on_empty() {
        let processor = OutputPostProcessor::for_turbo();
        let output = "Tasks:    1 successful, 1 total\nCached:    0 cached, 1 total\n";
        let result = processor.process(output);
        assert_eq!(result, "turbo: all tasks completed successfully");
    }
}
