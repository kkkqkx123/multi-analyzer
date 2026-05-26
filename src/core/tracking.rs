//! Execution tracking and history
//! Records command executions, issue counts, and timing for historical analysis

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Local, Timelike};
use serde::{Deserialize, Serialize};

/// The tracking data file name
const TRACKING_FILE: &str = "analyzer-history.json";

/// A single recorded execution entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Timestamp of execution
    pub timestamp: String,
    /// The tech stack used (e.g., "cargo", "npm", "mypy")
    pub tech_stack: String,
    /// The command executed (e.g., "check", "lint", "audit")
    pub command: String,
    /// Number of issues found
    pub issue_count: usize,
    /// Number of errors found
    pub error_count: usize,
    /// Number of warnings found
    pub warning_count: usize,
    /// Execution time in milliseconds
    pub exec_time_ms: u64,
    /// Whether the analysis succeeded
    pub success: bool,
    /// Working directory
    pub project_dir: String,
}

impl HistoryEntry {
    pub fn new(
        tech_stack: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: Local::now().to_rfc3339(),
            tech_stack: tech_stack.into(),
            command: command.into(),
            issue_count: 0,
            error_count: 0,
            warning_count: 0,
            exec_time_ms: 0,
            success: true,
            project_dir: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        }
    }

    /// Set issue counts
    pub fn with_issue_counts(mut self, total: usize, errors: usize, warnings: usize) -> Self {
        self.issue_count = total;
        self.error_count = errors;
        self.warning_count = warnings;
        self
    }

    /// Set execution time in milliseconds
    pub fn with_exec_time(mut self, ms: u64) -> Self {
        self.exec_time_ms = ms;
        self
    }

    /// Set success status
    pub fn with_success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }
}

/// Execution history storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    /// All recorded entries
    pub entries: Vec<HistoryEntry>,
    /// Maximum number of entries to store
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: default_max_entries(),
        }
    }
}

fn default_max_entries() -> usize {
    1000
}

impl History {
    /// Load history from the default file
    pub fn load() -> Self {
        let path = Self::default_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(history) = serde_json::from_str(&content) {
                    return history;
                }
            }
        }
        Self::default()
    }

    /// Save history to the default file
    pub fn save(&self) {
        if let Some(parent) = Self::default_path().parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(Self::default_path(), &content);
        }
    }

    /// Add a new entry to the history
    pub fn add_entry(&mut self, entry: HistoryEntry) {
        self.entries.push(entry);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
        self.save();
    }

    /// Get recent entries (most recent first)
    pub fn recent(&self, count: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(count).collect()
    }

    /// Get total number of entries
    pub fn total_count(&self) -> usize {
        self.entries.len()
    }

    /// Get summary statistics
    pub fn summary(&self) -> HistorySummary {
        let total = self.entries.len();
        let total_issues: usize = self.entries.iter().map(|e| e.issue_count).sum();
        let total_errors: usize = self.entries.iter().map(|e| e.error_count).sum();
        let total_warnings: usize = self.entries.iter().map(|e| e.warning_count).sum();
        let total_time: u64 = self.entries.iter().map(|e| e.exec_time_ms).sum();
        let successful = self.entries.iter().filter(|e| e.success).count();

        // Count by tech stack
        let mut by_stack: Vec<(String, usize)> = {
            let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for entry in &self.entries {
                *counts.entry(entry.tech_stack.clone()).or_insert(0) += 1;
            }
            let mut v: Vec<_> = counts.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            v
        };

        // Today's count
        let today = Local::now().date_naive();
        let today_count = self
            .entries
            .iter()
            .filter(|e| {
                DateTime::parse_from_rfc3339(&e.timestamp)
                    .map(|dt| dt.date_naive() == today)
                    .unwrap_or(false)
            })
            .count();

        HistorySummary {
            total,
            successful,
            failed: total - successful,
            total_issues,
            total_errors,
            total_warnings,
            total_time_ms: total_time,
            today_count,
            by_stack,
        }
    }

    /// Get the default path for the tracking file
    fn default_path() -> PathBuf {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."));
        data_dir.join("analyzer").join(TRACKING_FILE)
    }
}

/// Summary statistics for history
#[derive(Debug)]
pub struct HistorySummary {
    /// Total number of executions
    pub total: usize,
    /// Number of successful executions
    pub successful: usize,
    /// Number of failed executions
    pub failed: usize,
    /// Total issues found across all runs
    pub total_issues: usize,
    /// Total errors found
    pub total_errors: usize,
    /// Total warnings found
    pub total_warnings: usize,
    /// Total execution time in milliseconds
    pub total_time_ms: u64,
    /// Number of executions today
    pub today_count: usize,
    /// Breakdown by tech stack (sorted by frequency)
    pub by_stack: Vec<(String, usize)>,
}

/// Struct for timing command execution
pub struct TimedExecution {
    start: std::time::Instant,
}

impl TimedExecution {
    /// Start a new timed execution
    pub fn start() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }

    /// Record execution and return elapsed ms
    pub fn record(&self, entry: HistoryEntry) -> HistoryEntry {
        let elapsed = self.start.elapsed().as_millis() as u64;
        entry.with_exec_time(elapsed)
    }

    /// Get elapsed time in milliseconds without recording
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_entry_creation() {
        let entry = HistoryEntry::new("cargo", "check")
            .with_issue_counts(5, 2, 3)
            .with_exec_time(1500);
        assert_eq!(entry.tech_stack, "cargo");
        assert_eq!(entry.command, "check");
        assert_eq!(entry.issue_count, 5);
        assert_eq!(entry.error_count, 2);
        assert_eq!(entry.warning_count, 3);
        assert_eq!(entry.exec_time_ms, 1500);
    }

    #[test]
    fn test_history_add_and_recent() {
        let mut history = History::default();
        history.add_entry(HistoryEntry::new("npm", "lint").with_issue_counts(3, 1, 2));
        history.add_entry(HistoryEntry::new("mypy", "mypy").with_issue_counts(1, 0, 1));
        assert_eq!(history.total_count(), 2);
        let recent = history.recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].tech_stack, "mypy");
    }

    #[test]
    fn test_history_summary() {
        let mut history = History::default();
        history.add_entry(HistoryEntry::new("cargo", "check").with_issue_counts(5, 2, 3));
        history.add_entry(HistoryEntry::new("npm", "lint").with_issue_counts(3, 1, 2));
        let summary = history.summary();
        assert_eq!(summary.total, 2);
        assert_eq!(summary.total_issues, 8);
        assert_eq!(summary.total_errors, 3);
        assert_eq!(summary.total_warnings, 5);
    }

    #[test]
    fn test_timed_execution() {
        let timer = TimedExecution::start();
        let entry = HistoryEntry::new("go", "build");
        let entry = timer.record(entry);
        assert!(entry.exec_time_ms > 0);
    }
}