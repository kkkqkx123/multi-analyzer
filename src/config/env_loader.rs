use super::global::AppConfig;

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