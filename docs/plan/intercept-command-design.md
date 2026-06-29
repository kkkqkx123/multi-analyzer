# 拦截命令功能设计方案

## 1. 概述

### 1.1 目标

将 multi-analyzer 从当前固定的 `<tech-stack> <command>` 二段式调用，扩展为支持直接接收原始 Shell 命令的能力。用户输入任意构建命令，analyzer 内部自动完成技术栈识别、子命令提取、命令构建、执行、解析、输出。

### 1.2 参考来源

设计参考 `ref/rtk-develop/` 中 RTK 的命令重写引擎（`src/discover/registry.rs`）、规则表（`src/discover/rules.rs`）和 runner 模式（`src/core/runner.rs`）。

### 1.3 核心原则

- 完全 CLI 调用，不涉及 AI Agent hook 集成
- 通过新增子命令入口实现，不破坏现有 `analyzer <tech-stack> <command>` 用法
- 数据输出格式参照 RTK 的 JSON 字段命名
- 直接输出能力通过新增 `raw` 格式实现

---

## 2. 新增 CLI 入口

### 2.1 `analyzer run <raw_shell_command>` 

单步命令：接收原始 shell 命令，内部完成改写 + 执行 + 解析 + 输出。

```bash
# 用法示例
analyzer run "cargo check --all-targets"
analyzer run "npm run lint"
analyzer run "pytest -v"
analyzer run "go vet ./..."
analyzer run "mvn test -pl core"
```

**内部流程：**

```
raw_shell_command
    │
    ▼
[1] 命令分类 (classify_command)
    ├── 正则匹配 RULES 表
    ├── 提取 base_command + subcommand
    └── 映射到 TechStack
    │
    ▼
[2] 命令重写 (rewrite_command)
    ├── 构建 analyzer 等价命令参数
    └── 返回 (TechStack, SubCommand, 额外参数)
    │
    ▼
[3] 插件调度
    ├── PluginRegistry.get(tech_stack)
    └── analyzer.analyze(options)
    │
    ▼
[4] 执行 + 解析 + 输出
```

### 2.2 `analyzer rewrite <raw_shell_command>`

预览命令：显示改写结果，不实际执行。

```bash
# 用法示例
$ analyzer rewrite "cargo check --all-targets"
analyzer cargo check --all-targets

$ analyzer rewrite "npm run lint"
analyzer npm run lint

$ analyzer rewrite "go vet ./..."
analyzer go vet ./...
```

### 2.3 `analyzer run` 的选项

| 参数 | 说明 |
|------|------|
| `--format <fmt>` | 输出格式：`markdown`, `json`, `html`, `raw` (默认：`markdown`) |
| `--output <file>` | 输出文件路径（默认：`analysis_report.<ext>`） |
| `--stdout` | 仅输出到 stdout，不写文件 |
| `--filter-warnings` | 过滤警告 |
| `--filter-paths <paths>` | 按路径过滤 |
| `--verbose` | 详细模式 |
| `--quiet` | 最小化输出 |

### 2.4 退出码协议

参考 RTK `rewrite_cmd.rs` 的退出码设计：

| 退出码 | stdout | 含义 |
|--------|--------|------|
| 0 | (正常输出) | 改写成功 + 执行成功 |
| 1 | (none) / (错误) | 无匹配的技术栈 / 执行失败 |
| 2 | (none) | 不支持的子命令 (SubCommand not supported) |

---

## 3. 命令分类与重写引擎

### 3.1 新增模块：`src/discover/`

```
src/discover/
├── mod.rs              # 模块入口
├── registry.rs         # classify_command() + rewrite_command() 入口
├── rules.rs            # 静态规则表 RULES
└── lexer.rs            # 复合命令词法分析 (&&, ||, ;, |, &)
```

### 3.2 规则表格式

参考 RTK `src/discover/rules.rs` 的 `RULES` 静态数组格式：

