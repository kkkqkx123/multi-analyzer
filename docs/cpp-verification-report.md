# C++ 编译器分析功能验证报告

- 验证日期：2026-08-07
- 验证对象：multi-analyzer（analyzer v0.2.0）
- 验证范围：gcc、clang、msvc 三种编译器的 C++（cpp）与 C 项目

## 1. 构建结果

| 项目 | 结果 | 说明 |
| ---- | ---- | ---- |
| `cargo build --release` | 成功 | 修复编译错误后构建通过 |
| `cargo test --release` | 20 个测试套件全部通过 | 约 730 个用例，0 失败 |

### 1.1 修复的编译错误

源码存在两处编译错误，构建前必须先修复：

1. `CommandBuilder` 缺少 `with_verbose()` 方法，但 `pytest/analyzer.rs`、`core/stream.rs`、`core/test_analyzer.rs` 三处调用该方法（E0599）。
2. `CommandBuilder` 未实现 `Clone`，`core/stream.rs:59` 的 `builder.clone()` 返回 `&CommandBuilder` 导致类型不匹配（E0308）。

修复方式：在 `src/core/command.rs` 中为 `CommandBuilder` 增加 `with_verbose(bool)` 方法并添加 `#[derive(Clone)]`。

## 2. 测试项目

在工作区 `test-projects/` 下创建了 6 个验证项目，每个项目含一个错误文件和一个警告文件：

| 项目目录 | 编译器 | 语言 | 验证命令 |
| -------- | ------ | ---- | -------- |
| `gcc-cpp/` | g++ 12.2 | C++ | `analyzer gcc "-fsyntax-only src/main.cpp"` |
| `gcc-c/` | g++ 12.2（-x c） | C | `analyzer gcc "-fsyntax-only -x c src/main.c"` |
| `clang-cpp/` | clang++ 14 | C++ | `analyzer clang "-fsyntax-only src/main.cpp"` |
| `clang-c/` | clang++ 14（-x c） | C | `analyzer clang "-fsyntax-only -x c src/main.c"` |
| `msvc-cpp/` | clang 模拟 cl | C++ | `analyzer msvc "/Zs src/main.cpp"` |
| `msvc-c/` | clang 模拟 cl | C | `analyzer msvc "/Zs src/main.c"` |

注：Linux 环境无 MSVC，使用 `test-projects/tools/cl` 脚本调用 clang 并输出 MSVC 风格诊断（`file(line,col): error C9999: msg`），用于验证 msvc 插件链路。

## 3. 功能验证结果

### 3.1 直接分析模式（全部通过）

| 测试项 | 命令 | 结果 |
| ------ | ---- | ---- |
| gcc C++ 错误/警告识别 | `analyzer gcc "-fsyntax-only src/main.cpp"` | 1 error + 2 warning，行号/列号/错误码正确 |
| gcc C 项目 | `analyzer gcc "-fsyntax-only -x c src/main.c"` | 2 warning（-Wint-conversion、-Wunused-variable）正确 |
| clang C++ 项目 | `analyzer clang "-fsyntax-only src/main.cpp"` | 1 error + 1 warning + 5 info（含 note）正确 |
| clang C 项目 | `analyzer clang "-fsyntax-only -x c src/main.c"` | 2 warning 正确 |
| msvc C++ 项目 | `analyzer msvc "/Zs src/main.cpp"` | 1 error + 1 warning，MSVC 格式（C9999）解析正确 |
| msvc C 项目 | `analyzer msvc "/Zs src/main.c"` | 2 warning 正确 |
| C++ 标准选项 | `analyzer clang "-fsyntax-only src/utils.cpp" --cpp-std c++20` | 正确传递 `-std=c++20` |
| 目标文件指定 | `--target-files src/utils.cpp` | 只分析指定文件 |
| 包含路径/宏定义 | `-I src -D TEST_MACRO=1` | 正确传递 |
| clang-format | `analyzer clang-format "--dry-run --Werror src/utils.cpp"` | 识别 8 处 FORMAT 违规 |

### 3.2 报告格式（全部通过）

| 格式 | 验证结果 |
| ---- | -------- |
| markdown（默认） | 分组统计、严重级别、文件排名、错误码统计正常 |
| json | 结构化输出，metadata/summary/items 字段完整 |
| html | 样式化报告生成正常 |
| raw | `LEVEL\|CODE\|FILE:LINE:COL\|MESSAGE` 管道格式正常 |
| raw-json | JSON Lines 逐行输出（未单独验证，同 json 管线） |

