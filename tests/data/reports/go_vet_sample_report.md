# Go go vet (Sample) Analysis Report

**Command**: `go vet`

## Summary

- **Total Issues**: 4
- **Errors**: 0
- **Warnings**: 4
- **Info**: 0
- **Files with Issues**: 3

## Issue Details (Grouped by File)

### ./cmd/myapp/main.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 18 | 14 | Warning | Printf format %d has arg "hello" of wrong type string |
| 21 | 10 | Warning | return value of os.Setenv is not checked |

### pkg/utils/math.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 20 | 16 | Warning | return value of os.Open is not checked |

### internal/config/config.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 15 | 15 | Warning | return value of os.Setenv is not checked |

## Raw Output

View raw command output: [samples/go_vet_sample.txt](samples/go_vet_sample.txt)

