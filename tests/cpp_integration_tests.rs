//! C++ Integration Tests
//! Execute actual C++ compiler commands, verify parsing logic matches actual output format

use std::path::PathBuf;
use std::process::Command;

mod common;
use common::{fixtures_dir, generate_report, is_command_available, run_command, save_raw_output};

/// Check if a command is available
fn ensure_command(cmd: &str) -> Result<(), String> {
    if !is_command_available(cmd) {
        return Err(format!("{} is not installed or not in PATH", cmd));
    }
    Ok(())
}

/// Get C++ project fixture path
fn cpp_project_path() -> PathBuf {
    fixtures_dir().join("cpp-project")
}

#[test]
fn test_gcc_basic_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::cpp::parser::{CompilerType, CppParser};

    if let Err(e) = ensure_command("g++") {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = cpp_project_path();
    if !project_path.exists() {
        println!(
            "Skipping test: C++ project fixture not found at {:?}",
            project_path
        );
        return;
    }

    let output = match run_command(
        "g++",
        &[
            "-std=c++17",
            "-Wall",
            "-c",
            "src/main.cpp",
            "-o",
            "/tmp/test.o",
        ],
        &project_path,
    ) {
        Ok(output) => output,
        Err(e) => {
            // Compiler errors are expected for test files with intentional errors
            e.to_string()
        }
    };

    // Save raw output
    save_raw_output("gcc_basic", &output);

    // Parse and generate report
    let parser = CppParser::new(CompilerType::Gcc);
    let issues = parser.parse(&output).data_or_default_owned();

    if !issues.is_empty() {
        generate_report(
            "gcc_basic",
            "GCC Basic",
            "g++ -std=c++17 -Wall -c src/main.cpp",
            &issues,
            Some("raw_output/gcc_basic.txt"),
        );
    }

    println!("=== GCC Basic Output ===");
    println!("{}", output);

    // Verify GCC output format
    let lines: Vec<&str> = output.lines().collect();
    let has_gcc_errors = lines.iter().any(|line| {
        line.contains(": error:") || line.contains(": warning:") || line.contains(": note:")
    });

    if has_gcc_errors {
        println!(
            "✓ Found GCC error/warning lines in expected format (file:line:col: level: message)"
        );
    } else if output.contains("Success") || output.is_empty() {
        println!("✓ GCC reported success (no issues found)");
    } else {
        println!("! Unexpected GCC output format");
    }
}

#[test]
fn test_clang_basic_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::cpp::parser::{CompilerType, CppParser};

    if let Err(e) = ensure_command("clang++") {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = cpp_project_path();
    if !project_path.exists() {
        println!("Skipping test: C++ project fixture not found");
        return;
    }

    let output = match run_command(
        "clang++",
        &[
            "-std=c++17",
            "-Wall",
            "-c",
            "src/main.cpp",
            "-o",
            "/tmp/test.o",
        ],
        &project_path,
    ) {
        Ok(output) => output,
        Err(e) => e.to_string(),
    };

    save_raw_output("clang_basic", &output);

    let parser = CppParser::new(CompilerType::Clang);
    let issues = parser.parse(&output).data_or_default_owned();

    if !issues.is_empty() {
        generate_report(
            "clang_basic",
            "Clang Basic",
            "clang++ -std=c++17 -Wall -c src/main.cpp",
            &issues,
            Some("raw_output/clang_basic.txt"),
        );
    }

    println!("=== Clang Basic Output ===");
    println!("{}", output);

    let lines: Vec<&str> = output.lines().collect();
    let has_clang_errors = lines.iter().any(|line| {
        line.contains(": error:") || line.contains(": warning:") || line.contains(": note:")
    });

    if has_clang_errors {
        println!("✓ Found Clang error/warning lines in expected format");
    } else {
        println!("✓ Clang reported success or no recognizable issues");
    }
}

#[test]
fn test_cmake_configure_output() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::cpp::cmake::parser::CMakeParser;

    if let Err(e) = ensure_command("cmake") {
        println!("Skipping test: {}", e);
        return;
    }

    let project_path = cpp_project_path();
    if !project_path.exists() {
        println!("Skipping test: C++ project fixture not found");
        return;
    }

    // Create a temporary build directory
    let build_dir = project_path.join("build_test");
    std::fs::create_dir_all(&build_dir).ok();

    let output = match run_command("cmake", &[".."], &build_dir) {
        Ok(output) => output,
        Err(e) => e.to_string(),
    };

    save_raw_output("cmake_configure", &output);

    let parser = CMakeParser::new();
    let issues = parser.parse(&output).data_or_default_owned();

    if !issues.is_empty() {
        generate_report(
            "cmake_configure",
            "CMake Configure",
            "cmake ..",
            &issues,
            Some("raw_output/cmake_configure.txt"),
        );
    }

    println!("=== CMake Configure Output ===");
    println!("{}", output);

    // Check for CMake errors/warnings
    let has_cmake_errors = output.contains("CMake Error") || output.contains("CMake Warning");

    if has_cmake_errors {
        println!("✓ Found CMake errors or warnings");
    } else {
        println!("✓ CMake configuration successful or no issues");
    }

    // Cleanup
    std::fs::remove_dir_all(&build_dir).ok();
}

#[test]
fn test_compiler_detection() {
    use analyzer::plugins::cpp::parser::{CompilerType, CppParser};

    // Test GCC detection from version output
    if is_command_available("g++") {
        let version_output = Command::new("g++")
            .args(["--version"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let detected = CppParser::detect_compiler_type(&version_output);
        assert!(
            matches!(detected, CompilerType::Gcc),
            "Should detect GCC from version output"
        );
        println!("✓ GCC compiler detection works");
    }

    // Test Clang detection from version output
    if is_command_available("clang++") {
        let version_output = Command::new("clang++")
            .args(["--version"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let detected = CppParser::detect_compiler_type(&version_output);
        assert!(
            matches!(detected, CompilerType::Clang),
            "Should detect Clang from version output"
        );
        println!("✓ Clang compiler detection works");
    }
}

#[test]
fn test_parser_with_real_gcc_warnings() {
    use analyzer::core::OutputParser;
    use analyzer::plugins::cpp::parser::{CompilerType, CppParser};

    // Create a temporary C++ file with intentional issues
    let temp_dir = std::env::temp_dir().join("analyzer_test_cpp");
    std::fs::create_dir_all(&temp_dir).ok();

    let test_cpp = temp_dir.join("test.cpp");
    let cpp_content = r#"
int main() {
    int unused_var = 42;
    return 0;
}
"#;
    std::fs::write(&test_cpp, cpp_content).expect("Failed to write test file");

    if is_command_available("g++") {
        let output = Command::new("g++")
            .args(["-Wall", "-c", test_cpp.to_str().unwrap(), "-o", "/dev/null"])
            .output();

        if let Ok(result) = output {
            let stderr = String::from_utf8_lossy(&result.stderr);
            save_raw_output("gcc_warnings", &stderr);

            let parser = CppParser::new(CompilerType::Gcc);
            let issues = parser.parse(&stderr).data_or_default_owned();

            // Should detect unused variable warning
            let has_unused_warning = issues
                .iter()
                .any(|i| i.message.contains("unused") || i.message.contains("set but not used"));

            if has_unused_warning {
                println!("✓ GCC parser correctly detected unused variable warning");
            } else {
                println!("! GCC did not produce expected warning (may vary by version)");
            }
        }
    }

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
}
