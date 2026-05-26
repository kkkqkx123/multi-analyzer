//! NPM/Node.js Output Parser
//! Parsing the output of npm/pnpm/yarn lint and type-check

use crate::core::{
    strip_ansi, BaseParser, Issue, IssueLevel, Location, OutputParser, ParseResult, ParsedTestOutput,
    TestCase, TestOutputParser, TestStatus, TestSummary,
};

pub struct NpmParser {
    base: BaseParser,
}

impl NpmParser {
    pub fn new() -> Self {
        Self {
            base: BaseParser::new(),
        }
    }

    /// Check if a line is a file path (file paths are on a separate line in ESLint format)
    fn is_file_path_line(&self, line: &str) -> bool {
        let trimmed = line.trim();
        // File paths usually contain / or \ and do not begin with a number
        !trimmed.is_empty()
            && (trimmed.contains('/') || trimmed.contains('\\'))
            && !trimmed.chars().next().unwrap_or(' ').is_ascii_digit()
            && !trimmed.starts_with('✖')
            && !trimmed.starts_with('│')
            && !trimmed.starts_with('├')
            && !trimmed.starts_with('└')
            && !trimmed.starts_with("npm error")
            && !trimmed.starts_with("error")
    }

    /// Find the file path corresponding to the current line
    /// Prioritizes the nearest file path line, and looks up if there is none.
    fn find_eslint_file_path(&self, lines: &[String], current_index: usize) -> String {
        // First look up the file path line
        for i in (0..current_index).rev() {
            let line = &lines[i];
            if self.is_file_path_line(line) {
                return line.trim().to_string();
            }
        }
        String::from("unknown")
    }

    fn parse_eslint_format(
        &self,
        lines: &[String],
        start_index: usize,
    ) -> (Option<Issue>, usize) {
        if start_index >= lines.len() {
            return (None, start_index);
        }

        let line = &lines[start_index];
        let trimmed = line.trim();

        // ESLint 格式: " 3:7 warning message rule-name"
        // There may be a space at the beginning of the line, followed by the line number:column number
        let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
        if parts.len() < 2 {
            return (None, start_index + 1);
        }

        // Parsing line numbers (handling leading spaces)
        let line_num = parts[0].trim().parse::<u32>().ok();
        if line_num.is_none() {
            return (None, start_index + 1);
        }

        let rest = parts[1];
        // The column number is followed by the level and message
        // 格式: "7 warning message rule-name"
        let rest_parts: Vec<&str> = rest.splitn(2, |c: char| c.is_whitespace()).collect();
        if rest_parts.len() < 2 {
            return (None, start_index + 1);
        }

        let col_num = rest_parts[0].trim().parse::<u32>().ok();
        let after_col = rest_parts[1].trim();

        // Resolution Levels and Messages
        // 格式: "warning message rule-name" 或 "error message rule-name"
        // Also support format: "warning  message  rule-name" (multiple spaces)
        let level_msg_parts: Vec<&str> = after_col.splitn(2, |c: char| c.is_whitespace()).collect();
        if level_msg_parts.is_empty() {
            return (None, start_index + 1);
        }

        let level_str = level_msg_parts[0].trim();
        let level = match level_str.to_lowercase().as_str() {
            "error" => IssueLevel::Error,
            "warning" | "warn" => IssueLevel::Warning,
            "info" => IssueLevel::Info,
            _ => return (None, start_index + 1),
        };

        // Extract message and rule
        // Format: "message  @typescript-eslint/no-unused-vars"
        // or: "'parseAndValidateAgentLoopConfig' is defined but never used. Allowed unused vars must match /^_/u  @typescript-eslint/no-unused-vars"
        let message = if level_msg_parts.len() > 1 {
            let msg_and_rule = level_msg_parts[1].trim();
            // Try to extract rule name from the end (format: "message  rule-name")
            self.extract_eslint_message(msg_and_rule)
        } else {
            String::new()
        };

        // Using Improved File Path Finding
        let file_path = self.find_eslint_file_path(lines, start_index);

        let location = if let Some(col) = col_num {
            Location::new(file_path)
                .with_line(line_num.unwrap())
                .with_column(col)
        } else {
            Location::new(file_path).with_line(line_num.unwrap())
        };

        (Some(Issue::new(level, message, location)), start_index + 1)
    }

    /// Extract ESLint message, separating the rule name from the message
    /// Format: "message text  @typescript-eslint/rule-name" or "message rule-name"
    fn extract_eslint_message(&self, msg_and_rule: &str) -> String {
        // Look for rule name pattern at the end (e.g., "@typescript-eslint/no-unused-vars" or "rule-name")
        // Rule names typically contain "/" and may be prefixed with "@"
        // First try with double space (verbose format)
        if let Some(last_double_space) = msg_and_rule.rfind("  ") {
            let potential_rule = &msg_and_rule[last_double_space + 2..];
            if potential_rule.contains('/') || potential_rule.starts_with('@') {
                // This looks like a rule name, return just the message
                return msg_and_rule[..last_double_space].trim().to_string();
            }
        }
        
        // Try with single space, but check if the last part looks like a rule name
        // A rule name typically contains "-" (kebab-case) and may contain "/"
        if let Some(last_space) = msg_and_rule.rfind(' ') {
            let potential_rule = &msg_and_rule[last_space + 1..];
            // Check if it looks like a rule name: contains "-" or "/" or "@"
            let looks_like_rule = potential_rule.contains('-') 
                || potential_rule.contains('/') 
                || potential_rule.starts_with('@');
            
            if looks_like_rule {
                return msg_and_rule[..last_space].trim().to_string();
            }
        }
        
        // If no rule pattern found, return the whole thing
        msg_and_rule.to_string()
    }

