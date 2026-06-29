//! CMake Parser Unit Tests
//! Test CMakeParser for CMake configuration and build output

use std::fs;

mod common;
use common::samples_dir;

use analyzer::core::{IssueLevel, OutputParser};
use analyzer::plugins::cpp::cmake::parser::CMakeParser;

/// Read sample file content
fn read_sample(name: &str) -> String {
    let path = samples_dir().join(format!("{}.txt", name));
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read sample file {}: {}", path.display(), e))
}

#[test]
fn test_cmake_parser_basic() {
    let content = read_sample("cmake_basic_sample");
    let parser = CMakeParser::new();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // Should parse both CMake errors/warnings and compiler errors
    assert!(!issues.is_empty(), "Should parse issues from CMake output");

    // Check for CMake errors
    let cmake_errors: Vec<_> = issues
        .iter()
        .filter(|i| i.code.as_ref().is_some_and(|c| c.contains("CMake Error")))
        .collect();
    assert!(!cmake_errors.is_empty(), "Should have CMake errors");

    // Check for CMake warnings
    let cmake_warnings: Vec<_> = issues
        .iter()
        .filter(|i| i.code.as_ref().is_some_and(|c| c.contains("CMake Warning")))
        .collect();
    assert!(!cmake_warnings.is_empty(), "Should have CMake warnings");

    // Check for compiler errors (parsed by CppParser)
    let compiler_errors: Vec<_> = issues
        .iter()
        .filter(|i| {
            matches!(i.level, IssueLevel::Error)
                && i.code.as_ref().is_none_or(|c| !c.contains("CMake"))
        })
        .collect();

    println!("✓ CMake parser correctly parsed {} issues ({} CMake errors, {} CMake warnings, {} compiler issues)",
             issues.len(), cmake_errors.len(), cmake_warnings.len(), compiler_errors.len());
}

#[test]
fn test_cmake_error_parsing() {
    let output = r#"
CMake Error at CMakeLists.txt:10 (add_executable):
  Cannot find source file:
    src/main.cpp
"#;

    let parser = CMakeParser::new();
    let issues = OutputParser::parse(&parser, output).data_or_default_owned();

    assert_eq!(issues.len(), 1, "Should parse exactly 1 CMake error");

    let issue = &issues[0];
    assert!(matches!(issue.level, IssueLevel::Error));
    assert!(issue.location.file_path.contains("CMakeLists.txt"));
    assert_eq!(issue.location.line_number, Some(10));
    // Now captures multi-line message
    assert!(
        issue.message.contains("Cannot find source file"),
        "Expected message to contain 'Cannot find source file', got: '{}'",
        issue.message
    );
    assert!(issue.code.as_ref().unwrap().contains("CMake Error"));

    println!(
        "✓ CMake error parsing works correctly (message: '{}')",
        issue.message
    );
}

#[test]
fn test_cmake_warning_parsing() {
    let output = r#"
CMake Warning at cmake/FindPackage.cmake:25 (find_package):
  Could not find a package configuration file
"#;

    let parser = CMakeParser::new();
    let issues = OutputParser::parse(&parser, output).data_or_default_owned();

    assert_eq!(issues.len(), 1, "Should parse exactly 1 CMake warning");

    let issue = &issues[0];
    assert!(matches!(issue.level, IssueLevel::Warning));
    assert!(issue.location.file_path.contains("FindPackage.cmake"));
    assert_eq!(issue.location.line_number, Some(25));
    // Now captures multi-line message
    assert!(
        issue.message.contains("Could not find"),
        "Expected message to contain 'Could not find', got: '{}'",
        issue.message
    );
    assert!(issue.code.as_ref().unwrap().contains("CMake Warning"));

    println!(
        "✓ CMake warning parsing works correctly (message: '{}')",
        issue.message
    );
}

#[test]
fn test_cmake_with_compiler_output() {
    let output = r#"
[ 50%] Building CXX object CMakeFiles/myapp.dir/src/main.cpp.o
src/main.cpp:10:5: error: 'x' was not declared in this scope
   10 |     int y = x + 1;
      |     ^~~~~
[ 75%] Building CXX object CMakeFiles/myapp.dir/src/utils.cpp.o
src/utils.cpp:25:12: warning: unused variable 'tmp' [-Wunused-variable]
"#;

    let parser = CMakeParser::new();
    let issues = OutputParser::parse(&parser, output).data_or_default_owned();

    assert_eq!(issues.len(), 2, "Should parse 2 compiler issues");

    // Check first error
    let error = &issues[0];
    assert!(matches!(error.level, IssueLevel::Error));
    assert!(error.location.file_path.contains("main.cpp"));

    // Check second warning
    let warning = &issues[1];
    assert!(matches!(warning.level, IssueLevel::Warning));
    assert!(warning.location.file_path.contains("utils.cpp"));

    println!("✓ CMake with compiler output parsing works correctly");
}

#[test]
fn test_empty_output() {
    let parser = CMakeParser::new();
    let issues = OutputParser::parse(&parser, "").data_or_default_owned();
    assert!(issues.is_empty(), "Empty output should produce no issues");

    println!("✓ Empty output handling works correctly");
}

#[test]
fn test_cmake_target_link_error() {
    let output = r#"
CMake Error at CMakeLists.txt:15 (target_link_libraries):
  Cannot specify link libraries for target "myapp" which is not built by
  this project.
"#;

    let parser = CMakeParser::new();
    let issues = OutputParser::parse(&parser, output).data_or_default_owned();

    assert_eq!(issues.len(), 1);
    let issue = &issues[0];
    assert!(matches!(issue.level, IssueLevel::Error));
    assert!(issue.location.file_path.contains("CMakeLists.txt"));
    assert_eq!(issue.location.line_number, Some(15));
    // Now captures multi-line message
    assert!(
        issue.message.contains("Cannot specify link libraries"),
        "Expected message to contain 'Cannot specify link libraries', got: '{}'",
        issue.message
    );

    println!(
        "✓ CMake target_link_libraries error parsing works correctly (message: '{}')",
        issue.message
    );
}

#[test]
fn test_cmake_find_package_warning() {
    let output = r#"
CMake Warning at cmake/FindBoost.cmake:42 (find_package):
  Could not find a package configuration file provided by "Boost" with any
  of the following names:
    BoostConfig.cmake
    boost-config.cmake
"#;

    let parser = CMakeParser::new();
    let issues = OutputParser::parse(&parser, output).data_or_default_owned();

    assert_eq!(issues.len(), 1);
    let issue = &issues[0];
    assert!(matches!(issue.level, IssueLevel::Warning));
    assert!(issue.location.file_path.contains("FindBoost.cmake"));
    assert_eq!(issue.location.line_number, Some(42));
    // Now captures multi-line message
    assert!(
        issue
            .message
            .contains("Could not find a package configuration file"),
        "Expected message to contain 'Could not find a package configuration file', got: '{}'",
        issue.message
    );

    println!(
        "✓ CMake find_package warning parsing works correctly (message: '{}')",
        issue.message
    );
}
