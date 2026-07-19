# Clang Compile Analysis Report

**Command**: `clang++ -std=c++17 -Wall -c src/main.cpp`

## Summary

- **Total Issues**: 5
- **Errors**: 2
- **Warnings**: 0
- **Info**: 3
- **Files with Issues**: 4

## Issue Details (Grouped by File)

### /usr/include/x86_64-linux-gnu/bits/mathcalls-narrow.h

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 24 | 20 | Info | 'fadd' declared here |

### /workspace/multi-analyzer/tests/data/fixtures/cpp-cmake-project/src/main.cpp

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 5 | 18 | Error | use of undeclared identifier 'undefined_var' |
| 10 | 12 | Error | use of undeclared identifier 'add'; did you mean 'fadd'? |

### /usr/include/math.h

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 565 | 32 | Info | expanded from macro '__MATHCALL_NAME' |

### <scratch space>

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 171 | 1 | Info | expanded from here |

## Raw Output

View raw command output: [raw_output/clang_compile.txt](raw_output/clang_compile.txt)