    /// Remove ANSI escape codes from output
    fn strip_ansi_codes(&self, text: &str) -> String {
        strip_ansi(text)
    }

    /// Extract package name and clean content from a line with turbo/pnpm prefix
    /// Returns (package_name, cleaned_content)
    fn extract_package_and_content(&self, line: &str) -> (Option<String>, String) {
        let trimmed = line.trim();
        
        // Skip empty lines
        if trimmed.is_empty() {
            return (None, line.to_string());
        }
        
        // Skip TUI border/decoration lines
        if trimmed.starts_with('╭') || trimmed.starts_with('╰') || 
           trimmed.starts_with('┌') || trimmed.starts_with('└') ||
           trimmed.starts_with('│') || trimmed.starts_with('─') ||
           trimmed.starts_with('├') || trimmed.starts_with('┤') ||
           trimmed.starts_with('•') || trimmed.starts_with('>') ||
           trimmed.starts_with('✖') || trimmed.starts_with('━') ||
           trimmed.starts_with('┏') || trimmed.starts_with('┗') ||
           trimmed.starts_with('┃') || trimmed.starts_with('┣') ||
           trimmed.starts_with('┫') || trimmed.starts_with('╋') ||
           trimmed.starts_with('┳') || trimmed.starts_with('┻') {
            return (None, line.to_string());
        }
        
        // Skip cache hit lines
        if trimmed.contains("cache hit") || trimmed.contains("replaying logs") {
            return (None, line.to_string());
        }
        
        // Skip update notification lines
        if trimmed.contains("Update available") || 
           trimmed.contains("Changelog:") ||
           trimmed.contains("Follow @turborepo") {
            return (None, line.to_string());
        }
        
        // Skip command echo lines
        if trimmed.starts_with('>') && (trimmed.contains("eslint") || trimmed.contains("@graph-agent")) {
            return (None, line.to_string());
        }
        
        // Handle format: "@scope/package#task:" (pnpm workspace style with hash)
        // This must be checked before the colon format because @scope/pkg#task: has a #
        if line.starts_with('@') && line.contains('#') {
            if let Some(hash_pos) = line.find('#') {
                // Extract package name: @scope/package
                let package_name = line[..hash_pos].to_string();
                
                if let Some(colon_pos) = line[hash_pos..].find(':') {
                    let full_prefix_end = hash_pos + colon_pos + 1;
                    if full_prefix_end < line.len() {
                        return (Some(package_name), line[full_prefix_end..].to_string());
                    }
                }
            }
        }
        
        // Handle format: "@scope/package:task:" (scoped package with colon separator)
        // Format: @graph-agent/common-utils:lint:content
        if line.starts_with('@') {
            if let Some(first_colon) = line.find(':') {
                let package_name = line[..first_colon].to_string();
                let after_first = &line[first_colon + 1..];
                if let Some(second_colon) = after_first.find(':') {
                    let between_colons = &after_first[..second_colon];
                    if between_colons.parse::<u32>().is_err() {
                        let content_start = first_colon + 1 + second_colon + 1;
                        if content_start <= line.len() {
                            return (Some(package_name), line[content_start..].to_string());
                        }
                    }
                }
            }
        }
        
        // Handle format: "package:task:" (standard turbo style for non-scoped packages)
        if !line.starts_with('@') {
            if let Some(first_colon) = line.find(':') {
                let before_first = &line[..first_colon];
                
                // If it contains path separators, it's a file path, not a package prefix
                if before_first.contains('/') || before_first.contains('\\') {
                    return (None, line.to_string());
                }
                
                // Extract package name
                let package_name = before_first.to_string();
                
                if let Some(second_colon) = line[first_colon + 1..].find(':') {
                    let second_colon_pos = first_colon + 1 + second_colon;
                    let between_colons = &line[first_colon + 1..second_colon_pos];
                    
                    // If between colons is a number, it's a line number, not a task
                    if between_colons.parse::<u32>().is_ok() {
                        return (None, line.to_string());
                    }
                    
                    if second_colon_pos + 1 <= line.len() {
                        return (Some(package_name), line[second_colon_pos + 1..].to_string());
                    }
                }
            }
        }
        
        (None, line.to_string())
    }

