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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_internal_default_when_no_config() {
        let loader = ConfigLoader::new();
        // No config path set and no global config exists → returns default
        let config = loader.load_internal();
        assert_eq!(config.version, "1.0");
    }

    #[test]
    fn test_load_project_finds_and_parses_config() {
        let dir = std::env::temp_dir().join(format!("analyzer_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let config_path = dir.join("analyzer.toml");
        let toml_content = r#"
            [report]
            format = "json"
            verbosity = "verbose"
        "#;
        std::fs::write(&config_path, toml_content).unwrap();

        let project_config = ConfigLoader::load_project(&dir);
        assert_eq!(project_config.report.as_ref().unwrap().format, "json");
        assert_eq!(project_config.report.as_ref().unwrap().verbosity, "verbose");

        // Cleanup
        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_load_project_returns_default_when_no_config() {
        let dir = std::env::temp_dir().join(format!("analyzer_test_empty_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        let project_config = ConfigLoader::load_project(&dir);
        assert!(project_config.report.is_none());

        // Cleanup
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_load_project_parses_hidden_config() {
        let dir = std::env::temp_dir().join(format!("analyzer_test_hidden_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let config_path = dir.join(".analyzer.toml");
        let toml_content = r#"
            [filter]
            strip_ansi = true
        "#;
        std::fs::write(&config_path, toml_content).unwrap();

        let project_config = ConfigLoader::load_project(&dir);
        assert!(project_config.filter.as_ref().unwrap().strip_ansi);

        // Cleanup
        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_dir(&dir);
    }
}
