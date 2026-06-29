use serde::{Deserialize, Serialize};

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    pub exec: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tech_stacks: Vec<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}
