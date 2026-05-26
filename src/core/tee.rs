//! Tee functionality for saving command output to files
//! Allows preserving raw command output for debugging and replay

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;

/// Default tee output directory name
const TEE_DIR_NAME: &str = "analyzer-tee";

/// Configuration for tee output
#[derive(Debug, Clone)]
pub struct TeeConfig {
    /// Directory to store tee files
    pub output_dir: Option<PathBuf>,
    /// Maximum number of tee files to keep (0 = unlimited)
    pub max_files: usize,
    /// Whether tee is enabled by default
    pub enabled: bool,
}

impl Default for TeeConfig {
    fn default() -> Self {
        Self {
            output_dir: None,
            max_files: 50,
            enabled: false,
        }
    }
}

impl TeeConfig {
    /// Get the effective output directory
    pub fn resolved_dir(&self) -> PathBuf {
        self.output_dir
            .clone()
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(TEE_DIR_NAME)
            })
    }
}

/// Represents a saved tee file
#[derive(Debug)]
pub struct TeeFile {
    /// Path to the saved file
    pub path: PathBuf,
    /// Size of the saved file in bytes
    pub size: usize,
    /// Label identifying the command/context
    pub label: String,
}

/// Save command output to a tee file
pub fn save_output(
    output: &str,
    label: &str,
    config: &TeeConfig,
) -> Option<TeeFile> {
    if !config.enabled {
        return None;
    }

    let dir = config.resolved_dir();
    fs::create_dir_all(&dir).ok()?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let safe_label = label.replace(&['/', '\\', ' ', ':', '.', '\0'][..], "_");
    let filename = format!("{}_{}.txt", timestamp, safe_label);
    let path = dir.join(&filename);

    let content = format!("--- tee: {} at {} ---\n{}\n", label, timestamp, output);
    fs::write(&path, &content).ok()?;

    if config.max_files > 0 {
        cleanup_old_files(&dir, config.max_files);
    }

    let size = content.len();
    Some(TeeFile {
        path,
        size,
        label: label.to_string(),
    })
}

/// Load a previously saved tee file
pub fn load_output(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// List all saved tee files in the directory
pub fn list_files(config: &TeeConfig) -> Vec<TeeFile> {
    let dir = config.resolved_dir();
    let mut files: Vec<TeeFile> = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "txt").unwrap_or(false) {
                let label = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let size = fs::metadata(&path).map(|m| m.len() as usize).unwrap_or(0);
                files.push(TeeFile { path, size, label });
            }
        }
    }

    files.sort_by(|a, b| b.path.cmp(&a.path));
    files
}

/// Remove old tee files exceeding the maximum count
fn cleanup_old_files(dir: &Path, max_files: usize) {
    let mut entries: Vec<_> = match fs::read_dir(dir) {
        Ok(e) => e.flatten().collect(),
        Err(_) => return,
    };

    entries.sort_by_key(|e| e.path());

    while entries.len() > max_files {
        if let Some(oldest) = entries.first() {
            let _ = fs::remove_file(oldest.path());
            entries.remove(0);
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_save_and_load_output() {
        let dir = std::env::temp_dir().join("analyzer-tee-test");
        let _ = fs::remove_dir_all(&dir);

        let config = TeeConfig {
            output_dir: Some(dir.clone()),
            max_files: 10,
            enabled: true,
        };

        let result = save_output("test output content", "test-label", &config);
        assert!(result.is_some());
        let saved = result.unwrap();
        assert!(saved.path.exists());

        let loaded = load_output(&saved.path);
        assert!(loaded.is_some());
        let content = loaded.unwrap();
        assert!(content.contains("test output content"));
        assert!(content.contains("test-label"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_tee_disabled() {
        let config = TeeConfig {
            enabled: false,
            ..Default::default()
        };
        let result = save_output("test", "test", &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_list_files() {
        let dir = std::env::temp_dir().join("analyzer-tee-list-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let config = TeeConfig {
            output_dir: Some(dir.clone()),
            max_files: 10,
            enabled: true,
        };

        save_output("content1", "cmd1", &config);
        save_output("content2", "cmd2", &config);

        let files = list_files(&config);
        assert_eq!(files.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }
}