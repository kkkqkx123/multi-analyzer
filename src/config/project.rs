use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::modules::{CommandConfig, FilterConfig, ReportConfig, TechStackConfig};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectAppConfig {
    #[serde(default)]
    pub report: Option<ReportConfig>,
    #[serde(default)]
    pub filter: Option<FilterConfig>,
    #[serde(default)]
    pub commands: Option<HashMap<String, CommandConfig>>,
    #[serde(default)]
    pub tech_stacks: Option<HashMap<String, TechStackConfig>>,
}

pub struct ProjectConfigPaths;

impl ProjectConfigPaths {
    pub const fn config_file_name() -> &'static str {
        "analyzer.toml"
    }

    pub const fn hidden_config_file_name() -> &'static str {
        ".analyzer.toml"
    }

    pub const fn config_dir_name() -> &'static str {
        ".analyzer"
    }

    pub fn find_project_config(start_dir: &Path) -> Option<PathBuf> {
        let mut current = Some(start_dir);
        while let Some(dir) = current {
            // Check analyzer.toml
            let paths = [
                dir.join(Self::config_file_name()),
                dir.join(Self::hidden_config_file_name()),
                dir.join(Self::config_dir_name())
                    .join(Self::config_file_name()),
            ];
            for path in &paths {
                if path.exists() {
                    return Some(path.clone());
                }
            }
            current = dir.parent();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_project_config_direct() {
        let dir = std::env::temp_dir().join(format!("proj_test_direct_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let config_path = dir.join("analyzer.toml");
        std::fs::write(&config_path, "").unwrap();

        let found = ProjectConfigPaths::find_project_config(&dir);
        assert!(found.is_some());
        assert_eq!(found.unwrap().file_name().unwrap(), "analyzer.toml");

        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_find_project_config_hidden() {
        let dir = std::env::temp_dir().join(format!("proj_test_hidden_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let config_path = dir.join(".analyzer.toml");
        std::fs::write(&config_path, "").unwrap();

        let found = ProjectConfigPaths::find_project_config(&dir);
        assert!(found.is_some());
        assert_eq!(found.unwrap().file_name().unwrap(), ".analyzer.toml");

        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_find_project_config_subdir() {
        let dir = std::env::temp_dir().join(format!("proj_test_subdir_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let config_dir = dir.join(".analyzer");
        let _ = std::fs::create_dir_all(&config_dir);
        let config_path = config_dir.join("analyzer.toml");
        std::fs::write(&config_path, "").unwrap();

        let found = ProjectConfigPaths::find_project_config(&dir);
        assert!(found.is_some());
        assert_eq!(found.unwrap().file_name().unwrap(), "analyzer.toml");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_project_config_not_found() {
        let dir = std::env::temp_dir().join(format!("proj_test_notfound_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        let found = ProjectConfigPaths::find_project_config(&dir);
        assert!(found.is_none());

        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_find_project_config_priority() {
        // analyzer.toml should take priority over .analyzer.toml
        let dir = std::env::temp_dir().join(format!("proj_test_priority_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("analyzer.toml"), "").unwrap();
        std::fs::write(dir.join(".analyzer.toml"), "").unwrap();

        let found = ProjectConfigPaths::find_project_config(&dir);
        assert!(found.is_some());
        // analyzer.toml is checked first
        assert_eq!(found.unwrap().file_name().unwrap(), "analyzer.toml");

        let _ = std::fs::remove_file(dir.join("analyzer.toml"));
        let _ = std::fs::remove_file(dir.join(".analyzer.toml"));
        let _ = std::fs::remove_dir(&dir);
    }
}
