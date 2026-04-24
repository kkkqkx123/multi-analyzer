# Mypy Basic Analysis Report

**Command**: `mypy src/`

## Summary

- **Total Issues**: 24
- **Errors**: 18
- **Warnings**: 0
- **Info**: 6
- **Files with Issues**: 3

## Issue Details (Grouped by File)

### src\utils.py

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 9 | 5 | Error | Returning Any from function declared to return "dict[Any, Any]"  [no-any-return] |
| 24 | 1 | Error | Function is missing a type annotation  [no-untyped-def] |
| 53 | 5 | Error | Function is missing a type annotation for one or more arguments  [no-untyped-def] |
| 54 | 9 | Error | Returning Any from function declared to return "str"  [no-any-return] |
| 70 | 5 | Error | Function is missing a type annotation for one or more arguments  [no-untyped-def] |
| 75 | 1 | Error | Function is missing a return type annotation  [no-untyped-def] |
| 79 | 20 | Error | Argument 1 to "append" of "list" has incompatible type "str"; expected "int"  [arg-type] |
| 83 | 14 | Error | Unsupported operand types for + ("None" and "int")  [operator] |
| 83 | 14 | Info | Left operand is of type "Optional[int]" |

### setup.py

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 1 | 1 | Error | Library stubs not installed for "setuptools"  [import-untyped] |
| 1 | 1 | Info | Hint: "python3 -m pip install types-setuptools" |
| 1 | 1 | Info | (or run "mypy --install-types" to install all missing stub packages) |
| 1 | 1 | Info | See https://mypy.readthedocs.io/en/stable/running_mypy.html#missing-imports |

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

## Raw Output

View raw command output: [samples/mypy_basic.txt](samples/mypy_basic.txt)

