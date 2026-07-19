/// Command classification and rewrite engine.
///
/// Given a raw shell command (e.g. "cargo check --all-targets"), this module:
/// 1. Strips environment variable prefixes (ENV=val ...)
/// 2. Matches the command against the static RULES table
/// 3. Falls back to config-defined commands when no static rule matches
/// 4. Extracts the target TechStack, subcommand, and extra arguments
/// 5. Returns a Classification result
///
/// Exit code contract:
///   classify_command → Classification::Matched    (command recognized)
///   classify_command → Classification::Unmatched  (no rule matched)
use std::collections::HashMap;

use regex::Regex;

use crate::config::modules::CommandConfig;

#[allow(unused_imports)]
use super::lexer;
use super::rules;
use crate::core::TechStack;

/// Result of classifying a raw shell command
#[derive(Debug, Clone)]
pub enum Classification {
    /// A matching rule was found
    Matched {
        /// Target technology stack
        tech_stack: TechStack,
        /// Subcommand string extracted from the raw command
        subcommand: String,
        /// Extra arguments present in the raw command beyond the prefix and subcommand
        extra_args: Vec<String>,
        /// Index of the matching rule in RULES
        #[allow(dead_code)]
        rule_index: usize,
    },
    /// No rule matched this command
    Unmatched {
        /// The base command (first word)
        base_command: String,
    },
}

impl Classification {
    /// True when a matching rule was found
    #[allow(dead_code)]
    pub fn is_matched(&self) -> bool {
        matches!(self, Classification::Matched { .. })
    }
}

/// Check if a string is a valid POSIX environment variable name.
///
/// Valid env var names start with `[a-zA-Z_]` and contain only `[a-zA-Z0-9_]`.
/// This prevents compiler flags like `-DFOO=bar` from being treated as env vars.
fn is_valid_env_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = name.as_bytes();
    if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_' {
        return false;
    }
    for &b in &bytes[1..] {
        if !b.is_ascii_alphanumeric() && b != b'_' {
            return false;
        }
    }
    true
}

/// Strip environment variable prefixes from a raw command.
///
/// Handles:
///   FOO=bar cmd        → cmd
///   FOO=bar BAZ=qux cmd → cmd
///   KEY="val with spaces" cmd → cmd
///
/// Does NOT strip compiler flags like `-DFOO=bar` (only valid POSIX env var names).
fn strip_env_prefixes(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    while i < len {
        let ch = bytes[i];

        if ch == b' ' || ch == b'\t' {
            i += 1;
            continue;
        }

        let mut j = i;
        while j < len && bytes[j] != b' ' && bytes[j] != b'\t' && bytes[j] != b'=' {
            j += 1;
        }
        if j >= len || bytes[j] != b'=' {
            break;
        }

        // Validate that the key is a valid POSIX env var name.
        // This prevents compiler flags like -DFOO=bar from being stripped.
        let key = std::str::from_utf8(&bytes[i..j]).unwrap_or("");
        if !is_valid_env_var_name(key) {
            break;
        }

        j += 1;

        if j >= len {
            break;
        }

        if bytes[j] == b'"' {
            j += 1;
            while j < len && bytes[j] != b'"' {
                if bytes[j] == b'\\' && j + 1 < len {
                    j += 2;
                    continue;
                }
                j += 1;
            }
            if j < len {
                j += 1;
            }
        } else if bytes[j] == b'\'' {
            j += 1;
            while j < len && bytes[j] != b'\'' {
                j += 1;
            }
            if j < len {
                j += 1;
            }
        } else {
            while j < len && bytes[j] != b' ' && bytes[j] != b'\t' {
                j += 1;
            }
        }

        i = j;
    }

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    std::str::from_utf8(&bytes[i..]).unwrap_or(raw)
}

