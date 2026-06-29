//! Analysis performance tracking.
//!
//! Records timing, success rate, and issue counts for each analysis run.
//! Provides aggregated statistics for monitoring and optimization.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// A single analysis run record.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AnalysisRecord {
    pub tech_stack: String,
    pub command: String,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub issue_count: usize,
    pub output_bytes: usize,
    pub success: bool,
}

/// Aggregated tracking statistics.
#[derive(Debug, Clone, Default)]
pub struct TrackingStats {
    pub total_runs: usize,
    pub successful_runs: usize,
    pub failed_runs: usize,
    pub total_issues: usize,
    pub total_duration_ms: u64,
    pub total_output_bytes: usize,
}

impl TrackingStats {
    pub fn success_rate(&self) -> f64 {
        if self.total_runs == 0 {
            0.0
        } else {
            self.successful_runs as f64 / self.total_runs as f64
        }
    }

    pub fn avg_duration_ms(&self) -> f64 {
        if self.total_runs == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.total_runs as f64
        }
    }

    pub fn avg_issues_per_run(&self) -> f64 {
        if self.total_runs == 0 {
            0.0
        } else {
            self.total_issues as f64 / self.total_runs as f64
        }
    }

    pub fn avg_output_kb(&self) -> f64 {
        if self.total_runs == 0 {
            0.0
        } else {
            self.total_output_bytes as f64 / self.total_runs as f64 / 1024.0
        }
    }

    /// Format as a summary string for reporting.
    pub fn summary(&self) -> String {
        format!(
            "Runs: {} total ({} success, {} failed) | {:.0}% success rate | Avg: {:.0}ms / {:.1} issues / {:.1}KB output",
            self.total_runs,
            self.successful_runs,
            self.failed_runs,
            self.success_rate() * 100.0,
            self.avg_duration_ms(),
            self.avg_issues_per_run(),
            self.avg_output_kb(),
        )
    }
}

/// Maximum number of records to keep in memory.
const MAX_RECORDS: usize = 100;

/// Internal tracker with ring-buffer storage.
struct Tracker {
    records: Vec<AnalysisRecord>,
    next_index: usize,
}

impl Tracker {
    fn new() -> Self {
        Self {
            records: Vec::with_capacity(MAX_RECORDS),
            next_index: 0,
        }
    }

    fn record(&mut self, record: AnalysisRecord) {
        if self.records.len() < MAX_RECORDS {
            self.records.push(record);
        } else {
            self.records[self.next_index] = record;
            self.next_index = (self.next_index + 1) % MAX_RECORDS;
        }
    }

    fn stats(&self) -> TrackingStats {
        let mut stats = TrackingStats::default();
        for r in &self.records {
            stats.total_runs += 1;
            if r.success {
                stats.successful_runs += 1;
            } else {
                stats.failed_runs += 1;
            }
            stats.total_issues += r.issue_count;
            stats.total_duration_ms += r.duration_ms;
            stats.total_output_bytes += r.output_bytes;
        }
        stats
    }

    #[allow(dead_code)]
    fn records(&self) -> &[AnalysisRecord] {
        &self.records
    }
}

/// Thread-safe tracking state.
fn tracker() -> &'static Mutex<Tracker> {
    static TRACKER: OnceLock<Mutex<Tracker>> = OnceLock::new();
    TRACKER.get_or_init(|| Mutex::new(Tracker::new()))
}

/// Timing guard that records a measurement on drop.
pub struct TimingGuard {
    start: Instant,
    tech_stack: String,
    command: String,
    output_bytes: usize,
}

impl TimingGuard {
    /// Start timing a new analysis run. Does NOT record until `complete` is called.
    pub fn start(tech_stack: &str, command: &str) -> Self {
        Self {
            start: Instant::now(),
            tech_stack: tech_stack.to_string(),
            command: command.to_string(),
            output_bytes: 0,
        }
    }

    pub fn set_output_bytes(&mut self, bytes: usize) {
        self.output_bytes = bytes;
    }

    /// Record the completed analysis. Returns the duration.
    pub fn complete(self, exit_code: Option<i32>, issue_count: usize, success: bool) -> Duration {
        let duration = self.start.elapsed();
        let record = AnalysisRecord {
            tech_stack: self.tech_stack,
            command: self.command,
            duration_ms: duration.as_millis() as u64,
            exit_code,
            issue_count,
            output_bytes: self.output_bytes,
            success,
        };
        if let Ok(mut t) = tracker().lock() {
            t.record(record);
        }
        duration
    }
}

/// Get aggregated tracking statistics for all recorded runs.
pub fn stats() -> TrackingStats {
    tracker()
        .lock()
        .map(|t| t.stats())
        .unwrap_or_default()
}

/// Get all recorded analysis runs.
#[allow(dead_code)]
pub fn records() -> Vec<AnalysisRecord> {
    tracker()
        .lock()
        .map(|t| t.records().to_vec())
        .unwrap_or_default()
}

/// Reset all tracking data.
#[allow(dead_code)]
pub fn reset() {
    if let Ok(mut t) = tracker().lock() {
        *t = Tracker::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_guard_basic() {
        reset();
        let mut guard = TimingGuard::start("cargo", "cargo check");
        guard.set_output_bytes(1000);
        let duration = guard.complete(Some(0), 3, true);
        assert!(duration >= Duration::ZERO);

        let s = stats();
        assert_eq!(s.total_runs, 1);
        assert_eq!(s.successful_runs, 1);
        assert_eq!(s.total_issues, 3);
    }

    #[test]
    fn test_stats_aggregation() {
        reset();
        for i in 0..5 {
            let mut guard = TimingGuard::start("cargo", "cargo check");
            guard.set_output_bytes(1000);
            guard.complete(Some(0), i * 2, i % 2 == 0);
        }

        let s = stats();
        assert_eq!(s.total_runs, 5);
        assert_eq!(s.successful_runs, 3);
        assert_eq!(s.failed_runs, 2);
        assert_eq!(s.total_issues, 20);
        assert!((s.success_rate() - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_stats_empty() {
        reset();
        let s = stats();
        assert_eq!(s.total_runs, 0);
        assert_eq!(s.success_rate(), 0.0);
        assert_eq!(s.avg_duration_ms(), 0.0);
    }

    #[test]
    fn test_summary_format() {
        reset();
        let mut guard = TimingGuard::start("cargo", "cargo check");
        guard.set_output_bytes(2048);
        guard.complete(Some(0), 5, true);

        let summary = stats().summary();
        assert!(summary.contains("1 total"));
        assert!(summary.contains("5.0 issues"));
        assert!(summary.contains("2.0KB"));
    }
}
