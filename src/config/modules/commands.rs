use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    pub exec: String,
    #[serde(default)]
    pub tech_stacks: Vec<String>,
}