# Go go build (Sample) Analysis Report

**Command**: `go build`

## Summary

- **Total Issues**: 6
- **Errors**: 3
- **Warnings**: 3
- **Info**: 0
- **Files with Issues**: 3

## Issue Details (Grouped by File)

### internal/config/config.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 15 | 15 | Warning | os.Setenv call has possible formatting directive %v |

### ./main.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 15 | 2 | Error | undefined: unusedVar |
| 18 | 14 | Warning | cannot use "hello" |
| 21 | 10 | Warning | os.Setenv call has possible formatting directive %s |
| 23 | 2 | Error | cfg declared but not used |

### pkg/utils/math.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 20 | 16 | Error | undefined: os |

## Raw Output

View raw command output: [samples/go_build_sample.txt](samples/go_build_sample.txt)

