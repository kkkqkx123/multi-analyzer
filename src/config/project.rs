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