    /// Strip turbo prefixes from output lines
    /// Handles multiple turbo formats:
    /// - "web:lint:    4:7   error    ..." -> ("web", "    4:7   error    ...")
    /// - "@scope/pkg#lint:    4:7   error    ..." -> ("@scope/pkg", "    4:7   error    ...")
    /// - "┌─ @scope/pkg#lint > ..." (TUI border lines) -> (None, "")
    #[allow(dead_code)]
    pub fn strip_turbo_prefixes(&self, output: &str) -> String {
        // First, strip ANSI codes
        let clean_output = self.strip_ansi_codes(output);
        
        // Process lines and handle merged/split lines
        let lines: Vec<String> = clean_output.lines().map(|s| s.to_string()).collect();
        let processed_lines = self.merge_and_clean_lines(&lines);
        
        processed_lines
            .into_iter()
            .filter_map(|line| {
                let (_package, content) = self.extract_package_and_content(&line);
                let trimmed = content.trim();
                
                // Skip empty lines
                if trimmed.is_empty() {
                    return None;
                }
                
                // Skip TUI border/decoration lines (already handled in extract_package_and_content)
                // But we need to check the content for summary lines
                if trimmed.starts_with('╭') || trimmed.starts_with('╰') || 
                   trimmed.starts_with('┌') || trimmed.starts_with('└') ||
                   trimmed.starts_with('│') || trimmed.starts_with('─') ||
                   trimmed.starts_with('├') || trimmed.starts_with('┤') ||
                   trimmed.starts_with('•') || trimmed.starts_with('>') ||
                   trimmed.starts_with('✖') || trimmed.starts_with('━') ||
                   trimmed.starts_with('┏') || trimmed.starts_with('┗') ||
                   trimmed.starts_with('┃') || trimmed.starts_with('┣') ||
                   trimmed.starts_with('┫') || trimmed.starts_with('╋') ||
                   trimmed.starts_with('┳') || trimmed.starts_with('┻') {
                    if trimmed.contains("error") || trimmed.contains("warning") {
                        // Keep summary lines
                    } else {
                        return None;
                    }
                }
                
                Some(content)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    
    /// New method: Strip turbo prefixes and return lines with package information
    /// Returns a vector of (package_name, content) tuples
    pub fn strip_turbo_prefixes_with_package(&self, output: &str) -> Vec<(Option<String>, String)> {
        // First, strip ANSI codes
        let clean_output = self.strip_ansi_codes(output);
        
        // Process lines and handle merged/split lines
        let lines: Vec<String> = clean_output.lines().map(|s| s.to_string()).collect();
        let processed_lines = self.merge_and_clean_lines(&lines);
        
        processed_lines
            .into_iter()
            .filter_map(|line| {
                let (package, content) = self.extract_package_and_content(&line);
                let trimmed = content.trim();
                
                // Skip empty lines
                if trimmed.is_empty() {
                    return None;
                }
                
                // Skip TUI border/decoration lines
                if trimmed.starts_with('╭') || trimmed.starts_with('╰') || 
                   trimmed.starts_with('┌') || trimmed.starts_with('└') ||
                   trimmed.starts_with('│') || trimmed.starts_with('─') ||
                   trimmed.starts_with('├') || trimmed.starts_with('┤') ||
                   trimmed.starts_with('•') || trimmed.starts_with('>') ||
                   trimmed.starts_with('✖') || trimmed.starts_with('━') ||
                   trimmed.starts_with('┏') || trimmed.starts_with('┗') ||
                   trimmed.starts_with('┃') || trimmed.starts_with('┣') ||
                   trimmed.starts_with('┫') || trimmed.starts_with('╋') ||
                   trimmed.starts_with('┳') || trimmed.starts_with('┻') {
                    if trimmed.contains("error") || trimmed.contains("warning") {
                        // Keep summary lines
                    } else {
                        return None;
                    }
                }
                
                Some((package, content))
            })
            .collect()
    }
    
    /// Merge lines that have been split or concatenated incorrectly
    /// Handles cases where:
    /// 1. Rule names are split across lines: "@typescript-eslint/no-ex" + "xplicit-any"
    /// 2. File paths are concatenated with previous lines: "...rule-nameD:\path\file.ts"
    fn merge_and_clean_lines(&self, lines: &[String]) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        let mut i = 0;
        
        while i < lines.len() {
            let line = &lines[i];
            let trimmed = line.trim();
            
            // Skip empty lines
            if trimmed.is_empty() {
                i += 1;
                continue;
            }
            
            // Check if this line ends with a partial rule name that should be merged with next line
            // Pattern: line ends with "@typescript-eslint/no-ex" and next line starts with "xplicit-any"
            if i + 1 < lines.len() {
                let next_line = &lines[i + 1];
                let next_trimmed = next_line.trim();
                
                // Check for split rule names
                let line_ends_with_partial = trimmed.ends_with("/no-ex") || 
                    trimmed.ends_with("/no-unus") || 
                    trimmed.ends_with("/no-expl") ||
                    trimmed.ends_with("/no-impl") ||
                    trimmed.ends_with("/requ") ||
                    trimmed.ends_with("/no-un") ||
                    trimmed.ends_with("licit-") ||
                    trimmed.ends_with("used-") ||
                    trimmed.ends_with("plicit-") ||
                    trimmed.ends_with("quired-") ||
                    trimmed.ends_with("nused-") ||
                    trimmed.ends_with("vars ") ||
                    trimmed.ends_with("any ");
                
                let next_starts_with_continuation = next_trimmed.starts_with("xplicit-any") ||
                    next_trimmed.starts_with("ed-vars") ||
                    next_trimmed.starts_with("icit-any") ||
                    next_trimmed.starts_with("icit-any") ||
                    next_trimmed.starts_with("ire") ||
                    next_trimmed.starts_with("ed-v") ||
                    next_trimmed.starts_with("ars") ||
                    next_trimmed.starts_with("ny");
                
                if line_ends_with_partial && next_starts_with_continuation {
                    // Merge the lines
                    let merged = format!("{}{}", line, next_line);
                    result.push(merged);
                    i += 2;
                    continue;
                }
            }
            
            // Check if this line has a file path concatenated at the end
            // Pattern: "...rule-nameD:\path\file.ts" -> split into "...rule-name" and "D:\path\file.ts"
            if let Some(path_start) = trimmed.find("D:\\") {
                // Check if there's content before the path (like a rule name without space)
                if path_start > 0 {
                    let before_path = &trimmed[..path_start];
                    let path_part = &trimmed[path_start..];
                    
                    // If before_path doesn't end with space, it's likely concatenated
                    if !before_path.ends_with(' ') && !before_path.ends_with('\t') {
                        // Add the rule part to previous line if exists, or as separate line
                        if let Some(last) = result.last_mut() {
                            *last = format!("{} {}", last.trim(), before_path.trim());
                        } else {
                            result.push(before_path.to_string());
                        }
                        // Add the path as a new line
                        result.push(path_part.to_string());
                        i += 1;
                        continue;
                    }
                }
            }
            
            // Check for Unix-style paths too
            if trimmed.contains('/') && !trimmed.starts_with("@") {
                // Find the last potential rule name before a path
                if let Some(pos) = trimmed.rfind("  /") {
                    let potential_rule = &trimmed[pos + 2..];
                    if potential_rule.contains('/') && !potential_rule.contains(':') {
                        // This might be a split line, try to find where path starts
                    }
                }
            }
            
            result.push(line.clone());
            i += 1;
        }
        
        result
    }

    fn parse_typescript_format(&self, line: &str) -> Option<Issue> {
        self.base.parse_parentheses_format(line).or_else(|| {
            let parts: Vec<&str> = line.splitn(4, ':').collect();
            if parts.len() >= 3 {
                let file_path = parts[0].trim();
                let line_num = parts[1].trim().parse::<u32>().ok()?;
                let rest = parts[2..].join(":");

                let rest_parts: Vec<&str> =
                    rest.splitn(2, ['-', ':']).collect();
                if rest_parts.len() >= 2 {
                    let col_num = rest_parts[0].trim().parse::<u32>().ok()?;
                    let level_msg = rest_parts[1].trim();

                    let level = self.base.detect_level(level_msg)?;

                    let message = if let Some(colon_pos) = level_msg.find(':') {
                        level_msg[colon_pos + 1..].trim().to_string()
                    } else {
                        level_msg.to_string()
                    };

                    let location = Location::new(file_path.to_string())
                        .with_line(line_num)
                        .with_column(col_num);

                    return Some(Issue::new(level, message, location));
                }
            }
            None
        })
    }

    fn parse_generic_error(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();

        if trimmed.to_uppercase().starts_with("ERROR") {
            let message = if let Some(space) = trimmed.find(|c: char| c.is_whitespace()) {
                trimmed[space..].trim().to_string()
            } else {
                trimmed.to_string()
            };

            return Some(Issue::new(
                IssueLevel::Error,
                message,
                Location::new("unknown"),
            ));
        }

        None
    }

    /// Parsing NPM Audit Error Formats
    /// 格式: "npm error code CODE" 或 "npm error message"
    fn parse_npm_error(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();

        // 匹配 "npm error code XXX"
        if trimmed.starts_with("npm error code") {
            let code = trimmed.strip_prefix("npm error code").unwrap_or("").trim();
            return Some(Issue::new(
                IssueLevel::Error,
                format!("NPM error code: {}", code),
                Location::new("package.json"),
            ));
        }

        // 匹配 "npm error XXX"（不包括 code 行）
        if trimmed.starts_with("npm error") && !trimmed.starts_with("npm error code") {
            let message = trimmed.strip_prefix("npm error").unwrap_or("").trim();
            // Skip some known non-error message lines
            if message.starts_with("A complete log")
                || message.starts_with("audit")
                || message.is_empty()
            {
                return None;
            }
            return Some(Issue::new(
                IssueLevel::Error,
                message.to_string(),
                Location::new("package.json"),
            ));
        }

        None
    }

    /// Resolving NPM Dependency Missing Errors
    /// 格式: "npm error missing: package@version, required by package@version"
    fn parse_npm_missing_dep(&self, line: &str) -> Option<Issue> {
        let trimmed = line.trim();

        if trimmed.starts_with("npm error missing:") {
            // Extracting missing package information
            let rest = trimmed.strip_prefix("npm error missing:").unwrap_or("").trim();
            return Some(Issue::new(
                IssueLevel::Error,
                format!("Missing dependency: {}", rest),
                Location::new("package.json"),
            ));
        }

        None
    }

    /// Analyzing npm audit security vulnerability reports
    /// Format.
    ///   package  version_range
    ///   Severity: level
    ///   description - https://...
    fn parse_npm_audit_vulnerability(&self, lines: &[String], start_index: usize) -> (Option<Issue>, usize) {
        if start_index >= lines.len() {
            return (None, start_index);
        }

        let line = &lines[start_index];
        let trimmed = line.trim();

        // Check if it is a package name line (format: "package version_range")
        // This is usually the package name and version range before the severity line in the audit report.
        if trimmed.starts_with("Severity:") || trimmed.is_empty() {
            return (None, start_index + 1);
        }

        // Look ahead one row to check if it is a Severity row
        if start_index + 1 >= lines.len() {
            return (None, start_index + 1);
        }

        let next_line = &lines[start_index + 1];
        if !next_line.trim().starts_with("Severity:") {
            return (None, start_index + 1);
        }

        // Extract package name and version range
        let package_info = trimmed.to_string();

        // 解析 severity
        let severity_line = next_line.trim();
        let severity = severity_line
            .strip_prefix("Severity:")
            .unwrap_or("")
            .trim()
            .to_lowercase();

        let level = match severity.as_str() {
            "critical" => IssueLevel::Error,
            "high" => IssueLevel::Error,
            "moderate" => IssueLevel::Warning,
            "low" => IssueLevel::Info,
            _ => IssueLevel::Warning,
        };

        // Collect descriptive information (may be on multiple lines)
        let mut descriptions = Vec::new();
        let mut i = start_index + 2;

        while i < lines.len() {
            let desc_line = &lines[i];
            let desc_trimmed = desc_line.trim();

            // Stop conditions: empty line, next set of vulnerabilities, dependency tree or fix suggestion
            if desc_trimmed.is_empty()
                || desc_trimmed.starts_with("fix available")
                || desc_trimmed.starts_with("node_modules/")
                || desc_trimmed.starts_with("Severity:")
                || (desc_trimmed.contains(" vulnerabilities") && desc_trimmed.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
            {
                break;
            }

            // Collection description (excluding dependency tree rows)
            if !desc_trimmed.starts_with("Depends on vulnerable")
                && !desc_trimmed.starts_with("node_modules/")
            {
                descriptions.push(desc_trimmed.to_string());
            }

            i += 1;
        }

        let message = if descriptions.is_empty() {
            format!("NPM error: Security vulnerability in {}", package_info)
        } else {
            format!(
                "NPM error: Security vulnerability in {} - {}",
                package_info,
                descriptions.join(" ")
            )
        };

        (
            Some(Issue::new(level, message, Location::new("package.json"))),
            i,
        )
    }
}

impl Default for NpmParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for NpmParser {
    // Custom parse implementation for NPM output
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        // Pre-process: strip turbo prefixes from lines with package information
        // Turbo format: "web:lint:    4:7   error    ..."
        let lines_with_packages = self.strip_turbo_prefixes_with_package(output);
        
        let mut issues = Vec::new();
        let mut i = 0;

        // Track current package context
        let mut current_package: Option<String> = None;

        while i < lines_with_packages.len() {
            let (package, line) = &lines_with_packages[i];
            
            // Update current package context when we see a new package
            if package.is_some() {
                current_package = package.clone();
            }

            // Prioritize parsing of TypeScript formats (formats with parentheses)
            if let Some(mut issue) = self.parse_typescript_format(line) {
                // Add package information
                if let Some(ref pkg) = current_package {
                    issue = issue.with_package(pkg);
                }
                issues.push(issue);
                i += 1;
                continue;
            }

            // Parsing the ESLint Format
            // Need to pass the lines without package info for context
            let lines: Vec<String> = lines_with_packages.iter().map(|(_, l)| l.clone()).collect();
            let (issue_opt, new_index) = self.parse_eslint_format(&lines, i);
            if let Some(mut issue) = issue_opt {
                // Add package information
                if let Some(ref pkg) = current_package {
                    issue.package = Some(pkg.clone());
                }
                issues.push(issue);
                i = new_index;
                continue;
            }

            // Parsing NPM Errors
            if let Some(mut issue) = self.parse_npm_error(line) {
                if let Some(ref pkg) = current_package {
                    issue = issue.with_package(pkg);
                }
                issues.push(issue);
                i += 1;
                continue;
            }

            // Resolving NPM Dependency Missing Errors
            if let Some(mut issue) = self.parse_npm_missing_dep(line) {
                if let Some(ref pkg) = current_package {
                    issue = issue.with_package(pkg);
                }
                issues.push(issue);
                i += 1;
                continue;
            }

            // Analyzing npm audit security vulnerabilities
            let (audit_issue_opt, new_index) = self.parse_npm_audit_vulnerability(&lines, i);
            if let Some(mut issue) = audit_issue_opt {
                if let Some(ref pkg) = current_package {
                    issue.package = Some(pkg.clone());
                }
                issues.push(issue);
                i = new_index;
                continue;
            }

            // Generic error analysis
            if let Some(mut issue) = self.parse_generic_error(line) {
                if let Some(ref pkg) = current_package {
                    issue = issue.with_package(pkg);
                }
                issues.push(issue);
            }

            i += 1;
        }

        ParseResult::Full(issues)
    }
}

impl TestOutputParser for NpmParser {
    fn parse_test_output(&self, output: &str) -> ParsedTestOutput {
        let mut result = ParsedTestOutput::new();

        // 1. Reuse of existing logic to resolve compilation/type-checking issues
        result.compile_issues = <Self as OutputParser>::parse(self, output).data_or_default_owned();

        // 2. Parsing test execution results
        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // Parse Jest test case line: "✓ <name> (<time>)" or "✕ <name> (<time>)"
            if let Some(test_case) = self.parse_jest_test_line(line) {
                match test_case.status {
                    TestStatus::Passed => result.passed_tests.push(test_case),
                    TestStatus::Failed => result.failed_tests.push(test_case),
                    TestStatus::Ignored(_) => result.ignored_tests.push(test_case),
                }
                i += 1;
                continue;
            }

            // Parsing Vitest test case line: " ✓ <name> <time>"
            if let Some(test_case) = self.parse_vitest_test_line(line) {
                match test_case.status {
                    TestStatus::Passed => result.passed_tests.push(test_case),
                    TestStatus::Failed => result.failed_tests.push(test_case),
                    TestStatus::Ignored(_) => result.ignored_tests.push(test_case),
                }
                i += 1;
                continue;
            }

            // Parsing Mocha Test Case Lines
            if let Some(test_case) = self.parse_mocha_test_line(line) {
                match test_case.status {
                    TestStatus::Passed => result.passed_tests.push(test_case),
                    TestStatus::Failed => result.failed_tests.push(test_case),
                    TestStatus::Ignored(_) => result.ignored_tests.push(test_case),
                }
                i += 1;
                continue;
            }

            // Parsing Jest Test Results Summary
            if line.starts_with("Tests:") {
                result.test_summary = self.parse_jest_summary(line);
            }

            // Analyzing Vitest Test Results Summary
            if line.contains("Test Files") && line.contains("tests") {
                result.test_summary = self.parse_vitest_summary(&lines, i);
            }

            // Analyzing Mocha Test Results Summary
            if line.starts_with("  ") && line.contains(" passing") && line.contains(" failing") {
                result.test_summary = self.parse_mocha_summary(line);
            }

            i += 1;
        }

        result
    }
}

impl NpmParser {
    /// Parsing Jest Test Case Lines
    /// 格式: "✓ test name (5 ms)" 或 "✕ test name (5 ms)"
    fn parse_jest_test_line(&self, line: &str) -> Option<TestCase> {
        let trimmed = line.trim();

        // 通过: "✓ test name"
        if let Some(name) = trimmed.strip_prefix('✓') {
            let name = name.trim();
            // 提取时间: "test name (5 ms)"
            let (name, time) = self.extract_time_from_name(name);
            return Some(TestCase {
                name,
                status: TestStatus::Passed,
                location: None,
                failure_details: None,
                execution_time: time,
            });
        }

        // 失败: "✕ test name"
        if let Some(name) = trimmed.strip_prefix('✕') {
            let name = name.trim();
            let (name, time) = self.extract_time_from_name(name);
            return Some(TestCase {
                name,
                status: TestStatus::Failed,
                location: None,
                failure_details: None,
                execution_time: time,
            });
        }

        // 跳过: "○ test name"
        if let Some(name) = trimmed.strip_prefix('○') {
            let name = name.trim();
            return Some(TestCase {
                name: name.to_string(),
                status: TestStatus::Ignored(None),
                location: None,
                failure_details: None,
                execution_time: None,
            });
        }

        None
    }

