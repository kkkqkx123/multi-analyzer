# Go go build (Sample) Analysis Report

**Command**: `go build`

## Summary

- **Total Issues**: 6
- **Errors**: 6
- **Warnings**: 0
- **Info**: 0
- **Files with Issues**: 3

## Issue Details (Grouped by File)

### ./main.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 15 | 2 | Error | undefined: unusedVar |
| 18 | 14 | Error | cannot use "hello" (type string) as type int in argument to fmt.Printf |
| 21 | 10 | Error | os.Setenv call has possible formatting directive %s |
| 23 | 2 | Error | cfg declared but not used |

### pkg/utils/math.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 20 | 16 | Error | undefined: os |

### internal/config/config.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 15 | 15 | Error | os.Setenv call has possible formatting directive %v |

## Raw Output

View raw command output: [samples/go_build_sample.txt](samples/go_build_sample.txt)

