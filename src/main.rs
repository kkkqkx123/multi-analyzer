//! Analyzer - Multilingual Build Tool Error Analyzer
//!
//! Usage: analyzer <tech-stack> <command> [options]
//!
//! Examples:
//!   analyzer cargo "check"
//!   analyzer npm "run typecheck"
//!   analyzer pnpm "exec tsc --noEmit"

use std::env;
use std::path::Path;

// Reuse the library crate instead of recompiling its modules into the binary.
// Recompiling via `mod` made every `pub` item a private, "dead" symbol inside
// the bin, which produced spurious `never used` warnings.
use analyzer::{
    config,
    core::{
        self, AnalysisResult, AnalyzeOptions, ReporterFactory, SubCommand, TechStack, TestOptions,
        Verbosity,
    },
    discover, plugins,
};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        show_help();
        std::process::exit(1);
    }

    // Capture the original first token so the supportability check below is
    // stable even after `run` rewrites the argument vector.
    let original_first = args[1].clone();

    if args[1] == "config" {
        handle_config(&args);
        return;
    }

    // --- Stats subcommand ---
    if args[1] == "stats" {
        handle_stats(&args);
        return;
    }

    // Load configuration from file (global + project merge)
    let config = config::ConfigLoader::new().load();

    // --- Discovery subcommands ---
    // `rewrite` prints the equivalent invocation without running it, while
    // `run` resolves the tech stack and continues into the normal pipeline.
    if args[1] == "rewrite" {
        handle_rewrite(&args, &config);
        return;
    }

    let args = if args[1] == "run" {
        match resolve_run_args(&args, &config) {
            Some(resolved) => resolved,
            // resolve_run_args already reported the reason.
            None => std::process::exit(1),
        }
    } else {
        args
    };

    // Direct/resolved mode: the first token must be a supported tech stack.
    // An unknown first token (neither a known subcommand nor a valid tech
    // stack, and not a global flag) is reported as "subcommand not supported"
    // with exit code 2, per the reference documentation.
    if !original_first.starts_with('-')
        && !matches!(
            original_first.as_str(),
            "run" | "rewrite" | "config" | "stats"
        )
        && original_first.parse::<TechStack>().is_err()
    {
        eprintln!(
            "Error: unsupported subcommand or tech stack '{}'",
            original_first
        );
        eprintln!("Run 'analyzer --help' to see supported subcommands and tech stacks.");
        std::process::exit(2);
    }

    // Parse arguments (CLI overrides config)
    let (tech_stack, options) = parse_arguments(&args, &config);

    run_orchestrator(tech_stack, options, &config);
}

/// Resolve `analyzer run "<raw command>"` into the equivalent direct-mode
/// argument vector: `analyzer <tech-stack> "<subcommand>" [options]`.
///
/// Returns `None` when no rule matches, after printing the reason to stderr.
fn resolve_run_args(args: &[String], config: &config::AppConfig) -> Option<Vec<String>> {
    let raw_cmd = match args.get(2) {
        Some(cmd) if !cmd.trim().is_empty() => cmd,
        _ => {
            eprintln!("Error: 'run' requires a command, e.g. analyzer run \"cargo check\"");
            return None;
        }
    };

    let (tech_stack, subcommand, extra_args) = resolve_raw_command(raw_cmd, config)?;

    // Rebuild the argv as if the user had typed the direct form, then append
    // any analyzer flags that followed the command string.
    let mut resolved = vec![args[0].clone(), tech_stack.as_str().to_string()];
    let mut command = subcommand;
    for extra in extra_args {
        command.push(' ');
        command.push_str(&extra);
    }
    resolved.push(command);
    resolved.extend(args.iter().skip(3).cloned());

    Some(resolved)
}

/// Print the analyzer-equivalent form of a raw shell command without running it.
fn handle_rewrite(args: &[String], config: &config::AppConfig) {
    let raw_cmd = match args.get(2) {
        Some(cmd) if !cmd.trim().is_empty() => cmd,
        _ => {
            eprintln!("Error: 'rewrite' requires a command, e.g. analyzer rewrite \"cargo check\"");
            std::process::exit(1);
        }
    };

    let Some((tech_stack, subcommand, extra_args)) = resolve_raw_command(raw_cmd, config) else {
        std::process::exit(1);
    };

    let mut command = subcommand;
    for extra in extra_args {
        command.push(' ');
        command.push_str(&extra);
    }

    println!("analyzer {} \"{}\"", tech_stack.as_str(), command);
}