    /// Parsing Vitest Test Case Lines
    /// 格式: " ✓ test name 5ms" 或 " ✗ test name 5ms"
    fn parse_vitest_test_line(&self, line: &str) -> Option<TestCase> {
        let trimmed = line.trim();

        // 通过: "✓ test name 5ms"
        if trimmed.starts_with("✓ ") {
            let rest = &trimmed[2..];
            let (name, time) = self.extract_vitest_time(rest);
            return Some(TestCase {
                name,
                status: TestStatus::Passed,
                location: None,
                failure_details: None,
                execution_time: time,
            });
        }

        // 失败: "✗ test name 5ms"
        if trimmed.starts_with("✗ ") {
            let rest = &trimmed[2..];
            let (name, time) = self.extract_vitest_time(rest);
            return Some(TestCase {
                name,
                status: TestStatus::Failed,
                location: None,
                failure_details: None,
                execution_time: time,
            });
        }

        // 跳过: "⏭ test name"
        if trimmed.starts_with("⏭ ") {
            let name = trimmed[2..].trim().to_string();
            return Some(TestCase {
                name,
                status: TestStatus::Ignored(None),
                location: None,
                failure_details: None,
                execution_time: None,
            });
        }

        None
    }

    /// Parsing Mocha Test Case Lines
    /// 格式: " ✓ test name" 或 " 1) test name"
    fn parse_mocha_test_line(&self, line: &str) -> Option<TestCase> {
        let trimmed = line.trim();

        // 通过: "✓ test name"
        if trimmed.starts_with("✓ ") {
            let name = trimmed[2..].trim().to_string();
            return Some(TestCase {
                name,
                status: TestStatus::Passed,
                location: None,
                failure_details: None,
                execution_time: None,
            });
        }

        // 失败: "1) test name"
        if let Some(caps) = regex::Regex::new(r"^\d+\)\s+(.+)$").ok()?.captures(trimmed) {
            let name = caps.get(1)?.as_str().to_string();
            return Some(TestCase {
                name,
                status: TestStatus::Failed,
                location: None,
                failure_details: None,
                execution_time: None,
            });
        }

        None
    }

