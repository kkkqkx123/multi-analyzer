use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::modules::{CommandConfig, FilterConfig, LimitsConfig, ReportConfig, TechStackConfig, TeeConfig};
use super::project::ProjectAppConfig;

fn default_version() -> String {
    "1.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub report: ReportConfig,
    #[serde(default)]
    pub filter: FilterConfig,
    #[serde(default)]
    pub commands: HashMap<String, CommandConfig>,
    #[serde(default)]
    pub tech_stacks: HashMap<String, TechStackConfig>,
    #[serde(default)]
    pub tee: TeeConfig,
    #[serde(default)]
    pub limits: LimitsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            report: ReportConfig::default(),
            filter: FilterConfig::default(),
            commands: HashMap::new(),
            tech_stacks: HashMap::new(),
            tee: TeeConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl AppConfig {
    /// Merge project-level config into global config.
    /// Project values take precedence where present.
    pub fn merge_with_project(self, project: &ProjectAppConfig) -> Self {
        AppConfig {
            version: self.version,
            report: project.report.clone().unwrap_or(self.report),
            filter: project.filter.clone().unwrap_or(self.filter),
            commands: {
                let mut cmds = self.commands;
                if let Some(project_cmds) = &project.commands {
                    cmds.extend(project_cmds.iter().map(|(k, v)| (k.clone(), v.clone())));
                }
                cmds
            },
            tech_stacks: {
                let mut stacks = self.tech_stacks;
                if let Some(project_stacks) = &project.tech_stacks {
                    stacks.extend(project_stacks.iter().map(|(k, v)| (k.clone(), v.clone())));
                }
                stacks
            },
            tee: self.tee,
            limits: self.limits,
        }
    }

    /// Load config from the global config path, falling back to defaults.
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::global_config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to the global config path.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::global_config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Create a default config file at the global config path, returns the path.
    pub fn create_default() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config = Self::default();
        config.save()?;
        Ok(Self::global_config_path())
    }

    /// Returns the global config file path: ~/.config/analyzer/config.toml
    fn global_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("analyzer")
            .join("config.toml")
    }

    /// Print the full current config (with file path) to stdout for debugging.
    pub fn show_config() -> Result<(), Box<dyn std::error::Error>> {
        let path = AppConfig::global_config_path();
        println!("Config: {}", path.display());
        println!();
        if path.exists() {
            let config = AppConfig::load()?;
            println!("{}", toml::to_string_pretty(&config)?);
        } else {
            println!("(default config, file not created)");
            println!();
            let config = AppConfig::default();
            println!("{}", toml::to_string_pretty(&config)?);
        }
        // Print discover engine stats
        crate::discover::print_rules_stats();
        Ok(())
    }

    /// Resolve a tech-stack script name to its actual framework.
    ///
    /// Example: `resolve_script("npm", "test")` returns `Some("jest")` if
    /// the `[tech_stacks.npm]` section has `scripts.test = "jest"`.
    pub fn resolve_script(&self, tech_stack: &str, script: &str) -> Option<String> {
        self.tech_stacks
            .get(tech_stack)?
            .scripts
            .get(script)
            .cloned()
    }

    /// Return the declared test framework for a given tech stack.
    ///
    /// Example: `test_framework_for("pnpm")` returns `Some("vitest")` if
    /// `[tech_stacks.pnpm]` has `test_framework = "vitest"`.
    pub fn test_framework_for(&self, tech_stack: &str) -> Option<String> {
        self.tech_stacks
            .get(tech_stack)?
            .test_framework
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_default_has_version() {
        let config = AppConfig::default();
        assert_eq!(config.version, "1.0");
    }

    #[test]
    fn test_app_config_serializes_version() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("version = \"1.0\""));
    }
}
