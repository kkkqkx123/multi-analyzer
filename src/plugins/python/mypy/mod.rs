//! Mypy plugin
//! Provides support for analyzing Python/Mypy projects.

pub mod analyzer;
pub mod parser;

pub use analyzer::MypyAnalyzer;
