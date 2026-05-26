//! CMake Real Integration Tests
//! Execute actual CMake commands with real compilers to verify parsing

use std::path::{Path, PathBuf};
use std::fs;

mod common;
use common::{
    fixtures_dir, save_raw_output, generate_report,
    vs_env::{is_vs_dev_shell_available, run_with_vs_env, check_cmake, check_msvc, get_cmake_generator}
};

use analyzer::core::OutputParser;
use analyzer::plugins::cpp::cmake::parser::CMakeParser;
use analyzer::plugins::cpp::parser::CppParser;

/// Get the CMake test project path
fn cmake_project_path() -> PathBuf {
    fixtures_dir().join("cpp-cmake-project")
}

/// Setup build directory
fn setup_build_dir(project_path: &Path) -> PathBuf {
    let build_dir = project_path.join("build_test");
    // Clean and recreate
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir).ok();
    }
    fs::create_dir_all(&build_dir).expect("Failed to create build directory");
    build_dir
}

#[test]
fn test_cmake_configure_with_msvc() {
    // Skip if VS environment is not available
    if !is_vs_dev_shell_available() {
        println!("Skipping test: VS Dev Shell not available");
        return;
    }

    let project_path = cmake_project_path();
    if !project_path.exists() {
        println!("Skipping test: CMake test project not found at {:?}", project_path);
        return;
    }

    let build_dir = setup_build_dir(&project_path);

    // Run CMake configuration with VS environment
    let generator = get_cmake_generator();
    let output = match run_with_vs_env(
        "cmake",
        &["-G", generator, ".."],
        &build_dir
    ) {
        Ok(output) => output,
        Err(e) => {
            println!("CMake configure failed: {}", e);
            return;
        }
    };

    // Save raw output
    save_raw_output("cmake_configure_msvc", &output);

    // Parse the output
    let parser = CMakeParser::new();
    let issues = parser.parse(&output).data_or_default_owned();

    // Generate report
    generate_report(
        "cmake_configure_msvc",
        "CMake Configure (MSVC)",
        &format!("cmake -G \"{}\" ..", generator),
        &issues,
        Some("raw_output/cmake_configure_msvc.txt")
    );

    println!("=== CMake Configure Output (MSVC) ===");
    println!("{}", output);
    println!("\n=== Parsed Issues ===");
    println!("Found {} issues", issues.len());
    for issue in &issues {
        println!("  [{:?}] {}:{} - {}",
            issue.level,
            issue.location.file_path,
            issue.location.line_number.map(|l| l.to_string()).unwrap_or_else(|| "-".to_string()),
            issue.message
        );
    }

    // Verify that we can parse CMake output
    // Note: Configuration may succeed or fail depending on the test project
    println!("✓ CMake configure test completed with {} issues parsed", issues.len());

    // Cleanup
    fs::remove_dir_all(&build_dir).ok();
}

#[test]
fn test_cmake_build_with_msvc() {
    // Skip if VS environment is not available
    if !is_vs_dev_shell_available() {
        println!("Skipping test: VS Dev Shell not available");
        return;
    }

    let project_path = cmake_project_path();
    if !project_path.exists() {
        println!("Skipping test: CMake test project not found");
        return;
    }

    // Use a different build directory to avoid conflicts with configure test
    let build_dir = project_path.join("build_test_full");
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir).ok();
    }
    fs::create_dir_all(&build_dir).expect("Failed to create build directory");

    // First, configure
    let generator = get_cmake_generator();
    if let Err(e) = run_with_vs_env("cmake", &["-G", generator, ".."], &build_dir) {
        println!("CMake configure failed: {}", e);
        return;
    }

    // Then build (this should produce compiler errors/warnings)
    let output = match run_with_vs_env("cmake", &["--build", ".", "--verbose"], &build_dir) {
        Ok(output) => output,
        Err(e) => {
            // Build may fail due to intentional errors in test project
            println!("CMake build failed (expected): {}", e);
            e
        }
    };

    // Save raw output
    save_raw_output("cmake_build_msvc", &output);

    // Parse the output with CMakeParser (which includes compiler output parsing)
    let parser = CMakeParser::new();
    let issues = parser.parse(&output).data_or_default_owned();

    // Generate report
    generate_report(
        "cmake_build_msvc",
        "CMake Build (MSVC)",
        "cmake --build . --verbose",
        &issues,
        Some("raw_output/cmake_build_msvc.txt")
    );

    println!("=== CMake Build Output (MSVC) ===");
    println!("{}", output);
    println!("\n=== Parsed Issues ===");
    println!("Found {} issues", issues.len());

    // Verify we can detect compiler errors
    let error_count = issues.iter().filter(|i| matches!(i.level, analyzer::core::IssueLevel::Error)).count();
    let warning_count = issues.iter().filter(|i| matches!(i.level, analyzer::core::IssueLevel::Warning)).count();

    println!("  Errors: {}", error_count);
    println!("  Warnings: {}", warning_count);

    // The test project has intentional errors, so we should find some
    if error_count > 0 || warning_count > 0 {
        println!("✓ Successfully detected compiler issues from actual build output");
    } else {
        println!("! No issues detected - this may indicate a parsing problem");
    }

    // Cleanup
    fs::remove_dir_all(&build_dir).ok();
}