/// Shared resolution step for `run` and `rewrite`.
///
/// Only the first segment of a compound command is classified; the remaining
/// segments are reported so the caller knows they were dropped.
fn resolve_raw_command(
    raw_cmd: &str,
    config: &config::AppConfig,
) -> Option<(TechStack, String, Vec<String>)> {
    let segments = discover::lexer::split_on_operators(raw_cmd);
    let first = segments.first().map(String::as_str).unwrap_or(raw_cmd);

    let resolved = discover::rewrite_command_with_config(first, &config.commands);

    if resolved.is_none() {
        eprintln!(
            "Error: no matching rule for command: {}\n\
             Only build tool commands from supported tech stacks are supported \
             (e.g. \"cargo check\", \"npm run lint\", \"go vet ./...\").",
            first.trim()
        );
        return None;
    }

    if segments.len() > 1 {
        eprintln!(
            "Note: only the first segment was used; {} trailing segment(s) ignored.",
            segments.len() - 1
        );
    }

    resolved
}

fn handle_config(args: &[String]) {
    if args.len() < 3 {
        eprintln!("Error: missing config subcommand");
        eprintln!("Usage: analyzer config <show|init>");
        std::process::exit(1);
    }
    match args[2].as_str() {
        "show" => {
            if let Err(e) = config::AppConfig::show_config() {
                eprintln!("Error showing config: {}", e);
                std::process::exit(1);
            }
        }
        "init" => {
            match config::AppConfig::create_default() {
                Ok(path) => {
                    println!("Created default config at: {}", path.display());
                }
                Err(e) => {
                    eprintln!("Error creating default config: {}", e);
                    std::process::exit(1);
                }
            }
        }
        other => {
            eprintln!("Error: unknown config subcommand '{}'", other);
            eprintln!("Usage: analyzer config <show|init>");
            std::process::exit(1);
        }
    }
}

fn handle_stats(args: &[String]) {
    let reset = args.len() > 2 && (args[2] == "--reset" || args[2] == "reset");
    if reset {
        core::tracking::reset();
        println!("Tracking data reset.");
        return;
    }

    let tracking_summary = core::tracking::stats().summary();
    if tracking_summary.contains("0 total") {
        println!("No analysis runs recorded yet.");
    } else {
        println!("Analysis Tracking Statistics:");
        println!("{}", tracking_summary);
        println!();
        println!("Per-run detail:");
        for (i, r) in core::tracking::records().iter().enumerate() {
            let exit = r
                .exit_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "?".to_string());
            println!(
                "  [{}] {} | {} | exit={} | {}ms | {} issues | {}",
                i + 1,
                r.tech_stack,
                r.command,
                exit,
                r.duration_ms,
                r.issue_count,
                if r.success { "ok" } else { "FAILED" }
            );
        }
        println!();
        println!("Usage: analyzer stats [--reset]");
    }
}

fn run_orchestrator(tech_stack: TechStack, options: AnalyzeOptions, config: &config::AppConfig) {
    // Creating a plug-in registry
    let registry = plugins::create_registry();

    // Get the corresponding analyzer
    let analyzer = match registry.get(tech_stack) {
        Some(a) => a,
        None => {
            eprintln!("Error: Unknown tech stack '{}'", tech_stack.as_str());
            eprintln!("Supported: {}", registry.list().join(", "));
            std::process::exit(1);
        }
    };

    // Check if analyzer is applicable for this project
    if let Err(e) = registry.check_applicable(tech_stack, Path::new(".")) {
        eprintln!("Warning: {}", e);
    }

    // Print supported commands for this analyzer
    if !options.verbosity.is_minimal() {
        eprintln!(
            "Supported commands: {}",
            analyzer.supported_commands().join(", ")
        );
    }

    // Check if this is a test command
    let is_test_command = is_test_subcommand(&options.subcommand);

    // Print subcommand category if available
    if let Some(ref cmd) = options.subcommand {
        if !options.verbosity.is_minimal() {
            eprintln!("Command category: {:?}", cmd.category());
            if cmd.is_custom() {
                eprintln!("Using custom command: {}", cmd.as_str());
            }
        }
    }

    if is_test_command {
        // Resolve test framework from config
        let ts_str = tech_stack.as_str();
        if let Some(framework) = config.test_framework_for(ts_str) {
            if !options.verbosity.is_minimal() {
                eprintln!("Test framework: {}", framework);
            }
        }
        // Run test analysis
        run_test_analysis(analyzer, &options);
    } else {
        // Run regular analysis
        run_analysis(analyzer, &options);
    }

    let tracking_summary = core::tracking::stats().summary();
    if !tracking_summary.contains("0 total") && !options.verbosity.is_minimal() {
        eprintln!("\n{}", tracking_summary);
    }
}

/// Analyzer flags that consume the following argument as their value.
///
/// This table and [`SWITCH_FLAGS`] are the single source of truth for CLI flag
/// metadata. Option parsing and positional-argument extraction both consult
/// them, which keeps a flag's value from being mistaken for a tech stack or a
/// build-tool command.
const VALUE_FLAGS: &[&str] = &[
    "--filter-paths",
    "--output",
    "-o",
    "--file",
    "-f",
    "--format",
    "--max-issues",
    // Cargo workspace / target / feature selection
    "--package",
    "-p",
    "--exclude",
    "--bin",
    "--test",
    "--example",
    "--bench",
    "--features",
    // C++ build options
    "--source-dir",
    "--build-dir",
    "--cmake-generator",
    "--target",
    "--target-files",
    "-I",
    "--include-path",
    "-D",
    "--define",
    "--cpp-std",
];

