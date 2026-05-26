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

mod core;
mod plugins;

use core::{
    AnalysisResult, AnalyzeOptions, ReportFormat, ReporterFactory, SubCommand, TechStack,
    TestAnalyzer, TestOptions, Verbosity,
};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        show_help();
        std::process::exit(1);
    }

    // Parse arguments
    let (tech_stack, options) = parse_arguments(&args);

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
    println!("Supported commands: {}", analyzer.supported_commands().join(", "));

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
        // Run test analysis
        run_test_analysis(analyzer, &options);
    } else {
        // Run regular analysis
        run_analysis(analyzer, &options);
    }
}

fn parse_arguments(args: &[String]) -> (TechStack, AnalyzeOptions) {
    let mut tech_stack_str = String::new();
    let mut command_str = String::new();
    let mut options = AnalyzeOptions::default();

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

    options.subcommand = Some(SubCommand::new(command_str));
    (tech_stack, options)
}

/// Convert AnalyzeOptions to ReportOptions
fn to_report_options(options: &AnalyzeOptions, tech_stack: &TechStack, subcommand: Option<&SubCommand>) -> core::reporter::ReportOptions {
    let tech_stack_name = match subcommand {
        Some(cmd) => format!("{} {}", tech_stack.as_str(), cmd.as_str()),
        None => tech_stack.as_str().to_string(),
    };
    core::reporter::ReportOptions {
        verbose: options.verbosity,
        success_short_circuit: true,
        tech_stack: Some(tech_stack_name),
    }
}

fn is_test_subcommand(subcommand: &Option<SubCommand>) -> bool {
    subcommand
        .as_ref()
        .map(|cmd| cmd.as_str().to_lowercase().contains("test"))
        .unwrap_or(false)
}

fn run_analysis(
    analyzer: &dyn core::BuildAnalyzer,
    options: &AnalyzeOptions,
) {
    let subcommand_name = options.subcommand.as_ref()
        .map(|s| s.as_str())
        .unwrap_or("default");
    println!("Analyzing project with {} {}...", analyzer.name(), subcommand_name);

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
            let reporter = ReporterFactory::create(ReportFormat::Markdown);
            let report_options = to_report_options(options, &analyzer.tech_stack(), options.subcommand.as_ref());
            let report = match reporter.generate_with_options(&result, report_options) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to generate report: {}", e);
                    std::process::exit(1);
                }
            };

            // output report
            let output_path = options
                .output_file
                .as_deref()
                .unwrap_or("analysis_report.md");

            if let Err(e) = reporter.write_to_file(&report, Path::new(output_path)) {
                eprintln!("Failed to write report: {}", e);
                std::process::exit(1);
            }

            println!("Report written to: {}", output_path);

            // Print summary
            print_summary(&result);
        }
        Err(e) => {
            eprintln!("Analysis failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_test_analysis(
    analyzer: &dyn core::BuildAnalyzer,
    options: &AnalyzeOptions,
) {
    // Try to downcast to TestAnalyzer
    let test_analyzer = match analyzer.as_any().downcast_ref::<&dyn TestAnalyzer>() {
        Some(ta) => *ta,
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
                println!("Tests: {} total, {} passed, {} failed, {} ignored",
                    summary.total, summary.passed, summary.failed, summary.ignored);
            }

            // Generate test report
            let reporter = ReporterFactory::create(ReportFormat::Markdown);
            let report_options = to_report_options(options, &analyzer.tech_stack(), options.subcommand.as_ref());
            let report = match reporter.generate_test_report_with_options(&test_output.into(), report_options) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to generate report: {}", e);
                    std::process::exit(1);
                }
            };

            let output_path = options
                .output_file
                .as_deref()
                .unwrap_or("test_report.md");

            if let Err(e) = reporter.write_to_file(&report, Path::new(output_path)) {
                eprintln!("Failed to write report: {}", e);
                std::process::exit(1);
            }

            println!("Test report written to: {}", output_path);
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
    println!("Usage: analyzer <tech-stack> <command> [options]");
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
    println!("  -q, --quiet              Minimal output (summary only)");
    println!("  -o, --output <file>     Output file (default: analysis_report.md)");
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
        files.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

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
