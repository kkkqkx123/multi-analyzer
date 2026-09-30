//! Include-What-You-Use Analyzer
//! Runs `include-what-you-use` (a clang plugin that checks whether
//! each #include is necessary and whether required headers are present).
//!
//! Reuses the Clang/GCC-style parser because IWYU emits diagnostics in the
//! same `file:line:col: level: message` format as clang itself.

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::clang::parser::ClangParser;

/// Default subcommand when none is provided. The user typically overrides
/// this via their own subcommand string (e.g. adding a compile database
/// flag with `-p build`).
const DEFAULT_IWYU_ARGS: &str = "--transitive_includes_only";

pub struct IwyuAnalyzer {
    parser: ClangParser,
}

impl IwyuAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: ClangParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("include-what-you-use");

        if let Some(ref sub) = options.subcommand {
            for arg in sub.as_str().split_whitespace() {
                builder = builder.arg(arg);
            }
        } else {
            // Sensible default: only report transitive-include issues so that
            // the output stays focused on headers the file truly needs.
            builder = builder.arg(DEFAULT_IWYU_ARGS);
        }

        // Compilation database (-p <build-dir>) if the user set --build-dir
        // and didn't already pass -p / --p=.
        let sub_str = options
            .subcommand
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("");
        let has_explicit_p = sub_str.contains("-p ") || sub_str.contains("-p=");
        if let Some(ref dir) = options.build_dir {
            if !has_explicit_p {
                builder = builder.arg("-p").arg(dir);
            }
        }

        // C++ standard
        if let Some(ref std_ver) = options.cpp_standard {
            builder = builder.arg(format!("-std={}", std_ver));
        }

        // Include paths
        for include_path in &options.include_paths {
            builder = builder.arg("-I").arg(include_path);
        }

        // Macro definitions
        for define in &options.defines {
            builder = builder.arg(format!("-D{}", define));
        }

        // Target files
        for file in &options.target_files {
            builder = builder.arg(file);
        }

        // Working directory
        if let Some(ref dir) = options.source_dir {
            builder = builder.current_dir(dir);
        }

        builder
    }
}

impl Default for IwyuAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for IwyuAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::IncludeWhatYouUse
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["include-what-you-use", "iwyu"]
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
