//! TOML filter configuration types, loading, and lookup.
//!
//! Three-priority loading (matching RTK strategy):
//! 1. `.analyzer/filters.toml` (project local)
//! 2. `~/.config/analyzer/filters.toml` (user global)
//! 3. Built-in filters (compile-time embedded)
//! 4. Passthrough (no filter) when no match found

use serde::Deserialize;
use std::path::PathBuf;

/// A single TOML replace rule.
#[derive(Debug, Clone, Deserialize)]
pub struct TomlReplaceRule {
    pub pattern: String,
    pub replacement: String,
}

/// A single TOML short-circuit rule.
#[derive(Debug, Clone, Deserialize)]
pub struct TomlShortCircuitRule {
    pub pattern: String,
    pub message: String,
    #[serde(default)]
    pub unless: Option<String>,
}

/// A single filter definition from a TOML config file.
#[derive(Debug, Clone, Deserialize)]
pub struct TomlFilterConfig {
    #[serde(default)]
    #[allow(dead_code)]
    pub description: Option<String>,
    pub match_command: String,
    #[serde(default)]
    pub strip_ansi: Option<bool>,
    #[serde(default)]
    pub strip_tui_frames: Option<bool>,
    #[serde(default)]
    pub strip_lines_matching: Option<Vec<String>>,
    #[serde(default)]
    pub keep_lines_matching: Option<Vec<String>>,
    #[serde(default)]
    pub replace: Option<Vec<TomlReplaceRule>>,
    #[serde(default)]
    pub short_circuit: Option<Vec<TomlShortCircuitRule>>,
    #[serde(default)]
    pub max_lines: Option<usize>,
    #[serde(default)]
    pub truncate_lines_at: Option<usize>,
    #[serde(default)]
    pub on_empty: Option<String>,
}

/// Top-level TOML filter file structure.
#[derive(Debug, Clone, Deserialize)]
pub struct FilterFile {
    #[serde(default)]
    pub schema_version: Option<u32>,
    #[serde(default)]
    pub filters: std::collections::HashMap<String, TomlFilterConfig>,
}

/// Registry that manages filter configuration loading and lookup.
pub struct FilterRegistry {
    /// Filters loaded from project-local `.analyzer/filters.toml`.
    project_filters: FilterFile,
    /// Filters loaded from user-global `~/.config/analyzer/filters.toml`.
    user_filters: FilterFile,
    /// Built-in filters embedded at compile time.
    builtin_filters: FilterFile,
}

const CURRENT_FILTER_SCHEMA_VERSION: u32 = 1;

impl FilterRegistry {
    /// Load filters from the standard three-priority locations.
    pub fn load() -> Self {
        let builtin_filters = Self::load_builtin();
        let user_filters = Self::load_user_config();
        let project_filters = Self::load_project_config();

        Self::validate_schema(&builtin_filters, "builtin");
        Self::validate_schema(&user_filters, "user");
        Self::validate_schema(&project_filters, "project");

        Self {
            project_filters,
            user_filters,
            builtin_filters,
        }
    }

    fn validate_schema(file: &FilterFile, source: &str) {
        if let Some(v) = file.schema_version {
            if v > CURRENT_FILTER_SCHEMA_VERSION {
                eprintln!(
                    "Warning: {} filter schema version {} exceeds current version {}, filters may not work correctly",
                    source, v, CURRENT_FILTER_SCHEMA_VERSION
                );
            }
        }
    }

    /// Create a registry with only built-in filters (for testing).
    #[allow(dead_code)]
    pub fn with_builtin_only() -> Self {
        Self {
            project_filters: FilterFile {
                schema_version: None,
                filters: Default::default(),
            },
            user_filters: FilterFile {
                schema_version: None,
                filters: Default::default(),
            },
            builtin_filters: Self::load_builtin(),
        }
    }

    fn load_builtin() -> FilterFile {
        // Compile-time embedded filter file (assembled by build.rs)
        let content = include_str!(concat!(env!("OUT_DIR"), "/builtin_filters.toml"));
        toml::from_str(content).unwrap_or_else(|e| {
            eprintln!("Warning: failed to parse builtin filters: {}", e);
            FilterFile {
                schema_version: None,
                filters: Default::default(),
            }
        })
    }