/// Analyzer flags that are standalone switches, taking no value argument.
const SWITCH_FLAGS: &[&str] = &[
    "--help",
    "-h",
    "--version",
    "-v",
    "--filter-warnings",
    "--verbose",
    "--quiet",
    "-q",
    "--stdout",
    "--no-short-circuit",
    // Cargo workspace / target / feature selection
    "--workspace",
    "--lib",
    "--bins",
    "--tests",
    "--examples",
    "--benches",
    "--all-targets",
    "--all-features",
    "--no-default-features",
];

/// True when `flag` is an option understood by the analyzer itself, as opposed
/// to an argument destined for the underlying build tool.
fn is_known_analyzer_flag(flag: &str) -> bool {
    SWITCH_FLAGS.contains(&flag) || VALUE_FLAGS.contains(&flag)
}

/// True when `flag` consumes the following argument as its value.
fn flag_takes_value(flag: &str) -> bool {
    VALUE_FLAGS.contains(&flag)
}

/// Populate `options` from the analyzer flags found in `args[start_index..]`.
///
/// Positional arguments, unknown flags and build-tool arguments are ignored —
/// extracting those is the caller's job. `--help` and `--version` are control
/// flow rather than options and are likewise left to the caller.
///
/// This function never terminates the process: malformed values are reported
/// through the returned list, which keeps it unit-testable.
fn parse_options_from_args(
    args: &[String],
    start_index: usize,
    options: &mut AnalyzeOptions,
) -> Vec<String> {
    let mut errors = Vec::new();
    let mut i = start_index;

    while i < args.len() {
        let arg = args[i].as_str();
        // Only value-taking flags look ahead; a missing value is tolerated.
        let value = if flag_takes_value(arg) {
            args.get(i + 1)
        } else {
            None
        };

        match arg {
            // Control flow, handled by the caller.
            "--help" | "-h" | "--version" | "-v" => {}

            // === General Options ===
            "--filter-warnings" => options.filter_warnings = true,
            "--verbose" => options.verbosity = Verbosity::Verbose,
            "--quiet" | "-q" => options.verbosity = Verbosity::Minimal,
            "--stdout" => options.stdout_only = true,
            "--no-short-circuit" => options.success_short_circuit = false,
            "--filter-paths" => {
                if let Some(v) = value {
                    options.filter_paths = v.split(',').map(|s| s.trim().to_string()).collect();
                }
            }
            "--output" | "-o" | "--file" | "-f" => {
                if let Some(v) = value {
                    options.output_file = Some(v.clone());
                    // An explicit output path overrides the stdout default,
                    // otherwise the report would only ever reach stdout.
                    options.stdout_only = false;
                }
            }
            "--format" => {
                if let Some(v) = value {
                    match v.parse() {
                        Ok(format) => options.report_format = format,
                        Err(e) => errors.push(format!(
                            "Invalid format '{}': {}\nSupported formats: markdown, json, html, raw, raw-json",
                            v, e
                        )),
                    }
                }
            }
            "--max-issues" => {
                if let Some(v) = value {
                    match v.parse::<usize>() {
                        Ok(n) => options.max_issues = Some(n),
                        Err(_) => errors.push(format!(
                            "Invalid value for --max-issues: '{}' (expected a non-negative integer)",
                            v
                        )),
                    }
                }
            }

            // === Cargo Workspace Options ===
            "--workspace" => options.workspace = true,
            "--package" | "-p" => {
                if let Some(v) = value {
                    options.package.push(v.clone());
                }
            }
            "--exclude" => {
                if let Some(v) = value {
                    options.exclude.push(v.clone());
                }
            }

            // === Cargo Target Options ===
            "--lib" => options.lib = true,
            "--bin" => {
                if let Some(v) = value {
                    options.bin.push(v.clone());
                }
            }
            "--bins" => options.bins = true,
            "--test" => {
                if let Some(v) = value {
                    options.test.push(v.clone());
                }
            }
            "--tests" => options.tests = true,
            "--example" => {
                if let Some(v) = value {
                    options.example.push(v.clone());
                }
            }
            "--examples" => options.examples = true,
            "--bench" => {
                if let Some(v) = value {
                    options.bench.push(v.clone());
                }
            }
            "--benches" => options.benches = true,
            "--all-targets" => options.all_targets = true,

            // === Cargo Feature Options ===
            "--features" => {
                if let Some(v) = value {
                    options.features.push(v.clone());
                }
            }
            "--all-features" => options.all_features = true,
            "--no-default-features" => options.no_default_features = true,

            // === C++ Build Options ===
            "--source-dir" => {
                if let Some(v) = value {
                    options.source_dir = Some(v.clone());
                }
            }
            "--build-dir" => {
                if let Some(v) = value {
                    options.build_dir = Some(v.clone());
                }
            }
            "--cmake-generator" => {
                if let Some(v) = value {
                    options.cmake_generator = Some(v.clone());
                }
            }
            "--target" => {
                if let Some(v) = value {
                    options.target = Some(v.clone());
                }
            }
            "--target-files" => {
                if let Some(v) = value {
                    options.target_files = v.split(',').map(|s| s.trim().to_string()).collect();
                }
            }
            "-I" | "--include-path" => {
                if let Some(v) = value {
                    options.include_paths.push(v.clone());
                }
            }
            "-D" | "--define" => {
                if let Some(v) = value {
                    options.defines.push(v.clone());
                }
            }
            "--cpp-std" => {
                if let Some(v) = value {
                    options.cpp_standard = Some(v.clone());
                }
            }

            // Positional arguments and build-tool flags are not our concern.
            _ => {}
        }

        if value.is_some() {
            i += 1;
        }
        i += 1;
    }

    errors
}

