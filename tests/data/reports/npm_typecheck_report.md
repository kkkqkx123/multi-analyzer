# TypeScript Type Check Analysis Report

**Command**: `npm run type-check`

## Summary

- **Total Issues**: 7
- **Errors**: 7
- **Warnings**: 0
- **Info**: 0
- **Files with Issues**: 2

## Issue Details (Grouped by File)

### src/index.ts

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 13 | 7 | Error | Argument of type 'number' is not assignable to parameter of type 'string'. |
| 20 | 23 | Error | Parameter 'data' implicitly has an 'any' type. |
| 26 | 1 | Error | 'calculate', which lacks return-type annotation, implicitly has an 'any' return type. |

### src/utils.ts

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 15 | 29 | Error | Parameter 'data' implicitly has an 'any' type. |
| 20 | 24 | Error | Parameter 'users' implicitly has an 'any' type. |
| 20 | 37 | Error | 'processUsers', which lacks return-type annotation, implicitly has an 'any' return type. |
| 53 | 5 | Error | Rest parameter 'args' implicitly has an 'any[]' type. |

## Raw Output

View raw command output: [samples/npm_typecheck_sample.txt](samples/npm_typecheck_sample.txt)