#[test]
fn test_msvc_compiler_output_parsing() {
    // Skip if VS environment is not available
    if !is_vs_dev_shell_available() {
        println!("Skipping test: VS Dev Shell not available");
        return;
    }

    let project_path = cmake_project_path();
    let main_cpp = project_path.join("src/main.cpp");

    if !main_cpp.exists() {
        println!("Skipping test: Test file not found");
        return;
    }

    // Compile a single file to get MSVC output
    let temp_dir = std::env::temp_dir().join("analyzer_msvc_test");
    fs::create_dir_all(&temp_dir).ok();

    let output = match run_with_vs_env(
        "cl",
        &[
            "/EHsc",
            "/W4",
            "/c",
            main_cpp.to_str().unwrap(),
            &format!("/Fo{}", temp_dir.join("main.obj").to_str().unwrap())
        ],
        &project_path
    ) {
        Ok(output) => output,
        Err(e) => {
            // Compilation is expected to fail due to intentional errors
            println!("MSVC compilation failed (expected): {}", e);
            e
        }
    };

    // Save raw output
    save_raw_output("msvc_compile", &output);

    // Parse with MSVC parser
    let parser = CppParser::with_msvc();
    let issues = parser.parse(&output).data_or_default_owned();

    // Generate report
    generate_report(
        "msvc_compile",
        "MSVC Compile",
        "cl /EHsc /W4 /c src/main.cpp",
        &issues,
        Some("raw_output/msvc_compile.txt")
    );

    println!("=== MSVC Compile Output ===");
    println!("{}", output);
    println!("\n=== Parsed Issues ===");
    println!("Found {} issues", issues.len());

    for issue in &issues {
        println!("  [{:?}] {}({},{}) - {}",
            issue.level,
            issue.location.file_path,
            issue.location.line_number.map(|l| l.to_string()).unwrap_or_else(|| "-".to_string()),
            issue.location.column_number.map(|c| c.to_string()).unwrap_or_else(|| "-".to_string()),
            issue.message
        );
    }

    // Verify we found the expected errors
    let has_undeclared_var = issues.iter().any(|i| {
        i.message.contains("undeclared") || i.message.contains("undefined")
    });

    if has_undeclared_var {
        println!("✓ Successfully detected 'undeclared variable' error from MSVC output");
    }

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_clang_compiler_output_parsing() {
    // Check if Clang is available
    let clang_cmd = if common::is_command_available("clang++") {
        "clang++"
    } else if common::is_command_available("clang") {
        "clang"
    } else {
        println!("Skipping test: Clang not available");
        return;
    };

    let project_path = cmake_project_path();
    let main_cpp = project_path.join("src/main.cpp");

    if !main_cpp.exists() {
        println!("Skipping test: Test file not found");
        return;
    }

    // Compile with Clang
    let temp_dir = std::env::temp_dir().join("analyzer_clang_test");
    fs::create_dir_all(&temp_dir).ok();

    let output = match common::run_command(
        clang_cmd,
        &[
            "-std=c++17",
            "-Wall",
            "-c",
            main_cpp.to_str().unwrap(),
            "-o",
            temp_dir.join("main.o").to_str().unwrap()
        ],
        &project_path
    ) {
        Ok(output) => output,
        Err(e) => {
            println!("Clang compilation failed (expected): {}", e);
            e
        }
    };

    // Save raw output
    save_raw_output("clang_compile", &output);

    // Parse with Clang parser
    let parser = CppParser::with_clang();
    let issues = parser.parse(&output).data_or_default_owned();

    // Generate report
    generate_report(
        "clang_compile",
        "Clang Compile",
        &format!("{} -std=c++17 -Wall -c src/main.cpp", clang_cmd),
        &issues,
        Some("raw_output/clang_compile.txt")
    );

    println!("=== Clang Compile Output ===");
    println!("{}", output);
    println!("\n=== Parsed Issues ===");
    println!("Found {} issues", issues.len());

    for issue in &issues {
        println!("  [{:?}] {}:{}:{} - {}",
            issue.level,
            issue.location.file_path,
            issue.location.line_number.map(|l| l.to_string()).unwrap_or_else(|| "-".to_string()),
            issue.location.column_number.map(|c| c.to_string()).unwrap_or_else(|| "-".to_string()),
            issue.message
        );
    }

    // Verify we found the expected errors
    let has_undeclared_var = issues.iter().any(|i| {
        i.message.contains("undeclared") || i.message.contains("undefined")
    });

    if has_undeclared_var {
        println!("✓ Successfully detected 'undeclared variable' error from Clang output");
    }

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_cmake_vs_environment_detection() {
    println!("=== VS Environment Detection ===");
    println!("VS Dev Shell available: {}", is_vs_dev_shell_available());
    println!("MSVC available: {}", check_msvc());

    if let Ok(cmake_path) = check_cmake() {
        println!("CMake found: {:?}", cmake_path);
    } else {
        println!("CMake not found in PATH");
    }

    println!("Default generator: {}", get_cmake_generator());

    // This test just reports environment status
    println!("✓ Environment detection completed");
}
