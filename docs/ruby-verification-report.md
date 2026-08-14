# Ruby 分析功能验证报告

- 验证日期：2026-08-09
- 验证对象：multi-analyzer（analyzer v0.2.0）
- 验证范围：Ruby 插件（rubocop / rspec / rake / ruby / rails）
- 测试项目：`test-projects/ruby-app/`

## 1. 测试项目

`test-projects/` 下新增 `ruby-app/` 验证项目，模拟真实 Ruby/Rails 工程，结构如下：

| 文件 | 内容 | 设计意图 |
| ---- | ---- | -------- |
| `Gemfile` | 含 `rails ~> 7.1.0`、`rspec`、`rubocop`、`rake` | 命中 deploy-website / publish-website 的 Ruby/Rails 检测规则 |
| `app.rb` | 含 Style/NilComparison、Lint/UselessAssignment 等违规 + 故意运行时错误（NoMethodError） | 验证 rubocop 违规识别与运行时错误解析 |
| `lib/calculator.rb` | 含 Lint/UselessAssignment、Naming/MethodParameterName 等违规 + 故意除零 | 同上 |
| `spec/calculator_spec.rb` | 4 个 RSpec 用例，其中 1 个故意失败 | 验证 rspec 失败测试解析 |
| `Rakefile` | `rake test` 任务调用 rspec | 验证 rake 测试链路 |
| `bin/main.rb` | 含故意运行时错误（NoMethodError） | 验证可执行脚本错误解析 |

## 2. 验证环境

| 组件 | 版本 |
| ---- | ---- |
| Ruby | 3.1.2 |
| Bundler | 2.3.15 |
| RuboCop | 1.89.0 |
| RSpec | 3.13 |
| Rails | 7.1.6（bundle install 全量安装，86 gems） |

注意：`bundle install` 时 `psych 5.4.0` 原生扩展构建失败（`yaml.h not found`），需先安装系统包 `libyaml-dev` 后重试成功。该问题源于 Ruby 3.1 + psych 5.x 的编译依赖，与 analyzer 无关。

## 3. 功能验证结果

### 3.1 直接分析模式

| 测试项 | 命令 | 结果 |
| ------ | ---- | ---- |
| RuboCop 违规识别 | `analyzer ruby "rubocop ."` | 通过（修复后）：`bundle exec rubocop . --format json`，解析出 22 个违规（11 类，6 文件） |
| RuboCop 显式 JSON | `analyzer ruby "rubocop --format json ."` | 通过：不重复注入 `--format` |
| RSpec 失败测试 | `analyzer ruby "rspec spec"` | 通过：识别 `spec/calculator_spec.rb:21` 的 `RSpec::Expectations::ExpectationNotMetError`（expected 42 got 1） |
| RSpec tech stack | `analyzer rspec "spec"` | 通过（修复后）：`bundle exec rspec spec --format json`，1 个失败测试 |
| Rake 测试链路 | `analyzer ruby "rake test"` | 通过：Test 路径，4 tests（3 passed / 1 failed） |
| 运行时错误（脚本） | `analyzer ruby "ruby app.rb"` | 通过：识别 `app.rb:21` `NoMethodError: undefined method 'upcase' for nil` |
| 运行时错误（库） | `analyzer ruby "ruby bin/main.rb"` | 通过：识别 `bin/main.rb` `NoMethodError` |
| rails 别名 + 完整命令 | `analyzer ruby "rails server -p 8000"` | 通过：命令拼接为 `bundle exec rails server -p 8000`，与 deploy-website 期望一致 |
| rails 别名 + 裸参数 | `analyzer rails "server -p 8000"` | 通过（修复后）：自动恢复工具名，执行 `bundle exec rails server -p 8000` |

### 3.2 报告格式

| 格式 | 结果 |
| ---- | ---- |
| markdown（默认） | 正常，rubocop/rspec 报告标题分别为 "RuboCop Report" / "RSpec Report"（修复后） |
| json | 正常，metadata/summary/items 完整 |
| raw / html | 未单独验证（与 C++ 共用管线） |

### 3.3 discover 子命令（run / rewrite）

| 测试项 | 命令 | 结果 |
| ------ | ---- | ---- |
| run 检测 rubocop | `analyzer run "rubocop ."` | 通过（修复后）：`bundle exec rubocop . --format json` → 22 issues |
| run 检测 rspec | `analyzer run "bundle exec rspec spec"` | 通过（修复后）：`bundle exec rspec spec --format json` → 1 个失败测试 |
| run 检测 rails | `analyzer run "bundle exec rails server -p 8000"` | 已知限制：no matching rule（discover 规则不含 rails；deploy-website 直接执行命令，不经 analyzer） |

## 4. skills 目录期望核对

| Skill | 期望 | 核对结果 |
| ----- | ---- | -------- |
| deploy-website | Ruby/Rails 检测：`Gemfile` 包含 `rails` | 通过：ruby-app 的 Gemfile 含 `rails`，可被检测 |
| deploy-website | 启动命令：`bundle exec rails server -p 8000` | 通过：`analyzer ruby "rails server -p 8000"` 生成正确命令（但 `analyzer rails "server -p 8000"` 别名用法有坑，见 5.3） |
| deploy-website | 依赖安装：`bundle install`（若存在 `Gemfile`） | 通过：ruby-app 可正常 `bundle install` |
| publish-website | `Gemfile` 含 `rails` / `sinatra` → 判定 backend | 通过：ruby-app 命中 backend 判定条件 |

## 5. 与预期不符的发现与修复

### 5.1 rubocop JSON 输出被 stderr 污染导致解析失败 —— 已修复

`analyzer ruby "rubocop ."` 实际执行 `bundle exec rubocop . --format json`（stdout 为合法 JSON，含 23 个 offenses），但修复前报告只输出 1 个伪 error：

