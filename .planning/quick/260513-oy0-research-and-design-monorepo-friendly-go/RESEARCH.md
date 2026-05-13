# Monorepo-friendly Go symbol lifecycle

## Decision

Go symbol/reference setup must not require a repository-root `go.mod`. polint
should keep lifecycle setup in the single `.polint.toml` file and support two
paths:

1. infer Go module roots from discovered Go files by walking to the nearest
   `go.mod`;
2. honor explicit `[languages.go].module_roots` for monorepos that want stable
   lifecycle control.

When package loading needs workspace mode for module roots below the repository
root, it should use a checked-in root `go.work` if present. If not present,
polint should create a temporary internal workspace for the sidecar run rather
than writing generated files into the repository.

## Evidence

- `golang.org/x/tools/go/packages` loads packages from patterns through the
  underlying build tool, normally `go`, and supports `file=` queries plus custom
  build drivers. This makes the package-loading working directory and patterns
  part of the analyzer lifecycle.
- The Go modules reference defines workspaces as a set of modules on disk used
  as main modules. `GOWORK=off` forces single-module mode, while a `go.work`
  path enables workspace mode.
- The Go modules reference notes that checked-in `go.work` files can be a poor
  fit for some CI and developer workflows. A temporary internal workspace is
  therefore better than requiring users to commit one just for polint.
- `go work use -r` exists because recursive module discovery is a normal
  monorepo workflow. polint should not make users duplicate generated workspace
  files when `.polint.toml` can express the same lifecycle.

## Architecture Rule

Root selection is a shared lifecycle concern, not a symbol-specific hack. Go
module graph, future call graph, CFG, dataflow, coverage, symbols, and references
should all use the same module-root model:

```toml
[languages.go]
module_roots = ["services/payments", "libs/money"]
package_patterns = ["./..."]
build_tags = ["enterprise"]
include_tests = true
```

`package_patterns` are interpreted inside each module root. The analyzer reports
all paths relative to the polint repository root.

## Sources

- https://pkg.go.dev/golang.org/x/tools/go/packages
- https://go.dev/ref/mod#workspaces
- https://go.dev/doc/tutorial/workspaces
- https://pkg.go.dev/cmd/go#hdr-Package_lists_and_patterns
