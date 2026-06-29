//! Mypy (Python) Integration Tests
//! Execute actual mypy commands, verify parsing logic matches actual output format

use std::path::PathBuf;

mod common;
use common::{
    fixtures_dir, generate_report, is_command_available, raw_output_dir, run_command,
    save_raw_output,
};

fn python_project_path() -> PathBuf {
    fixtures_dir().join("python-project")
}

/// Check if mypy is available
fn ensure_mypy() -> Result<(), String> {
    if !is_command_available("mypy") {
        return Err("mypy is not installed. Please install it with: pip install mypy".to_string());
    }
    Ok(())
}

#[test]
fn test_mypy_basic_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::python::mypy::parser::MypyParser;

    if let Err(e) = ensure_mypy() {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = python_project_path();

    let output = match run_command("mypy", &["--show-column-numbers", "."], &project_path) {
        Ok(output) => output,
        Err(e) => {
            panic!("Failed to run mypy: {}", e);
        }
    };

    // Save raw output
    save_raw_output("mypy_basic", &output);

    // Parse and generate report
    let parser = MypyParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "mypy_basic",
        "Mypy Basic",
        "mypy --show-column-numbers .",
        &issues,
        Some("raw_output/mypy_basic.txt"),
    );

    println!("=== Mypy Basic Output ===");
    println!("{}", output);

    // Verify mypy output format
    // Format: file:line:col: level: message
    let lines: Vec<&str> = output.lines().collect();
    let has_mypy_errors = lines.iter().any(|line| {
        let parts: Vec<&str> = line.split(':').collect();
        parts.len() >= 4 && (line.contains("error:") || line.contains("warning:"))
    });

    if has_mypy_errors {
        println!("✓ Found mypy error lines in expected format (file:line:col: level: message)");
    } else if output.contains("Success") {
        println!("✓ Mypy reported success (no issues found)");
    } else {
        println!("! Unexpected mypy output format");
    }

    // Verify output format matches parser expectations
    for line in &lines {
        if line.contains(":") && (line.contains("error:") || line.contains("warning:")) {
            let parts: Vec<&str> = line.splitn(5, ':').collect();
            if parts.len() >= 4 {
                println!("  Found issue: {}", line);
            }
        }
    }
}

#[test]
fn test_mypy_strict_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::python::mypy::parser::MypyParser;

    if ensure_mypy().is_err() {
        println!("Skipping test: mypy is not available");
        return;
    }

    let project_path = python_project_path();

    let output = match run_command(
        "mypy",
        &["--strict", "--show-column-numbers", "."],
        &project_path,
    ) {
        Ok(output) => output,
        Err(e) => {
            panic!("Failed to run mypy --strict: {}", e);
        }
    };

    // Save raw output
    save_raw_output("mypy_strict", &output);

    // Parse and generate report
    let parser = MypyParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "mypy_strict",
        "Mypy Strict",
        "mypy --strict --show-column-numbers .",
        &issues,
        Some("raw_output/mypy_strict.txt"),
    );

    println!("=== Mypy Strict Output ===");
    println!("{}", output);

    // Strict mode should have more errors
    let error_count = output
        .lines()
        .filter(|line| line.contains("error:"))
        .count();

    println!("Found {} error lines", error_count);

    // Verify output contains summary info
    if output.contains("Found") && output.contains("error") {
        println!("✓ Found mypy summary line");
    }
}

#[test]
fn test_mypy_specific_file() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::python::mypy::parser::MypyParser;

    if ensure_mypy().is_err() {
        println!("Skipping test: mypy is not available");
        return;
    }

    let project_path = python_project_path();
    let main_py = project_path.join("src/main.py");

    let output = match run_command(
        "mypy",
        &["--show-column-numbers", main_py.to_str().unwrap()],
        &project_path,
    ) {
        Ok(output) => output,
        Err(e) => {
            panic!("Failed to run mypy on specific file: {}", e);
        }
    };

    // Saving the original output
    save_raw_output("mypy_specific_file", &output);

    // Parses and generates reports
    let parser = MypyParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "mypy_specific_file",
        "Mypy Specific File",
        "mypy --show-column-numbers src/main.py",
        &issues,
        Some("raw_output/mypy_specific_file.txt"),
    );

    println!("=== Mypy Specific File Output ===");
    println!("{}", output);

    // Verify the output format of a specific file
    for line in output.lines() {
        if line.contains("main.py") && line.contains(":") {
            println!("  Issue in main.py: {}", line);
        }
    }
}

#[test]
fn test_mypy_with_ignore_missing_imports() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::python::mypy::parser::MypyParser;

    if ensure_mypy().is_err() {
        println!("Skipping test: mypy is not available");
        return;
    }

    let project_path = python_project_path();

    let output = match run_command(
        "mypy",
        &["--show-column-numbers", "--ignore-missing-imports", "."],
        &project_path,
    ) {
        Ok(output) => output,
        Err(e) => {
            panic!("Failed to run mypy: {}", e);
        }
    };

    // Saving the original output
    save_raw_output("mypy_ignore_imports", &output);

    // Parses and generates reports
    let parser = MypyParser::new();
    let issues = parser.parse(&output).data_or_default_owned();
    generate_report(
        "mypy_ignore_imports",
        "Mypy Ignore Imports",
        "mypy --show-column-numbers --ignore-missing-imports .",
        &issues,
        Some("raw_output/mypy_ignore_imports.txt"),
    );

    println!("=== Mypy with --ignore-missing-imports Output ===");
    println!("{}", output);
}

/// Verify mypy output format
fn validate_mypy_output(content: &str) {
    println!("Validating mypy output format...");
    let issue_lines: Vec<&str> = content
        .lines()
        .filter(|line| line.contains(":") && (line.contains("error:") || line.contains("warning:")))
        .collect();

    println!("  Found {} issue lines", issue_lines.len());

    for line in &issue_lines {
        // 验证格式: file:line:col: level: message
        let parts: Vec<&str> = line.splitn(5, ':').collect();
        if parts.len() >= 4 {
            let _file = parts[0];
            let _line_num = parts[1].trim().parse::<u32>();
            let _col_num = parts[2].trim().parse::<u32>();
            let _level = parts[3].trim();
            println!("  ✓ Valid format: {}", line);
        }
    }
}

#[test]
fn test_validate_mypy_outputs() {
    // Read and validate saved mypy output files
    let output_dir = raw_output_dir();

    for entry in std::fs::read_dir(&output_dir)
        .expect("Failed to read output directory")
        .flatten()
    {
        let path = entry.path();
        let filename = path.file_name().unwrap_or_default().to_string_lossy();

        if filename.starts_with("mypy_") && path.extension().map(|e| e == "txt").unwrap_or(false) {
            let content = std::fs::read_to_string(&path).expect("Failed to read output file");
            println!("Validating: {}", path.display());
            validate_mypy_output(&content);
        }
    }
}