    /// Extract time from test name
    /// 格式: "test name (5 ms)" -> ("test name", Some(0.005))
    fn extract_time_from_name(&self, name: &str) -> (String, Option<f64>) {
        if let Some(start) = name.rfind("(") {
            if let Some(end) = name[start..].find(")") {
                let time_str = &name[start + 1..start + end];
                // Parsing "5 ms" or "0.5 s"
                let time = if time_str.contains("ms") {
                    time_str
                        .trim()
                        .strip_suffix("ms")
                        .unwrap_or("")
                        .trim()
                        .parse::<f64>()
                        .map(|t| t / 1000.0)
                        .ok()
                } else if time_str.contains('s') {
                    time_str
                        .trim()
                        .strip_suffix('s')
                        .unwrap_or("")
                        .trim()
                        .parse::<f64>()
                        .ok()
                } else {
                    None
                };
                return (name[..start].trim().to_string(), time);
            }
        }
        (name.to_string(), None)
    }

    /// Extracting time from Vitest rows
    /// 格式: "test name 5ms" -> ("test name", Some(0.005))
    fn extract_vitest_time(&self, rest: &str) -> (String, Option<f64>) {
        // lookup time from the end
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 2 {
            let last = parts.last().unwrap();
            if last.ends_with("ms") {
                if let Ok(time) = last.strip_suffix("ms").unwrap_or("").parse::<f64>() {
                    let name = parts[..parts.len() - 1].join(" ");
                    return (name, Some(time / 1000.0));
                }
            }
        }
        (rest.to_string(), None)
    }

