//! MSVC Analyzer Module
//! Provides analysis support for Microsoft Visual C++ compiler

pub mod analyzer;
pub mod parser;

pub use analyzer::MsvcAnalyzer;
