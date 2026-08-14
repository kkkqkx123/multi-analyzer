# C++ 技术栈补充验证报告（CMake / ClangFormat）

- 验证日期：2026-08-10
- 验证对象：multi-analyzer（analyzer v0.2.0）
- 验证范围：C++ 技术栈中此前未覆盖的 CMake、ClangFormat 插件，及 discover 子命令（run/rewrite）
- 测试项目：`test-projects/cmake-cpp/`、`test-projects/clang-format-cpp/`

## 1. 新增测试项目

skills 目录（`skills/analyzer-usage/`）期望支持的 C++ 技术栈为 CMake、GCC、Clang、MSVC、ClangFormat。
此前 `test-projects/` 仅覆盖 gcc-c/gcc-cpp/clang-c/clang-cpp/msvc-c/msvc-cpp，本次补齐缺失的 CMake 与 ClangFormat：

| 项目目录 | 技术栈 | 设计意图 |
| -------- | ------ | -------- |
| `cmake-cpp/` | CMake | `CMakeLists.txt` + `include/math.h` + `src/main.cpp` / `utils.cpp` / `math.cpp`；main.cpp 含未声明变量错误与未使用变量警告，utils.cpp 含类型不匹配错误与未使用参数警告，验证 `analyzer cmake "--build build"` 透传编译器诊断 |
| `clang-format-cpp/` | ClangFormat | `.clang-format`（Google 基样式、IndentWidth 4）+ `src/main.cpp` / `utils.cpp`（函数大括号位置、2 空格缩进、运算符缺空格等格式违规），验证 `analyzer clang-format "--dry-run --Werror <file>"` |

## 2. 验证环境

| 组件 | 版本 |
| ---- | ---- |
| analyzer | v0.2.0（`cargo build --release`） |
| CMake | 3.25.1（generator: Unix Makefiles） |
| clang-format | 14.0.6（Debian） |
| g++ / clang++ | 12.2.0 / 14.0.6 |

## 3. 功能验证结果

### 3.1 直接分析模式（通过）

| 测试项 | 命令 | 结果 |
| ------ | ---- | ---- |
| CMake 构建分析 | `analyzer cmake "--build build"`（先 `cmake -S . -B build`） | 2 issues：main.cpp 1 error（undefined_var，6:18）+ 1 warning（unused-variable，9:9） |
| CMake 配置阶段错误 | `analyzer cmake "-S . -B build"`（源文件缺失） | 2 个 `CMake Error` 块，含多行续行消息（Cannot find source file / No SOURCES given） |
| CMake run 模式 | `analyzer run "cmake --build build"` | 与直接模式输出一致（2 issues） |
| clang-format 单文件 | `analyzer clang-format "--dry-run --Werror src/main.cpp"` | 32 处 FORMAT 违规，行/列号精确 |
| clang-format 多文件 | `analyzer clang-format "--dry-run --Werror src/main.cpp src/utils.cpp"` | 45 处违规（32 + 13），2 文件 |
| clang-format run 模式 | `analyzer run "clang-format --dry-run --Werror src/main.cpp"` | 修复后与直接模式一致（32 issues） |
| 回归（三编译器） | `analyzer gcc/clang/msvc ...`（既有项目） | gcc-cpp 1 error+2 warning、clang-cpp 7 issues、msvc-cpp 2 个 C9999，无回归 |

### 3.2 报告格式与过滤（通过）

| 选项 | 结果 |
| ---- | ---- |
| markdown（默认） | 分组统计、严重级别、文件排名正常 |
| json | metadata/summary_by_level/items 结构完整 |
| html | `-o` 输出 5.8KB 样式化报告 |
| raw | `LEVEL\|CODE\|FILE:LINE:COL\|MESSAGE` 正常 |
| raw-json | JSON Lines 逐行输出正常 |
| `--filter-warnings` / `--filter-paths` / `--max-issues` / `-o` | 全部正常 |

### 3.3 discover 子命令（修复后通过）

