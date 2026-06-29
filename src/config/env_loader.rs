use super::global::AppConfig;
use super::modules::tee::TeeMode;

/// Apply ANALYZER_* environment variables to override config
pub fn apply_env_vars(config: &mut AppConfig) {
    if let Ok(val) = std::env::var("ANALYZER_FORMAT") {
        config.report.format = val;
    }
    if let Ok(val) = std::env::var("ANALYZER_VERBOSITY") {
        config.report.verbosity = val;
    }
    if let Ok(val) = std::env::var("ANALYZER_VERBOSE") {
        if val == "true" || val == "1" {
            config.report.verbosity = "verbose".to_string();
        } else {
            config.report.verbosity = "normal".to_string();
        }
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
    // Analyse la variable TEE
    if let Ok(val) = std::env::var("ANALYZER_TEE") {
        match val.as_str() {
            "false" | "0" => config.tee.enabled = false,
            "true" | "1" => config.tee.enabled = true,
            _ => {}
        }
    }
    if let Ok(val) = std::env::var("ANALYZER_TEE_DIR") {
        config.tee.directory = Some(std::path::PathBuf::from(val));
    }
    if let Ok(val) = std::env::var("ANALYZER_TEE_MODE") {
        match val.as_str() {
            "always" => config.tee.mode = TeeMode::Always,
            "failures" => config.tee.mode = TeeMode::Failures,
            "never" => config.tee.mode = TeeMode::Never,
            _ => {}
        }
    }
}