/// Classify a raw shell command against the RULES table, with optional
/// fallback to config-defined commands.
///
/// Tries static RULES first. When no rule matches, walks `commands` to find
/// a config-defined alias whose name matches the first token of the raw command.
///
/// Returns `Classification::Matched` when a rule or config command matches,
/// or `Classification::Unmatched` when nothing matches.
pub fn classify_command(raw_cmd: &str) -> Classification {
    classify_command_inner(raw_cmd, None)
}

/// Classify a raw shell command with config-defined command overrides.
///
/// This is the primary entry point for the `analyzer run` code path.
/// Config commands act as a fallback after the static RULES table.
pub fn classify_command_with_config(
    raw_cmd: &str,
    commands: &HashMap<String, CommandConfig>,
) -> Classification {
    classify_command_inner(raw_cmd, Some(commands))
}

fn classify_command_inner(
    raw_cmd: &str,
    commands: Option<&HashMap<String, CommandConfig>>,
) -> Classification {
    let cleaned = strip_env_prefixes(raw_cmd);

    // Stage 1: static RULES table
    for (idx, rule) in rules::RULES.iter().enumerate() {
        let regex = match Regex::new(&format!("(?i){}", rule.pattern)) {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "Warning: failed to compile regex pattern '{}': {}",
                    rule.pattern, e
                );
                continue;
            }
        };

        if let Some(captures) = regex.captures(cleaned) {
            let full_match = captures.get(0).unwrap();

            let subcommand = rule.subcommand_template.replace("{0}", full_match.as_str());

            let mut subcommand = fill_capture_refs(&subcommand, &captures);

            if subcommand.is_empty() {
                subcommand = full_match.as_str().to_string();
            }

            let matched_end = full_match.end();
            let rest = cleaned[matched_end..].trim();
            let extra_args: Vec<String> = if rest.is_empty() {
                Vec::new()
            } else {
                shell_words_split(rest)
            };

            return Classification::Matched {
                tech_stack: rule.tech_stack,
                subcommand,
                extra_args,
                rule_index: idx,
            };
        }
    }

    // Stage 2: fallback to config-defined commands
    if let Some(commands) = commands {
        let base = cleaned.split_whitespace().next().unwrap_or("").to_string();
        if !base.is_empty() {
            if let Some(cmd_config) = commands.get(&base) {
                if cmd_config.enabled {
                    if let Some(ts_name) = cmd_config.tech_stacks.first() {
                        if let Ok(ts) = ts_name.parse::<TechStack>() {
                            let subcommand = cmd_config.exec.clone();
                            let rest = cleaned[base.len()..].trim();
                            let extra_args: Vec<String> = if rest.is_empty() {
                                Vec::new()
                            } else {
                                shell_words_split(rest)
                            };
                            return Classification::Matched {
                                tech_stack: ts,
                                subcommand,
                                extra_args,
                                rule_index: usize::MAX,
                            };
                        }
                    }
                }
            }
        }
    }

    let base = cleaned.split_whitespace().next().unwrap_or("").to_string();

    Classification::Unmatched { base_command: base }
}

/// Fill {1}, {2}, ... capture group references in a template string.
fn fill_capture_refs(template: &str, captures: &regex::Captures) -> String {
    let mut result = template.to_string();
    for i in 1..=9 {
        let placeholder = format!("{{{}}}", i);
        if let Some(group) = captures.get(i) {
            result = result.replace(&placeholder, group.as_str());
        }
    }
    // Warn about any remaining unreplaced placeholders (e.g. {2} when only 1 capture group exists)
    if result.contains('{') && result.contains('}') {
        if let Some(start) = result.find('{') {
            if result[start..].contains('}') {
                eprintln!(
                    "Warning: unreplaced capture reference(s) found in template '{}': '{}'",
                    template, result
                );
            }
        }
    }
    result
}

