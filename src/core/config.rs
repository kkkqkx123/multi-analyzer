//! TOML-based configuration system
//! Provides customizable behavior for reports, commands, and filters

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::utils::OutputPostProcessor;

/// Top-level configuration
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Report output configuration
    #[serde(default)]
    pub report: ReportConfig,
    /// Command overrides per tech stack
    #[serde(default)]
    pub commands: HashMap<String, CommandConfig>,
    /// Filter configuration
    #[serde(default)]
    pub filter: FilterConfig,
}

/// Report output configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct ReportConfig {
    /// Default report format: markdown, json, html
    #[serde(default = "default_report_format")]
    pub format: String,
    /// Verbose output (show all issues without truncation)
    #[serde(default)]
    pub verbose: bool,
    /// Verbosity level: "minimal", "normal", "verbose"
    #[serde(default = "default_verbosity")]
    pub verbosity: String,
}

fn default_verbosity() -> String {
    "normal".to_string()
}

fn default_report_format() -> String {
    "markdown".to_string()
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            format: default_report_format(),
            verbose: false,
            verbosity: default_verbosity(),
        }
    }
}

/// Command configuration override
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    /// The executable/command to run
    pub exec: String,
    /// Tech stacks this command applies to (e.g. ["npm", "pnpm", "yarn"])
    #[serde(default)]
    pub tech_stacks: Vec<String>,
}

/// Filter configuration for output compression
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FilterConfig {
    /// File path patterns to ignore during analysis
    #[serde(default)]
    pub ignore_paths: Vec<String>,
    /// Regex patterns for noise lines to strip from output
    #[serde(default)]
    pub noise_patterns: Vec<String>,
    /// Regex patterns for lines to keep (opposite of noise)
    #[serde(default)]
    pub keep_patterns: Vec<String>,
    /// Maximum number of lines in output (0 = no limit)
    #[serde(default)]
    pub max_lines: usize,
    /// Maximum characters per line (0 = no limit)
    #[serde(default)]
    pub max_line_length: usize,
    /// Whether to strip ANSI escape codes
    #[serde(default)]
    pub strip_ansi: bool,
}

impl FilterConfig {
    /// Build an OutputPostProcessor from this filter configuration
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

impl Config {
    /// Load configuration from default paths
    /// Search order: analyzer.toml, .analyzer.toml, .analyzer/config.toml
    pub fn load() -> Self {
        let paths = ["analyzer.toml", ".analyzer.toml", ".analyzer/config.toml"];
        for path in &paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        Config::default()
    }

    /// Parse configuration from a string
    pub fn from_str(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    /// Get command config for a specific tech stack
    pub fn get_command(&self, name: &str, tech_stack: &str) -> Option<&CommandConfig> {
        self.commands.get(name).and_then(|cmd| {
            if cmd.tech_stacks.is_empty() || cmd.tech_stacks.iter().any(|s| s == tech_stack) {
                Some(cmd)
            } else {
                None
            }
        })
    }
}