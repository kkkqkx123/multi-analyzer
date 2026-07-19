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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: set an env var and return a guard that restores it on drop
    fn set_env_guard(key: &str, val: &str) -> EnvGuard {
        let prev = std::env::var(key).ok();
        std::env::set_var(key, val);
        EnvGuard {
            key: key.to_string(),
            previous: prev,
        }
    }

    struct EnvGuard {
        key: String,
        previous: Option<String>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(val) => std::env::set_var(&self.key, val),
                None => std::env::remove_var(&self.key),
            }
        }
    }

    #[test]
    fn test_apply_env_vars_format() {
        let _guard = set_env_guard("ANALYZER_FORMAT", "json");
        let mut config = AppConfig::default();
        assert_eq!(config.report.format, "markdown");
        apply_env_vars(&mut config);
        assert_eq!(config.report.format, "json");
    }

    #[test]
    fn test_apply_env_vars_verbosity() {
        let _guard = set_env_guard("ANALYZER_VERBOSITY", "minimal");
        let mut config = AppConfig::default();
        apply_env_vars(&mut config);
        assert_eq!(config.report.verbosity, "minimal");
    }

    #[test]
    fn test_apply_env_vars_verbose_true() {
        let _guard = set_env_guard("ANALYZER_VERBOSE", "true");
        let mut config = AppConfig::default();
        apply_env_vars(&mut config);
        assert_eq!(config.report.verbosity, "verbose");
    }

    #[test]
    fn test_apply_env_vars_verbose_false() {
        let _guard = set_env_guard("ANALYZER_VERBOSE", "false");
        let mut config = AppConfig::default();
        config.report.verbosity = "verbose".to_string();
        apply_env_vars(&mut config);
        assert_eq!(config.report.verbosity, "normal");
    }

    #[test]
    fn test_apply_env_vars_strip_ansi() {
        let _guard = set_env_guard("ANALYZER_STRIP_ANSI", "false");
        let mut config = AppConfig::default();
        assert!(config.filter.strip_ansi); // default is true
        apply_env_vars(&mut config);
        assert!(!config.filter.strip_ansi);
    }

    #[test]
    fn test_apply_env_vars_max_lines() {
        let _guard = set_env_guard("ANALYZER_MAX_LINES", "500");
        let mut config = AppConfig::default();
        apply_env_vars(&mut config);
        assert_eq!(config.filter.max_lines, 500);
    }

    #[test]
    fn test_apply_env_vars_max_line_length() {
        let _guard = set_env_guard("ANALYZER_MAX_LINE_LENGTH", "200");
        let mut config = AppConfig::default();
        apply_env_vars(&mut config);
        assert_eq!(config.filter.max_line_length, 200);
    }

    #[test]
    fn test_apply_env_vars_tee_enabled() {
        let _guard = set_env_guard("ANALYZER_TEE", "true");
        let mut config = AppConfig::default();
        config.tee.enabled = false;
        apply_env_vars(&mut config);
        assert!(config.tee.enabled);
    }

    #[test]
    fn test_apply_env_vars_tee_disabled() {
        let _guard = set_env_guard("ANALYZER_TEE", "false");
        let mut config = AppConfig::default();
        config.tee.enabled = true;
        apply_env_vars(&mut config);
        assert!(!config.tee.enabled);
    }

    #[test]
    fn test_apply_env_vars_tee_dir() {
        let _guard = set_env_guard("ANALYZER_TEE_DIR", "/tmp/analyzer-logs");
        let mut config = AppConfig::default();
        apply_env_vars(&mut config);
        assert_eq!(
            config.tee.directory,
            Some(std::path::PathBuf::from("/tmp/analyzer-logs"))
        );
    }

    #[test]
    fn test_apply_env_vars_tee_mode() {
        let _guard = set_env_guard("ANALYZER_TEE_MODE", "always");
        let mut config = AppConfig::default();
        apply_env_vars(&mut config);
        assert_eq!(config.tee.mode, TeeMode::Always);
    }
}
