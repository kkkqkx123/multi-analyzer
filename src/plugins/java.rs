//! Java Analyzer Module
//! Provides analysis support for Java build tools (maven, gradle)

pub mod gradle;
pub mod maven;

pub use gradle::GradleAnalyzer;
pub use maven::MavenAnalyzer;
