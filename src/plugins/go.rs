//! Go Plugin
//! Provide analysis support for Go projects

pub mod analyzer;
pub mod golangci_lint;
pub mod parser;

pub use analyzer::GoAnalyzer;
pub use golangci_lint::GolangciLintAnalyzer;