| 测试项 | 命令 | 结果 |
| ------ | ---- | ---- |
| rewrite clang-format | `analyzer rewrite "clang-format --dry-run --Werror src/utils.cpp"` | `analyzer clang-format "--dry-run --Werror src/utils.cpp"`（透传） |
| rewrite cmake | `analyzer rewrite "cmake --build build"` | `analyzer cmake "--build build"` |
| rewrite cmake -S | `analyzer rewrite "cmake -S . -B build"` | `analyzer cmake "-S . -B build"` |
| cmake --configure | `analyzer rewrite "cmake --configure -B build -S ."` | 修复后正确拒绝（exit 1）；`cmake --configure` 本身非法（CMake Error: Unknown argument） |

## 4. skills 目录期望核对

| Skill 期望 | 核对结果 |
| ---------- | -------- |
| `analyzer cmake "--build build"`（SKILL.md 示例） | 通过：识别 CMake 构建透传的编译器错误/警告 |
| `analyzer clang-format "--dry-run --Werror main.cpp"`（SKILL.md 示例） | 通过：识别 FORMAT 违规（行/列号、错误码） |
| 报告格式 markdown/json/html/raw/raw-json | 通过 |
| 过滤与统计选项（--filter-warnings/--filter-paths/--max-issues/-o） | 通过 |
| run/rewrite 自动检测（skill 文档列出的约束：仅构建工具命令、复合命令取首段、环境变量前缀剥离） | 通过 |

## 5. 发现的问题与修复

### 5.1 TUI 边框剥离破坏缩进，CMake 块续行丢失 —— 已修复（严重）

`analyzer cmake "-S . -B build"` 的 CMake Error 消息只显示命令名（`add_executable`），续行（Cannot find source file 等）全部丢失。

**根因**：`core/utils.rs` 的 `process_line_tui`（TUI 边框剥离，默认开启）对**每一行**无条件剥离行首空白，而 `CMakeParser::is_block_end` 依赖 `line.starts_with("  ")` 识别续行——缩进被剥离后第一个续行即被当作块结束，块内只剩首行。

**修复**：`strip_tui_prefix()` 仅在行首确实为 TUI 边框字符时才剥离前缀与后续空白；普通输出行保留原始缩进。`filter_tui_frame_lines` 同步修复。

```bash
analyzer cmake "-S . -B build" --format raw
# 修复前: error|CMake Error|CMakeLists.txt:3:|add_executable
# 修复后: error|CMake Error|CMakeLists.txt:3:|Cannot find source file: src/missing.cpp Tried extensions ...
```

### 5.2 clang-format run 模式注入 "format" 字面量 —— 已修复（严重）

```bash
analyzer run "clang-format --dry-run src/main.cpp"
# 修复前: 实际执行 clang-format format --dry-run src/main.cpp
# → clang-format: error: 'format': No such file or directory（真实违规全部丢失，仅 1 个伪 error）
```

**根因**：discover 规则 `subcommand_template = "format"` 是字面量（与先前 gcc/clang 的 `compile` 同类问题），被当作文件名传给 clang-format。

**修复**：clang-format 规则改为 `^clang-format\b\s*(.*)$` + 模板 `{1}` 透传原始参数（与 rubocop/mypy 规则一致）。**不能**像 gcc 那样注入固定标志：clang-format 14 对重复 `--dry-run`/`--Werror` 报错（may only occur zero or one times!），规范化会破坏 `clang-format --dry-run --Werror ...` 这一最常见检查命令。

```bash
analyzer run "clang-format --dry-run --Werror src/main.cpp"
# 修复后: clang-format --dry-run --Werror src/main.cpp → 32 issues
```

### 5.3 CMake 相邻错误块第二个块丢失 —— 已修复（中等）

首个 "CMake Error at ..." 块的终止行（第二个块的起始行）被 `collect_all_blocks` 消费后不再检查是否为新的块起点，导致相邻块的续行全部丢失（修复 5.1 前因缩进剥离掩盖为 "add_executable" 伪消息）。

