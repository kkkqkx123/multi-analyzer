//! Clang-Tidy Analyzer
//! Runs clang-tidy commands (preferring --format=json) and parses output.

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::parser::ClangTidyParser;

pub struct ClangTidyAnalyzer {
    parser: ClangTidyParser,
}

impl ClangTidyAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: ClangTidyParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("clang-tidy");

        if let Some(ref sub) = options.subcommand {
            // User-provided subcommand: pass through verbatim, but still
            // auto-prefix JSON format if they didn't specify one.
            let cmd = sub.as_str();
            let has_explicit_format = cmd.contains("--format");

            if !has_explicit_format {
                builder = builder.arg("--format=json");
            }

            // Forward each whitespace-separated token as its own arg so that
            // shell quoting (e.g. `--checks=-*,bugprone-*`) is preserved as a
            // single argument.
            for arg in cmd.split_whitespace() {
                builder = builder.arg(arg);
            }
        } else {
            // Default: JSON format + scan current directory.
            builder = builder.arg("--format=json").arg("--use-color=false").arg(".");
        }

        // clang-tidy needs compile_commands.json for correct parsing of
        // includes / defines / c++ standard. If the user set --build-dir and
        // didn't already pass -p / --p=, inject it automatically.
        let sub_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("");
        let has_explicit_p = sub_str.contains("-p") || sub_str.contains("--p=");
        if let Some(ref dir) = options.build_dir {
            if !has_explicit_p {
                builder = builder.arg("-p").arg(dir);
            }
        }

        // Working directory: source_dir if given, else nothing (clang-tidy
        // defaults to the current directory).
        if let Some(ref dir) = options.source_dir {
            builder = builder.current_dir(dir);
        }

        builder
    }
}

impl Default for ClangTidyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for ClangTidyAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::ClangTidy
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["clang-tidy", "clangtidy", "tidy"]
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
}
