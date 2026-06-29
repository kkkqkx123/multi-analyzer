//! Analyzer - Multilingual Build Tool Error Analyzer
//!
//! Library interface for integration testing and external calls

pub mod config;
pub mod core;
pub mod discover;
pub mod plugins;

// Re-export common types
pub use core::{
    AnalyzeOptions, AnalyzerError, BaseParser, BuildAnalyzer, CommandBuilder, CommandOutput, Issue,
    IssueLevel, Location, OutputParser, ReportFormat, SubCommand,
};

pub use discover::rewrite_command;
pub use discover::split_on_operators;