fn parse_arguments(args: &[String], config: &config::AppConfig) -> (TechStack, AnalyzeOptions) {
    let mut tech_stack_str = String::new();
    let mut command_str = String::new();
    // Seed options from configuration file, then let CLI args override
    let mut options = AnalyzeOptions::from_config(config);

    // Phase 1: analyzer options. Pure and unit-testable; never exits.
    let errors = parse_options_from_args(args, 1, &mut options);
    if !errors.is_empty() {
        for err in &errors {
            eprintln!("Error: {}", err);
        }
        std::process::exit(1);
    }

    // Phase 2: control-flow flags and positional arguments.
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_str();

        match arg {
            "--help" | "-h" => {
                show_help();
                std::process::exit(0);
            }
            "--version" | "-v" => {
                if tech_stack_str.is_empty() {
                    println!("analyzer 0.2.0");
                    std::process::exit(0);
                }
                // If -v appears after a tech stack, treat it as a command argument
                // (e.g. "analyzer pytest -v" should pass -v to pytest, not --version)
                if command_str.is_empty() {
                    command_str = arg.to_string();
                }
                i += 1;
                continue;
            }
            _ => {}
        }

        if is_known_analyzer_flag(arg) {
            // Consumed in phase 1; skip the flag together with its value so the
            // value is never mistaken for a tech stack or a command.
            if flag_takes_value(arg) {
                i += 1;
            }
        } else if !arg.starts_with('-') {
            if tech_stack_str.is_empty() {
                tech_stack_str = arg.to_string();
            } else if command_str.is_empty() {
                // Collect the full command string
                command_str = arg.to_string();
            }
        } else if !tech_stack_str.is_empty() && command_str.is_empty() {
            // Unknown flag after tech stack → treat as command
            // This handles: analyzer mypy "--strict ."
            command_str = arg.to_string();
        }

        i += 1;
    }

    if tech_stack_str.is_empty() {
        eprintln!("Error: No tech stack specified");
        show_help();
        std::process::exit(1);
    }

    if command_str.is_empty() {
        eprintln!("Error: No command specified");
        show_help();
        std::process::exit(1);
    }

    // Parse tech stack
    let tech_stack: TechStack = tech_stack_str.parse().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });

    // Look up command aliases in config.commands
    if !config.commands.is_empty() {
        if let Some(cmd_config) = config.commands.get(&command_str) {
            // Check if the command is restricted to specific tech stacks
            if cmd_config.tech_stacks.is_empty() || cmd_config.tech_stacks.contains(&tech_stack_str)
            {
                eprintln!(
                    "Using configured command '{}' for alias '{}'",
                    cmd_config.exec, command_str
                );
                command_str = cmd_config.exec.clone();
            }
        }
    }

    // Resolve script names to actual frameworks via tech_stacks config
    if let Some(resolved) = config.resolve_script(&tech_stack_str, &command_str) {
        eprintln!(
            "Resolved script '{}' to framework '{}' via tech_stacks.{}",
            command_str, resolved, tech_stack_str
        );
        command_str = resolved;
    }

    options.subcommand = Some(SubCommand::new(command_str));
    (tech_stack, options)
}

/// Convert AnalyzeOptions to ReportOptions
fn to_report_options(
    options: &AnalyzeOptions,
    tech_stack: &TechStack,
    subcommand: Option<&SubCommand>,
) -> core::reporter::ReportOptions {
    let tech_stack_name = match subcommand {
        Some(cmd) => format!("{} {}", tech_stack.as_str(), cmd.as_str()),
        None => tech_stack.as_str().to_string(),
    };
    core::reporter::ReportOptions {
        verbose: options.verbosity,
        success_short_circuit: options.success_short_circuit,
        tech_stack: Some(tech_stack_name),
    }
}

