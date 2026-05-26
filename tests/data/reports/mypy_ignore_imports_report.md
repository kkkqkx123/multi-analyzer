# Mypy Ignore Imports Analysis Report

**Command**: `mypy --show-column-numbers --ignore-missing-imports .`

## Summary

- **Total Issues**: 44
- **Errors**: 29
- **Warnings**: 0
- **Info**: 15
- **Files with Issues**: 4

## Issue Details (Grouped by File)

### src\main.py

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 14 | 18 | Error | Unsupported operand types for + ("int" and "str")  [operator] |
| 26 | 1 | Error | Function is missing a type annotation  [no-untyped-def] |
| 40 | 16 | Error | Argument 1 to "len" has incompatible type "Optional[str]"; expected "Sized"  [arg-type] |
| 54 | 5 | Error | Function is missing a type annotation  [no-untyped-def] |
| 62 | 5 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 62 | 5 | Info | Use "-> None" if function does not return a value |
| 90 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 90 | 1 | Info | Use "-> None" if function does not return a value |
| 93 | 17 | Error | Argument 1 to "add_numbers" has incompatible type "str"; expected "int"  [arg-type] |
| 96 | 14 | Error | Incompatible types in assignment (expression has type "int", variable has type "str")  [assignment] |
| 99 | 11 | Error | Name "undefined_variable" is not defined  [name-defined] |

### tests\test_utils.py

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 7 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 7 | 1 | Info | Use "-> None" if function does not return a value |
| 14 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 14 | 1 | Info | Use "-> None" if function does not return a value |
| 24 | 5 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 24 | 5 | Info | Use "-> None" if function does not return a value |
| 28 | 5 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 28 | 5 | Info | Use "-> None" if function does not return a value |

### src\utils.py

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 9 | 5 | Error | Returning Any from function declared to return "dict[Any, Any]"  [no-any-return] |
| 41 | 1 | Error | Function is missing a type annotation  [no-untyped-def] |
| 70 | 5 | Error | Function is missing a type annotation for one or more arguments  [no-untyped-def] |
| 71 | 9 | Error | Returning Any from function declared to return "str"  [no-any-return] |
| 87 | 5 | Error | Function is missing a type annotation for one or more arguments  [no-untyped-def] |
| 92 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 96 | 20 | Error | Argument 1 to "append" of "list" has incompatible type "str"; expected "int"  [arg-type] |
| 100 | 14 | Error | Unsupported operand types for + ("None" and "int")  [operator] |
| 100 | 14 | Info | Left operand is of type "Optional[int]" |

### tests\test_example.py

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 7 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 7 | 1 | Info | Use "-> None" if function does not return a value |
| 13 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 13 | 1 | Info | Use "-> None" if function does not return a value |
| 19 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 19 | 1 | Info | Use "-> None" if function does not return a value |
| 25 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 25 | 1 | Info | Use "-> None" if function does not return a value |
| 31 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 31 | 1 | Info | Use "-> None" if function does not return a value |
| 37 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 37 | 1 | Info | Use "-> None" if function does not return a value |
| 44 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 44 | 1 | Info | Use "-> None" if function does not return a value |
| 50 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 50 | 1 | Info | Use "-> None" if function does not return a value |

## Raw Output

View raw command output: [raw_output/mypy_ignore_imports.txt](raw_output/mypy_ignore_imports.txt)

