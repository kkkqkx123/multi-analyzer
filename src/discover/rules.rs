/// Static command rule table for classifying raw shell commands.
///
/// Each rule maps a shell command pattern to a TechStack and subcommand template.
/// The pattern uses regex capture groups; the subcommand_template references
/// captured groups via {1}, {2}, etc.
///
/// Rules are evaluated in order; the first match wins.
///
/// ## Rule Priority
///
/// Lower `priority` values are matched first. Within the same priority,
/// more specific patterns should appear before broad fallback patterns.
use crate::core::TechStack;

/// A single command classification rule
#[derive(Debug, Clone)]
pub struct CommandRule {
    /// Regex pattern for matching raw shell commands (case-insensitive flag applied at match time)
    pub pattern: &'static str,
    /// Target technology stack
    pub tech_stack: TechStack,
    /// Subcommand template with capture group references: {0}=full match, {1}=group 1, ...
    pub subcommand_template: &'static str,
    /// Known command prefixes for display / suggestion purposes
    pub prefixes: &'static [&'static str],
    /// Human-readable category label
    pub category: &'static str,
}

/// A logical group of related rules.
#[derive(Debug, Clone)]
pub struct RuleSet {
    /// Category label shared by all rules in this set
    pub category: &'static str,
    /// Rules in this set, evaluated in order (first match wins)
    pub rules: &'static [CommandRule],
}

impl RuleSet {
    /// Create a new rule set from a category and slice of rules.
    pub const fn new(category: &'static str, rules: &'static [CommandRule]) -> Self {
        Self { category, rules }
    }

    /// Number of rules in this set.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// True if the set has no rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Iterate over all rules in this set.
    pub fn iter(&self) -> std::slice::Iter<'static, CommandRule> {
        self.rules.iter()
    }
}

/// All rule sets, organized by category for selective matching.
pub const RULE_SETS: &[RuleSet] = &[
    RuleSet::new("Rust", RUST_RULES),
    RuleSet::new("Node.js", NODE_RULES),
    RuleSet::new("Python", PYTHON_RULES),
    RuleSet::new("Go", GO_RULES),
    RuleSet::new("Java", JAVA_RULES),
    RuleSet::new(".NET", DOTNET_RULES),
    RuleSet::new("Ruby", RUBY_RULES),
    RuleSet::new("C++", CPP_RULES),
    RuleSet::new("Fallback", FALLBACK_RULES),
];

/// Rust toolchain rules
pub const RUST_RULES: &[CommandRule] = &[
    CommandRule {
        pattern: r"^cargo\s+(check|clippy|test|build|fmt)",
        tech_stack: TechStack::Cargo,
        subcommand_template: "{1}",
        prefixes: &["cargo"],
        category: "Rust",
    },
    CommandRule {
        pattern: r"^cargo\s+nextest\s+(run|list|archive)\b",
        tech_stack: TechStack::Cargo,
        subcommand_template: "nextest {1}",
        prefixes: &["cargo"],
        category: "Rust",
    },
];

/// Node.js ecosystem rules
pub const NODE_RULES: &[CommandRule] = &[
    CommandRule {
        pattern: r"^npm\s+(run\s+)?(lint|typecheck|audit|test)",
        tech_stack: TechStack::Npm,
        subcommand_template: "run {2}",
        prefixes: &["npm"],
        category: "Node.js",
    },
    CommandRule {
        // Generic: pnpm [any flags/args] [run] <subcommand>
        pattern: r"^pnpm\s+(?:\S+\s+)*(?:run\s+)?(lint|typecheck|audit|test|exec\s+tsc)",
        tech_stack: TechStack::Pnpm,
        subcommand_template: "{1}",
        prefixes: &["pnpm"],
        category: "Node.js",
    },
    CommandRule {
        pattern: r"^yarn\s+(run\s+)?(lint|typecheck|audit|test)",
        tech_stack: TechStack::Yarn,
        subcommand_template: "run {2}",
        prefixes: &["yarn"],
        category: "Node.js",
    },
];

