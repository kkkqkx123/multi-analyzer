//! Ruby Analyzer
//! Run Ruby-related commands and parse the output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack, TestAnalyzer, TestOptions, TestOutputParser,
};

use super::parser::RubyParser;

pub struct RubyAnalyzer {
    parser: RubyParser,
    stack: TechStack,
}

impl RubyAnalyzer {
    pub fn new() -> Self {
        Self::with_stack(TechStack::Rubocop)
    }

    /// Create an analyzer bound to a specific Ruby tech stack.
    ///
    /// `TechStack::Rubocop` covers rubocop/ruby/rails aliases; `TechStack::Rspec`
    /// covers the rspec tech stack entry.
    pub fn with_stack(stack: TechStack) -> Self {
        Self {
            parser: RubyParser::new(),
            stack,
        }
    }

    pub fn rspec() -> Self {
        Self::with_stack(TechStack::Rspec)
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let command_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("");
        let raw_stack = options.raw_tech_stack.as_deref().unwrap_or("");
        let tokens: Vec<&str> = command_str.split_whitespace().collect();
        let first = tokens
            .first()
            .copied()
            .unwrap_or("")
            .to_lowercase();
        let has_format = command_str.contains("--format") || command_str.contains("-f");

        let mut builder = CommandBuilder::new("bundle");
        builder = builder.arg("exec");

        match first.as_str() {
            // Subcommand already starts with a known Ruby tool (e.g.
            // `analyzer ruby "rubocop ."` / `analyzer ruby "rails server -p 8000"`).
            "rubocop" | "rspec" | "rake" | "rails" | "ruby" => {
                for t in &tokens {
                    builder = builder.arg(*t);
                }
                if !has_format && (first == "rubocop" || first == "rspec") {
                    builder = builder.arg("--format").arg("json");
                }
            }
            // Subcommand starts with `bundle` (e.g. `bundle exec rspec spec`).
            // Rebuild as `bundle exec <rest>` avoiding a duplicated `exec`.
            "bundle" => {
                let skip_exec = tokens
                    .get(1)
                    .map(|t| t.eq_ignore_ascii_case("exec"))
                    .unwrap_or(false);
                for (i, t) in tokens.iter().enumerate() {
                    if i == 0 {
                        continue;
                    }
                    if i == 1 && skip_exec {
                        continue;
                    }
                    builder = builder.arg(*t);
                }
                let next = tokens.get(if skip_exec { 2 } else { 1 });
                if !has_format {
                    if let Some(a) = next {
                        let al = a.to_lowercase();
                        if al == "rubocop" || al == "rspec" {
                            builder = builder.arg("--format").arg("json");
                        }
                    }
                }
            }
            // Bare arguments: recover the tool name from the raw tech stack
            // alias (e.g. `analyzer rails "server -p 8000"`, or run-mode
            // rewrites like `analyzer rubocop "."`).
            _ => {
                let tool = match raw_stack {
                    "rubocop" | "ruby" | "rails" | "rspec" | "rake" => raw_stack,
                    _ => "",
                };
                if !tool.is_empty() {
                    builder = builder.arg(tool);
                }
                for t in &tokens {
                    builder = builder.arg(*t);
                }
                if !has_format && (tool == "rubocop" || tool == "rspec") {
                    builder = builder.arg("--format").arg("json");
                }
            }
        }

        builder
    }
}

impl Default for RubyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for RubyAnalyzer {
    fn tech_stack(&self) -> TechStack {
        self.stack
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["ruby", "rails", "rubocop", "rspec", "rake"]
    }

    fn analyze(&self, options: &AnalyzeOptions) -> Result<AnalysisResult, AnalyzerError> {
        let builder = self.create_command_builder(options);
        let result = run_analyzer(&builder, &self.parser, options)?;
        eprintln!("Found {} issues", result.total_issues);
        Ok(result)
    }

    fn parser(&self) -> &dyn OutputParser {
        &self.parser
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_test_analyzer(&self) -> Option<&dyn TestAnalyzer> {
        Some(self)
    }
}

impl TestAnalyzer for RubyAnalyzer {
    fn supports_test(&self) -> bool {
        true
    }

    fn build_test_command(&self, options: &TestOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("bundle");
        builder = builder.arg("exec");

        let command_str = if options.command.is_empty() {
            "rspec"
        } else {
            &options.command
        };

        for arg in command_str.split_whitespace() {
            builder = builder.arg(arg);
        }

        // Auto-inject JSON format for RSpec
        if command_str.starts_with("rspec")
            && !command_str.contains("--format")
            && !command_str.contains("-f")
        {
            builder = builder.arg("--format").arg("json");
        }

        if let Some(ref filter) = options.filter {
            builder = builder.arg("--example").arg(filter);
        }

        builder
    }

    fn test_parser(&self) -> Option<&dyn TestOutputParser> {
        Some(&self.parser)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SubCommand;

    #[test]
    fn test_ruby_analyzer_name() {
        let analyzer = RubyAnalyzer::new();
        assert_eq!(analyzer.name(), "rubocop");
    }

    #[test]
    fn test_ruby_analyzer_supported_commands() {
        let analyzer = RubyAnalyzer::new();
        let commands = analyzer.supported_commands();
        assert!(commands.contains(&"ruby"));
        assert!(commands.contains(&"rspec"));
    }

    #[test]
    fn test_ruby_analyzer_rspec_stack() {
        let analyzer = RubyAnalyzer::rspec();
        assert_eq!(analyzer.tech_stack(), TechStack::Rspec);
        assert_eq!(analyzer.name(), "rspec");
    }

    fn build_cmd(subcommand: &str, raw_stack: &str) -> String {
        let analyzer = RubyAnalyzer::new();
        let opts = AnalyzeOptions {
            raw_tech_stack: Some(raw_stack.to_string()),
            subcommand: Some(SubCommand::new(subcommand)),
            ..Default::default()
        };
        analyzer.create_command_builder(&opts).command_string()
    }

    #[test]
    fn test_command_builder_known_tool() {
        // analyzer ruby "rubocop ." → bundle exec rubocop . --format json
        assert_eq!(
            build_cmd("rubocop .", "ruby"),
            "bundle exec rubocop . --format json"
        );
        // analyzer ruby "rails server -p 8000" → as-is
        assert_eq!(
            build_cmd("rails server -p 8000", "ruby"),
            "bundle exec rails server -p 8000"
        );
        // analyzer ruby "ruby app.rb" → as-is (no json injection)
        assert_eq!(build_cmd("ruby app.rb", "ruby"), "bundle exec ruby app.rb");
        // user already specified a format → no double injection
        assert_eq!(
            build_cmd("rubocop --format json .", "ruby"),
            "bundle exec rubocop --format json ."
        );
    }

    #[test]
    fn test_command_builder_bare_args_recover_tool() {
        // analyzer rails "server -p 8000" → recover "rails" from alias
        assert_eq!(
            build_cmd("server -p 8000", "rails"),
            "bundle exec rails server -p 8000"
        );
        // run-mode rewrite: analyzer rubocop "." → recover tool + json
        assert_eq!(
            build_cmd(".", "rubocop"),
            "bundle exec rubocop . --format json"
        );
        // run-mode rewrite: analyzer rspec "spec" → recover tool + json
        assert_eq!(build_cmd("spec", "rspec"), "bundle exec rspec spec --format json");
    }

    #[test]
    fn test_command_builder_bundle_prefix() {
        // subcommand starting with bundle → avoid duplicated exec
        assert_eq!(
            build_cmd("bundle exec rspec spec", "ruby"),
            "bundle exec rspec spec --format json"
        );
    }
}
