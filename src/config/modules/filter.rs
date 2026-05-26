use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    #[serde(default)]
    pub ignore_paths: Vec<String>,
    #[serde(default)]
    pub noise_patterns: Vec<String>,
    #[serde(default)]
    pub keep_patterns: Vec<String>,
    #[serde(default)]
    pub max_lines: usize,
    #[serde(default)]
    pub max_line_length: usize,
    #[serde(default)]
    pub strip_ansi: bool,
    /// Strip TUI frame/border lines (e.g. ╭──╮ │ ── ├──┤) from output
    #[serde(default = "default_true")]
    pub strip_tui_frames: bool,
}

fn default_true() -> bool {
    true
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            ignore_paths: Vec::new(),
            noise_patterns: Vec::new(),
            keep_patterns: Vec::new(),
            max_lines: 0,
            max_line_length: 0,
            strip_ansi: true,
            strip_tui_frames: true,
        }
    }
}

