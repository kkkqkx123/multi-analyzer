use serde::{Deserialize, Serialize};

fn default_report_format() -> String {
    "markdown".to_string()
}

fn default_verbosity() -> String {
    "normal".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    #[serde(default = "default_report_format")]
    pub format: String,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default = "default_verbosity")]
    pub verbosity: String,
    #[serde(default)]
    pub success_short_circuit: bool,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            format: default_report_format(),
            verbose: false,
            verbosity: default_verbosity(),
            success_short_circuit: true,
        }
    }
}