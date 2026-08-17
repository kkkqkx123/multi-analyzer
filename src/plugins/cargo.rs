//! Cargo 插件
//! Provide analysis support for Rust/Cargo projects

pub mod analyzer;
pub mod parser;

pub use analyzer::CargoAnalyzer;
