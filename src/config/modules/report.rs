use serde::{Deserialize, Serialize};

fn default_report_format() -> String {
    "markdown".to_string()
}

fn default_verbosity() -> String {
    "normal".to_string()
}

fn default_success_short_circuit() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    #[serde(default = "default_report_format")]
    pub format: String,
    #[serde(default = "default_verbosity")]
    pub verbosity: String,
    #[serde(default = "default_success_short_circuit")]
    pub success_short_circuit: bool,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            format: default_report_format(),
            verbosity: default_verbosity(),
            success_short_circuit: default_success_short_circuit(),
        }
    }
}
