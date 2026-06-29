//! Gradle Plugin
//! Provides support for analyzing Java/Gradle projects

pub mod analyzer;
pub mod parser;

pub use analyzer::GradleAnalyzer;
