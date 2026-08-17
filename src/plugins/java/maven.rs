//! Maven 插件
//! Provides support for analyzing Java/Maven projects

pub mod analyzer;
pub mod parser;

pub use analyzer::MavenAnalyzer;
