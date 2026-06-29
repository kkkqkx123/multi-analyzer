use serde::{Deserialize, Serialize};

fn default_grep_max_results() -> usize {
    200
}

fn default_grep_max_per_file() -> usize {
    25
}

fn default_status_max_files() -> usize {
    15
}

fn default_status_max_untracked() -> usize {
    10
}

fn default_passthrough_max_chars() -> usize {
    2000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    #[serde(default = "default_grep_max_results")]
    pub grep_max_results: usize,
    #[serde(default = "default_grep_max_per_file")]
    pub grep_max_per_file: usize,
    #[serde(default = "default_status_max_files")]
    pub status_max_files: usize,
    #[serde(default = "default_status_max_untracked")]
    pub status_max_untracked: usize,
    #[serde(default = "default_passthrough_max_chars")]
    pub passthrough_max_chars: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            grep_max_results: default_grep_max_results(),
            grep_max_per_file: default_grep_max_per_file(),
            status_max_files: default_status_max_files(),
            status_max_untracked: default_status_max_untracked(),
            passthrough_max_chars: default_passthrough_max_chars(),
        }
    }
}
