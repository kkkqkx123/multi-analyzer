//! Ruby Plugin
//! Provide analysis support for Ruby projects (RuboCop, RSpec, Rake, Minitest)

pub mod analyzer;
pub mod parser;

pub use analyzer::RubyAnalyzer;