```
error: Command failed (exit code 1). Raw output: {"metadata":{...},"files":[...]}
```

**根因**：`core/stream.rs` 的 `execute_streaming` 将 stdout 与 stderr 合并 feed 给行过滤器。rubocop 将 "The following cops were added to RuboCop, but are not configured..." 提示（9.3KB）写入 stderr，与 stdout 的 JSON 拼接后，`serde_json::from_str` 因 trailing 内容解析失败 → 0 issues → `run_analyzer` 的 "command failed" 兜底逻辑把原始输出误报为错误，23 个真实违规全部丢失。

**修复**：`plugins/ruby/parser.rs` 新增 `extract_json_object()`，从合并输出中精确提取第一个顶层 JSON 对象（跳过字符串内的花括号），`detect_output_type`、`parse_rubocop_json`、`parse_rspec_json`、`parse_rspec_test_results` 统一改为解析提取后的 JSON 载荷。

```bash
analyzer ruby "rubocop ."
# 修复前: 1 个伪 error（Command failed + 原始 JSON）
# 修复后: 22 issues（11 类违规，6 文件，含 app.rb 的 Style/NilComparison 等）
```

### 5.2 `analyzer rspec` 报 Unknown tech stack —— 已修复

```bash
analyzer rspec "spec"
# 修复前: Error: Unknown tech stack 'rspec'
```

**根因**：`core/types.rs` 定义了 `TechStack::Rspec`、help 也列出 `rspec`，但 `plugins/mod.rs` 只注册了 `RubyAnalyzer`（`tech_stack()` 返回 `TechStack::Rubocop`），注册表中没有任何 `tech_stack() == Rspec` 的分析器，`registry.get(TechStack::Rspec)` 返回 None。

**修复**：`RubyAnalyzer` 增加 `stack` 字段与 `with_stack()` / `rspec()` 构造器，`plugins/mod.rs` 同时注册 `RubyAnalyzer::new()`（rubocop）与 `RubyAnalyzer::rspec()`。

```bash
analyzer rspec "spec"
# 修复后: bundle exec rspec spec --format json → 识别失败测试 spec/calculator_spec.rb:21
```

### 5.3 `analyzer rails "server -p 8000"` 丢失 rails 前缀 —— 已修复

```bash
analyzer rails "server -p 8000"
# 修复前: 实际执行 bundle exec server -p 8000 → bundler: command not found: server (exit 127)
```

**根因**：`RubyAnalyzer::create_command_builder` 对非 rubocop/rspec 命令走 `bundle exec <subcommand>` 分支，而 `rails` 作为 tech stack 别名传入时 subcommand 只有 `server -p 8000`，不含 `rails`。

**修复**：`AnalyzeOptions` 新增 `raw_tech_stack` 字段（`main.rs` 在参数解析时记录用户输入的原始 stack 字符串）；`create_command_builder` 重构为三段式：subcommand 以已知工具名开头（rubocop/rspec/rake/rails/ruby）→ 原样拼接；以 `bundle` 开头 → 跳过重复的 `exec`；裸参数 → 用 `raw_tech_stack` 恢复工具名。rubocop/rspec 自动注入 `--format json`（用户显式指定 `--format`/`-f` 时不重复注入）。

```bash
analyzer rails "server -p 8000"
# 修复后: bundle exec rails server -p 8000（与 deploy-website 期望一致）
```

### 5.4 run 模式 rubocop 前缀被剥离 —— 已修复

```bash
analyzer run "rubocop ."
# 修复前: 实际执行 bundle exec . → 0 issues（错误命令）
```

**根因**：`discover/rules.rs` 的 rubocop 规则 `^rubocop\b\s*(.*)$` 捕获参数部分作为 subcommand，重写为 `analyzer rubocop "."`。RubyAnalyzer 收到 subcommand `.` 后检测不到 rubocop/rspec 前缀，落入 `bundle exec .` 分支。

**修复**：复用 5.3 的裸参数恢复逻辑（`raw_tech_stack` = "rubocop" → 补前缀 + 注入 `--format json`）。

```bash
analyzer run "rubocop ."
# 修复后: bundle exec rubocop . --format json → 22 issues
```

### 5.5 run 模式 rspec 指向未注册 stack —— 已修复（5.2 + 5.4 连带）

```bash
analyzer run "bundle exec rspec spec"
# 修复前: 重写为 analyzer rspec "spec" → Unknown tech stack 'rspec'
# 修复后: bundle exec rspec spec --format json → 识别失败测试
```

### 5.6 报告标题不准确 —— 已修复

rubocop/rspec 的 markdown/html 报告标题此前显示 "Type Check Report"（reporter 的 `detect_report_type` 未覆盖 Ruby 类 stack，落入消息启发式的默认分支）。修复后 `detect_report_type` 优先基于 `ReportOptions.tech_stack` 判断，rubocop 显示 "RuboCop Report"、rspec 显示 "RSpec Report"，其余 stack 保持原启发式逻辑。

### 5.7 测试补充

`plugins/ruby/analyzer.rs` 新增 4 个命令构造测试（已知工具、裸参数恢复、bundle 前缀、rspec stack），`cargo test --release` 全量测试通过（ruby 插件 14 个用例）。

## 6. 结论

Ruby 插件的核心能力（RuboCop 违规、RSpec 失败测试、rake 测试链路、运行时错误、JSON 报告）符合预期；ruby-app 项目满足 deploy-website / publish-website 的 Ruby/Rails 检测条件。验证发现的 6 处问题（2 严重 / 3 中等 / 1 轻微）已全部修复并回归验证通过；已知限制：`analyzer run` 自动检测不含 rails 规则（rails server 属部署命令，deploy-website 直接执行，不经 analyzer）。
