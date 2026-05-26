//! Visual Studio Environment Setup
//! Provides utilities to activate VS Dev Shell for MSVC compiler access

#![cfg(test)]
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

/// Path to vcvarsall.bat for VS environment initialization
const VS_VARSALL_PATH: &str = r"D:\softwares\Visual Studio\VC\Auxiliary\Build\vcvarsall.bat";

/// Check if VS Dev Shell is available (via vcvarsall.bat)
pub fn is_vs_dev_shell_available() -> bool {
    PathBuf::from(VS_VARSALL_PATH).exists()
}

/// Run a command with VS environment activated
/// This executes the command through PowerShell with VS Dev Shell pre-activated
pub fn run_with_vs_env(cmd: &str, args: &[&str], cwd: &PathBuf) -> Result<String, String> {
    if !is_vs_dev_shell_available() {
        return Err(format!(
            "VS Dev Shell not found at: {}. Please install Visual Studio or update the path.",
            VS_VARSALL_PATH
        ));
    }

    println!("Activating VS Dev Shell environment...");

    // Build the command string (with proper quoting)
    let cmd_str = args.iter().fold(cmd.to_string(), |acc, arg| {
        format!("{} {}", acc, if arg.contains(' ') { format!("\"{}\"", arg) } else { arg.to_string() })
    });

    // Execute via cmd.exe with vcvarsall.bat to set VS environment variables
    // vcvarsall.bat sets environment in the cmd session, then runs the command
    let cmd_script = format!(
        "\"{}\" x64 && {}",
        VS_VARSALL_PATH,
        cmd_str
    );

    println!("Executing in VS environment: {}", cmd_str);

    let output = Command::new("cmd.exe")
        .args(["/D", "/S", "/C", &cmd_script])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("Failed to execute command in VS environment: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Merge stdout and stderr
    let full_output = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{}{}", stdout, stderr)
    };

    Ok(full_output)
}

/// Check if CMake is available (with or without VS environment)
pub fn check_cmake() -> Result<PathBuf, String> {
    // First try to find cmake directly
    if let Some(path) = super::resolve_command("cmake") {
        return Ok(path);
    }

    // If not found and we're on Windows, suggest VS environment
    #[cfg(windows)]
    {
        if is_vs_dev_shell_available() {
            return Err(
                "CMake not found in PATH. Try activating VS Dev Shell first or use run_with_vs_env().".to_string()
            );
        }
    }

    Err("CMake not found in PATH. Please install CMake or Visual Studio.".to_string())
}

/// Check if MSVC compiler is available
pub fn check_msvc() -> bool {
    // Check for cl.exe
    super::is_command_available("cl") || {
        // Try with VS environment
        if is_vs_dev_shell_available() {
            // Quick test to see if cl is available after activating VS shell via vcvarsall.bat
            let test_output = Command::new("cmd.exe")
                .args([
                    "/D", "/S", "/C",
                    &format!("\"{}\" x64 >nul 2>&1 && where cl >nul 2>&1", VS_VARSALL_PATH)
                ])
                .output();

            if let Ok(output) = test_output {
                return output.status.success();
            }
        }
        false
    }
}

/// Get the appropriate CMake generator for the current environment
pub fn get_cmake_generator() -> &'static str {
    #[cfg(windows)]
    {
        if check_msvc() {
            // Use Visual Studio generator if MSVC is available
            "Visual Studio 17 2022"
        } else {
            // Fall back to Ninja or MinGW
            "Ninja"
        }
    }
    #[cfg(not(windows))]
    {
        "Unix Makefiles"
    }
}
