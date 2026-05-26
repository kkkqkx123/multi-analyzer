use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::modules::{CommandConfig, FilterConfig, ReportConfig};
use super::project::ProjectAppConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub report: ReportConfig,
    #[serde(default)]
    pub filter: FilterConfig,
    #[serde(default)]
    pub commands: HashMap<String, CommandConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            report: ReportConfig::default(),
            filter: FilterConfig::default(),
            commands: HashMap::new(),
        }
    }
}

impl AppConfig {
    /// Merge project-level config into global config.
    /// Project values take precedence where present.
    pub fn merge_with_project(self, project: &ProjectAppConfig) -> Self {
        AppConfig {
            report: project.report.clone().unwrap_or(self.report),
            filter: project.filter.clone().unwrap_or(self.filter),
            commands: {
                let mut cmds = self.commands;
                if let Some(project_cmds) = &project.commands {
                    cmds.extend(project_cmds.iter().map(|(k, v)| (k.clone(), v.clone())));
                }
                cmds
            },
        }
    }
}