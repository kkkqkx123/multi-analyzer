use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::CommandConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TechStackConfig {
    #[serde(default)]
    pub commands: HashMap<String, CommandConfig>,
    #[serde(default)]
    pub scripts: HashMap<String, String>,
    #[serde(default)]
    pub test_framework: Option<String>,
}
