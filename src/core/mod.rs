//! Core Module
//! Provide traits and types that are common to all technology stacks

pub mod types;
pub mod parser;
pub mod analyzer;
pub mod reporter;
pub mod command;
pub mod test_analyzer;
pub mod utils;
pub mod config;
pub mod stream;
pub mod tee;
pub mod tracking;

pub use types::*;
pub use parser::*;
pub use analyzer::*;
pub use reporter::*;
// Config types are available for TOML-based configuration (not yet wired up)
#[allow(unused_imports)]
pub use config::*;
pub use command::CommandBuilder;
// CommandOutput is exported for testing and external use
#[allow(unused_imports)]
pub use command::CommandOutput;
// RunOptions is exported for external use
#[allow(unused_imports)]
pub use command::RunOptions;
pub use test_analyzer::*;
pub use utils::*;