/// Python ecosystem rules
pub const PYTHON_RULES: &[CommandRule] = &[
    CommandRule {
        pattern: r"^mypy\b\s*(.*)$",
        tech_stack: TechStack::Mypy,
        subcommand_template: "{1}",
        prefixes: &["mypy"],
        category: "Python",
    },
    CommandRule {
        pattern: r"^pytest\b\s*(.*)$",
        tech_stack: TechStack::Pytest,
        subcommand_template: "{1}",
        prefixes: &["pytest"],
        category: "Python",
    },
    CommandRule {
        pattern: r"^ruff\s+(check|format)\b",
        tech_stack: TechStack::Ruff,
        subcommand_template: "{1}",
        prefixes: &["ruff"],
        category: "Python",
    },
    CommandRule {
        pattern: r"^black\b\s*(.*)$",
        tech_stack: TechStack::Black,
        subcommand_template: "{1}",
        prefixes: &["black"],
        category: "Python",
    },
];

/// Go ecosystem rules
pub const GO_RULES: &[CommandRule] = &[
    CommandRule {
        pattern: r"^go\s+(build|test|vet|fmt)",
        tech_stack: TechStack::GoBuild,
        subcommand_template: "{1}",
        prefixes: &["go"],
        category: "Go",
    },
    CommandRule {
        pattern: r"^gofmt\b",
        tech_stack: TechStack::GoBuild,
        subcommand_template: "fmt",
        prefixes: &["gofmt"],
        category: "Go",
    },
    CommandRule {
        pattern: r"^golangci-lint\s+run\b",
        tech_stack: TechStack::GolangciLint,
        subcommand_template: "run",
        prefixes: &["golangci-lint"],
        category: "Go",
    },
];

/// Java ecosystem rules
pub const JAVA_RULES: &[CommandRule] = &[
    CommandRule {
        pattern: r"^mvn\s+(compile|test|verify|package)",
        tech_stack: TechStack::Maven,
        subcommand_template: "{1}",
        prefixes: &["mvn"],
        category: "Java",
    },
    CommandRule {
        pattern: r"^(gradle|gradlew)\s+(compileJava|test|check)",
        tech_stack: TechStack::Gradle,
        subcommand_template: "{2}",
        prefixes: &["gradle", "gradlew"],
        category: "Java",
    },
];

/// .NET ecosystem rules
pub const DOTNET_RULES: &[CommandRule] = &[
    CommandRule {
        pattern: r"^dotnet\s+(build|test|format)",
        tech_stack: TechStack::Dotnet,
        subcommand_template: "{1}",
        prefixes: &["dotnet"],
        category: ".NET",
    },
];

/// Ruby ecosystem rules
pub const RUBY_RULES: &[CommandRule] = &[
    CommandRule {
        pattern: r"^rubocop\b\s*(.*)$",
        tech_stack: TechStack::Rubocop,
        subcommand_template: "{1}",
        prefixes: &["rubocop"],
        category: "Ruby",
    },
    CommandRule {
        pattern: r"^(bundle\s+exec\s+)?rspec\b\s*(.*)$",
        tech_stack: TechStack::Rspec,
        subcommand_template: "{2}",
        prefixes: &["rspec", "bundle"],
        category: "Ruby",
    },
];

/// C++ ecosystem rules
pub const CPP_RULES: &[CommandRule] = &[
    CommandRule {
        pattern: r"^cmake\s+(--build|--configure)",
        tech_stack: TechStack::CMake,
        subcommand_template: "{1}",
        prefixes: &["cmake"],
        category: "C++",
    },
    CommandRule {
        pattern: r"^(gcc|g\+\+)\s+.*-c\b",
        tech_stack: TechStack::Gcc,
        subcommand_template: "compile",
        prefixes: &["gcc", "g++"],
        category: "C++",
    },
    CommandRule {
        pattern: r"^(clang|clang\+\+)\s+.*(-c\b|-fsyntax-only\b)",
        tech_stack: TechStack::Clang,
        subcommand_template: "compile",
        prefixes: &["clang", "clang++"],
        category: "C++",
    },
    CommandRule {
        pattern: r"^clang-format\b",
        tech_stack: TechStack::ClangFormat,
        subcommand_template: "format",
        prefixes: &["clang-format"],
        category: "C++",
    },
    CommandRule {
        pattern: r"^(cl\.exe|msvc)\s+",
        tech_stack: TechStack::Msvc,
        subcommand_template: "compile",
        prefixes: &["cl", "msvc"],
        category: "C++",
    },
];

