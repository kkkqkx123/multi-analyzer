//! .NET Plugin
//! Provide analysis support for .NET projects

pub mod parser;
pub mod analyzer;

pub use analyzer::DotnetAnalyzer;
