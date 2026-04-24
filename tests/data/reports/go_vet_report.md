# Go Vet Analysis Report

**Command**: `go vet ./...`

## Summary

- **Total Issues**: 3
- **Errors**: 0
- **Warnings**: 3
- **Info**: 0
- **Files with Issues**: 2

## Issue Details (Grouped by File)

### internal\config\config_test.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 7 | 2 | Warning | missing go.sum entry for module providing package github.com/stretchr/testify/assert |
| 7 | 2 | Warning | missing go.sum entry for module providing package github.com/stretchr/testify/assert |

### cmd\myapp\main.go

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 22 | 28 | Warning | fmt.Printf format %d has arg "hello" of wrong type string |

## Raw Output

View raw command output: [raw_output/go_vet.txt](raw_output/go_vet.txt)

