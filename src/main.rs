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

mod config;
mod core;
mod discover;
mod plugins;

use core::{
    AnalysisResult, AnalyzeOptions, ReporterFactory, SubCommand, TechStack, TestOptions, Verbosity,
};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        show_help();
        std::process::exit(1);
    }

    // --- Rewrite subcommand ---
    if args[1] == "rewrite" {
        handle_rewrite(&args);
        return;
    }

    // --- Run subcommand ---
    if args[1] == "run" {
        handle_run(&args);
        return;
    }

    // --- Config subcommand ---
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

    // Parse arguments (CLI overrides config)
    let (tech_stack, options) = parse_arguments(&args, &config);

    run_orchestrator(tech_stack, options, &config);
}

fn handle_rewrite(args: &[String]) {
    let raw_cmd = args[2..].join(" ");
    if raw_cmd.trim().is_empty() {
        eprintln!("Error: no command provided for rewrite");
        eprintln!("Usage: analyzer rewrite <raw_shell_command>");
        std::process::exit(1);
    }
    // Load config to resolve command aliases
    let config = config::ConfigLoader::new().load();
    match discover::rewrite_command_with_config(&raw_cmd, &config.commands) {
        Some((tech_stack, subcommand, extra_args)) => {
            print!("analyzer {} \"{}\"", tech_stack.as_str(), subcommand);
            for arg in &extra_args {
                print!(" {}", arg);
            }
            println!();
            std::process::exit(0);
        }
        None => {
            std::process::exit(1);
        }
    }
}

fn handle_run(args: &[String]) {
    if args.len() < 3 {
        eprintln!("Error: no command provided for run");
        eprintln!("Usage: analyzer run <raw_shell_command> [options]");
        std::process::exit(1);
    }

    let mut raw_cmd_parts: Vec<String> = Vec::new();
    let mut flag_start = args.len();

    for (i, arg) in args.iter().enumerate().skip(2) {
        if (arg.starts_with("--") && arg.len() > 2)
            || (arg.starts_with('-')
                && arg.len() == 2
                && arg.as_bytes()[1].is_ascii_alphabetic()
                && arg != "--")
        {
            flag_start = i;
            break;
        }
        raw_cmd_parts.push(arg.clone());
    }

    let raw_cmd = raw_cmd_parts.join(" ");
    if raw_cmd.trim().is_empty() {
        eprintln!("Error: no command provided for run");
        std::process::exit(1);
    }

    // Load config early so we can use command aliases during classification
    let config = config::ConfigLoader::new().load();

    // Split compound commands (&&, ||, ;, |, &) and only analyze the first segment
    let segments = discover::split_on_operators(raw_cmd.trim());
    let cmd_to_classify = match segments.len() {
        0 => {
            eprintln!("Error: no command provided for run");
            std::process::exit(1);
        }
        1 => segments[0].clone(),
        n => {
            eprintln!(
                "Note: Compound command detected ({} segments). Only the first segment '{}' will be analyzed.",
                n, segments[0]
            );
            segments[0].clone()
        }
    };

    let (tech_stack, subcommand, extra_args) =
        match discover::classify_command_with_config(&cmd_to_classify, &config.commands) {
            discover::Classification::Matched {
                tech_stack,
                subcommand,
                extra_args,
                ..
            } => (tech_stack, subcommand, extra_args),
            discover::Classification::Unmatched { base_command } => {
                eprintln!("Error: Unrecognized command '{}'", base_command);
                eprintln!(
                    "Try running: analyzer rewrite \"{}\" to check if it's supported",
                    cmd_to_classify.trim()
                );
                std::process::exit(1);
            }
        };

    let full_cmd = if extra_args.is_empty() {
        subcommand
    } else {
        format!("{} {}", subcommand, extra_args.join(" "))
    };

    let mut options = AnalyzeOptions::from_config(&config);
    options.subcommand = Some(SubCommand::new(full_cmd));

    parse_options_from_args(args, flag_start, &mut options);

    run_orchestrator(tech_stack, options, &config);
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
        println!("Usage: analyzer stats [--reset]");
    }
}