    fn load_user_config() -> FilterFile {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("analyzer")
            .join("filters.toml");

        Self::load_file(&config_path)
    }

    fn load_project_config() -> FilterFile {
        // Walk up from CWD looking for .analyzer/filters.toml
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        for ancestor in cwd.ancestors() {
            let candidate = ancestor.join(".analyzer").join("filters.toml");
            if candidate.exists() {
                let loaded = Self::load_file(&candidate);
                if !loaded.filters.is_empty() {
                    return loaded;
                }
            }
        }
        FilterFile {
            schema_version: None,
            filters: Default::default(),
        }
    }

    fn load_file(path: &PathBuf) -> FilterFile {
        match std::fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!(
                    "Warning: failed to parse filter config {}: {}",
                    path.display(),
                    e
                );
                FilterFile {
                    schema_version: None,
                    filters: Default::default(),
                }
            }),
            Err(_) => FilterFile {
                schema_version: None,
                filters: Default::default(),
            },
        }
    }

    /// Find the best matching filter for a given command.
    /// Priority: project > user > builtin.
    pub fn find_filter(&self, command: &str) -> Option<&TomlFilterConfig> {
        // Check project filters first
        for config in self.project_filters.filters.values() {
            let re = regex::Regex::new(&config.match_command).ok()?;
            if re.is_match(command) {
                return Some(config);
            }
        }

        // Then user filters
        for config in self.user_filters.filters.values() {
            let re = regex::Regex::new(&config.match_command).ok()?;
            if re.is_match(command) {
                return Some(config);
            }
        }

        // Finally built-in filters
        for config in self.builtin_filters.filters.values() {
            let re = regex::Regex::new(&config.match_command).ok()?;
            if re.is_match(command) {
                return Some(config);
            }
        }

        None
    }

    /// Find a filter by its name (e.g. "turbo").
    #[allow(dead_code)]
    pub fn find_filter_by_name(&self, name: &str) -> Option<&TomlFilterConfig> {
        self.project_filters
            .filters
            .get(name)
            .or_else(|| self.user_filters.filters.get(name))
            .or_else(|| self.builtin_filters.filters.get(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_builtin_turbo_filter() {
        let registry = FilterRegistry::with_builtin_only();
        let filter = registry.find_filter("turbo run lint");
        assert!(
            filter.is_some(),
            "Built-in turbo filter should match 'turbo run lint'"
        );
        let filter = filter.unwrap();
        assert!(filter.strip_tui_frames.unwrap_or(false));
        assert!(filter.strip_ansi.unwrap_or(false));
    }

    #[test]
    fn test_parse_builtin_turbo_with_pnpm() {
        let registry = FilterRegistry::with_builtin_only();
        let filter = registry.find_filter("pnpm exec turbo run lint");
        assert!(filter.is_some(), "Should match pnpm exec turbo");
    }

    #[test]
    fn test_find_nonexistent_filter() {
        let registry = FilterRegistry::with_builtin_only();
        let filter = registry.find_filter("nonexistent_command");
        assert!(filter.is_none());
    }

    #[test]
    fn test_find_filter_by_name() {
        let registry = FilterRegistry::with_builtin_only();
        let filter = registry.find_filter_by_name("turbo");
        assert!(filter.is_some());
        assert_eq!(
            filter.unwrap().description.as_deref().unwrap_or(""),
            "Strip Turborepo TUI decoration, keep task results"
        );
    }

    #[test]
    fn test_parse_toml_replace_rules() {
        let registry = FilterRegistry::with_builtin_only();
        let filter = registry.find_filter_by_name("turbo").unwrap();
        // Replace rules are optional - package prefix stripping is handled by NpmParser
        let replace_rules = filter.replace.as_ref();
        assert!(replace_rules.is_none() || replace_rules.unwrap().is_empty());
    }

    #[test]
    fn test_parse_toml_short_circuit() {
        let registry = FilterRegistry::with_builtin_only();
        let filter = registry.find_filter_by_name("turbo").unwrap();
        let circuits = filter.short_circuit.as_ref().unwrap();
        assert!(!circuits.is_empty());
    }
}
