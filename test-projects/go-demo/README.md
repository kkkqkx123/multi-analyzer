# go-demo test project

Go module used to verify the multi-analyzer Go support
(see `multi-analyzer/skills/analyzer-usage/`).

## Structure

```
go-demo/
├── go.mod                     # module go-demo (requires github.com/google/uuid)
├── go.sum                     # dependency checksums (created by `go mod tidy`)
├── cmd/app/
│   ├── main.go                # clean entrypoint using internal/utils and uuid
│   └── main_test.go           # TestMainFlow (passing)
├── internal/utils/
│   ├── utils.go               # Add / Greet / FormatDate helpers
│   └── utils_test.go          # TestAdd (passing) + TestFormatDate (deliberate failure)
├── internal/greeter/
│   └── greeter.go             # intentional fmt.Printf verb mismatch (go vet warning)
└── broken/
    └── broken.go              # deliberate compile errors (included via -tags broken)
```

The project intentionally contains a vet warning, a failing test, and (under the
`broken` build tag) compile errors so the analyzer has real issues to detect.
The `broken` package uses a `//go:build broken` constraint: it is skipped by
default and only compiled when the tag is enabled.

## Commands

```bash
go build ./...                  # succeeds
go build -tags broken ./...     # fails: Broken package compile errors
go vet ./...                    # fails: greeter.go Printf verb mismatch
go test ./...                   # fails: TestFormatDate + greeter vet error
```

## Analyzer Usage

```bash
analyzer go "build ./..."
analyzer go "build -tags broken ./..."
analyzer go "vet ./..."
analyzer go "test -v ./..."
analyzer run "go vet ./..."
analyzer rewrite "go build -tags broken ./..."
analyzer golangci-lint "run ./..."
```