**修复**：`core/parser.rs` 的 `collect_all_blocks` 在块结束时对同一行再执行一次 `is_block_start` 检查（与 `BlockIter` 迭代器行为对齐）。

```bash
# 修复后: 两个块均带完整消息
error|CMake Error|CMakeLists.txt:3:|Cannot find source file: src/missing.cpp ...
error|CMake Error|CMakeLists.txt:3:|No SOURCES given to target: app
```

### 5.4 cmake 规则匹配非法 `--configure` 模式 —— 已修复（中等）

discover 规则 `^cmake\s+(--build|--configure)` 把 `cmake --configure` 当作可分析命令，但该模式在 CMake 中不存在（实测 `CMake Error: Unknown argument --configure`），run 模式必然失败。

**修复**：规则改为匹配真实 CMake 调用 `(--build\b|-S\b)`，`cmake -S . -B build` 配置阶段错误现在可被识别分析；`cmake --configure ...` 正确返回无匹配（exit 1）。

### 5.5 测试补充

- `core/utils.rs` / `core/stream.rs`：TUI 剥离保留缩进回归测试
- `tests/cmake_parser_tests.rs`：相邻 CMake Error 块解析测试
- `tests/discover_integration_tests.rs`：clang-format 透传测试、`cmake --configure` 拒绝测试
- `src/discover/registry.rs`：clang-format 分类断言更新
- 全量回归：`cargo test --release` 20 个套件全部通过（约 730+ 用例，0 失败）

## 6. 已知限制

| 限制 | 说明 |
| ---- | ---- |
| clang-format 目录参数 | `clang-format --dry-run --Werror .` 报 "Is a directory"（clang-format 不递归目录，上游限制；analyzer 正确透出该错误）。需按文件/通配符指定 |
| MSVC 模拟 | Linux 环境仍使用 `test-projects/tools/cl` shim（clang 模拟 + MSVC 风格诊断 C9999），非真实 MSVC 工具链 |
| make 串行构建 | CMake 构建在首个编译错误处停止，utils.cpp 的错误需首文件无错或换 Ninja 生成器才可见（非 analyzer 问题） |
| 选项与子命令并存时子命令优先 | `analyzer cmake "--build build" --build-dir out` 中子命令透传、`--build-dir` 不参与组装。需选项驱动时省略子命令（见下文第 8 节） |

## 7. 结论

CMake 与 ClangFormat 两个 C++ 技术栈的直接分析、报告格式、过滤统计与 discover 子命令均符合 skills 目录期望。
验证发现 4 处问题（2 严重 / 2 中等）已全部修复并回归通过：

- 严重：TUI 剥离破坏缩进导致 CMake 块消息丢失（5.1）
- 严重：clang-format run 模式 "format" 字面量导致分析失败（5.2）
- 中等：CMake 相邻错误块第二个块丢失（5.3）
- 中等：cmake 规则匹配非法 `--configure` 模式（5.4）

后续补充验证发现 `--source-dir`/`--build-dir` 等 C++ 选项"被解析但未使用"的问题，已按设计文档实现（见第 8 节）。

## 8. C++ 选项驱动命令构建（补充修复，2026-08-11）

### 问题

`--source-dir`/`--build-dir`/`--cmake-generator`/`--target` 在 `main.rs` 被解析进 `AnalyzeOptions`，但没有任何插件使用（此前报告第 6 节将其记为已知限制）；同时 `analyzer cmake --build-dir out` 这类无子命令的选项驱动调用会因 "No command specified" 直接退出，与 `cpp-support-design.md` 4.1 节的示例（`analyzer cmake --source-dir ./src --build-dir ./build`）矛盾。

### 修复

1. **`CMakeAnalyzer::create_command_builder`**：有子命令时原样透传（SKILL.md 示例不变）；无子命令时按设计文档组装命令：
   - 配置模式（提供 `--source-dir` 或 `--cmake-generator`）：`cmake -S <src|"."> -B <build|"build"> [-G <gen>]`
   - 构建模式（默认，或仅提供 `--build-dir`/`--target`）：`cmake --build <build|"build"> [--target <target>]`
