//! Command Execution Tests
//! Test the execute_with_status method and error handling behavior

use analyzer::core::CommandBuilder;

mod common;

/// Get a simple command that works on both Windows and Unix
#[cfg(windows)]
fn get_test_command() -> (&'static str, Vec<&'static str>) {
    ("cmd", vec!["/c", "echo", "Hello, World!"])
}

#[cfg(not(windows))]
fn get_test_command() -> (&'static str, Vec<&'static str>) {
    ("echo", vec!["Hello, World!"])
}

/// Get a command that will fail
#[cfg(windows)]
fn get_failing_command() -> (&'static str, Vec<&'static str>) {
    ("cmd", vec!["/c", "exit", "1"])
}

#[cfg(not(windows))]
fn get_failing_command() -> (&'static str, Vec<&'static str>) {
    ("false", vec![])
}

/// Get a command that outputs to stderr
#[cfg(windows)]
fn get_stderr_command() -> (&'static str, Vec<&'static str>) {
    ("cmd", vec!["/c", "echo error message 1>&2"])
}

#[cfg(not(windows))]
fn get_stderr_command() -> (&'static str, Vec<&'static str>) {
    ("sh", vec!["-c", "echo error message >&2"])
}

#[test]
fn test_command_execute_with_status_success() {
    // Test successful command execution
    let (cmd, args) = get_test_command();
    let mut builder = CommandBuilder::new(cmd);
    for arg in args {
        builder = builder.arg(arg);
    }
    let result = builder.execute_with_status();

    assert!(result.is_ok(), "Command should execute successfully");

    let output = result.unwrap();
    assert!(output.success(), "Command should return success status");
    assert!(
        output.combined.contains("Hello, World!"),
        "Output should contain the echoed text"
    );
    assert!(output.code().is_some(), "Exit code should be available");
    assert_eq!(output.code(), Some(0), "Exit code should be 0 for success");
}

#[test]
fn test_command_execute_with_status_failure() {
    // Test failed command execution
    let (cmd, args) = get_failing_command();
    let mut builder = CommandBuilder::new(cmd);
    for arg in args {
        builder = builder.arg(arg);
    }
    let result = builder.execute_with_status();

    // Command should execute (not fail to spawn)
    assert!(result.is_ok(), "Command should be able to spawn");

    let output = result.unwrap();
    assert!(!output.success(), "Command should return failure status");
    assert!(output.code().is_some(), "Exit code should be available");
    assert_ne!(
        output.code(),
        Some(0),
        "Exit code should be non-zero for failure"
    );
}

#[test]
fn test_command_execute_with_status_stderr_capture() {
    // Test that stderr is captured properly
    let (cmd, args) = get_stderr_command();
    let mut builder = CommandBuilder::new(cmd);
    for arg in args {
        builder = builder.arg(arg);
    }

    let result = builder.execute_with_status();
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.success());
    // Combined output should contain stderr
    assert!(
        output.combined.contains("error message"),
        "Combined output should capture stderr"
    );
}

#[test]
fn test_command_execute_backward_compatibility() {
    // Test that the original execute() method still works
    let (cmd, args) = get_test_command();
    let mut builder = CommandBuilder::new(cmd);
    for arg in args {
        builder = builder.arg(arg);
    }
    let result = builder.execute();

    assert!(result.is_ok());
    assert!(result.unwrap().contains("Hello, World!"));
}

#[test]
fn test_command_execute_with_status_nonexistent_command() {
    // Test behavior with non-existent command
    let builder = CommandBuilder::new("this_command_definitely_does_not_exist_12345");
    let result = builder.execute_with_status();

    // Should return an error because the command cannot be found
    assert!(
        result.is_err(),
        "Non-existent command should return an error"
    );
}

#[test]
fn test_command_output_fields() {
    // Test that all CommandOutput fields are populated correctly
    let (cmd, args) = get_test_command();
    let mut builder = CommandBuilder::new(cmd);
    for arg in args {
        builder = builder.arg(arg);
    }
    let result = builder.execute_with_status().unwrap();

    // Check that stdout and combined are populated
    assert!(!result.stdout.is_empty(), "stdout should not be empty");
    assert!(
        result.stdout.contains("Hello, World!"),
        "stdout should contain the output"
    );
    assert!(
        result.combined.contains("Hello, World!"),
        "combined should contain the output"
    );
}
