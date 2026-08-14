# pnpm-turbo test project

pnpm + Turborepo monorepo test project used to verify the multi-analyzer PNPM/turbo
support (see `multi-analyzer/skills/analyzer-usage/`).

## Structure

```
pnpm-turbo/
├── turbo.json                 # turbo task pipeline (build/lint/typecheck)
├── pnpm-workspace.yaml        # pnpm workspace packages
├── eslint.config.mjs          # shared ESLint 9 flat config
├── tsconfig.base.json         # shared TS compiler options
└── packages/
    ├── utils/                 # shared library (deliberate lint/type errors)
    └── web/                   # app depending on utils (deliberate lint/type errors)
```

Both packages intentionally contain ESLint and TypeScript errors so the analyzer
has real issues to detect.

## Commands

```bash
pnpm install
pnpm lint          # turbo run lint
pnpm typecheck     # turbo run typecheck
pnpm build         # turbo run build
```

## Analyzer verification

```bash
analyzer pnpm "lint"
analyzer pnpm "typecheck"
analyzer run "pnpm exec turbo run lint"
```

## Verification results (2026-08-11)

Tested with `multi-analyzer/target/release/analyzer` v0.2.0 against this project.

| Operation | Expected (per analyzer-usage skill) | Actual | Verdict |
| --------- | ----------------------------------- | ------ | ------- |
| `analyzer pnpm "lint"` | Parse ESLint errors from pnpm+PNPM workspace | 8 real errors parsed, grouped by package (`@pnpm-turbo/web`/`@pnpm-turbo/utils`) | Match |
| `analyzer pnpm "typecheck"` | Parse TS errors, extract error codes | 4 TS errors parsed, TS2322/TS2741 extracted (incl. cross-package `../utils/src/index.ts`) | Match |
| `analyzer run "pnpm exec turbo run lint"` | turbo filter matches `pnpm exec turbo` | Detected as `analyzer pnpm "lint"`, analyzed correctly | Match |
| `analyzer rewrite "pnpm exec turbo run lint"` | Rewrites to analyzer form | `analyzer pnpm "lint"` | Match |
| Turbo output parsing | Strip `@pnpm-turbo/web:lint:` prefixes / TUI frames | Prefixes stripped, file paths and line:col extracted correctly | Match |

**Fixed (2026-08-11)**: turbo's failure summary line
`@pnpm-turbo/web#lint:  ERROR  command (...) /usr/local/bin/pnpm run lint exited (1)`
used to be parsed as one extra bogus error issue per failed package. It is now
filtered out in `NpmParser::parse_generic_error` (parser.rs), because it is
task-level meta-information (package + exit code) with no file/line/rule.
Real failures remain visible via the parsed issues or the `command_failed`
fallback in `run_analyzer`. Covered by unit tests
`test_parse_generic_error_skips_turbo_failure_summary` and
`test_parse_turbo_output_with_failure_summary`.

**Fixed (2026-08-11)**: when turbo runs package tasks in parallel, streaming
output lines could interleave and the ESLint file-path context
(`find_eslint_file_path`, which scans backwards for the nearest path line)
occasionally attributed issues to the wrong package/file. The scan is now
scoped to the current package context (`NpmParser::parse_eslint_format` +
`find_eslint_file_path`), so issues from one package never pick up another
package's file path. Verified stable across repeated runs (8/8 lint issues,
4/4 typecheck issues grouped correctly every time). Covered by unit tests
`test_eslint_file_path_scoped_to_current_package` and
`test_parse_interleaved_turbo_output`.

**Fixed (2026-08-12)**: the package-scoped scan still leaked in one
interleaving case — an *unprefixed* path line (prefix lost by
`merge_and_clean_lines` when it splits a trailing `...rule-nameD:\path`
off a line, or dropped while turbo streams) is ambiguous and was accepted
by any package. `find_eslint_file_path` now tracks whether the backwards
scan has crossed into another package's output run and rejects unprefixed
path lines behind that boundary, so an issue can no longer adopt a foreign
package's bare path line. Covered by regression test
`test_eslint_file_path_skips_unprefixed_path_from_foreign_run`.
