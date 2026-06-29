use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const DEFAULT_MAX_FILES: usize = 20;
const DEFAULT_MAX_FILE_SIZE: usize = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TeeMode {
    #[default]
    Failures,
    Always,
    Never,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeConfig {
    #[serde(default = "default_tee_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub mode: TeeMode,
    #[serde(default = "default_tee_max_files")]
    pub max_files: usize,
    #[serde(default = "default_tee_max_file_size")]
    pub max_file_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<PathBuf>,
}

fn default_tee_enabled() -> bool {
    true
}

fn default_tee_max_files() -> usize {
    DEFAULT_MAX_FILES
}

fn default_tee_max_file_size() -> usize {
    DEFAULT_MAX_FILE_SIZE
}

impl Default for TeeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: TeeMode::default(),
            max_files: DEFAULT_MAX_FILES,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            directory: None,
        }
    }
}
