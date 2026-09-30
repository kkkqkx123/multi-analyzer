//! Clang-Check Analyzer
//! Runs `clang-check --analyze` to perform clang's static-analysis pass on a
//! set of source files. Falls back to `clang++ -fsyntax-only` when
//! clang-check itself is not available (e.g. when only libclang is installed).
//!
//! Reuses the Clang/GCC-style parser because clang-check emits diagnostics in
//! the same `file:line:col: level: message` format as clang itself.

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::clang::parser::ClangParser;

/// Fallback command when `clang-check` binary is not available.
const FALLBACK_EXEC: &str = "clang++";
const FALLBACK_FLAG: &str = "-fsyntax-only";

pub struct ClangCheckAnalyzer {
    parser: ClangParser,
}

impl ClangCheckAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: ClangParser::new(),
        }
    }

    /// Returns `true` when the given executable exists on PATH.
    fn command_exists(exec: &str) -> bool {
        std::process::Command::new(exec)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        // Prefer `clang-check` when present, fall back to `clang++ -fsyntax-only`.
        let mut builder = if Self::command_exists("clang-check") {
            CommandBuilder::new("clang-check").arg("--analyze")
        } else {
            CommandBuilder::new(FALLBACK_EXEC).arg(FALLBACK_FLAG)
        };

        if let Some(ref sub) = options.subcommand {
            for arg in sub.as_str().split_whitespace() {
                builder = builder.arg(arg);
            }
        }

        // Compilation database (-p <build-dir>) when available.
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

impl Default for ClangCheckAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for ClangCheckAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::ClangCheck
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["clang-check", "clangcheck"]
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
