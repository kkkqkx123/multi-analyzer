//! command execution tool
//! Provides unified command construction and execution functions and supports cross-platform command lookup

#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::analyzer::AnalyzerError;
use super::stream::{FilterMode, StreamResult};

/// Default command timeout (5 minutes)
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// Command execution result
/// Contains the output and success status of the command
#[derive(Debug)]
pub struct CommandOutput {
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Combined stdout and stderr
    pub combined: String,
    pub status: ExitStatus,
}

impl CommandOutput {
    /// Check if the command was successful
    pub fn success(&self) -> bool {
        self.status.success()
    }

    /// Get the exit code
    pub fn code(&self) -> Option<i32> {
        self.status.code()
    }

    /// Get stdout output
    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    /// Get stderr output
    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    /// Get combined stdout and stderr output
    pub fn combined(&self) -> &str {
        &self.combined
    }
}

/// Chainable options for configuring command execution
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Working directory for the command
    pub current_dir: Option<PathBuf>,
    /// Environment variable overrides
    pub envs: HashMap<String, String>,
    /// Whether to show the command being executed
    pub verbose: bool,
    /// Command timeout duration
    pub timeout: Duration,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            current_dir: None,
            envs: HashMap::new(),
            verbose: true,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

impl RunOptions {
    /// Set working directory
    pub fn with_current_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(dir.into());
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.insert(key.into(), value.into());
        self
    }

    /// Suppress command logging
    pub fn quiet(mut self) -> Self {
        self.verbose = false;
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Get full path to commands (cross-platform)
/// On Windows, executable extensions such as.cmd, .bat, and.exe are first found
pub fn resolve_command(cmd: &str) -> Option<PathBuf> {
    let path = Path::new(cmd);
    if path.is_absolute() || path.components().count() > 1 {
        return Some(path.to_path_buf());
    }

    #[cfg(windows)]
    let check_cmd = "where";
    #[cfg(not(windows))]
    let check_cmd = "which";

    let output = Command::new(check_cmd).arg(cmd).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let paths: Vec<PathBuf> = stdout.lines().map(PathBuf::from).collect();

    #[cfg(windows)]
    {
        let priority = ["cmd", "bat", "exe"];
        for ext in &priority {
            if let Some(path) = paths.iter().find(|p| {
                p.extension()
                    .map(|e| e.to_string_lossy().to_lowercase() == *ext)
                    .unwrap_or(false)
            }) {
                return Some(path.clone());
            }
        }
    }

    paths.into_iter().next()
}

/// command builder
/// Used to build and execute external commands
pub struct CommandBuilder {
    program: String,
    args: Vec<String>,
    options: RunOptions,
}

impl CommandBuilder {
    /// Create a new command builder
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            options: RunOptions::default(),
        }
    }

    /// Create a command builder from an execution string (e.g., "cargo check --all-targets")
    pub fn from_exec_string(exec_str: &str) -> Self {
        let parts: Vec<&str> = exec_str.split_whitespace().collect();
        if parts.is_empty() {
            return Self::new("");
        }

        let mut builder = Self::new(parts[0]);
        for arg in &parts[1..] {
            builder = builder.arg(*arg);
        }
        builder
    }

    /// Add a single parameter
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Set working directory
    pub fn current_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.options.current_dir = Some(dir.into());
        self
    }

    /// Add an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.envs.insert(key.into(), value.into());
        self
    }

    /// Set command timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = timeout;
        self
    }

    /// Suppress command execution logging
    pub fn quiet(mut self) -> Self {
        self.options.verbose = false;
        self
    }

    /// Apply run options to the builder
    pub fn with_options(mut self, options: RunOptions) -> Self {
        self.options = options;
        self
    }

    /// Build a std::process::Command from the builder configuration
    pub fn build(&self) -> Command {
        let program = resolve_command(&self.program)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| self.program.clone());

        let mut cmd = Command::new(&program);
        cmd.args(&self.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(dir) = &self.options.current_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in &self.options.envs {
            cmd.env(key, value);
        }

        cmd
    }

    /// Get the full command string for filter registry lookup (e.g. "cargo clippy --all-targets").
    pub fn command_string(&self) -> String {
        let mut parts = vec![self.program.as_str()];
        parts.extend(self.args.iter().map(|s| s.as_str()));
        parts.join(" ")
    }

    /// Execute command and capture output (with timeout)
    /// Returns the combined stdout and stderr output
    pub fn execute(&self) -> Result<String, AnalyzerError> {
        let output = self.execute_with_status()?;
        let mut combined = output.combined;

        let exit_code = output.status.code().unwrap_or(1);
        let cmd_slug = self.command_string();
        if let Some(hint) = crate::config::tee_writer::tee_and_hint(&combined, &cmd_slug, exit_code) {
            combined.push('\n');
            combined.push_str(&hint);
        }

        Ok(combined)
    }

    /// Execute command and capture output with full status information
    pub fn execute_with_status(&self) -> Result<CommandOutput, AnalyzerError> {
        let program = resolve_command(&self.program)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| self.program.clone());

        if self.options.verbose {
            println!("Running: {} {}", program, self.args.join(" "));
        }

        let (tx, rx) = mpsc::channel();
        let program = program.to_string();
        let args = self.args.clone();
        let timeout = self.options.timeout;
        let current_dir = self.options.current_dir.clone();
        let envs = self.options.envs.clone();

        thread::spawn(move || {
            let mut cmd = Command::new(&program);
            cmd.args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            if let Some(dir) = &current_dir {
                cmd.current_dir(dir);
            }

            for (key, value) in &envs {
                cmd.env(key, value);
            }

            let output = cmd.output();
            let _ = tx.send(output);
        });

        let result = rx
            .recv_timeout(timeout)
            .map_err(|_| AnalyzerError::Timeout(timeout))?;

        let output = result.map_err(|e| {
            AnalyzerError::CommandFailed(format!(
                "Failed to execute {}: {}. Hint: Make sure '{}' is installed and in PATH",
                self.program, e, self.program
            ))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{}{}", stdout, stderr);

        Ok(CommandOutput {
            stdout,
            stderr,
            combined,
            status: output.status,
        })
    }

    /// Execute command with streaming line-by-line filtering.
    ///
    /// Spawns the command, reads stdout/stderr in parallel threads,
    /// merges via mpsc channel, and applies a LineFilter to each line.
    /// Returns raw output plus filtered result.
    ///
    /// # Modes
    /// - `FilterMode::Streaming(filter)`: apply line filter during execution (memory efficient)
    /// - `FilterMode::Passthrough`: inherit stdio, don't capture output
    pub fn execute_streaming(&self, mode: FilterMode<'_>) -> Result<StreamResult, AnalyzerError> {
        match mode {
            FilterMode::Passthrough => {
                let mut cmd = self.build();
                cmd.stdin(Stdio::inherit());
                cmd.stdout(Stdio::inherit());
                cmd.stderr(Stdio::inherit());
                let status = cmd.status().map_err(|e| {
                    AnalyzerError::CommandFailed(format!(
                        "Failed to execute {}: {}",
                        self.program, e
                    ))
                })?;
                Ok(StreamResult {
                    exit_code: status.code().unwrap_or(1),
                    raw_stdout: None,
                    raw_stderr: None,
                    filtered: String::new(),
                })
            }
            FilterMode::Streaming(mut filter) => {
                let mut cmd = self.build();
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());

                if self.options.verbose {
                    println!("Running: {} {}", self.program, self.args.join(" "));
                }

                let mut child = cmd.spawn().map_err(|e| {
                    AnalyzerError::CommandFailed(format!("Failed to spawn {}: {}", self.program, e))
                })?;

                let stdout = child.stdout.take().ok_or_else(|| {
                    AnalyzerError::CommandFailed("No child stdout handle".to_string())
                })?;
                let stderr = child.stderr.take().ok_or_else(|| {
                    AnalyzerError::CommandFailed("No child stderr handle".to_string())
                })?;

                #[derive(Debug)]
                enum StreamLine {
                    Stdout(String),
                    Stderr(String),
                }

                let (tx, rx) = mpsc::channel::<StreamLine>();
                let tx_out = tx.clone();

                thread::spawn(move || {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        if tx_out.send(StreamLine::Stdout(line)).is_err() {
                            break;
                        }
                    }
                });

                let tx_err = tx;
                thread::spawn(move || {
                    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                        if tx_err.send(StreamLine::Stderr(line)).is_err() {
                            break;
                        }
                    }
                });

                let mut raw_stdout = String::new();
                let mut raw_stderr = String::new();
                let mut filtered = String::new();
                let collect_raw = self.options.verbose;

                for msg in rx {
                    match msg {
                        StreamLine::Stdout(line) => {
                            if collect_raw {
                                raw_stdout.push_str(&line);
                                raw_stdout.push('\n');
                            }
                            if let Some(f) = filter.feed_line(&line) {
                                filtered.push_str(&f);
                                filtered.push('\n');
                            }
                        }
                        StreamLine::Stderr(line) => {
                            if collect_raw {
                                raw_stderr.push_str(&line);
                                raw_stderr.push('\n');
                            }
                            if let Some(f) = filter.feed_line(&line) {
                                filtered.push_str(&f);
                                filtered.push('\n');
                            }
                        }
                    }
                }

                for tail_line in filter.on_complete() {
                    filtered.push_str(&tail_line);
                    filtered.push('\n');
                }

                let status = child.wait().map_err(AnalyzerError::IoError)?;

                Ok(StreamResult {
                    exit_code: status.code().unwrap_or(1),
                    raw_stdout: if collect_raw { Some(raw_stdout) } else { None },
                    raw_stderr: if collect_raw { Some(raw_stderr) } else { None },
                    filtered,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_command_cargo() {
        let resolved = resolve_command("cargo");
        assert!(resolved.is_some(), "cargo should be found in PATH");
    }

    #[test]
    fn test_resolve_command_nonexistent() {
        let resolved = resolve_command("this_command_definitely_does_not_exist_12345");
        assert!(resolved.is_none());
    }

    #[test]
    fn test_run_options_chain() {
        let opts = RunOptions::default()
            .quiet()
            .with_timeout(Duration::from_secs(60));
        assert!(!opts.verbose);
        assert_eq!(opts.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_command_builder_chain() {
        let builder = CommandBuilder::new("cargo")
            .arg("check")
            .quiet()
            .timeout(Duration::from_secs(120));
        assert!(!builder.options.verbose);
        assert_eq!(builder.args, vec!["check"]);
    }

    #[test]
    fn test_from_exec_string() {
        let builder = CommandBuilder::from_exec_string("cargo check --all-targets");
        assert_eq!(builder.program, "cargo");
        assert_eq!(builder.args, vec!["check", "--all-targets"]);
    }
}
