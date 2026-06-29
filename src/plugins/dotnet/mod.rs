//! .NET Plugin
//! Provide analysis support for .NET projects

pub mod analyzer;
pub mod parser;

pub use analyzer::DotnetAnalyzer;