    /// Parsing Jest Test Results Summary
    /// 格式: "Tests: 5 passed, 1 failed, 2 skipped, 10 total"
    fn parse_jest_summary(&self, line: &str) -> Option<TestSummary> {
        let re = regex::Regex::new(
            r"Tests:\s+(\d+)\s+passed,?\s*(?:(\d+)\s+failed,?)?\s*(?:(\d+)\s+skipped,?)?\s*(?:(\d+)\s+total)?",
        )
        .ok()?;

        let caps = re.captures(line)?;

        let passed: usize = caps.get(1)?.as_str().parse().ok()?;
        let failed: usize = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        let ignored: usize = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);

        Some(TestSummary {
            total: passed + failed + ignored,
            passed,
            failed,
            ignored,
            measured: 0,
            filtered: 0,
            execution_time: None,
        })
    }

    /// Analyzing Vitest Test Results Summary
    /// Format Multiline.
    /// " Test Files  1 passed (1)"
    /// "      Tests  5 passed (5)"
    fn parse_vitest_summary(&self, lines: &[&str], start_index: usize) -> Option<TestSummary> {
        let mut passed = 0;
        let mut failed = 0;
        let ignored = 0;

        let passed_regex = regex::Regex::new(r"Tests\s+(\d+)\s+passed").ok()?;
        let failed_regex = regex::Regex::new(r"(\d+)\s+failed").ok()?;

        // Look back a few rows from the current row
        for line in lines.iter().skip(start_index).take(5) {
            // 匹配 "Tests 5 passed (5)"
            if let Some(caps) = passed_regex.captures(line) {
                passed = caps.get(1)?.as_str().parse().ok()?;
            }
            // Number of failed matches
            if let Some(caps) = failed_regex.captures(line) {
                failed = caps.get(1)?.as_str().parse().ok()?;
            }
        }

        Some(TestSummary {
            total: passed + failed + ignored,
            passed,
            failed,
            ignored,
            measured: 0,
            filtered: 0,
            execution_time: None,
        })
    }

