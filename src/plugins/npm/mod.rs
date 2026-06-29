//! NPM Plugin
//! Provide support for analyzing Node.js/npm/pnpm/yarn projects

pub mod analyzer;
pub mod parser;

pub use analyzer::NpmAnalyzer;
