//! C++ Parser Unit Tests
//! Test CppParser for GCC, Clang, and MSVC output formats

use std::fs;

mod common;
use common::samples_dir;

use analyzer::core::{IssueLevel, OutputParser};
use analyzer::plugins::cpp::parser::{CppParser, CompilerType};

/// Read sample file content
fn read_sample(name: &str) -> String {
    let path = samples_dir().join(format!("{}.txt", name));
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read sample file {}: {}", path.display(), e))
}

/// Count issues by level
fn count_issues_by_level(issues: &[analyzer::core::Issue]) -> (usize, usize, usize, usize) {
    let errors = issues.iter().filter(|i| matches!(i.level, IssueLevel::Error)).count();
    let warnings = issues.iter().filter(|i| matches!(i.level, IssueLevel::Warning)).count();
    let infos = issues.iter().filter(|i| matches!(i.level, IssueLevel::Info)).count();
    let hints = issues.iter().filter(|i| matches!(i.level, IssueLevel::Hint)).count();
    (errors, warnings, infos, hints)
}

#[test]
fn test_gcc_parser_basic() {
    let content = read_sample("gcc_basic_sample");
    let parser = CppParser::with_gcc();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // Should parse 4 issues (2 errors, 1 warning, 1 note)
    assert_eq!(issues.len(), 4, "Should parse 4 issues from GCC output");

    let (errors, warnings, infos, _) = count_issues_by_level(&issues);
    assert_eq!(errors, 2, "Should have 2 errors");
    assert_eq!(warnings, 1, "Should have 1 warning");
    assert_eq!(infos, 1, "Should have 1 info (note)");

    // Check first error
    let first = &issues[0];
    assert!(first.location.file_path.contains("main.cpp"));
    assert_eq!(first.location.line_number, Some(10));
    assert_eq!(first.location.column_number, Some(5));
    assert!(matches!(first.level, IssueLevel::Error));
    assert!(first.message.contains("not declared"));

    // Check warning
    let warning = &issues[1];
    assert!(warning.location.file_path.contains("utils.cpp"));
    assert_eq!(warning.location.line_number, Some(25));
    assert!(matches!(warning.level, IssueLevel::Warning));
    assert!(warning.message.contains("unused variable"));

    // Check note
    let note = &issues[2];
    assert!(note.location.file_path.contains("math.cpp"));
    assert!(matches!(note.level, IssueLevel::Info));
    assert!(note.message.contains("suggested alternative"));

    println!("✓ GCC parser correctly parsed {} issues ({} errors, {} warnings, {} infos)",
             issues.len(), errors, warnings, infos);
}

#[test]
fn test_clang_parser_basic() {
    let content = read_sample("clang_basic_sample");
    let parser = CppParser::with_clang();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // Should parse 4 issues
    assert_eq!(issues.len(), 4, "Should parse 4 issues from Clang output");

    let (errors, warnings, infos, _) = count_issues_by_level(&issues);
    assert_eq!(errors, 2, "Should have 2 errors");
    assert_eq!(warnings, 1, "Should have 1 warning");
    assert_eq!(infos, 1, "Should have 1 info (note)");

    // Check error
    let error = &issues[0];
    assert!(error.location.file_path.contains("main.cpp"));
    assert_eq!(error.location.line_number, Some(10));
    assert_eq!(error.location.column_number, Some(5));
    assert!(matches!(error.level, IssueLevel::Error));
    assert!(error.message.contains("undeclared identifier"));

    println!("✓ Clang parser correctly parsed {} issues ({} errors, {} warnings, {} infos)",
             issues.len(), errors, warnings, infos);
}

