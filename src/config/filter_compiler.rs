//! Compiles `TomlFilterConfig` into `OutputPostProcessor` instances.

use super::filter_registry::TomlFilterConfig;
use crate::core::utils::{OutputPostProcessor, ShortCircuitRule};

/// Compile a TOML filter config into an OutputPostProcessor.
pub fn compile_toml_filter(config: &TomlFilterConfig) -> OutputPostProcessor {
    let mut processor = OutputPostProcessor::new()
        .with_strip_ansi(config.strip_ansi.unwrap_or(false))
        .with_strip_tui_frames(config.strip_tui_frames.unwrap_or(false));

    // Replace patterns
    if let Some(ref replace_rules) = config.replace {
        let patterns: Vec<(String, String)> = replace_rules
            .iter()
            .map(|r| (r.pattern.clone(), r.replacement.clone()))
            .collect();
        processor = processor.with_replace_patterns(patterns);
    }

    // Short-circuit rules
    if let Some(ref short_circuits) = config.short_circuit {
        let rules: Vec<ShortCircuitRule> = short_circuits
            .iter()
            .map(|s| ShortCircuitRule {
                pattern: s.pattern.clone(),
                message: s.message.clone(),
                unless: s.unless.clone(),
            })
            .collect();
        processor = processor.with_short_circuits(rules);
    }

    // Noise patterns (strip_lines_matching)
    if let Some(ref noise) = config.strip_lines_matching {
        if !noise.is_empty() {
            processor = processor.with_noise_patterns(noise.clone());
        }
    }

    // Keep patterns
    if let Some(ref keep) = config.keep_lines_matching {
        if !keep.is_empty() {
            processor = processor.with_keep_patterns(keep.clone());
        }
    }

    // Line truncation
    if let Some(max_len) = config.truncate_lines_at {
        if max_len > 0 {
            processor = processor.with_max_line_length(max_len);
        }
    }

    // Max lines
    if let Some(max_lines) = config.max_lines {
        if max_lines > 0 {
            processor = processor.with_max_lines(max_lines);
        }
    }

    // On-empty fallback
    if let Some(ref msg) = config.on_empty {
        processor = processor.with_on_empty(msg.clone());
    }

    processor
}