/// Rewrite a raw shell command into its analyzer-equivalent form.
///
/// Returns `Some((TechStack, subcommand, extra_args))` when a rule matches,
/// or `None` when no rule matches.
pub fn rewrite_command(raw_cmd: &str) -> Option<(TechStack, String, Vec<String>)> {
    match classify_command(raw_cmd) {
        Classification::Matched {
            tech_stack,
            subcommand,
            extra_args,
            ..
        } => Some((tech_stack, subcommand, extra_args)),
        Classification::Unmatched { .. } => None,
    }
}

/// Rewrite with config-defined command overrides as fallback.
pub fn rewrite_command_with_config(
    raw_cmd: &str,
    commands: &HashMap<String, CommandConfig>,
) -> Option<(TechStack, String, Vec<String>)> {
    match classify_command_with_config(raw_cmd, commands) {
        Classification::Matched {
            tech_stack,
            subcommand,
            extra_args,
            ..
        } => Some((tech_stack, subcommand, extra_args)),
        Classification::Unmatched { .. } => None,
    }
}

/// Rewrite the first segment of a compound command (up to the first operator).
///
/// For compound commands like "cargo check && cargo test", only the first segment
/// before any shell operator is rewritten.
#[allow(dead_code)]
pub fn rewrite_first_segment(raw_cmd: &str) -> Option<(TechStack, String, Vec<String>)> {
    let segments = lexer::split_on_operators(raw_cmd);
    if let Some(first) = segments.first() {
        rewrite_command(first)
    } else {
        None
    }
}

/// Classify using only a specific rule set (category-filtered matching).
///
/// Falls back to the full `RULES` table if the rule set yields no match.
#[allow(dead_code)]
pub fn classify_with_ruleset(raw_cmd: &str, rule_set: &rules::RuleSet) -> Classification {
    let cleaned = strip_env_prefixes(raw_cmd);

    for (idx, rule) in rule_set.iter().enumerate() {
        let regex = match Regex::new(&format!("(?i){}", rule.pattern)) {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "Warning: failed to compile regex pattern '{}': {}",
                    rule.pattern, e
                );
                continue;
            }
        };

        if let Some(captures) = regex.captures(cleaned) {
            let full_match = captures.get(0).unwrap();
            let subcommand = rule.subcommand_template.replace("{0}", full_match.as_str());
            let mut subcommand = fill_capture_refs(&subcommand, &captures);
            if subcommand.is_empty() {
                subcommand = full_match.as_str().to_string();
            }

            let matched_end = full_match.end();
            let rest = cleaned[matched_end..].trim();
            let extra_args: Vec<String> = if rest.is_empty() {
                Vec::new()
            } else {
                shell_words_split(rest)
            };

            return Classification::Matched {
                tech_stack: rule.tech_stack,
                subcommand,
                extra_args,
                rule_index: idx,
            };
        }
    }

    classify_command(raw_cmd)
}

/// Classify using a category filter, matching only rules in that category.
///
/// Returns the first match from the matching rule set.
#[allow(dead_code)]
pub fn classify_by_category(raw_cmd: &str, category: &str) -> Classification {
    if let Some(rule_set) = rules::rule_set_by_category(category) {
        classify_with_ruleset(raw_cmd, rule_set)
    } else {
        classify_command(raw_cmd)
    }
}

