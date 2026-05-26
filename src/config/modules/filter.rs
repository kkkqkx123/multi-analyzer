use serde::{Deserialize, Serialize};

use crate::core::utils::OutputPostProcessor;

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
        }
    }
}

impl FilterConfig {
    #[allow(dead_code)]
    pub fn to_post_processor(&self) -> OutputPostProcessor {
        let mut processor = OutputPostProcessor::new()
            .with_strip_ansi(self.strip_ansi)
            .with_noise_patterns(self.noise_patterns.clone())
            .with_keep_patterns(self.keep_patterns.clone());
        if self.max_lines > 0 {
            processor = processor.with_max_lines(self.max_lines);
        }
        if self.max_line_length > 0 {
            processor = processor.with_max_line_length(self.max_line_length);
        }
        processor
    }
}