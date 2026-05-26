//! Shared utility functions for text processing and command execution.

use regex::Regex;
use std::sync::OnceLock;

/// Unicode TUI frame/box-drawing characters used by tools like
/// Turbo (turborepo) in TUI mode, Nx, and other monorepo runners.
pub const TUI_BORDER_CHARS: &[char] = &[
    '╭', '╰', '╮', '╯', // rounded corner box
    '┌', '└', '┐', '┘', // regular corner box
    '│', '─', '├', '┤', // straight lines & T-junctions
    '┏', '┗', '┓', '┖', // double-line corner box
    '┃', '━', '┣', '┫', // thick lines
    '╋', '┳', '┻', '┧', '┨', // cross / side junctions
    '•', '✖', '◆', '▸', '▹', // bullet / mark symbols
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
    line.chars().all(|c| c.is_whitespace() || TUI_BORDER_CHARS.contains(&c))
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
            // Strip leading TUI border prefix from content lines (e.g. "│ ./main.go:...")
            let mut cleaned = line;
            while let Some(first) = cleaned.chars().next() {
                if TUI_BORDER_CHARS.contains(&first) || first == ' ' {
                    cleaned = &cleaned[first.len_utf8()..];
                } else {
                    break;
                }
            }
            cleaned.to_string()
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}



/// Truncate output to at most `max_lines` lines, showing "… (+N more)" if truncated.
pub fn truncate_output(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        output.to_string()
    } else {
        let head: Vec<&str> = lines[..max_lines].to_vec();
        format!("{}\n... (+{} more lines)", head.join("\n"), lines.len() - max_lines)
    }
}

/// Post-process output string: apply ANSI stripping, TUI frame filtering,
/// noise/keep patterns, and line truncation.
pub struct OutputPostProcessor {
    pub strip_ansi: bool,
    pub strip_tui_frames: bool,
    pub max_lines: Option<usize>,
    pub max_line_length: Option<usize>,
    pub noise_patterns: Vec<String>,
    pub keep_patterns: Vec<String>,
}

#[allow(dead_code)]
impl OutputPostProcessor {
    pub fn new() -> Self {
        Self {
            strip_ansi: true,
            strip_tui_frames: true,
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

    pub fn with_strip_tui_frames(mut self, enabled: bool) -> Self {
        self.strip_tui_frames = enabled;
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
    /// 2. TUI frame/border line removal
    /// 3. Turbo/nx prefix stripping
    /// 4. Noise line stripping (if patterns provided)
    /// 5. Keep line matching (if patterns provided)
    /// 6. Per-line length truncation
    /// 7. Total line count truncation
    pub fn process(&self, output: &str) -> String {
        let mut result = output.to_string();

        if self.strip_ansi {
            result = strip_ansi(&result);
        }

        if self.strip_tui_frames {
            result = filter_tui_frame_lines(&result);
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
            result = truncate_output(&result, max_lines);
        }

        result
    }
}

impl Default for OutputPostProcessor {
    fn default() -> Self {
        Self::new()
    }
}