2. **`TechStack::allows_default_command()`**：C++ 技术栈（cmake/gcc/clang/msvc/clang-format）允许空子命令，`main.rs` 在命令为空且技术栈允许默认命令时放行，`subcommand` 保持 `None`；其他技术栈仍报 "No command specified"。
3. **GCC/Clang `--source-dir`**：作为编译器工作目录（`CommandBuilder::current_dir`），子命令/`--target-files` 中的路径相对该目录解析，使 `analyzer-reference.md` 中 "--source-dir for CMake/GCC/Clang" 的声称成立。
4. **文档**：`analyzer-reference.md` C++ Build Options 部分重写（含选项驱动示例与"子命令并存时子命令优先"说明）；`main.rs` `--help` 文案同步。

### 验证

| 命令 | 实际执行 | 结果 |
| ---- | -------- | ---- |
| `analyzer cmake` | `cmake --build build` | 2 issues |
| `analyzer cmake --build-dir out` | `cmake --build out` | 命令组装正确 |
| `analyzer cmake --build-dir build --target cmakedemo` | `cmake --build build --target cmakedemo` | 2 issues |
| `analyzer cmake --source-dir . --build-dir /tmp/x --cmake-generator "Unix Makefiles"` | `cmake -S . -B /tmp/x -G Unix Makefiles` | 配置成功、0 issues |
| `analyzer cmake-build --build-dir build`（别名） | `cmake --build build` | 2 issues |
| `analyzer gcc --target-files src/main.cpp` | `g++ -Wall -Wextra -Wpedantic -fsyntax-only src/main.cpp` | 3 issues |
| `analyzer gcc --source-dir gcc-cpp --target-files src/main.cpp`（外部目录运行） | 以 gcc-cpp 为工作目录编译 | 3 issues（工作目录生效） |
| `analyzer msvc --target-files src/main.cpp` | `cl /W4 /EHsc /nologo /Zs src/main.cpp`（shim） | 2 C9999 |
| `analyzer cargo`（非 C++） | — | 仍报 "No command specified" |
| 回归：`analyzer cmake "--build build"` / `run` / `rewrite` | — | 行为不变 |

新增 7 个 CMakeAnalyzer 命令组装单元测试（`src/plugins/cpp/cmake/analyzer.rs`），全量 `cargo test --release` 通过。

## 9. 遗留项清理（2026-08-11）

排查各插件对 `AnalyzeOptions` 字段的使用时发现 clang 插件的 `json_output` 为死分支：

- **问题**：`AnalyzeOptions.json_output` 字段在 `src/plugins/cpp/clang/analyzer.rs` 中触发 `-fdiagnostics-format=json`，但该字段没有任何 CLI（`--json-output`）或配置文件入口，永远为 `false`；且 `CppParser` 仅解析 clang 文本诊断（`file:line:col: level: message`），即使启用 JSON 诊断也无法解析。属于不可达死代码（与 `--format json` 报告格式无关）。
- **修复**：删除 `json_output` 字段定义、clang analyzer 中的分支，以及 3 个测试文件中的 `json_output: false` 初始化（`tests/turbo_tui_integration_tests.rs`、`tests/npm_error_handling_tests.rs`、`tests/npm_command_conversion_tests.rs`）。
- **同类排查**：其余零插件使用字段（`filter_warnings`、`filter_paths`、`noise_patterns`、`keep_patterns`、`max_output_lines`、`max_line_length`、`strip_ansi`、`strip_tui_frames`、`output_file`、`stdout_only`、`report_format`、`success_short_circuit`、`max_issues`）均由核心层（`core/analyzer.rs`、`core/stream.rs`、reporter）消费，属于正常分层，非死代码。
- **验证**：清理后全量 `cargo test --release` 通过（0 失败）。
