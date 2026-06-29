//! Compound command lexer for splitting raw shell commands on operators.
//!
//! Handles shell operators (&&, ||, ;, |, &) while respecting:
//! - Single-quoted strings: '...'
//! - Double-quoted strings: "..."
//! - Backslash-escaped characters: \X
//!
//! Usage:
//! ```
//! # use analyzer::discover::lexer::split_on_operators;
//! let segments = split_on_operators("cargo check && cargo test");
//! assert_eq!(segments, vec!["cargo check", "cargo test"]);
//! ```

/// Shell operators that separate compound commands
const SHELL_OPERATORS: &[&str] = &["&&", "||", ";", "|", "&"];

/// Split a compound command string into individual segments at shell operator boundaries.
/// Returns a vector of trimmed segments. Operators and empty segments are discarded.
///
/// The lexer respects quoting:
/// - Content inside single quotes ('...') is treated as literal text.
/// - Content inside double quotes ("...") is treated as literal text.
/// - Backslash escapes the following character.
pub fn split_on_operators(raw_cmd: &str) -> Vec<String> {
    let bytes = raw_cmd.as_bytes();
    let len = bytes.len();
    let mut segments: Vec<String> = Vec::new();
    let mut segment_start = 0usize;
    let mut i = 0usize;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while i < len {
        let ch = bytes[i];

        if in_single_quote {
            if ch == b'\'' {
                in_single_quote = false;
            }
            i += 1;
            continue;
        }

        if in_double_quote {
            if ch == b'\\' && i + 1 < len {
                i += 2;
                continue;
            }
            if ch == b'"' {
                in_double_quote = false;
            }
            i += 1;
            continue;
        }

        if ch == b'\'' {
            in_single_quote = true;
            i += 1;
            continue;
        }

        if ch == b'"' {
            in_double_quote = true;
            i += 1;
            continue;
        }

        if ch == b'\\' && i + 1 < len {
            i += 2;
            continue;
        }

        let mut matched_operator = None;
        for op in SHELL_OPERATORS {
            let op_bytes = op.as_bytes();
            if i + op_bytes.len() <= len && &bytes[i..i + op_bytes.len()] == op_bytes {
                matched_operator = Some(op_bytes.len());
                break;
            }
        }

        if let Some(op_len) = matched_operator {
            if i > segment_start {
                let segment = std::str::from_utf8(&bytes[segment_start..i])
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !segment.is_empty() {
                    segments.push(segment);
                }
            }
            i += op_len;
            segment_start = i;
            continue;
        }

        i += 1;
    }

    if segment_start < len {
        let segment = std::str::from_utf8(&bytes[segment_start..])
            .unwrap_or("")
            .trim()
            .to_string();
        if !segment.is_empty() {
            segments.push(segment);
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        let result = split_on_operators("cargo check");
        assert_eq!(result, vec!["cargo check"]);
    }

    #[test]
    fn test_double_ampersand() {
        let result = split_on_operators("cargo fmt && cargo check");
        assert_eq!(result, vec!["cargo fmt", "cargo check"]);
    }

    #[test]
    fn test_pipe() {
        let result = split_on_operators("cargo test | tee output.txt");
        assert_eq!(result, vec!["cargo test", "tee output.txt"]);
    }

    #[test]
    fn test_semicolon() {
        let result = split_on_operators("cargo build; cargo test");
        assert_eq!(result, vec!["cargo build", "cargo test"]);
    }

    #[test]
    fn test_double_pipe() {
        let result = split_on_operators("cargo build || echo failed");
        assert_eq!(result, vec!["cargo build", "echo failed"]);
    }

    #[test]
    fn test_background() {
        let result = split_on_operators("cargo build & cargo test &");
        assert_eq!(result, vec!["cargo build", "cargo test"]);
    }

    #[test]
    fn test_quoted_string_with_operator() {
        let result = split_on_operators("echo \"a && b\" && cargo check");
        assert_eq!(result, vec!["echo \"a && b\"", "cargo check"]);
    }

    #[test]
    fn test_single_quoted_string() {
        let result = split_on_operators("echo 'foo || bar' && cargo test");
        assert_eq!(result, vec!["echo 'foo || bar'", "cargo test"]);
    }

    #[test]
    fn test_empty_input() {
        let result = split_on_operators("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_only_operator() {
        let result = split_on_operators("&&");
        assert!(result.is_empty());
    }

    #[test]
    fn test_escaped_characters() {
        let result = split_on_operators(r"echo \& cargo check");
        assert_eq!(result, vec![r"echo \& cargo check"]);
    }
}