### 3.3 过滤与统计（通过）

| 选项 | 结果 |
| ---- | ---- |
| `--filter-warnings` | 正确过滤 warning，保留 error/info |
| `--filter-paths` | 按文件路径过滤正常 |
| `--max-issues N` | 限制问题数量正常 |
| `-o <file>` | 报告写入文件正常 |
| `--no-short-circuit` | 禁用成功短路正常 |

### 3.4 discover 子命令（run/rewrite）

| 测试项 | 命令 | 结果 |
| ------ | ---- | ---- |
| rewrite 支持命令 | `analyzer rewrite "gcc -c main.c"` | `analyzer gcc "compile main.c"`，符合规则表 |
| rewrite clang | `analyzer rewrite "clang -fsyntax-only main.cpp"` | `analyzer clang "compile main.cpp"` |
| rewrite msvc | `analyzer rewrite "cl.exe /c main.cpp"` | `analyzer msvc "compile /c main.cpp"` |
| 环境变量前缀 | `CC=clang gcc -c main.c` | 自动剥离，正常匹配 |
| 复合命令 | `gcc -c main.c && gcc -c utils.c` | 只分析第一段并提示 |
| shell 内置命令 | `analyzer rewrite "ls -la"` | 正确拒绝（exit 1） |
| 不支持的命令 | `analyzer rewrite "g++ main.cpp"` | 拒绝（gcc 规则要求含 `-c`） |

## 4. 与预期不符的发现与修复

### 4.1 run 模式下 `compile` 字面量被当作编译参数 —— 已修复

`run`/`rewrite` 将 gcc/clang/msvc 规则映射为 `subcommand = "compile"`，该字面量会作为参数传给编译器：

```bash
analyzer run "clang++ -c src/main.cpp"
# 修复前: clang++ -Wall -Wextra -Wpedantic compile src/main.cpp
# → clang: error: no such file or directory: 'compile'
```

gcc 场景碰巧成功（g++ 将无扩展名文件当 linker input 忽略）；clang、msvc 场景直接失败。

**修复**：将三个编译规则的 `subcommand_template` 从 `"compile"` 改为真实编译标志（gcc/clang 为 `-fsyntax-only`，msvc 为 `/Zs`），使 run 模式生成的命令与直接模式一致：

```bash
analyzer run "clang++ -c src/main.cpp"
# 修复后: clang++ -Wall -Wextra -Wpedantic -fsyntax-only src/main.cpp → 7 issues
analyzer run "msvc /c src/main.cpp"
# 修复后: cl /W4 /EHsc /nologo /Zs /c src/main.cpp → 2 issues
```

### 4.2 测试子命令误判 —— 已修复

`is_test_subcommand` 通过命令字符串包含 "test" 判断，导致文件路径含 "test" 的 clang-format 命令被误判为测试分析：

```bash
analyzer clang-format "--dry-run --Werror /tmp/fmt_test.cpp"
# → Error: Test analysis not supported for clang-format
```

**修复**：`is_test_subcommand` 改为 token 级判断（按空白拆分后匹配 `test`/`nextest`/`tests` 前缀），文件路径中的 `fmt_test.cpp` 不再触发测试分析：

```bash
analyzer clang-format "--dry-run --Werror /tmp/fmt_test.cpp"
# → 正常分析，识别 8 处 FORMAT 违规
```

### 4.3 gcc 规则要求 `-c` —— 已修复

discover 的 gcc 规则 `^(gcc|g\+\+)\s+.*-c\b` 要求命令必须包含 `-c`，因此 `gcc -fsyntax-only main.cpp` 无法被 run/rewrite 识别（SKILL.md 示例使用的正是 `-fsyntax-only`）。clang 规则同时接受 `-c` 与 `-fsyntax-only`，gcc 与 clang 规则不一致。

**修复**：gcc 规则模式改为 `^(gcc|g\+\+)\s+.*(-c\b|-fsyntax-only\b)`，与 clang 对齐：

```bash
analyzer run "gcc -fsyntax-only src/main.c"
# → 修复前: no matching rule；修复后: g++ -Wall -Wextra -Wpedantic -fsyntax-only src/main.c → 2 issues
```

## 5. 结论

C++/C 分析核心功能（三编译器、双语言、五格式报告、过滤统计）符合预期；discover 子命令的 3 处边界行为偏差已全部修复，修复后 `cargo test --release` 全量测试（20 套件）通过，且 run 模式与直接模式输出一致。
