# ❌ Test Report - Issues Found

## Summary

- **Total**: 7 test(s) (calculated: 1185)
- **Passed**: ✅ 6 (85.7%)
- **Failed**: ❌ 1

## Failed Tests (1 item(s))

### 1. `test_turbo_tui_output_capture`

## Passed Tests (1184 item(s))

✅ 1184 tests passed

---
*Test output was successfully captured*
## Build Issues

The following issues were found during compilation:

# Type Check Report

## Type Issues Summary

- **Total**: 1594
- **❌** error: 24
- **⚠️** warning: 1570
- **Categories**: 14
- **Files Affected**: 1032

### Top Error Codes

- `FORMAT`: 1570 occurrence(s)


## Breakdown by Category

- **FORMAT**: 1570 occurrence(s)
- **Function is missing a type**: 5 occurrence(s)
- **Function is missing a return**: 3 occurrence(s)
- **Pattern matching is only supported**: 3 occurrence(s)
- **Returning Any from function declared**: 2 occurrence(s)
- **Unsupported operand types for +**: 2 occurrence(s)
- **Library stubs not installed for**: 2 occurrence(s)
- **Argument 1 to "append" of**: 1 occurrence(s)
- **use of undeclared identifier 'undefined_var'**: 1 occurrence(s)
- **Argument 1 to "len" has**: 1 occurrence(s)
- **Argument 1 to "add_numbers" has**: 1 occurrence(s)
- **Incompatible types in assignment (expression**: 1 occurrence(s)
- **Name "undefined_variable" is not defined**: 1 occurrence(s)
- **use of undeclared identifier 'add';**: 1 occurrence(s)

## Details by File

### `src/main.py` (9 item(s))

- ❌ **error** at line 14:18: Unsupported operand types for + ("int" and "str")  [operator]
- ❌ **error** at line 26:1: Function is missing a type annotation  [no-untyped-def]
- ❌ **error** at line 40:16: Argument 1 to "len" has incompatible type "str | None"; expected "Sized"  [arg-type]
- ❌ **error** at line 54:5: Function is missing a type annotation  [no-untyped-def]
- ❌ **error** at line 62:5: Function is missing a return type annotation  [no-untyped-def]
- ❌ **error** at line 90:1: Function is missing a return type annotation  [no-untyped-def]
- ❌ **error** at line 93:17: Argument 1 to "add_numbers" has incompatible type "str"; expected "int"  [arg-type]
- ❌ **error** at line 96:14: Incompatible types in assignment (expression has type "int", variable has type "str")  [assignment]
- ❌ **error** at line 99:11: Name "undefined_variable" is not defined  [name-defined]

### `src/utils.py` (8 item(s))

- ❌ **error** at line 9:5: Returning Any from function declared to return "dict[Any, Any]"  [no-any-return]
- ❌ **error** at line 41:1: Function is missing a type annotation  [no-untyped-def]
- ❌ **error** at line 70:5: Function is missing a type annotation for one or more parameters  [no-untyped-def]
- ❌ **error** at line 71:9: Returning Any from function declared to return "str"  [no-any-return]
- ❌ **error** at line 87:5: Function is missing a type annotation for one or more parameters  [no-untyped-def]
- ❌ **error** at line 92:1: Function is missing a return type annotation  [no-untyped-def]
- ❌ **error** at line 96:20: Argument 1 to "append" of "list" has incompatible type "str"; expected "int"  [arg-type]
- ❌ **error** at line 100:14: Unsupported operand types for + ("None" and "int")  [operator]

### `Note: Recompile with -Xlint:unchecked for details.` (4 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `Note: /workspace/multi-analyzer/tests/data/fixtures/gradle-project/src/main/java/com/example/App.java uses unchecked or unsafe operations.` (4 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `internal/config/config_test.go:7:2: missing go.sum entry for module providing package github.com/stretchr/testify/assert (imported by example.com/myproject/internal/config); to add:` (4 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `go get -t example.com/myproject/internal/config` (4 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `Note: /workspace/multi-analyzer/tests/data/fixtures/gradle-project/src/main/java/com/example/Utils.java uses or overrides a deprecated API.` (4 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `/workspace/multi-analyzer/tests/data/fixtures/gradle-project/src/main/java/com/example/Broken.java:7: error: cannot find symbol` (4 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `/workspace/multi-analyzer/tests/data/fixtures/gradle-project/src/main/java/com/example/Broken.java:10: error: cannot find symbol` (4 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `Note: Recompile with -Xlint:deprecation for details.` (4 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `Validating mypy output format...` (4 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `/root/.pyenv/versions/3.14.4/lib/python3.14/site-packages/_pytest/terminal.py` (3 item(s))

- ❌ **error** at line 1729:9: Pattern matching is only supported in Python 3.10 and greater  [syntax]
- ❌ **error** at line 1729:9: Pattern matching is only supported in Python 3.10 and greater  [syntax]
- ❌ **error** at line 1729:9: Pattern matching is only supported in Python 3.10 and greater  [syntax]

### `Validating ESLint output format...` (3 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `[Error] Line Some(5): class Broken is public, should be declared in a file named Broken.java` (3 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `✓ Valid format: /root/.pyenv/versions/3.14.4/lib/python3.14/site-packages/_pytest/terminal.py:1729:9: error: Pattern matching is only supported in Python 3.10 and greater  [syntax]` (3 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `test core::parser::base_parser_tests::test_detect_level_info ... ok` (2 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `test plugins::cargo::parser::tests::test_extract_package_from_path_unknown_prefix ... ok` (2 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `test core::types::types_tests::test_issue_builder_basic ... ok` (2 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `test core::parser::base_parser_tests::test_parse_parentheses_format_invalid_numbers ... ok` (2 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

### `test plugins::cargo::parser::tests::test_parse_test_summary_ok ... ok` (2 item(s))

- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied
- ⚠️ **warning** `[FORMAT]` at line -: Environment is not satisfied

*... and 1012 more files (use --verbose to see all)*

