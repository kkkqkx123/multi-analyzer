//! Ruby Plugin
//! Provide analysis support for Ruby projects (RuboCop, RSpec, Rake, Minitest)

pub mod parser;
pub mod analyzer;

pub use analyzer::RubyAnalyzer;
