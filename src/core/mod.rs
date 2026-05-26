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

pub use types::*;
pub use parser::BaseParser;
#[allow(unused_imports)]
pub use parser::BlockCollector;
#[allow(unused_imports)]
pub use parser::BlockIter;
pub use parser::OutputParser;
pub use parser::ParseResult;
pub use analyzer::*;
pub use reporter::*;
#[allow(unused_imports)]
pub use config::*;
pub use command::CommandBuilder;
#[allow(unused_imports)]
pub use command::CommandOutput;
#[allow(unused_imports)]
pub use command::RunOptions;
pub use test_analyzer::*;
pub use utils::*;
pub use stream::run_analysis_pipeline;
