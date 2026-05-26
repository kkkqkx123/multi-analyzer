# Go golangci-lint (Sample) Analysis Report

**Command**: `golangci-lint`

## Summary

- **Total Issues**: 7
- **Errors**: 0
- **Warnings**: 7
- **Info**: 0
- **Files with Issues**: 3

## Issue Details (Grouped by File)

### internal/config/config.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 15 | 15 | Warning | Error return value of `os.Setenv` is not checked |
| 26 | 10 | Warning | error strings should not be capitalized |

### cmd/myapp/main.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 15 | 2 | Warning | `unusedVar` is unused |
| 18 | 14 | Warning | Printf format %d has arg "hello" of wrong type string |
| 21 | 10 | Warning | Error return value of `os.Setenv` is not checked |
| 23 | 2 | Warning | `cfg` is unused |

### pkg/utils/math.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 20 | 16 | Warning | Error return value of `os.Open` is not checked |

## Raw Output

View raw command output: [samples/golangci_lint_sample.txt](samples/golangci_lint_sample.txt)

