# CMake Build (MSVC) Analysis Report

**Command**: `cmake --build . --verbose`

## Summary

- **Total Issues**: 16
- **Errors**: 16
- **Warnings**: 0
- **Info**: 0
- **Files with Issues**: 6

## Issue Details (Grouped by File)

### D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\src\main.cpp

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 5 | 18 | Error | “undefined_var”: 未声明的标识符 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |

###   D:\softwares\Visual Studio\VC\Tools\MSVC\14.43.34808\include\cstdlib

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 24 | 18 | Error | "fabs": 不是 "`global namespace'" 的成员 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |
| 24 | 18 | Error | “fabs”: 找不到标识符 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |
| 28 | 18 | Error | "fabsf": 不是 "`global namespace'" 的成员 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |
| 28 | 18 | Error | “fabsf”: 找不到标识符 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |
| 32 | 18 | Error | "fabsl": 不是 "`global namespace'" 的成员 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |
| 32 | 18 | Error | “fabsl”: 找不到标识符 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |

### D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\src\utils.cpp

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 9 | 19 | Error | “初始化”: 无法从“int”转换为“std::basic_string<char,std::char_traits<char>,std::allocator<char>>” [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |

### D:\softwares\Visual Studio\VC\Tools\MSVC\14.43.34808\include\cstdlib

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 24 | 18 | Error | "fabs": 不是 "`global namespace'" 的成员 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |
| 24 | 18 | Error | “fabs”: 找不到标识符 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |
| 28 | 18 | Error | "fabsf": 不是 "`global namespace'" 的成员 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |
| 28 | 18 | Error | “fabsf”: 找不到标识符 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |
| 32 | 18 | Error | "fabsl": 不是 "`global namespace'" 的成员 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |
| 32 | 18 | Error | “fabsl”: 找不到标识符 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |

###   D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\src\utils.cpp

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 9 | 19 | Error | “初始化”: 无法从“int”转换为“std::basic_string<char,std::char_traits<char>,std::allocator<char>>” [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |

###   D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\src\main.cpp

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 5 | 18 | Error | “undefined_var”: 未声明的标识符 [D:\项目\cli\analyzer\tests\data\fixtures\cpp-cmake-project\build_test_full\test_app.vcxproj] |

## Raw Output

View raw command output: [raw_output/cmake_build_msvc.txt](raw_output/cmake_build_msvc.txt)