```rust
pub struct CommandRule {
    /// 匹配正则 (匹配原始 shell 命令)
    pub pattern: &'static str,
    /// 对应的 TechStack
    pub tech_stack: TechStack,
    /// 子命令 (提取后传给插件)
    pub subcommand_template: &'static str,
    /// 触发改写的前缀列表
    pub prefixes: &'static [&'static str],
    /// 分类标签
    pub category: &'static str,
}

pub const RULES: &[CommandRule] = &[
    // Rust
    CommandRule {
        pattern: r"^cargo\s+(check|clippy|test|build|fmt)",
        tech_stack: TechStack::Cargo,
        subcommand_template: "{1}",  // check/clippy/test/build/fmt
        prefixes: &["cargo"],
        category: "Rust",
    },
    // JS/TS - npm
    CommandRule {
        pattern: r"^npm\s+(run\s+)?(lint|typecheck|audit|test)",
        tech_stack: TechStack::Npm,
        subcommand_template: "run {2}",
        prefixes: &["npm"],
        category: "Node.js",
    },
    // JS/TS - pnpm
    CommandRule {
        pattern: r"^pnpm\s+(run\s+)?(lint|typecheck|audit|test|exec\s+tsc)",
        tech_stack: TechStack::Pnpm,
        subcommand_template: "{0}",
        prefixes: &["pnpm"],
        category: "Node.js",
    },
    // JS/TS - yarn
    CommandRule {
        pattern: r"^yarn\s+(run\s+)?(lint|typecheck|audit|test)",
        tech_stack: TechStack::Yarn,
        subcommand_template: "run {2}",
        prefixes: &["yarn"],
        category: "Node.js",
    },
    // Python - mypy
    CommandRule {
        pattern: r"^mypy\b",
        tech_stack: TechStack::Mypy,
        subcommand_template: "mypy",
        prefixes: &["mypy"],
        category: "Python",
    },
    // Python - pytest
    CommandRule {
        pattern: r"^pytest\b",
        tech_stack: TechStack::Pytest,
        subcommand_template: "pytest",
        prefixes: &["pytest"],
        category: "Python",
    },
    // Go
    CommandRule {
        pattern: r"^go\s+(build|test|vet)",
        tech_stack: TechStack::GoBuild,
        subcommand_template: "{1}",
        prefixes: &["go"],
        category: "Go",
    },
    // Go - golangci-lint
    CommandRule {
        pattern: r"^golangci-lint\s+run\b",
        tech_stack: TechStack::GolangciLint,
        subcommand_template: "run",
        prefixes: &["golangci-lint"],
        category: "Go",
    },
    // Java - Maven
    CommandRule {
        pattern: r"^mvn\s+(compile|test|verify|package)",
        tech_stack: TechStack::Maven,
        subcommand_template: "{1}",
        prefixes: &["mvn"],
        category: "Java",
    },
    // Java - Gradle
    CommandRule {
        pattern: r"^(gradle|gradlew)\s+(compileJava|test|check)",
        tech_stack: TechStack::Gradle,
        subcommand_template: "{2}",
        prefixes: &["gradle", "gradlew"],
        category: "Java",
    },
    // .NET
    CommandRule {
        pattern: r"^dotnet\s+(build|test)",
        tech_stack: TechStack::Dotnet,
        subcommand_template: "{1}",
        prefixes: &["dotnet"],
        category: ".NET",
    },
    // Ruby
    CommandRule {
        pattern: r"^rubocop\b",
        tech_stack: TechStack::Rubocop,
        subcommand_template: "rubocop",
        prefixes: &["rubocop"],
        category: "Ruby",
    },
    CommandRule {
        pattern: r"^(bundle\s+exec\s+)?rspec\b",
        tech_stack: TechStack::Rspec,
        subcommand_template: "rspec",
        prefixes: &["rspec", "bundle"],
        category: "Ruby",
    },
    // C++ - CMake
    CommandRule {
        pattern: r"^cmake\s+(--build|--configure)",
        tech_stack: TechStack::CMake,
        subcommand_template: "{1}",
        prefixes: &["cmake"],
        category: "C++",
    },
    // C++ - GCC
    CommandRule {
        pattern: r"^(gcc|g\+\+)\s+.*-c\b",
        tech_stack: TechStack::Gcc,
        subcommand_template: "compile",
        prefixes: &["gcc", "g++"],
        category: "C++",
    },
    // C++ - Clang
    CommandRule {
        pattern: r"^(clang|clang\+\+)\s+.*-c\b",
        tech_stack: TechStack::Clang,
        subcommand_template: "compile",
        prefixes: &["clang", "clang++"],
        category: "C++",
    },
    // C++ - MSVC
    CommandRule {
        pattern: r"^(cl\.exe|msvc)\s+",
        tech_stack: TechStack::Msvc,
        subcommand_template: "compile",
        prefixes: &["cl", "msvc"],
        category: "C++",
    },
];
```

