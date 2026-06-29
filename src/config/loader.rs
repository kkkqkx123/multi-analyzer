use std::path::Path;

use super::env_loader;
use super::global::AppConfig;
use super::project::{ProjectAppConfig, ProjectConfigPaths};

pub struct ConfigLoader {
    config_path: Option<std::path::PathBuf>,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self { config_path: None }
    }

    /// Load project-level configuration from project root directory
    pub fn load_project(project_root: &Path) -> ProjectAppConfig {
        ProjectConfigPaths::find_project_config(project_root)
            .and_then(|path| {
                std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|content| toml::from_str(&content).ok())
            })
            .unwrap_or_default()
    }

    /// Full load: global + project merge + env vars
    pub fn load(self) -> AppConfig {
        let mut config = self.load_internal();

        // Try to find and merge project config
        if let Ok(cwd) = std::env::current_dir() {
            let project_config = Self::load_project(&cwd);
            config = config.merge_with_project(&project_config);
        }

        // Apply environment variable overrides (highest priority among non-CLI)
        env_loader::apply_env_vars(&mut config);

        config
    }

    fn load_internal(&self) -> AppConfig {
        // Try specified path first
        if let Some(path) = &self.config_path {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(config) = toml::from_str(&content) {
                        return config;
                    }
                }
            }
        }

        // Try global config path: ~/.config/analyzer/config.toml
        if let Some(global_dir) = dirs::config_dir() {
            let global_path = global_dir.join("analyzer").join("config.toml");
            if global_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&global_path) {
                    if let Ok(config) = toml::from_str(&content) {
                        return config;
                    }
                }
            }
        }

        AppConfig::default()
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}
