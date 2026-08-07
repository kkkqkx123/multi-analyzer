//! Command discovery and rewrite engine.
//!
//! Maps raw shell commands (e.g. "cargo check --all-targets") to
//! their corresponding analyzer technology stack and subcommand.
//!
//! ## Modules
//!
//! - `rules`: Static RULES table mapping command patterns to TechStack
//! - `lexer`: Compound command splitting (&&, ||, ;, |, &)
//! - `registry`: classify_command() + rewrite_command() engine

pub mod lexer;
pub mod registry;
pub mod rules;

// Re-export the engine's public surface so callers can use
// `analyzer::discover::classify_command(..)` without reaching into
// the `registry` submodule.
pub use registry::{
    classify_command, classify_command_with_config, rewrite_command, rewrite_command_with_config,
    Classification,
};

pub fn print_rules_stats() {
    println!();
    println!("--- Discover Engine ---");
    println!("Total rules:     {}", rules::flat_rule_count());
    println!("Total rule sets: {}", rules::total_rule_count());
    println!();
    println!("Categories:");
    for cat in rules::all_categories() {
        let by_cat = rules::find_rules_by_category(cat);
        println!("  {:12}  {} rule(s)", cat, by_cat.len());
    }
}