### 3.3 分类与重写核心函数

```rust
/// 分类结果
pub enum Classification {
    /// 匹配成功，返回对应的技术栈和子命令
    Matched {
        tech_stack: TechStack,
        subcommand: String,
        extra_args: Vec<String>,  // 原始命令中除前缀和子命令外的额外参数
        rule_index: usize,
    },
    /// 无匹配规则
    Unmatched {
        base_command: String,
    },
}

/// 对原始 shell 命令进行分类
pub fn classify_command(raw_cmd: &str) -> Classification {
    // 1. 剥离环境变量前缀: ENV=val cmd → cmd
    // 2. 使用 RegexSet 匹配 RULES
    // 3. 提取捕获组，组装 TechStack + subcommand + extra_args
}

/// 重写命令：raw_shell → (tech_stack, subcommand, extra_args)
pub fn rewrite_command(raw_cmd: &str) -> Option<(TechStack, String, Vec<String>)> {
    match classify_command(raw_cmd) {
        Classification::Matched { tech_stack, subcommand, extra_args, .. } => {
            Some((tech_stack, subcommand, extra_args))
        }
        Classification::Unmatched { .. } => None,
    }
}
```

### 3.4 复合命令处理

参考 RTK `src/discover/lexer.rs` 的处理方式：

```rust
/// 按 shell 运算符拆分复合命令
/// "cargo fmt && cargo check" → ["cargo fmt", "cargo check"]
/// "go test | tee output.txt" → ["go test", "tee output.txt"]
pub fn split_on_operators(cmd: &str) -> Vec<String> {
    // 识别运算符: &&, ||, ;, |, &
    // 每个 segment 独立调用 classify_command 和 rewrite
    // 仅改写左侧，管道右侧保持不变
}
```

---

## 4. 直接输出能力

### 4.1 新增 `raw` 输出格式

在 `ReportFormat` 枚举中新增 `Raw` 变体：

```rust
pub enum ReportFormat {
    Markdown,
    Json,
    Html,
    Raw,  // [新增] 纯文本/机器可读，无排版标记
}
```

### 4.2 Raw Reporter

新增 `src/core/reporter/raw.rs`：

```rust
pub struct RawReporter;

impl Reporter for RawReporter {
    fn generate(&self, result: &AnalysisResult) -> Result<String, ReporterError> {
        // 输出格式参考 RTK 的 JSON 输出，字段命名保持一致：
        //   - issues: 问题列表
        //   - summary: 汇总统计
        //   - metadata: 元信息
        //
        // 每行一个 issue，管道符分隔：
        //   LEVEL|CODE|FILE:LINE:COL|MESSAGE
        //
        // 或 JSON 行格式 (--format raw-json)：
        //   {"level":"error","code":"E0308","file":"src/main.rs","line":10,"message":"..."}

        let mut output = String::new();
        for (file_path, issues) in &result.issues_by_file {
            for issue in issues {
                let line = issue.location.line_number.map(|n| n.to_string()).unwrap_or_default();
                let col = issue.location.column_number.map(|n| n.to_string()).unwrap_or_default();
                let code = issue.code.as_deref().unwrap_or("-");
                output.push_str(&format!(
                    "{}|{}|{}:{}:{}|{}\n",
                    issue.level, code, file_path, line, col, issue.message
                ));
            }
        }
        Ok(output)
    }
}
```

### 4.3 `--stdout` 参数

新增 `--stdout` 参数，输出到 stdout 而不写文件：

```rust
// main.rs parse_arguments 中
"--stdout" => {
    options.stdout_only = true;
}
```

对应的 `AnalyzeOptions` 新增字段：

```rust
pub struct AnalyzeOptions {
    // ... existing fields
    /// 仅输出到 stdout，不写文件
    pub stdout_only: bool,
}
```

### 4.4 `--output-format` 参数（已有，扩展值域）

现有 `--format` 参数扩展支持 `raw`：

