# Maven / Gradle 分析功能验证报告

- 验证日期：2026-08-14
- 验证对象：multi-analyzer（analyzer v0.2.0，含本次 html/raw 测试报告修复）
- 验证范围：Java 插件（maven / gradle，含 mvn / gradlew 别名）
- 测试项目：`test-projects/maven-java/`、`test-projects/gradle-java/`

## 1. 测试项目

`test-projects/` 下新增两个验证项目，模拟真实 Java 工程（与既有 `pnpm-turbo`、`ruby-app` 等项目的设计一致）。

### maven-java/

| 文件 | 内容 | 设计意图 |
| ---- | ---- | -------- |
| `pom.xml` | JUnit 5 + compiler/surefire 插件，`showWarnings`/`showDeprecation` 开启 | 编译期输出 warnings 供 analyzer 解析 |
| `src/main/java/com/example/App.java` | 故意 raw type（unchecked warning） | 验证 warning 识别 |
| `src/main/java/com/example/Utils.java` | 故意 deprecated API 调用（getYear/getMonth/getDate） | 同上 |
| `src/broken/java/com/example/Broken.java` | 类名与文件名不符 + 未定义变量/方法 | 仅 `-Pbroken` profile 加入构建，验证编译错误识别 |
| `src/test/java/com/example/AppTest.java` | 4 个通过用例 | 验证测试通过统计 |
| `src/test/java/com/example/UtilsTest.java` | 3 个通过 + 1 个故意失败（testFormatDate） | 验证失败测试解析 |

`-Pbroken` profile 通过 build-helper-maven-plugin 将 `src/broken/java` 加入源码目录，使"默认可构建可测试"与"故意编译失败"两种状态互不干扰。

### gradle-java/

| 文件 | 内容 | 设计意图 |
| ---- | ---- | -------- |
| `build.gradle` | Java 插件 + JUnit 5，`-Xlint:unchecked/deprecation`，testLogging 输出 `PASSED/FAILED/SKIPPED` 事件行 | 编译 warnings 与测试事件行供 analyzer 解析 |
| `src/main/java/com/example/App.java` | 故意 raw type warning | 验证 warning 识别 |
| `src/main/java/com/example/Utils.java` | 故意 deprecated API 调用 | 同上 |
| `src/broken/java/com/example/Broken.java` | 类名不符 + 未定义变量/方法 | 仅 `-Pbroken` 属性加入构建，验证编译错误识别 |
| `src/test/java/com/example/AppTest.java` | 4 个通过用例 | 验证测试通过统计 |
| `src/test/java/com/example/UtilsTest.java` | 3 个通过 + 1 个故意失败（testFormatDate） | 验证失败测试解析 |

`sourceSets` 根据 `project.hasProperty('broken')` 决定是否包含 `src/broken/java`。

## 2. 验证环境

| 组件 | 版本 |
| ---- | ---- |
| Java | OpenJDK 17.0.20 |
| Maven | 3.8.7 |
| Gradle | 8.14.2 |
| JUnit | 5.9.1 / 5.9.2 |

依赖安装：`mvn dependency:go-offline` 与 `gradle dependencies` 均成功；`mvn clean test` 与 `gradle clean test` 实际运行 8 个用例（7 通过 / 1 失败），确认依赖完整、项目可构建。

## 3. 功能验证结果

### 3.1 直接分析模式

| 测试项 | 命令 | 结果 |
| ------ | ---- | ---- |
| Maven 编译 warnings | `analyzer maven "clean compile"` | 通过：3 个 deprecation warning，文件/行/列正确（`Utils.java:18:24` 等） |
| Maven 编译错误 | `analyzer maven "-Pbroken clean compile"` | 通过：3 errors（Broken.java 2 个 cannot find symbol + pom.xml 1 个 Failed to execute goal）+ 3 warnings；Maven 重复打印的编译错误已去重 |
| Maven 测试 | `analyzer maven "test"` | 通过：Test 路径，8 tests（7 passed / 1 failed），失败名规范化为 `com.example.UtilsTest::testFormatDate`，失败详情完整 |
| Gradle 编译 warnings | `analyzer gradle "clean compileJava"` | 通过：4 warnings（3 deprecation + 1 unchecked） |
| Gradle 编译错误 | `analyzer gradle "-Pbroken clean compileJava"` | 通过：2 errors（Broken.java:10/13）+ 4 warnings；"What went wrong" 重复段落已去重 |
| Gradle 测试 | `analyzer gradle "test"` | 通过：8 tests（7 passed / 1 failed），失败测试 `UtilsTest::testFormatDate()` 详情完整，7 个通过用例逐条列出 |

