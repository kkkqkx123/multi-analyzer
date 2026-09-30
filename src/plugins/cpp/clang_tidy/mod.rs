//! Clang-Tidy Analyzer Module
//! Provides clang-tidy static-analysis support for C/C++

pub mod analyzer;
pub mod parser;

pub use analyzer::ClangTidyAnalyzer;