/// Simple shell word splitter that respects quoting.
///
/// Splits a string into tokens respecting single quotes, double quotes,
/// and backslash escapes. This is NOT a full POSIX shell parser but
/// handles the common cases encountered in build tool invocations.
fn shell_words_split(input: &str) -> Vec<String> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0usize;

    loop {
        while i < len && bytes[i] == b' ' {
            i += 1;
        }

        if i >= len {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            break;
        }

        while i < len {
            let ch = bytes[i];

            if in_single {
                if ch == b'\'' {
                    in_single = false;
                } else {
                    word.push(ch as char);
                }
                i += 1;
                continue;
            }

            if in_double {
                if ch == b'\\' && i + 1 < len {
                    word.push(bytes[i + 1] as char);
                    i += 2;
                    continue;
                }
                if ch == b'"' {
                    in_double = false;
                } else {
                    word.push(ch as char);
                }
                i += 1;
                continue;
            }

            if ch == b'\'' {
                in_single = true;
                i += 1;
                continue;
            }

            if ch == b'"' {
                in_double = true;
                i += 1;
                continue;
            }

            if ch == b'\\' && i + 1 < len {
                word.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }

            if ch == b' ' {
                i += 1;
                break;
            }

            word.push(ch as char);
            i += 1;
        }

        if !word.is_empty() && (i >= len || bytes[i - 1] != b'\\') {
            words.push(std::mem::take(&mut word));
        }
    }

    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::rules::CommandRule;

    #[test]
    fn test_strip_env_prefixes_simple() {
        let result = strip_env_prefixes("FOO=bar cargo check");
        assert_eq!(result, "cargo check");
    }

    #[test]
    fn test_strip_env_prefixes_multiple() {
        let result = strip_env_prefixes("A=1 B=2 cargo check");
        assert_eq!(result, "cargo check");
    }

    #[test]
    fn test_strip_env_prefixes_no_env() {
        let result = strip_env_prefixes("cargo check");
        assert_eq!(result, "cargo check");
    }

    #[test]
    fn test_strip_env_prefixes_quoted() {
        let result = strip_env_prefixes(r#"RUSTFLAGS="-C target-cpu=native" cargo build"#);
        assert_eq!(result, "cargo build");
    }

    #[test]
    fn test_classify_cargo_check() {
        let result = classify_command("cargo check --all-targets");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
        if let Classification::Matched {
            subcommand,
            extra_args,
            ..
        } = result
        {
            assert_eq!(subcommand, "check");
            assert!(extra_args.iter().any(|a| a == "--all-targets"));
        }
    }

    #[test]
    fn test_classify_cargo_clippy() {
        let result = classify_command("cargo clippy");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_npm_lint() {
        let result = classify_command("npm run lint");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Npm,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "run lint");
        }
    }

    #[test]
    fn test_classify_pytest() {
        let result = classify_command("pytest -v tests/");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Pytest,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "-v tests/");
        }
    }

    #[test]
    fn test_classify_go_vet() {
        let result = classify_command("go vet ./...");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::GoBuild,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "vet");
        }
    }

    #[test]
    fn test_classify_maven() {
        let result = classify_command("mvn test -pl core");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Maven,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "test");
        }
    }

    #[test]
    fn test_classify_dotnet() {
        let result = classify_command("dotnet build");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Dotnet,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_rubocop() {
        let result = classify_command("rubocop");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Rubocop,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_cmake() {
        let result = classify_command("cmake --build .");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::CMake,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_unmatched() {
        let result = classify_command("echo hello");
        assert!(matches!(result, Classification::Unmatched { .. }));
    }

    #[test]
    fn test_classify_with_env_prefix() {
        let result = classify_command("RUST_BACKTRACE=1 cargo test");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "test");
        }
    }

    #[test]
    fn test_rewrite_command_cargo() {
        let result = rewrite_command("cargo check");
        assert!(result.is_some());
        let (ts, sub, extra) = result.unwrap();
        assert_eq!(ts, TechStack::Cargo);
        assert_eq!(sub, "check");
        assert!(extra.is_empty());
    }

    #[test]
    fn test_rewrite_command_unmatched() {
        let result = rewrite_command("ls -la");
        assert!(result.is_none());
    }

    #[test]
    fn test_rewrite_first_segment_compound() {
        let result = rewrite_first_segment("cargo check && cargo test");
        assert!(result.is_some());
        let (ts, sub, _) = result.unwrap();
        assert_eq!(ts, TechStack::Cargo);
        assert_eq!(sub, "check");
    }

    #[test]
    fn test_shell_words_split_simple() {
        let result = shell_words_split("--all-targets --workspace");
        assert_eq!(result, vec!["--all-targets", "--workspace"]);
    }

    #[test]
    fn test_shell_words_split_quoted() {
        let result = shell_words_split(r#"--features "feat1 feat2""#);
        assert_eq!(result, vec!["--features", "feat1 feat2"]);
    }

    #[test]
    fn test_classify_golangci_lint() {
        let result = classify_command("golangci-lint run");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::GolangciLint,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_rspec() {
        let result = classify_command("bundle exec rspec spec/");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Rspec,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_gcc() {
        let result = classify_command("gcc -c src/main.c -o main.o");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Gcc,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_clang() {
        let result = classify_command("clang++ -c src/main.cpp -o main.o");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Clang,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_cargo_nextest() {
        let result = classify_command("cargo nextest run");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "nextest run");
        }
    }

    #[test]
    fn test_classify_cargo_nextest_list() {
        let result = classify_command("cargo nextest list --no-tests=pass");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
        if let Classification::Matched {
            subcommand,
            extra_args,
            ..
        } = result
        {
            assert_eq!(subcommand, "nextest list");
            assert!(extra_args.contains(&"--no-tests=pass".to_string()));
        }
    }

    #[test]
    fn test_fill_capture_refs_missing_group() {
        // Template references {2} but only 1 capture group exists
        let re = Regex::new(r"^cargo\s+nextest\s+(run|list|archive)\b").unwrap();
        let captures = re.captures("cargo nextest run").unwrap();
        let result = fill_capture_refs("nextest {2}", &captures);
        // {2} is not replaced and should be detected by the warning logic
        assert!(result.contains("{2}"), "unreplaced placeholder should remain in result");
    }

    // ============================================================================
    // strip_env_prefixes edge cases
    // ============================================================================

    #[test]
    fn test_strip_env_prefixes_single_quoted() {
        let result = strip_env_prefixes(r#"KEY='val with spaces' cmd"#);
        assert_eq!(result, "cmd");
    }

    #[test]
    fn test_strip_env_prefixes_tab_separated() {
        let result = strip_env_prefixes("FOO=bar\tcargo check");
        assert_eq!(result, "cargo check");
    }

    #[test]
    fn test_strip_env_prefixes_empty_value() {
        let result = strip_env_prefixes("KEY= cmd");
        assert_eq!(result, "cmd");
    }

    #[test]
    fn test_strip_env_prefixes_dash_flag() {
        // -DFOO=bar is a compiler flag, NOT an environment variable
        let result = strip_env_prefixes("-DFOO=bar gcc -c src.c");
        assert_eq!(result, "-DFOO=bar gcc -c src.c");
    }

    #[test]
    fn test_strip_env_prefixes_no_command() {
        let result = strip_env_prefixes("FOO=bar");
        assert_eq!(result, "");
    }

    #[test]
    fn test_strip_env_prefixes_leading_whitespace() {
        let result = strip_env_prefixes("  FOO=bar cargo check");
        assert_eq!(result, "cargo check");
    }

    #[test]
    fn test_strip_env_prefixes_invalid_name_start_with_digit() {
        // 1FOO=bar is not a valid env var name (starts with digit)
        let result = strip_env_prefixes("1FOO=bar cargo check");
        assert_eq!(result, "1FOO=bar cargo check");
    }

    #[test]
    fn test_strip_env_prefixes_invalid_name_with_hyphen() {
        // FOO-BAR=qux is not a valid env var name (contains hyphen)
        let result = strip_env_prefixes("FOO-BAR=qux cargo check");
        assert_eq!(result, "FOO-BAR=qux cargo check");
    }

    // ============================================================================
    // shell_words_split edge cases
    // ============================================================================

    #[test]
    fn test_shell_words_split_empty() {
        let result = shell_words_split("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_shell_words_split_only_spaces() {
        let result = shell_words_split("   ");
        assert!(result.is_empty());
    }

    #[test]
    fn test_shell_words_split_single_quotes() {
        let result = shell_words_split("--name 'John Doe'");
        assert_eq!(result, vec!["--name", "John Doe"]);
    }

    #[test]
    fn test_shell_words_split_mixed_quotes() {
        let result = shell_words_split(r#"--name "John 'mid' Doe""#);
        assert_eq!(result, vec!["--name", "John 'mid' Doe"]);
    }

    #[test]
    fn test_shell_words_split_backslash_escape() {
        let result = shell_words_split(r"echo\ hello");
        assert_eq!(result, vec!["echo hello"]);
    }

    // ============================================================================
    // fill_capture_refs edge cases
    // ============================================================================

    #[test]
    fn test_fill_capture_refs_no_placeholders() {
        let re = Regex::new(r"^cargo\s+(check|clippy)").unwrap();
        let captures = re.captures("cargo check").unwrap();
        let result = fill_capture_refs("check", &captures);
        assert_eq!(result, "check");
    }

    #[test]
    fn test_fill_capture_refs_all_groups() {
        let re = Regex::new(
            r"^(\w+)\s+(\w+)\s+(\w+)\s+(\w+)\s+(\w+)\s+(\w+)\s+(\w+)\s+(\w+)\s+(\w+)",
        )
        .unwrap();
        let captures = re.captures("a b c d e f g h i").unwrap();
        let result = fill_capture_refs("{1}/{2}/{3}/{4}/{5}/{6}/{7}/{8}/{9}", &captures);
        assert_eq!(result, "a/b/c/d/e/f/g/h/i");
    }

    // ============================================================================
    // classify_command edge cases
    // ============================================================================

    #[test]
    fn test_classify_cargo_alone() {
        // "cargo" alone should not match any rule (no subcommand)
        let result = classify_command("cargo");
        assert!(matches!(result, Classification::Unmatched { .. }));
    }

    #[test]
    fn test_classify_ruff_alone() {
        // "ruff" alone should not match (pattern requires subcommand check|format)
        let result = classify_command("ruff");
        assert!(matches!(result, Classification::Unmatched { .. }));
    }

    #[test]
    fn test_classify_empty_string() {
        let result = classify_command("");
        assert!(matches!(result, Classification::Unmatched { .. }));
    }

    #[test]
    fn test_classify_only_spaces() {
        let result = classify_command("   ");
        assert!(matches!(result, Classification::Unmatched { .. }));
    }

    #[test]
    fn test_classify_with_tab_separator() {
        let result = classify_command("cargo\tcheck");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "check");
        }
    }

    #[test]
    fn test_classify_gcc_linking() {
        // gcc without -c flag should NOT match the compile rule
        let result = classify_command("gcc -o output main.c");
        assert!(matches!(result, Classification::Unmatched { .. }));
    }

    #[test]
    fn test_classify_clang_linking() {
        // clang without -c flag should NOT match the compile rule
        let result = classify_command("clang -o output main.c");
        assert!(matches!(result, Classification::Unmatched { .. }));
    }

    #[test]
    fn test_classify_clang_format() {
        let result = classify_command("clang-format -i src/*.cpp");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::ClangFormat,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "format");
        }
    }

    #[test]
    fn test_classify_msvc() {
        let result = classify_command("msvc /c src/main.cpp");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Msvc,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "compile");
        }
    }

    #[test]
    fn test_classify_golangci_lint_run() {
        let result = classify_command("golangci-lint run --timeout=5m");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::GolangciLint,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "run");
        }
    }

    #[test]
    fn test_classify_gradle_check() {
        let result = classify_command("gradle check");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Gradle,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "check");
        }
    }

    #[test]
    fn test_classify_gradlew_test() {
        let result = classify_command("gradlew test --info");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Gradle,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "test");
        }
    }

    #[test]
    fn test_classify_dotnet_format() {
        let result = classify_command("dotnet format --check");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Dotnet,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "format");
        }
    }

    #[test]
    fn test_classify_mvn_compile() {
        let result = classify_command("mvn compile -q");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Maven,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "compile");
        }
    }

    #[test]
    fn test_classify_yarn_audit() {
        let result = classify_command("yarn audit --production");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Yarn,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "run audit");
        }
    }

    #[test]
    fn test_classify_bundle_exec_rspec() {
        let result = classify_command("bundle exec rspec spec/models/");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Rspec,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "spec/models/");
        }
    }

    #[test]
    fn test_classify_npm_build_fallback() {
        // "npm run build" uses the fallback rule since "build" is not in the specific list
        let result = classify_command("npm run build");
        // The fallback rule pattern "^npm\s+(?:run\s+)?(\S+)" with template "run {1}"
        // captures "build" as group 1, producing "run build"
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Npm,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "run build");
        }
    }

    #[test]
    fn test_classify_dash_flag_env_not_stripped() {
        // -DFOO=bar is a compiler flag, not an env var, so it should NOT be stripped
        // The command stays as "-DFOO=bar gcc -c src.c", which doesn't start with gcc/g++
        // so it should be Unmatched (the GCC rule requires the command to start with gcc/g++)
        let result = classify_command("-DFOO=bar gcc -c src.c");
        assert!(matches!(result, Classification::Unmatched { .. }));
    }

    // ============================================================================
    // classify_command_with_config
    // ============================================================================

    #[test]
    fn test_classify_with_config_fallback() {
        use std::collections::HashMap;
        let mut commands = HashMap::new();
        commands.insert(
            "my-tool".to_string(),
            CommandConfig {
                exec: "check".to_string(),
                description: Some("my custom tool".to_string()),
                tech_stacks: vec!["cargo".to_string()],
                enabled: true,
            },
        );
        let result = classify_command_with_config("my-tool --verbose", &commands);
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
        if let Classification::Matched {
            subcommand,
            extra_args,
            ..
        } = result
        {
            assert_eq!(subcommand, "check");
            assert_eq!(extra_args, vec!["--verbose"]);
        }
    }

    #[test]
    fn test_classify_with_config_disabled() {
        use std::collections::HashMap;
        let mut commands = HashMap::new();
        commands.insert(
            "my-tool".to_string(),
            CommandConfig {
                exec: "check".to_string(),
                description: Some("my custom tool".to_string()),
                tech_stacks: vec!["cargo".to_string()],
                enabled: false,
            },
        );
        // Disabled command should not match, fall through to unmatched
        let result = classify_command_with_config("my-tool --verbose", &commands);
        assert!(matches!(result, Classification::Unmatched { .. }));
    }

    #[test]
    fn test_classify_with_config_empty_commands() {
        use std::collections::HashMap;
        let commands = HashMap::new();
        // Should fall through to static rules
        let result = classify_command_with_config("cargo check", &commands);
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_with_config_empty_tech_stacks() {
        use std::collections::HashMap;
        let mut commands = HashMap::new();
        commands.insert(
            "my-tool".to_string(),
            CommandConfig {
                exec: "check".to_string(),
                description: None,
                tech_stacks: vec![],  // empty tech_stacks should not match
                enabled: true,
            },
        );
        let result = classify_command_with_config("my-tool", &commands);
        // No tech_stacks means config fallback can't determine the tech stack
        assert!(matches!(result, Classification::Unmatched { .. }));
    }

    #[test]
    fn test_classify_with_config_static_rule_takes_priority() {
        // Static rules should match before config fallback is even tried
        use std::collections::HashMap;
        let mut commands = HashMap::new();
        commands.insert(
            "cargo".to_string(),
            CommandConfig {
                exec: "custom".to_string(),
                description: None,
                tech_stacks: vec!["npm".to_string()],
                enabled: true,
            },
        );
        // Even though there's a config for "cargo", the static rule should match first
        let result = classify_command_with_config("cargo check", &commands);
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "check");
        }
    }

    // ============================================================================
    // rewrite_command_with_config
    // ============================================================================

    #[test]
    fn test_rewrite_with_config() {
        use std::collections::HashMap;
        let mut commands = HashMap::new();
        commands.insert(
            "my-tool".to_string(),
            CommandConfig {
                exec: "check".to_string(),
                description: Some("my custom tool".to_string()),
                tech_stacks: vec!["cargo".to_string()],
                enabled: true,
            },
        );
        let result = rewrite_command_with_config("my-tool --verbose", &commands);
        assert!(result.is_some());
        let (ts, sub, extra) = result.unwrap();
        assert_eq!(ts, TechStack::Cargo);
        assert_eq!(sub, "check");
        assert_eq!(extra, vec!["--verbose"]);
    }

    #[test]
    fn test_rewrite_with_config_unmatched() {
        use std::collections::HashMap;
        let commands = HashMap::new();
        let result = rewrite_command_with_config("unknown-tool", &commands);
        assert!(result.is_none());
    }

    // ============================================================================
    // classify_by_category / classify_with_ruleset
    // ============================================================================

    #[test]
    fn test_classify_by_category_rust() {
        let result = classify_by_category("cargo check", "Rust");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
        if let Classification::Matched { subcommand, .. } = result {
            assert_eq!(subcommand, "check");
        }
    }

    #[test]
    fn test_classify_by_category_python_fallback() {
        // Python category rules don't match "cargo check", falls back to full RULES
        let result = classify_by_category("cargo check", "Python");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_by_category_nonexistent() {
        // Non-existent category should fall back to full RULES
        let result = classify_by_category("cargo check", "NonExistent");
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
    }

    #[test]
    fn test_classify_with_ruleset_custom() {
        let custom_rules = &[CommandRule {
            pattern: r"^my-tool\s+(check|test)",
            tech_stack: TechStack::Cargo,
            subcommand_template: "{1}",
            prefixes: &["my-tool"],
            category: "Custom",
        }];
        let custom_set = rules::RuleSet::new("Custom", custom_rules);
        let result = classify_with_ruleset("my-tool check --all", &custom_set);
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
        if let Classification::Matched {
            subcommand,
            extra_args,
            ..
        } = result
        {
            assert_eq!(subcommand, "check");
            assert_eq!(extra_args, vec!["--all"]);
        }
    }

    #[test]
    fn test_classify_with_ruleset_fallback() {
        // Custom ruleset doesn't match "cargo check", falls back to full RULES
        let custom_rules = &[CommandRule {
            pattern: r"^my-tool\s+(check|test)",
            tech_stack: TechStack::Cargo,
            subcommand_template: "{1}",
            prefixes: &["my-tool"],
            category: "Custom",
        }];
        let custom_set = rules::RuleSet::new("Custom", custom_rules);
        let result = classify_with_ruleset("cargo check", &custom_set);
        assert!(matches!(
            result,
            Classification::Matched {
                tech_stack: TechStack::Cargo,
                ..
            }
        ));
    }

    // ============================================================================
    // is_valid_env_var_name
    // ============================================================================

    #[test]
    fn test_is_valid_env_var_name_valid() {
        assert!(is_valid_env_var_name("FOO"));
        assert!(is_valid_env_var_name("foo"));
        assert!(is_valid_env_var_name("_FOO"));
        assert!(is_valid_env_var_name("FOO_BAR"));
        assert!(is_valid_env_var_name("FOO123"));
    }

    #[test]
    fn test_is_valid_env_var_name_invalid() {
        assert!(!is_valid_env_var_name(""));
        assert!(!is_valid_env_var_name("-DFOO"));
        assert!(!is_valid_env_var_name("1FOO"));
        assert!(!is_valid_env_var_name("FOO-BAR"));
        assert!(!is_valid_env_var_name(".FOO"));
    }
}