#[test]
fn test_msvc_parser_basic() {
    let content = read_sample("msvc_basic_sample");
    let parser = CppParser::with_msvc();
    let issues = OutputParser::parse(&parser, &content).data_or_default_owned();

    // Should parse 4 issues (3 errors including fatal, 1 warning)
    assert_eq!(issues.len(), 4, "Should parse 4 issues from MSVC output");

    let (errors, warnings, _, _) = count_issues_by_level(&issues);
    assert_eq!(errors, 3, "Should have 3 errors (including fatal error)");
    assert_eq!(warnings, 1, "Should have 1 warning");

    // Check first error
    let first = &issues[0];
    assert!(first.location.file_path.contains("main.cpp"));
    assert_eq!(first.location.line_number, Some(10));
    assert_eq!(first.location.column_number, Some(5));
    assert!(matches!(first.level, IssueLevel::Error));
    assert!(first.message.contains("undeclared identifier"));

    // Check warning
    let warning = &issues[1];
    assert!(warning.location.file_path.contains("utils.cpp"));
    assert_eq!(warning.location.line_number, Some(25));
    assert!(matches!(warning.level, IssueLevel::Warning));
    assert!(warning.message.contains("unreferenced local variable"));

    // Check fatal error
    let fatal = &issues[2];
    assert!(fatal.location.file_path.contains("math.cpp"));
    assert_eq!(fatal.location.line_number, Some(42));
    assert!(matches!(fatal.level, IssueLevel::Error));
    assert!(fatal.message.contains("Cannot open include file"));

    println!("✓ MSVC parser correctly parsed {} issues ({} errors, {} warnings)",
             issues.len(), errors, warnings);
}

#[test]
fn test_compiler_type_detection() {
    // Test GCC detection
    let gcc_output = "gcc version 11.2.0 (Ubuntu 11.2.0-19ubuntu1)";
    assert!(matches!(CppParser::detect_compiler_type(gcc_output), CompilerType::Gcc));

    // Test Clang detection
    let clang_output = "clang version 14.0.0";
    assert!(matches!(CppParser::detect_compiler_type(clang_output), CompilerType::Clang));

    // Test MSVC detection
    let msvc_output = "Microsoft (R) C/C++ Optimizing Compiler Version 19.29";
    assert!(matches!(CppParser::detect_compiler_type(msvc_output), CompilerType::Msvc));

    // Test default (unknown should default to GCC)
    let unknown = "some random output";
    assert!(matches!(CppParser::detect_compiler_type(unknown), CompilerType::Gcc));

    println!("✓ Compiler type detection works correctly");
}



#[test]
fn test_gcc_parser_with_error_code() {
    let output = "src/test.cpp:15:10: error: invalid conversion [-fpermissive]";
    let parser = CppParser::with_gcc();
    let issues = OutputParser::parse(&parser, output).data_or_default_owned();

    assert_eq!(issues.len(), 1);
    let issue = &issues[0];
    assert!(issue.code.is_some());
    assert!(issue.code.as_ref().unwrap().contains("fpermissive"));

    println!("✓ GCC parser correctly extracts error code");
}

#[test]
fn test_msvc_parser_with_error_code() {
    let output = "src\\test.cpp(15,10): error C2440: 'initializing': cannot convert";
    let parser = CppParser::with_msvc();
    let issues = OutputParser::parse(&parser, output).data_or_default_owned();

    assert_eq!(issues.len(), 1);
    let issue = &issues[0];
    assert!(issue.code.is_some());
    assert!(issue.code.as_ref().unwrap().contains("C2440"));

    println!("✓ MSVC parser correctly extracts error code");
}

#[test]
fn test_empty_output() {
    let parsers = [
        CppParser::with_gcc(),
        CppParser::with_clang(),
        CppParser::with_msvc(),
    ];

    for parser in &parsers {
        let issues = OutputParser::parse(parser, "").data_or_default_owned();
        assert!(issues.is_empty(), "Empty output should produce no issues");
    }

    println!("✓ Empty output handling works correctly");
}

#[test]
fn test_mixed_content() {
    // Output with both issues and non-issue lines
    let output = r#"
[ 50%] Building CXX object CMakeFiles/app.dir/src/main.cpp.o
src/main.cpp:10:5: error: 'x' was not declared in this scope
   10 |     int y = x + 1;
      |     ^~~~~
[ 75%] Building CXX object CMakeFiles/app.dir/src/utils.cpp.o
src/utils.cpp:25:12: warning: unused variable 'tmp' [-Wunused-variable]
[100%] Linking CXX executable app
"#;

    let parser = CppParser::with_gcc();
    let issues = OutputParser::parse(&parser, output).data_or_default_owned();

    assert_eq!(issues.len(), 2, "Should parse exactly 2 issues from mixed output");

    println!("✓ Mixed content parsing works correctly");
}