fn parse_options_from_args(args: &[String], start: usize, options: &mut AnalyzeOptions) {
    let mut i = start;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                show_help();
                std::process::exit(0);
            }
            "--version" | "-v" => {
                println!("analyzer 0.2.0");
                std::process::exit(0);
            }
            "--filter-warnings" => {
                options.filter_warnings = true;
            }
            "--verbose" => {
                options.verbosity = Verbosity::Verbose;
            }
            "--quiet" | "-q" => {
                options.verbosity = Verbosity::Minimal;
            }
            "--stdout" => {
                options.stdout_only = true;
            }
            "--filter-paths" => {
                if i + 1 < args.len() {
                    options.filter_paths = args[i + 1]
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                    i += 1;
                }
            }
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    options.output_file = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--format" => {
                if i + 1 < args.len() {
                    let format_str = &args[i + 1];
                    options.report_format = format_str.parse().unwrap_or_else(|e| {
                        eprintln!("Error: Invalid format '{}': {}", format_str, e);
                        eprintln!("Supported formats: markdown, json, html, raw, raw-json");
                        std::process::exit(1);
                    });
                    i += 1;
                }
            }
            "--workspace" => {
                options.workspace = true;
            }
            "--package" | "-p" => {
                if i + 1 < args.len() {
                    options.package.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--exclude" => {
                if i + 1 < args.len() {
                    options.exclude.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--lib" => {
                options.lib = true;
            }
            "--bin" => {
                if i + 1 < args.len() {
                    options.bin.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--bins" => {
                options.bins = true;
            }
            "--test" => {
                if i + 1 < args.len() {
                    options.test.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--tests" => {
                options.tests = true;
            }
            "--example" => {
                if i + 1 < args.len() {
                    options.example.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--examples" => {
                options.examples = true;
            }
            "--bench" => {
                if i + 1 < args.len() {
                    options.bench.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--benches" => {
                options.benches = true;
            }
            "--all-targets" => {
                options.all_targets = true;
            }
            "--features" => {
                if i + 1 < args.len() {
                    options.features.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--all-features" => {
                options.all_features = true;
            }
            "--no-default-features" => {
                options.no_default_features = true;
            }
            "--no-short-circuit" => {
                options.success_short_circuit = false;
            }
            // === Result Limits ===
            "--max-issues" => {
                if i + 1 < args.len() {
                    let val = args[i + 1].parse::<usize>().unwrap_or_else(|e| {
                        eprintln!("Error: Invalid --max-issues value '{}': {}", args[i + 1], e);
                        std::process::exit(1);
                    });
                    options.max_issues = Some(val);
                    i += 1;
                }
            }
            // === C++ Build Options ===
            "--source-dir" => {
                if i + 1 < args.len() {
                    options.source_dir = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--build-dir" => {
                if i + 1 < args.len() {
                    options.build_dir = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--cmake-generator" => {
                if i + 1 < args.len() {
                    options.cmake_generator = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--target" => {
                if i + 1 < args.len() {
                    options.target = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--target-files" => {
                if i + 1 < args.len() {
                    options.target_files = args[i + 1]
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                    i += 1;
                }
            }
            "-I" | "--include-path" => {
                if i + 1 < args.len() {
                    options.include_paths.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "-D" | "--define" => {
                if i + 1 < args.len() {
                    options.defines.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--cpp-std" => {
                if i + 1 < args.len() {
                    options.cpp_standard = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            _ => {
                // Unknown args are silently ignored in run mode
            }
        }
        i += 1;
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
    println!(
        "Supported commands: {}",
        analyzer.supported_commands().join(", ")
    );

    // Check if this is a test command
    let is_test_command = is_test_subcommand(&options.subcommand);

    // Print subcommand category if available
    if let Some(ref cmd) = options.subcommand {
        println!("Command category: {:?}", cmd.category());
        if cmd.is_custom() {
            println!("Using custom command: {}", cmd.as_str());
        }
    }

    if is_test_command {
        // Resolve test framework from config
        let ts_str = tech_stack.as_str();
        if let Some(framework) = config.test_framework_for(ts_str) {
            println!("Test framework: {}", framework);
        }
        // Run test analysis
        run_test_analysis(analyzer, &options);
    } else {
        // Run regular analysis
        run_analysis(analyzer, &options);
    }

    let tracking_summary = core::tracking::stats().summary();
    if !tracking_summary.contains("0 total") {
        println!("\n{}", tracking_summary);
    }
}

fn parse_arguments(args: &[String], config: &config::AppConfig) -> (TechStack, AnalyzeOptions) {
    let mut tech_stack_str = String::new();
    let mut command_str = String::new();
    // Seed options from configuration file, then let CLI args override
    let mut options = AnalyzeOptions::from_config(config);

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                show_help();
                std::process::exit(0);
            }
            "--version" | "-v" => {
                println!("analyzer 0.2.0");
                std::process::exit(0);
            }
            "--filter-warnings" => {
                options.filter_warnings = true;
            }
            "--verbose" => {
                options.verbosity = Verbosity::Verbose;
            }
            "--quiet" | "-q" => {
                options.verbosity = Verbosity::Minimal;
            }
            "--filter-paths" => {
                if i + 1 < args.len() {
                    options.filter_paths = args[i + 1]
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                    i += 1;
                }
            }
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    options.output_file = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--format" => {
                if i + 1 < args.len() {
                    let format_str = &args[i + 1];
                    options.report_format = format_str.parse().unwrap_or_else(|e| {
                        eprintln!("Error: Invalid format '{}': {}", format_str, e);
                        eprintln!("Supported formats: markdown, json, html, raw, raw-json");
                        std::process::exit(1);
                    });
                    i += 1;
                }
            }
            "--stdout" => {
                options.stdout_only = true;
            }
            // === Cargo Workspace Options ===
            "--workspace" => {
                options.workspace = true;
            }
            "--package" | "-p" => {
                if i + 1 < args.len() {
                    options.package.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--exclude" => {
                if i + 1 < args.len() {
                    options.exclude.push(args[i + 1].clone());
                    i += 1;
                }
            }
            // === Cargo Target Options ===
            "--lib" => {
                options.lib = true;
            }
            "--bin" => {
                if i + 1 < args.len() {
                    options.bin.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--bins" => {
                options.bins = true;
            }
            "--test" => {
                if i + 1 < args.len() {
                    options.test.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--tests" => {
                options.tests = true;
            }
            "--example" => {
                if i + 1 < args.len() {
                    options.example.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--examples" => {
                options.examples = true;
            }
            "--bench" => {
                if i + 1 < args.len() {
                    options.bench.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--benches" => {
                options.benches = true;
            }
            "--all-targets" => {
                options.all_targets = true;
            }
            // === Cargo Feature Options ===
            "--features" => {
                if i + 1 < args.len() {
                    options.features.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--all-features" => {
                options.all_features = true;
            }
            "--no-default-features" => {
                options.no_default_features = true;
            }
            "--no-short-circuit" => {
                options.success_short_circuit = false;
            }
            // === C++ Build Options ===
            "--source-dir" => {
                if i + 1 < args.len() {
                    options.source_dir = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--build-dir" => {
                if i + 1 < args.len() {
                    options.build_dir = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--cmake-generator" => {
                if i + 1 < args.len() {
                    options.cmake_generator = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--target" => {
                if i + 1 < args.len() {
                    options.target = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--target-files" => {
                if i + 1 < args.len() {
                    options.target_files = args[i + 1]
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                    i += 1;
                }
            }
            "-I" | "--include-path" => {
                if i + 1 < args.len() {
                    options.include_paths.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "-D" | "--define" => {
                if i + 1 < args.len() {
                    options.defines.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--cpp-std" => {
                if i + 1 < args.len() {
                    options.cpp_standard = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            arg => {
                if !arg.starts_with('-') {
                    if tech_stack_str.is_empty() {
                        tech_stack_str = arg.to_string();
                    } else if command_str.is_empty() {
                        // Collect the full command string
                        command_str = arg.to_string();
                    }
                }
            }
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
                println!(
                    "Using configured command '{}' for alias '{}'",
                    cmd_config.exec, command_str
                );
                command_str = cmd_config.exec.clone();
            }
        }
    }

    // Resolve script names to actual frameworks via tech_stacks config
    if let Some(resolved) = config.resolve_script(&tech_stack_str, &command_str) {
        println!(
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
    println!(
        "Analyzing project with {} {}...",
        analyzer.name(),
        subcommand_name
    );

    // Use the parser method to demonstrate it's being used
    let _parser = analyzer.parser();
    println!("Using parser: {}", std::any::type_name_of_val(_parser));

    // The OutputParser trait is implemented by various parsers
    // and provides line-by-line parsing capabilities via template method pattern

    match analyzer.analyze(options) {
        Ok(result) => {
            println!("\nAnalysis complete!");
            println!("Total issues: {}", result.total_issues);

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

                println!("Report written to: {}", output_path);
            }

            // Print summary
            print_summary(&result);
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

    println!("Running tests for {}...", analyzer.name());

    // Convert AnalyzeOptions to TestOptions
    let test_options = TestOptions::from(options);

    match test_analyzer.run_tests(&test_options) {
        Ok(test_output) => {
            println!("\nTest analysis complete!");
            println!("Compile issues: {}", test_output.compile_issues.len());

            if let Some(ref summary) = test_output.test_summary {
                println!(
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

                println!("Test report written to: {}", output_path);
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
    println!("  analyzer run <raw_shell_command> [options]");
    println!("  analyzer rewrite <raw_shell_command>");
    println!();
    println!("Subcommands:");
    println!("  run        Execute any build tool command through the analyzer");
    println!("  rewrite    Preview the analyzer-equivalent command without executing it");
    println!("  config     Show or initialize configuration");
    println!("  stats      Show analysis tracking statistics");
    println!();
    println!("Run exit codes:");
    println!("  0  Success (rewritten and executed successfully)");
    println!("  1  No matching rule / execution failed");
    println!("  2  Subcommand not supported");
    println!();
    println!("Rewrite exit codes:");
    println!("  0  Successfully rewritten (command printed to stdout)");
    println!("  1  No matching rule (nothing printed)");
    println!();
    println!("Tech Stacks:");
    println!("  cargo         Rust/Cargo projects");
    println!("  mypy          Python/Mypy projects");
    println!("  pytest        Python/Pytest projects");
    println!("  npm           Node.js/npm projects");
    println!("  pnpm          Node.js/pnpm projects");
    println!("  yarn          Node.js/yarn projects");
    println!("  go            Go projects");
    println!("  maven         Java/Maven projects");
    println!("  gradle        Java/Gradle projects");
    println!("  cmake         C++/CMake projects");
    println!("  gcc           C++/GCC projects");
    println!("  clang         C++/Clang projects");
    println!("  msvc          C++/MSVC projects");
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
    println!("  -o, --output <file>     Output file (default: analysis_report.md/.json/.html)");
    println!("  --stdout                Output to stdout only, do not write file");
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
    println!("Run examples:");
    println!("  analyzer run \"cargo check --all-targets\"");
    println!("  analyzer run \"npm run lint\" --format json --stdout");
    println!("  analyzer run \"pytest -v\"");
    println!("  analyzer run \"go vet ./...\" --format raw --stdout");
    println!();
    println!("Rewrite examples:");
    println!("  analyzer rewrite \"cargo check --all-targets\"");
    println!("  analyzer rewrite \"npm run lint\"");
    println!("  analyzer rewrite \"go vet ./...\"");
    println!("  analyzer rewrite \"mvn test\"");
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

fn print_summary(result: &AnalysisResult) {
    println!("\n=== Summary ===");
    println!("Total issues: {}", result.total_issues);

    // Use error_count() and warning_count() methods
    println!("  Errors: {}", result.error_count());
    println!("  Warnings: {}", result.warning_count());

    // Use errors() and warnings() methods for detailed counts
    let errors = result.errors();
    let warnings = result.warnings();
    println!("  (via errors() method: {})", errors.len());
    println!("  (via warnings() method: {})", warnings.len());

    for (level, count) in &result.issues_by_level {
        println!("  {}s: {}", level, count);
    }

    if !result.issues_by_file.is_empty() {
        println!("\nTop files with issues:");
        let mut files: Vec<_> = result.issues_by_file.iter().collect();
        files.sort_by_key(|b| std::cmp::Reverse(b.1.len()));

        for (file, issues) in files.iter().take(5) {
            println!("  {}: {} issues", file, issues.len());
        }
    }

    // Print first few errors if any
    if !errors.is_empty() {
        println!("\nFirst {} error(s):", std::cmp::min(3, errors.len()));
        for error in errors.iter().take(3) {
            println!("  - [{}] {}", error.location.file_path, error.message);
        }
    }
}
