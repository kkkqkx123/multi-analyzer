//! TOML-based configuration system
//! Provides customizable behavior for reports, commands, and filters

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
}

fn default_report_format() -> String {
    "markdown".to_string()
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            format: default_report_format(),
            verbose: false,
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

/// Filter configuration
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FilterConfig {
    /// File path patterns to ignore during analysis
    #[serde(default)]
    pub ignore_paths: Vec<String>,
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