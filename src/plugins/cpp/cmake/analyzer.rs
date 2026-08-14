//! CMake Analyzer
//! Runs CMake commands and parses output

use crate::core::{
    run_analyzer, AnalysisResult, AnalyzeOptions, AnalyzerError, BuildAnalyzer, CommandBuilder,
    OutputParser, TechStack,
};

use super::parser::CMakeParser;

pub struct CMakeAnalyzer {
    parser: CMakeParser,
}

impl CMakeAnalyzer {
    pub fn new() -> Self {
        Self {
            parser: CMakeParser::new(),
        }
    }

    fn create_command_builder(&self, options: &AnalyzeOptions) -> CommandBuilder {
        let mut builder = CommandBuilder::new("cmake");

        // A user-supplied subcommand takes precedence and is passed through
        // verbatim (e.g. `analyzer cmake "--build build"`).
        if let Some(sub) = options.subcommand.as_ref() {
            for arg in sub.as_str().split_whitespace() {
                builder = builder.arg(arg);
            }
            return builder;
        }

        // Without a subcommand the command is assembled from the C++ build
        // options (cpp-support-design.md section 4.1):
        //   - configure mode:  cmake -S <src> -B <build> [-G <gen>]
        //   - build mode:      cmake --build <build> [--target <target>]
        let source_dir = options.source_dir.as_deref().unwrap_or(".");
        let build_dir = options.build_dir.as_deref().unwrap_or("build");

        if options.source_dir.is_some() || options.cmake_generator.is_some() {
            builder = builder.arg("-S").arg(source_dir).arg("-B").arg(build_dir);
            if let Some(ref gen) = options.cmake_generator {
                builder = builder.arg("-G").arg(gen);
            }
        } else {
            builder = builder.arg("--build").arg(build_dir);
            if let Some(ref target) = options.target {
                builder = builder.arg("--target").arg(target);
            }
        }

        builder
    }
}

impl Default for CMakeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAnalyzer for CMakeAnalyzer {
    fn tech_stack(&self) -> TechStack {
        TechStack::CMake
    }

    fn supported_commands(&self) -> Vec<&str> {
        vec!["cmake", "cmake-build"]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SubCommand;

    fn analyze_options(subcommand: Option<&str>) -> AnalyzeOptions {
        let mut o = AnalyzeOptions::default();
        if let Some(s) = subcommand {
            o.subcommand = Some(SubCommand::new(s.to_string()));
        }
        o
    }

    /// Materialize the command as (program, args) for assertions.
    fn cmd_args(builder: &CommandBuilder) -> (String, Vec<String>) {
        let cmd = builder.build();
        (
            cmd.get_program().to_string_lossy().to_string(),
            cmd.get_args()
                .map(|a| a.to_string_lossy().to_string())
                .collect(),
        )
    }

    fn assert_builder(builder: &CommandBuilder, expected_args: &[&str]) {
        let (prog, args) = cmd_args(builder);
        assert!(prog.ends_with("cmake"), "unexpected program: {}", prog);
        assert_eq!(args, expected_args);
    }

    #[test]
    fn test_subcommand_passthrough() {
        let analyzer = CMakeAnalyzer::new();
        let builder = analyzer.create_command_builder(&analyze_options(Some("--build build")));
        assert_builder(&builder, &["--build", "build"]);
    }

    #[test]
    fn test_default_is_build_mode() {
        let analyzer = CMakeAnalyzer::new();
        let builder = analyzer.create_command_builder(&analyze_options(None));
        assert_builder(&builder, &["--build", "build"]);
    }

    #[test]
    fn test_build_dir_option() {
        let mut options = analyze_options(None);
        options.build_dir = Some("cmake-out".to_string());
        let builder = CMakeAnalyzer::new().create_command_builder(&options);
        assert_builder(&builder, &["--build", "cmake-out"]);
    }

    #[test]
    fn test_build_target_option() {
        let mut options = analyze_options(None);
        options.build_dir = Some("out".to_string());
        options.target = Some("myapp".to_string());
        let builder = CMakeAnalyzer::new().create_command_builder(&options);
        assert_builder(&builder, &["--build", "out", "--target", "myapp"]);
    }

    #[test]
    fn test_configure_mode_from_source_dir() {
        let mut options = analyze_options(None);
        options.source_dir = Some("src".to_string());
        options.build_dir = Some("build".to_string());
        let builder = CMakeAnalyzer::new().create_command_builder(&options);
        assert_builder(&builder, &["-S", "src", "-B", "build"]);
    }

    #[test]
    fn test_configure_mode_from_generator() {
        let mut options = analyze_options(None);
        options.cmake_generator = Some("Unix Makefiles".to_string());
        let builder = CMakeAnalyzer::new().create_command_builder(&options);
        // Generator keeps spaces as a single argument.
        assert_builder(&builder, &["-S", ".", "-B", "build", "-G", "Unix Makefiles"]);
    }

    #[test]
    fn test_configure_mode_defaults() {
        let mut options = analyze_options(None);
        options.cmake_generator = Some("Ninja".to_string());
        let builder = CMakeAnalyzer::new().create_command_builder(&options);
        assert_builder(&builder, &["-S", ".", "-B", "build", "-G", "Ninja"]);
    }
}