### 3.2 discover 子命令（run / rewrite）

| 测试项 | 命令 | 结果 |
| ------ | ---- | ---- |
| rewrite（Maven） | `analyzer rewrite "mvn test"` | 通过：输出 `analyzer maven "test"`（exit 0） |
| rewrite（Gradle） | `analyzer rewrite "gradle compileJava --quiet"` | 通过：输出 `analyzer gradle "compileJava --quiet"`（exit 0） |
| rewrite（非标准命令） | `analyzer rewrite "mvn -Pbroken compile"` | 符合预期：exit 1，提示 "no matching rule"（discover 规则表只覆盖标准构建命令，文档已声明该约束） |
| run（Maven） | `analyzer run "mvn test"` | 通过：自动识别 maven，测试分析结果与直接模式一致 |
| run（Gradle） | `analyzer run "gradle test"` | 通过：自动识别 gradle，测试分析结果与直接模式一致 |

### 3.3 报告格式

| 格式 | 结果 |
| ---- | ---- |
| markdown（默认） | 正常，编译报告标题 "Analysis Report" / 测试报告标题 "Test Report - Issues Found" |
| json | 正常，metadata（total_issues/compile_errors/compile_warnings/total_tests/collected_tests）、test_summary、failed_tests/passed_tests 完整 |
| html | 修复后正常：完整 HTML 测试报告（Summary / Failed Tests 表格 / Passed Tests 列表），HTML 特殊字符已转义 |
| raw | 修复后正常：`TEST_SUMMARY\|total=..\|passed=..\|failed=..\|ignored=..` + `TEST\|FAILED/PASSED/SKIPPED\|名称\|详情` 行 |
| raw-json | 修复后正常：每行一个 JSON 对象（test_summary / test / issue） |

### 3.4 常用选项

| 选项 | 结果 |
| ---- | ---- |
| `--filter-warnings` | 通过：只保留 errors（3 errors / 0 warnings） |
| `--filter-paths Broken.java` | 通过：只保留匹配路径的 issues |
| `--max-issues 2` | 通过：只输出前 2 个 issues |
| `-o <file>` | 通过：报告写入指定文件 |
| `--no-short-circuit` | 通过：禁用成功短回路 |
| `config show` / `config init` | 通过：显示默认配置 / 创建 `~/.config/analyzer/config.toml` |
| `stats` | 符合预期：首次使用前无记录（"No analysis runs recorded yet"） |

## 4. 与 skills 目录预期的一致性

对照 `multi-analyzer/skills/analyzer-usage/SKILL.md` 与 `references/analyzer-reference.md`：

- 文档中的 Maven/Gradle 用法示例（`analyzer maven "compile -q"`、`analyzer maven "test"`、`analyzer gradle "compileJava --quiet"`、`analyzer gradle "test"`）全部可用，命令构造（`mvn <subcommand>` / `gradle <subcommand>` 透传）与文档一致。
- tech stack 别名（`maven`/`mvn`、`gradle`/`gradlew`）在 `--help` 中正确展示。
- `run` / `rewrite` 行为（标准命令匹配、非标准命令 exit 1）与文档约束一致。
- 报告格式（markdown/json/html/raw/raw-json）与文档描述一致。

## 5. 发现并修复的缺陷

测试过程中发现 **html 与 raw 格式的测试报告丢失测试结果**：

| 缺陷 | 现象 | 修复 |
| ---- | ---- | ---- |
| `HtmlReporter` 未实现测试报告 | `analyzer gradle "test" --format html` 输出 "gradle test: no issues found"（默认实现仅渲染编译 issues，测试结果缺失；编译 issues 为空时还触发成功短回路） | 在 `src/core/reporter/html.rs` 覆写 `generate_test_report_with_options`：输出 Summary / Failed Tests / Ignored Tests / Passed Tests / Compile Issues 完整 HTML，并增加 HTML 转义辅助函数 |
| `RawReporter` 未实现测试报告 | `analyzer gradle "test" --format raw/raw-json` 输出为空 | 在 `src/core/reporter/raw.rs` 覆写 `generate_test_report_with_options`：pipe-delimited 模式输出 `TEST_SUMMARY` + `TEST\|STATUS\|...` 行；JSON lines 模式每行输出一个测试结果对象 |

（MarkdownReporter 与 JsonReporter 已覆写测试报告，无此问题。）

## 6. 回归验证

- `cargo test --release`：全部通过（单元测试 + 各插件集成测试 + doc-tests）。
- `cargo test --release --test maven_integration_tests --test gradle_integration_tests`：10 个用例全部通过（maven 5 + gradle 5）。