/// Broad fallback rules evaluated after all specific categories
pub const FALLBACK_RULES: &[CommandRule] = &[
    CommandRule {
        pattern: r"^npm\s+(?:run\s+)?(\S+)",
        tech_stack: TechStack::Npm,
        subcommand_template: "run {1}",
        prefixes: &["npm"],
        category: "Fallback",
    },
    CommandRule {
        pattern: r"^dotnet\s+(\S+)",
        tech_stack: TechStack::Dotnet,
        subcommand_template: "{1}",
        prefixes: &["dotnet"],
        category: "Fallback",
    },
];

/// Master rule table: all rules flattened in evaluation order.
///
/// Specific rules appear first, fallback rules last.
/// Rules are evaluated in order; the first match wins.
pub const RULES: &[CommandRule] = &[
    // ============ Rust ============
    CommandRule {
        pattern: r"^cargo\s+(check|clippy|test|build|fmt)",
        tech_stack: TechStack::Cargo,
        subcommand_template: "{1}",
        prefixes: &["cargo"],
        category: "Rust",
    },
    CommandRule {
        pattern: r"^cargo\s+nextest\s+(run|list|archive)\b",
        tech_stack: TechStack::Cargo,
        subcommand_template: "nextest {1}",
        prefixes: &["cargo"],
        category: "Rust",
    },
    // ============ Node.js ============
    CommandRule {
        pattern: r"^npm\s+(run\s+)?(lint|typecheck|audit|test)",
        tech_stack: TechStack::Npm,
        subcommand_template: "run {2}",
        prefixes: &["npm"],
        category: "Node.js",
    },
    CommandRule {
        // Generic: pnpm [any flags/args] [run] <subcommand>
        pattern: r"^pnpm\s+(?:\S+\s+)*(?:run\s+)?(lint|typecheck|audit|test|exec\s+tsc)",
        tech_stack: TechStack::Pnpm,
        subcommand_template: "{1}",
        prefixes: &["pnpm"],
        category: "Node.js",
    },
    CommandRule {
        pattern: r"^yarn\s+(run\s+)?(lint|typecheck|audit|test)",
        tech_stack: TechStack::Yarn,
        subcommand_template: "run {2}",
        prefixes: &["yarn"],
        category: "Node.js",
    },
    // ============ Python ============
    CommandRule {
        pattern: r"^mypy\b\s*(.*)$",
        tech_stack: TechStack::Mypy,
        subcommand_template: "{1}",
        prefixes: &["mypy"],
        category: "Python",
    },
    CommandRule {
        pattern: r"^pytest\b\s*(.*)$",
        tech_stack: TechStack::Pytest,
        subcommand_template: "{1}",
        prefixes: &["pytest"],
        category: "Python",
    },
    CommandRule {
        pattern: r"^ruff\s+(check|format)\b",
        tech_stack: TechStack::Ruff,
        subcommand_template: "{1}",
        prefixes: &["ruff"],
        category: "Python",
    },
    CommandRule {
        pattern: r"^black\b\s*(.*)$",
        tech_stack: TechStack::Black,
        subcommand_template: "{1}",
        prefixes: &["black"],
        category: "Python",
    },
    // ============ Go ============
    CommandRule {
        pattern: r"^go\s+(build|test|vet|fmt)",
        tech_stack: TechStack::GoBuild,
        subcommand_template: "{1}",
        prefixes: &["go"],
        category: "Go",
    },
    CommandRule {
        pattern: r"^gofmt\b",
        tech_stack: TechStack::GoBuild,
        subcommand_template: "fmt",
        prefixes: &["gofmt"],
        category: "Go",
    },
    CommandRule {
        pattern: r"^golangci-lint\s+run\b",
        tech_stack: TechStack::GolangciLint,
        subcommand_template: "run",
        prefixes: &["golangci-lint"],
        category: "Go",
    },
    // ============ Java ============
    CommandRule {
        pattern: r"^mvn\s+(compile|test|verify|package)",
        tech_stack: TechStack::Maven,
        subcommand_template: "{1}",
        prefixes: &["mvn"],
        category: "Java",
    },
    CommandRule {
        pattern: r"^(gradle|gradlew)\s+(compileJava|test|check)",
        tech_stack: TechStack::Gradle,
        subcommand_template: "{2}",
        prefixes: &["gradle", "gradlew"],
        category: "Java",
    },
    // ============ .NET ============
    CommandRule {
        pattern: r"^dotnet\s+(build|test|format)",
        tech_stack: TechStack::Dotnet,
        subcommand_template: "{1}",
        prefixes: &["dotnet"],
        category: ".NET",
    },
    // ============ Ruby ============
    CommandRule {
        pattern: r"^rubocop\b\s*(.*)$",
        tech_stack: TechStack::Rubocop,
        subcommand_template: "{1}",
        prefixes: &["rubocop"],
        category: "Ruby",
    },
    CommandRule {
        pattern: r"^(bundle\s+exec\s+)?rspec\b\s*(.*)$",
        tech_stack: TechStack::Rspec,
        subcommand_template: "{2}",
        prefixes: &["rspec", "bundle"],
        category: "Ruby",
    },
    // ============ C++ ============
    CommandRule {
        pattern: r"^cmake\s+(--build|--configure)",
        tech_stack: TechStack::CMake,
        subcommand_template: "{1}",
        prefixes: &["cmake"],
        category: "C++",
    },
    CommandRule {
        pattern: r"^(gcc|g\+\+)\s+.*-c\b",
        tech_stack: TechStack::Gcc,
        subcommand_template: "compile",
        prefixes: &["gcc", "g++"],
        category: "C++",
    },
    CommandRule {
        pattern: r"^(clang|clang\+\+)\s+.*(-c\b|-fsyntax-only\b)",
        tech_stack: TechStack::Clang,
        subcommand_template: "compile",
        prefixes: &["clang", "clang++"],
        category: "C++",
    },
    CommandRule {
        pattern: r"^clang-format\b",
        tech_stack: TechStack::ClangFormat,
        subcommand_template: "format",
        prefixes: &["clang-format"],
        category: "C++",
    },
    CommandRule {
        pattern: r"^(cl\.exe|msvc)\s+",
        tech_stack: TechStack::Msvc,
        subcommand_template: "compile",
        prefixes: &["cl", "msvc"],
        category: "C++",
    },
    // ============ Broad fallback patterns (lower priority, after specific rules) ============
    // npm: any subcommand not covered above (handle `npm run <script>` and `npm <cmd>`)
    CommandRule {
        pattern: r"^npm\s+(?:run\s+)?(\S+)",
        tech_stack: TechStack::Npm,
        subcommand_template: "run {1}",
        prefixes: &["npm"],
        category: "Fallback",
    },
    // dotnet: any subcommand not covered above
    CommandRule {
        pattern: r"^dotnet\s+(\S+)",
        tech_stack: TechStack::Dotnet,
        subcommand_template: "{1}",
        prefixes: &["dotnet"],
        category: "Fallback",
    },
];