    /// Analyzing Mocha Test Results Summary
    /// 格式: " 5 passing (10ms)" 或 " 5 passing (10ms)\n 1 failing"
    fn parse_mocha_summary(&self, line: &str) -> Option<TestSummary> {
        let re = regex::Regex::new(r"(\d+)\s+passing").ok()?;
        let caps = re.captures(line)?;
        let passed: usize = caps.get(1)?.as_str().parse().ok()?;

        let failed_re = regex::Regex::new(r"(\d+)\s+failing").ok()?;
        let failed = failed_re
            .captures(line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);

        Some(TestSummary {
            total: passed + failed,
            passed,
            failed,
            ignored: 0,
            measured: 0,
            filtered: 0,
            execution_time: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_typescript_parentheses() {
        let parser = NpmParser::new();
        let line = "src/file.ts(10,5): error TS1234: Message";
        let issue = parser.parse_typescript_format(line).expect("Failed to parse");

        assert_eq!(issue.location.file_path, "src/file.ts");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.message, "Message");
        assert!(matches!(issue.level, IssueLevel::Error));
        assert!(issue.code.is_some());
    }

    #[test]
    fn test_parse_eslint_format() {
        let parser = NpmParser::new();
        let lines = vec![
            "/path/to/file.js".to_string(),
            "  10:5  error  Message  rule-name".to_string(),
        ];

        let (issue, next_index) = parser.parse_eslint_format(&lines, 1);
        let issue = issue.unwrap();

        assert_eq!(issue.location.file_path, "/path/to/file.js");
        assert_eq!(issue.location.line_number, Some(10));
        assert_eq!(issue.location.column_number, Some(5));
        assert_eq!(issue.message, "Message");
        assert_eq!(next_index, 2);
    }

    #[test]
    fn test_strip_turbo_prefixes() {
        let parser = NpmParser::new();
        
        // Test basic turbo prefix stripping
        let input = "web:lint:    4:7   error    'unusedVariable' is assigned a value but never used  @typescript-eslint/no-unused-vars";
        let result = parser.strip_turbo_prefixes(input);
        assert_eq!(result, "    4:7   error    'unusedVariable' is assigned a value but never used  @typescript-eslint/no-unused-vars");
        
        // Test multi-line output
        let input = "web:lint: line 1\nweb:lint:    4:7   error    message  rule\nnormal line";
        let result = parser.strip_turbo_prefixes(input);
        assert_eq!(result, " line 1\n    4:7   error    message  rule\nnormal line");
        
        // Test with task name containing hyphen
        let input = "docs:type-check:    1:1   error    message  rule";
        let result = parser.strip_turbo_prefixes(input);
        assert_eq!(result, "    1:1   error    message  rule");
    }

    #[test]
    fn test_parse_with_turbo_prefix() {
        let parser = NpmParser::new();
        
        // Test parsing output with turbo prefixes
        let output = r#"web:lint: src/index.ts
web:lint:    4:7   error    'unusedVariable' is assigned a value but never used  @typescript-eslint/no-unused-vars
web:lint:    7:24  warning  'unusedParam' is defined but never used              @typescript-eslint/no-unused-vars"#;
        
        let issues = parser.parse(output).data_or_default_owned();
        assert_eq!(issues.len(), 2, "Should parse 2 issues with turbo prefix");
        
        let first = &issues[0];
        assert_eq!(first.location.line_number, Some(4));
        assert!(matches!(first.level, IssueLevel::Error));
        
        let second = &issues[1];
        assert_eq!(second.location.line_number, Some(7));
        assert!(matches!(second.level, IssueLevel::Warning));
    }

    #[test]
    fn test_parse_turbo_tui_format() {
        let parser = NpmParser::new();
        
        // Test parsing output with turbo TUI format (from user's real output)
        let output = r#"┌─ @graph-agent/storage#lint > cache hit, replaying logs f184635148ed46e6 

> @graph-agent/storage@1.0.0 lint D:\project\packages\storage
> eslint . --ext .ts


D:\project\packages\storage\src\json\base-json-storage.ts
   16:3   warning  'CompressionResult' is defined but never used  @typescript-eslint/no-unused-vars
   422:17  warning  'id' is assigned a value but never used        @typescript-eslint/no-unused-vars

✖ 7 problems (0 errors, 7 warnings)
└─ @graph-agent/storage#lint ──"#;
        
        let issues = parser.parse(output).data_or_default_owned();
        assert_eq!(issues.len(), 2, "Should parse 2 ESLint issues from turbo TUI output, found {}", issues.len());
        
        let first = &issues[0];
        assert_eq!(first.location.file_path, "D:\\project\\packages\\storage\\src\\json\\base-json-storage.ts");
        assert_eq!(first.location.line_number, Some(16));
        assert_eq!(first.location.column_number, Some(3));
        assert!(matches!(first.level, IssueLevel::Warning));
        assert!(first.message.contains("CompressionResult"));
        
        let second = &issues[1];
        assert_eq!(second.location.line_number, Some(422));
        assert_eq!(second.location.column_number, Some(17));
        assert!(matches!(second.level, IssueLevel::Warning));
        assert!(second.message.contains("id"));
    }

    #[test]
    fn test_strip_turbo_tui_borders() {
        let parser = NpmParser::new();
        
        // Test TUI border lines are stripped
        let input = "┌─ @graph-agent/storage#lint > cache hit, replaying logs";
        let result = parser.strip_turbo_prefixes(input);
        assert_eq!(result, "");
        
        // Test border lines with content are kept
        let input = "│   16:3   warning  'CompressionResult' is defined but never used";
        let result = parser.strip_turbo_prefixes(input);
        assert!(result.contains("warning"));
        
        // Test scoped package format
        let input = "@graph-agent/storage#lint:   16:3   warning  message  rule";
        let result = parser.strip_turbo_prefixes(input);
        assert_eq!(result, "   16:3   warning  message  rule");
    }

    #[test]
    fn test_parse_eslint_compact_format() {
        let parser = NpmParser::new();
        
        // Test ESLint compact format: file:line:col: level message
        let line = "src/index.js:1:1: error: Missing semicolon semi";
        let issue = parser.parse_typescript_format(line);
        
        assert!(issue.is_some(), "Should parse ESLint compact format");
        let issue = issue.unwrap();
        assert_eq!(issue.location.file_path, "src/index.js");
        assert_eq!(issue.location.line_number, Some(1));
        assert_eq!(issue.location.column_number, Some(1));
        assert!(matches!(issue.level, IssueLevel::Error));
        assert!(issue.message.contains("Missing semicolon"));
    }

    #[test]
    fn test_parse_eslint_compact_format_warning() {
        let parser = NpmParser::new();
        
        // Test ESLint compact format with warning
        let line = "src/index.js:2:5: warning: Unused variable 'foo' no-unused-vars";
        let issue = parser.parse_typescript_format(line);
        
        assert!(issue.is_some(), "Should parse ESLint compact format warning");
        let issue = issue.unwrap();
        assert_eq!(issue.location.file_path, "src/index.js");
        assert_eq!(issue.location.line_number, Some(2));
        assert_eq!(issue.location.column_number, Some(5));
        assert!(matches!(issue.level, IssueLevel::Warning));
        assert!(issue.message.contains("Unused variable"));
    }

    #[test]
    fn test_extract_package_from_turbo_output() {
        let parser = NpmParser::new();
        
        // Test scoped package with hash format
        let line = "@graph-agent/storage#lint:   16:3   warning  message  rule";
        let (package, content) = parser.extract_package_and_content(line);
        assert_eq!(package, Some("@graph-agent/storage".to_string()));
        assert_eq!(content, "   16:3   warning  message  rule");
        
        // Test scoped package with colon format
        let line = "@graph-agent/common-utils:lint:   10:5  error  message";
        let (package, content) = parser.extract_package_and_content(line);
        assert_eq!(package, Some("@graph-agent/common-utils".to_string()));
        assert_eq!(content, "   10:5  error  message");
        
        // Test non-scoped package
        let line = "web:lint:   4:7   error  message  rule";
        let (package, content) = parser.extract_package_and_content(line);
        assert_eq!(package, Some("web".to_string()));
        assert_eq!(content, "   4:7   error  message  rule");
        
        // Test file path (should not extract package)
        let line = "src/index.ts:   4:7   error  message";
        let (package, _content) = parser.extract_package_and_content(line);
        assert_eq!(package, None);
    }

    #[test]
    fn test_parse_with_package_information() {
        let parser = NpmParser::new();
        
        // Test parsing output with multiple packages
        let output = r#"@graph-agent/storage#lint: D:\project\packages\storage\src\index.ts
@graph-agent/storage#lint:    16:3   warning  'CompressionResult' is defined but never used  @typescript-eslint/no-unused-vars
@graph-agent/common-utils#lint: D:\project\packages\common-utils\src\utils.ts
@graph-agent/common-utils#lint:    10:5  error  Unexpected any  @typescript-eslint/no-explicit-any"#;
        
        let issues = parser.parse(output).data_or_default_owned();
        assert_eq!(issues.len(), 2, "Should parse 2 issues from different packages");
        
        // Verify first issue has package information
        let first = &issues[0];
        assert_eq!(first.package, Some("@graph-agent/storage".to_string()));
        assert_eq!(first.location.line_number, Some(16));
        assert!(matches!(first.level, IssueLevel::Warning));
        
        // Verify second issue has package information
        let second = &issues[1];
        assert_eq!(second.package, Some("@graph-agent/common-utils".to_string()));
        assert_eq!(second.location.line_number, Some(10));
        assert!(matches!(second.level, IssueLevel::Error));
    }

    #[test]
    fn test_strip_turbo_prefixes_with_package() {
        let parser = NpmParser::new();
        
        let output = r#"web:lint: src/index.ts
web:lint:    4:7   error  'unusedVariable' is assigned a value but never used  @typescript-eslint/no-unused-vars
app:lint: src/utils.ts
app:lint:    10:5  warning  Unexpected any  @typescript-eslint/no-explicit-any"#;
        
        let lines_with_packages = parser.strip_turbo_prefixes_with_package(output);
        
        // Should have 4 lines (2 file paths + 2 issues)
        assert!(lines_with_packages.len() >= 2, "Should have at least 2 lines with content");
        
        // Check that package information is preserved
        let package_lines: Vec<_> = lines_with_packages.iter()
            .filter(|(pkg, _)| pkg.is_some())
            .collect();
        assert!(package_lines.len() >= 2, "Should have at least 2 lines with package info");
    }
}