```bash
analyzer run "cargo check" --format raw --stdout
# 输出:
# error|E0308|src/main.rs:10:5|mismatched types
# warning|dead_code|src/lib.rs:3:1|function is never used
```

支持 `raw-json` 子格式：

```bash
analyzer run "cargo check" --format raw-json --stdout
# 输出 (每行一个 JSON 对象):
# {"level":"error","code":"E0308","file":"src/main.rs","line":10,"column":5,"message":"mismatched types"}
# {"level":"warning","code":"dead_code","file":"src/lib.rs","line":3,"column":1,"message":"function is never used"}
```

---

## 5. 模块变更总结

### 5.1 新增模块

| 路径 | 说明 |
|------|------|
| `src/discover/mod.rs` | 模块入口 |
| `src/discover/rules.rs` | 静态 RULES 规则表 (~18 条规则) |
| `src/discover/registry.rs` | `classify_command()` + `rewrite_command()` |
| `src/discover/lexer.rs` | 复合命令拆分 `split_on_operators()` |
| `src/core/reporter/raw.rs` | RawReporter 实现 |
| `docs/plan/intercept-command-design.md` | 本文档 |

### 5.2 修改模块

| 路径 | 变更 |
|------|------|
| `src/main.rs` | 新增 `run` 和 `rewrite` 子命令入口；新增 `--stdout` 参数解析 |
| `src/core/types.rs` | `ReportFormat` 新增 `Raw`；`AnalyzeOptions` 新增 `stdout_only` |
| `src/core/reporter/mod.rs` | `ReporterFactory` 支持 `Raw` 格式 |
| `src/lib.rs` | 导出新增模块 |

### 5.3 不修改

- 现有 `analyzer <tech-stack> <command>` 用法保持不变
- 现有插件系统 (`src/plugins/`) 保持不变
- 现有配置加载机制保持不变

---

## 6. 数据输出格式参考

### 6.1 标准 JSON 输出 (`--format json`)

参考 RTK 的 JSON reporter 输出和 `AnalysisResult` 结构：

```json
{
  "metadata": {
    "total": 5,
    "categories": 3,
    "files_affected": 2
  },
  "summary_by_level": {
    "error": 3,
    "warning": 2
  },
  "summary_by_code": {
    "E0308": 2,
    "dead_code": 1
  },
  "issues": [
    {
      "level": "error",
      "code": "E0308",
      "file": "src/main.rs",
      "line": 10,
      "column": 5,
      "message": "mismatched types",
      "context": null,
      "package": null
    }
  ],
  "top_files": [
    {"file": "src/main.rs", "count": 3},
    {"file": "src/lib.rs", "count": 2}
  ]
}
```

### 6.2 Raw 管道输出 (`--format raw`)

```
LEVEL|CODE|FILE:LINE:COL|MESSAGE
error|E0308|src/main.rs:10:5|mismatched types
warning|dead_code|src/lib.rs:3:1|function is never used
error|E0004|src/main.rs:15:9|non-exhaustive patterns
```

### 6.3 Raw-JSON 行输出 (`--format raw-json`)

```json
{"level":"error","code":"E0308","file":"src/main.rs","line":10,"column":5,"message":"mismatched types"}
{"level":"warning","code":"dead_code","file":"src/lib.rs","line":3,"column":1,"message":"function is never used"}
```

### 6.4 Markdown 输出 (`--format markdown`, 默认)

保持现有格式不变。

---

## 7. 实现阶段

### Phase 1: 命令发现与重写引擎

- 实现 `src/discover/rules.rs` (RULES 规则表)
- 实现 `src/discover/registry.rs` (classify + rewrite)
- 实现 `src/discover/lexer.rs` (复合命令拆分)
- 添加 `analyzer rewrite` 子命令

### Phase 2: `analyzer run` 入口

- 在 `main.rs` 添加 `run` 子命令
- 实现原始命令 → 技术栈自动调度
- 将额外参数传递到插件的 `AnalyzeOptions`

### Phase 3: 直接输出支持

- 新增 `Raw` ReportFormat 和 `RawReporter`
- 新增 `--stdout` 参数
- 新增 `--format raw` 和 `--format raw-json` CLI 支持

### Phase 4: 测试与文档

- 添加 `tests/discover_integration_tests.rs`
- 添加 `tests/raw_reporter_tests.rs`
- 更新 README_zh.md