fn is_test_subcommand(subcommand: &Option<SubCommand>) -> bool {
    subcommand
        .as_ref()
        .map(|cmd| cmd.as_str().to_lowercase().contains("test"))
        .unwrap_or(false)
}

fn run_analysis(analyzer: &dyn core::BuildAnalyzer, options: &AnalyzeOptions) {
    let subcommand_name = options
        .subcommand
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("default");
    if !options.verbosity.is_minimal() {
        eprintln!(
            "Analyzing project with {} {}...",
            analyzer.name(),
            subcommand_name
        );

        // Use the parser method to demonstrate it's being used
        let _parser = analyzer.parser();
        eprintln!("Using parser: {}", std::any::type_name_of_val(_parser));
    } else {
        let _parser = analyzer.parser();
    }

    // The OutputParser trait is implemented by various parsers
    // and provides line-by-line parsing capabilities via template method pattern

    match analyzer.analyze(options) {
        Ok(result) => {
            if !options.verbosity.is_minimal() {
                eprintln!("\nAnalysis complete!");
                eprintln!("Total issues: {}", result.total_issues);
            }

            // Generating reports
            let reporter = ReporterFactory::create(options.report_format);
            let report_options =
                to_report_options(options, &analyzer.tech_stack(), options.subcommand.as_ref());
            let report = match reporter.generate_with_options(&result, report_options) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to generate report: {}", e);
                    std::process::exit(1);
                }
            };

            if options.stdout_only {
                // Output to stdout only
                println!("{}", report);
            } else {
                // output report
                let default_name = format!("analysis_report.{}", options.report_format.extension());
                let output_path = options.output_file.as_deref().unwrap_or(&default_name);

                if let Err(e) = reporter.write_to_file(&report, Path::new(output_path)) {
                    eprintln!("Failed to write report: {}", e);
                    std::process::exit(1);
                }

                eprintln!("Report written to: {}", output_path);
            }

            // Print summary
            print_summary(&result, options.verbosity);
        }
        Err(e) => {
            eprintln!("Analysis failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_test_analysis(analyzer: &dyn core::BuildAnalyzer, options: &AnalyzeOptions) {
    // Try to get TestAnalyzer from BuildAnalyzer
    let test_analyzer = match analyzer.as_test_analyzer() {
        Some(ta) => ta,
        None => {
            eprintln!("Error: Test analysis not supported for {}", analyzer.name());
            std::process::exit(1);
        }
    };

    if !test_analyzer.supports_test() {
        eprintln!("Error: Test analysis not supported for {}", analyzer.name());
        std::process::exit(1);
    }

    eprintln!("Running tests for {}...", analyzer.name());

    // Convert AnalyzeOptions to TestOptions
    let test_options = TestOptions::from(options);

    match test_analyzer.run_tests(&test_options) {
        Ok(test_output) => {
            eprintln!("\nTest analysis complete!");
            eprintln!("Compile issues: {}", test_output.compile_issues.len());

            if let Some(ref summary) = test_output.test_summary {
                eprintln!(
                    "Tests: {} total, {} passed, {} failed, {} ignored",
                    summary.total, summary.passed, summary.failed, summary.ignored
                );
            }

            // Generate test report
            let reporter = ReporterFactory::create(options.report_format);
            let report_options =
                to_report_options(options, &analyzer.tech_stack(), options.subcommand.as_ref());
            let report = match reporter
                .generate_test_report_with_options(&test_output.into(), report_options)
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to generate report: {}", e);
                    std::process::exit(1);
                }
            };

            if options.stdout_only {
                // Output to stdout only
                println!("{}", report);
            } else {
                let default_name = format!("test_report.{}", options.report_format.extension());
                let output_path = options.output_file.as_deref().unwrap_or(&default_name);

                if let Err(e) = reporter.write_to_file(&report, Path::new(output_path)) {
                    eprintln!("Failed to write report: {}", e);
                    std::process::exit(1);
                }

                eprintln!("Test report written to: {}", output_path);
            }
        }
        Err(e) => {
            eprintln!("Test analysis failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn show_help() {
    println!("analyzer - Multi-language build tool error analyzer");
    println!();
    println!("Usage:");
    println!("  analyzer <tech-stack> <command> [options]");
    println!();
    println!("Subcommands:");
    println!("  run        Auto-detect the tech stack from a raw shell command and analyze it");
    println!("  rewrite    Print the equivalent analyzer command without executing it");
    println!("  config     Show or initialize configuration");
    println!("  stats      Show analysis tracking statistics");
    println!();
    println!("Exit codes:");
    println!("  0  Success");
    println!("  1  Execution failed / no matching rule");
    println!();
    println!("Tech Stacks:");
    println!("  cargo         Rust/Cargo projects            (aliases: rust)");
    println!("  cargo-nextest Rust nextest test framework    (aliases: nextest)");
    println!("  mypy          Python/Mypy projects");
    println!("  pytest        Python/Pytest projects         (aliases: py.test)");
    println!("  ruff          Python linter                  (aliases: python-lint)");
    println!("  black         Python code formatter");
    println!("  npm           Node.js/npm projects           (aliases: node)");
    println!("  pnpm          Node.js/pnpm projects");
    println!("  yarn          Node.js/yarn projects");
    println!("  go            Go projects                    (aliases: golang)");
    println!("  golangci-lint Go linter");
    println!("  dotnet        .NET / C# / MSBuild projects   (aliases: msbuild, csharp)");
    println!("  rubocop       Ruby Rubocop linter            (aliases: ruby, rails)");
    println!("  rspec         Ruby RSpec test framework");
    println!("  maven         Java/Maven projects            (aliases: mvn)");
    println!("  gradle        Java/Gradle projects           (aliases: gradlew)");
    println!("  cmake         C++/CMake projects             (aliases: cmake-build)");
    println!("  gcc           C++/GCC projects               (aliases: g++)");
    println!("  clang         C++/Clang projects             (aliases: clang++)");
    println!("  clang-format  C++/ClangFormat formatter      (aliases: cpp-format)");
    println!("  msvc          C++/MSVC projects              (aliases: cl)");
    println!();
    println!("The <command> is passed directly to the build tool.");
    println!("Use quotes for commands with spaces.");
    println!();
    println!("Global Options:");
    println!("  -h, --help              Show this help message");
    println!("  -v, --version           Show version");
    println!("  --filter-warnings       Filter out warnings, show only errors");
    println!("  --filter-paths <paths>  Filter by file paths (comma-separated)");
    println!("  --verbose               Show all issues without truncation");
    println!("  -q, --quiet             Minimal output (summary only)");
    println!("  -o, --output, -f <file> Write the report to a file instead of stdout");
    println!("  --stdout                No-op; stdout is the default");
    println!("  --format <format>       Report format: markdown, json, html, raw, raw-json (default: markdown)");
    println!("  --no-short-circuit      Disable success short-circuit (always show full report)");
    println!("  --max-issues <N>        Limit analysis to the first N issues (default: unlimited)");
    println!();
    println!("Examples:");
    println!("  analyzer cargo \"check\"");
    println!("  analyzer cargo \"clippy --all-targets\"");
    println!("  analyzer cargo \"test\" --filter-warnings");
    println!("  analyzer npm \"run lint\"");
    println!("  analyzer npm \"run typecheck\"");
    println!("  analyzer pnpm \"exec tsc --noEmit\"");
    println!("  analyzer yarn \"audit\"");
    println!("  analyzer mypy \"--strict .\"");
    println!("  analyzer pytest \"-v\"");
    println!("  analyzer go \"vet ./...\"");
    println!("  analyzer maven \"compile\"");
    println!("  analyzer gradle \"test\"");
    println!();
    println!("Cargo Workspace Options:");
    println!("  --workspace             Analyze all workspace members");
    println!("  -p, --package <SPEC>    Analyze specific package (can be used multiple times)");
    println!("  --exclude <SPEC>        Exclude specific package from analysis");
    println!();
    println!("Cargo Target Options:");
    println!("  --lib                   Analyze only the library target");
    println!("  --bin <NAME>            Analyze specific binary target");
    println!("  --bins                  Analyze all binary targets");
    println!("  --test <NAME>           Analyze specific test target");
    println!("  --tests                 Analyze all test targets");
    println!("  --example <NAME>        Analyze specific example target");
    println!("  --examples              Analyze all example targets");
    println!("  --bench <NAME>          Analyze specific benchmark target");
    println!("  --benches               Analyze all benchmark targets");
    println!("  --all-targets           Analyze all targets");
    println!();
    println!("Cargo Feature Options:");
    println!("  --features <FEATURES>   Space-separated list of features to enable");
    println!("  --all-features          Enable all available features");
    println!("  --no-default-features   Do not enable the default feature");
    println!();
    println!("Cargo Examples:");
    println!("  analyzer cargo check --workspace");
    println!("  analyzer cargo check --package my-crate");
    println!("  analyzer cargo check --lib");
    println!("  analyzer cargo check --bin my-app");
    println!("  analyzer cargo check --tests --all-features");
    println!("  analyzer cargo clippy --workspace --all-targets");
    println!("  analyzer cargo check --package foo --features \"feat1 feat2\"");
    println!();
    println!("C++ Build Options:");
    println!("  --source-dir <DIR>      Source directory for CMake/GCC/Clang builds");
    println!("  --build-dir <DIR>       Build directory for CMake builds");
    println!("  --cmake-generator <GEN> CMake generator (e.g. \"Ninja\", \"Unix Makefiles\")");
    println!("  --target <NAME>         Build target name");
    println!("  --target-files <FILES>  Comma-separated target source files");
    println!("  -I, --include-path <DIR>  Add include search path (repeatable)");
    println!("  -D, --define <MACRO>      Add preprocessor define (repeatable)");
    println!("  --cpp-std <STANDARD>    C++ standard (e.g. c++17, c++20)");
}

fn print_summary(result: &AnalysisResult, verbosity: core::Verbosity) {
    eprintln!("\nTotal issues: {}", result.total_issues);

    if verbosity.is_minimal() {
        return;
    }

    // Use error_count() and warning_count() methods
    eprintln!("  Errors: {}", result.error_count());
    eprintln!("  Warnings: {}", result.warning_count());

    // Use errors() and warnings() methods for detailed counts
    let errors = result.errors();
    let warnings = result.warnings();
    eprintln!("  (via errors() method: {})", errors.len());
    eprintln!("  (via warnings() method: {})", warnings.len());

    for (level, count) in &result.issues_by_level {
        eprintln!("  {}s: {}", level, count);
    }

    if !result.issues_by_file.is_empty() {
        eprintln!("\nTop files with issues:");
        let mut files: Vec<_> = result.issues_by_file.iter().collect();
        files.sort_by_key(|b| std::cmp::Reverse(b.1.len()));

        for (file, issues) in files.iter().take(5) {
            eprintln!("  {}: {} issues", file, issues.len());
        }
    }

    // Print first few errors if any
    if !errors.is_empty() {
        eprintln!("\nFirst {} error(s):", std::cmp::min(3, errors.len()));
        for error in errors.iter().take(3) {
            eprintln!("  - [{}] {}", error.location.file_path, error.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_known_analyzer_flag ──────────────────────────────────────

    #[test]
    fn test_is_known_analyzer_flag_help() {
        assert!(is_known_analyzer_flag("--help"));
        assert!(is_known_analyzer_flag("-h"));
    }

    #[test]
    fn test_is_known_analyzer_flag_version() {
        assert!(is_known_analyzer_flag("--version"));
        assert!(is_known_analyzer_flag("-v"));
    }

    #[test]
    fn test_is_known_analyzer_flag_filter_warnings() {
        assert!(is_known_analyzer_flag("--filter-warnings"));
    }

    #[test]
    fn test_is_known_analyzer_flag_verbose() {
        assert!(is_known_analyzer_flag("--verbose"));
    }

    #[test]
    fn test_is_known_analyzer_flag_quiet() {
        assert!(is_known_analyzer_flag("--quiet"));
        assert!(is_known_analyzer_flag("-q"));
    }

    #[test]
    fn test_is_known_analyzer_flag_stdout() {
        assert!(is_known_analyzer_flag("--stdout"));
    }

    #[test]
    fn test_is_known_analyzer_flag_filter_paths() {
        assert!(is_known_analyzer_flag("--filter-paths"));
    }

    #[test]
    fn test_is_known_analyzer_flag_output() {
        assert!(is_known_analyzer_flag("--output"));
        assert!(is_known_analyzer_flag("-o"));
    }

    #[test]
    fn test_is_known_analyzer_flag_format() {
        assert!(is_known_analyzer_flag("--format"));
    }

    #[test]
    fn test_is_known_analyzer_flag_workspace() {
        assert!(is_known_analyzer_flag("--workspace"));
    }

    #[test]
    fn test_is_known_analyzer_flag_package() {
        assert!(is_known_analyzer_flag("--package"));
        assert!(is_known_analyzer_flag("-p"));
    }

    #[test]
    fn test_is_known_analyzer_flag_exclude() {
        assert!(is_known_analyzer_flag("--exclude"));
    }

    #[test]
    fn test_is_known_analyzer_flag_target_options() {
        assert!(is_known_analyzer_flag("--lib"));
        assert!(is_known_analyzer_flag("--bin"));
        assert!(is_known_analyzer_flag("--bins"));
        assert!(is_known_analyzer_flag("--test"));
        assert!(is_known_analyzer_flag("--tests"));
        assert!(is_known_analyzer_flag("--example"));
        assert!(is_known_analyzer_flag("--examples"));
        assert!(is_known_analyzer_flag("--bench"));
        assert!(is_known_analyzer_flag("--benches"));
        assert!(is_known_analyzer_flag("--all-targets"));
        assert!(is_known_analyzer_flag("--features"));
        assert!(is_known_analyzer_flag("--all-features"));
        assert!(is_known_analyzer_flag("--no-default-features"));
    }

    #[test]
    fn test_is_known_analyzer_flag_cpp_options() {
        assert!(is_known_analyzer_flag("--source-dir"));
        assert!(is_known_analyzer_flag("--build-dir"));
        assert!(is_known_analyzer_flag("--cmake-generator"));
        assert!(is_known_analyzer_flag("--target"));
        assert!(is_known_analyzer_flag("--target-files"));
        assert!(is_known_analyzer_flag("-I"));
        assert!(is_known_analyzer_flag("--include-path"));
        assert!(is_known_analyzer_flag("-D"));
        assert!(is_known_analyzer_flag("--define"));
        assert!(is_known_analyzer_flag("--cpp-std"));
    }

    #[test]
    fn test_is_known_analyzer_flag_other_known() {
        assert!(is_known_analyzer_flag("--no-short-circuit"));
        assert!(is_known_analyzer_flag("--max-issues"));
    }

    #[test]
    fn test_is_known_analyzer_flag_not_known() {
        assert!(!is_known_analyzer_flag("cargo"));
        assert!(!is_known_analyzer_flag("check"));
        assert!(!is_known_analyzer_flag("src/main.rs"));
        assert!(!is_known_analyzer_flag("--unknown-flag"));
    }

    // ── is_test_subcommand ──────────────────────────────────────────

    #[test]
    fn test_is_test_subcommand_test() {
        assert!(is_test_subcommand(&Some(SubCommand::new("test"))));
    }

    #[test]
    fn test_is_test_subcommand_test_with_args() {
        assert!(is_test_subcommand(&Some(SubCommand::new("test --features ci"))));
    }

    #[test]
    fn test_is_test_subcommand_not_test() {
        assert!(!is_test_subcommand(&Some(SubCommand::new("check"))));
        assert!(!is_test_subcommand(&Some(SubCommand::new("build"))));
    }

    #[test]
    fn test_is_test_subcommand_none() {
        assert!(!is_test_subcommand(&None));
    }

    // ── parse_options_from_args ─────────────────────────────────────

    #[test]
    fn test_parse_options_from_args_filter_warnings() {
        let args = vec!["--filter-warnings".to_string(), "cargo".to_string()];
        let mut opts = AnalyzeOptions::default();
        parse_options_from_args(&args, 0, &mut opts);
        assert!(opts.filter_warnings);
    }

    #[test]
    fn test_parse_options_from_args_verbose() {
        let args = vec!["--verbose".to_string()];
        let mut opts = AnalyzeOptions::default();
        parse_options_from_args(&args, 0, &mut opts);
        assert_eq!(opts.verbosity, Verbosity::Verbose);
    }

    #[test]
    fn test_parse_options_from_args_quiet() {
        let args = vec!["--quiet".to_string()];
        let mut opts = AnalyzeOptions::default();
        parse_options_from_args(&args, 0, &mut opts);
        assert_eq!(opts.verbosity, Verbosity::Minimal);
    }

    #[test]
    fn test_parse_options_from_args_stdout() {
        let args = vec!["--stdout".to_string()];
        let mut opts = AnalyzeOptions::default();
        parse_options_from_args(&args, 0, &mut opts);
        assert!(opts.stdout_only);
    }

    #[test]
    fn test_parse_options_from_args_filter_paths() {
        let args = vec!["--filter-paths".to_string(), "src,tests".to_string()];
        let mut opts = AnalyzeOptions::default();
        parse_options_from_args(&args, 0, &mut opts);
        assert_eq!(opts.filter_paths.len(), 2);
        assert!(opts.filter_paths.contains(&"src".to_string()));
        assert!(opts.filter_paths.contains(&"tests".to_string()));
    }

    #[test]
    fn test_parse_options_from_args_filter_paths_missing_arg() {
        // No next arg for --filter-paths → should not panic, just skip
        let args = vec!["--filter-paths".to_string()];
        let mut opts = AnalyzeOptions::default();
        parse_options_from_args(&args, 0, &mut opts);
        assert!(opts.filter_paths.is_empty());
    }

    #[test]
    fn test_parse_options_from_args_max_issues() {
        let args = vec!["--max-issues".to_string(), "10".to_string()];
        let mut opts = AnalyzeOptions::default();
        parse_options_from_args(&args, 0, &mut opts);
        assert_eq!(opts.max_issues, Some(10));
    }

    #[test]
    fn test_parse_options_from_args_no_short_circuit() {
        let args = vec!["--no-short-circuit".to_string()];
        let mut opts = AnalyzeOptions {
            success_short_circuit: true,
            ..Default::default()
        };
        parse_options_from_args(&args, 0, &mut opts);
        assert!(!opts.success_short_circuit);
    }

    #[test]
    fn test_parse_options_from_args_unknown_flag() {
        let args = vec!["--unknown-flag".to_string()];
        let mut opts = AnalyzeOptions::default();
        // Should not panic, just skip unknown flags
        parse_options_from_args(&args, 0, &mut opts);
        // Options should remain default
        assert!(!opts.filter_warnings);
        assert_eq!(opts.verbosity, Verbosity::Normal);
    }
}
