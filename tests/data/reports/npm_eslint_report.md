# ESLint Analysis Report

**Command**: `npm run lint`

## Summary

- **Total Issues**: 12
- **Errors**: 4
- **Warnings**: 8
- **Info**: 0
- **Files with Issues**: 2

## Issue Details (Grouped by File)

### D:\项目\cli\analyze-cargo\tests\data\fixtures\npm-project\src\utils.ts

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 4 | 10 | Error | 'readFileSync' is defined but never used |
| 15 | 29 | Warning | Parameter 'data' implicitly has an 'any' type |
| 20 | 24 | Warning | Parameter 'users' implicitly has an 'any' type |
| 20 | 37 | Warning | Return type annotation is missing |
| 23 | 7 | Warning | 'API_URL' is assigned a value but never used |

### D:\项目\cli\analyze-cargo\tests\data\fixtures\npm-project\src\index.ts

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 3 | 7 | Warning | 'unusedVariable' is assigned a value but never used |
| 10 | 5 | Error | Unexpected console statement |
| 13 | 7 | Error | Argument of type 'number' is not assignable to parameter of type 'string' |
| 17 | 10 | Warning | 'unusedFunction' is defined but never used |
| 20 | 23 | Warning | Parameter 'data' implicitly has an 'any' type |
| 26 | 1 | Warning | Missing return type on function |
| 30 | 1 | Error | Unexpected var, use let or const instead |

## Raw Output

View raw command output: [samples/npm_eslint_sample.txt](samples/npm_eslint_sample.txt)

