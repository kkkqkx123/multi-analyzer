#![allow(dead_code)]

use super::global::AppConfig;

/// Load environment variables from .env file if it exists (simple key=value parser)
pub fn load_dotenv() {
    let candidates = find_dotenv_candidates();
    for path in &candidates {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, value)) = line.split_once('=') {
                        let key = key.trim();
                        let value = value.trim().trim_matches('"').trim_matches('\'');
                        if !key.is_empty() && std::env::var(key).is_err() {
                            std::env::set_var(key, value);
                        }
                    }
                }
            }
        }
    }
}

fn find_dotenv_candidates() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(".env"));
        let mut current = Some(cwd.as_path());
        while let Some(dir) = current {
            if dir.join("analyzer.toml").exists() || dir.join("Cargo.toml").exists() {
                candidates.push(dir.join(".env"));
                break;
            }
            current = dir.parent();
        }
    }
    candidates
}

/// Resolve `${VAR_NAME}` placeholders in a string using environment variables
pub fn resolve_env_placeholders(input: &str) -> String {
    let mut result = input.to_string();
    let mut start = 0;
    while let Some(pos) = result[start..].find("${") {
        let var_start = start + pos;
        if let Some(end) = result[var_start..].find('}') {
            let var_end = var_start + end;
            let var_name = &result[var_start + 2..var_end];
            if let Ok(val) = std::env::var(var_name) {
                result.replace_range(var_start..=var_end, &val);
                continue;
            }
        }
        start = var_start + 2;
    }
    result
}

/// Apply ANALYZER_* environment variables to override config
pub fn apply_env_vars(config: &mut AppConfig) {
    if let Ok(val) = std::env::var("ANALYZER_FORMAT") {
        config.report.format = val;
    }
    if let Ok(val) = std::env::var("ANALYZER_VERBOSITY") {
        config.report.verbosity = val;
    }
    if let Ok(val) = std::env::var("ANALYZER_VERBOSE") {
        config.report.verbose = val == "true" || val == "1";
    }
    if let Ok(val) = std::env::var("ANALYZER_STRIP_ANSI") {
        config.filter.strip_ansi = val == "true" || val == "1";
    }
    if let Ok(val) = std::env::var("ANALYZER_MAX_LINES") {
        if let Ok(n) = val.parse::<usize>() {
            config.filter.max_lines = n;
        }
    }
    if let Ok(val) = std::env::var("ANALYZER_MAX_LINE_LENGTH") {
        if let Ok(n) = val.parse::<usize>() {
            config.filter.max_line_length = n;
        }
    }
}