/// Look up a rule by its 0-based index in RULES
pub fn rule_by_index(idx: usize) -> Option<&'static CommandRule> {
    RULES.get(idx)
}

/// Find all rules that match a given base command prefix.
///
/// Returns rules whose `prefixes` includes the given prefix string.
pub fn find_rules_by_prefix(prefix: &str) -> Vec<&'static CommandRule> {
    RULES
        .iter()
        .filter(|r| r.prefixes.contains(&prefix))
        .collect()
}

/// Find all rules belonging to a specific category.
pub fn find_rules_by_category(category: &str) -> Vec<&'static CommandRule> {
    RULES
        .iter()
        .filter(|r| r.category == category)
        .collect()
}

/// Get a `RuleSet` by category name.
pub fn rule_set_by_category(category: &str) -> Option<&'static RuleSet> {
    RULE_SETS
        .iter()
        .find(|rs| rs.category == category)
}

/// Total number of rules across all rule sets.
pub fn total_rule_count() -> usize {
    RULE_SETS.iter().map(|rs| rs.len()).sum()
}

/// Total number of rules in the flat RULES table.
pub fn flat_rule_count() -> usize {
    RULES.len()
}

/// Get all unique categories from the rule sets.
pub fn all_categories() -> Vec<&'static str> {
    RULE_SETS
        .iter()
        .map(|rs| rs.category)
        .collect()
}

/// Check if a command string matches any rule (quick classification check).
pub fn has_matching_rule(cmd: &str) -> bool {
    RULES.iter().any(|rule| {
        regex::Regex::new(&format!("(?i){}", rule.pattern))
            .map(|re| re.is_match(cmd))
            .unwrap_or(false)
    })
}
