# Go Import Boundaries Example

This example models a project-specific architecture boundary.

The policy is `local/go-import-boundaries`. It reads forbidden imports from
`.polint.toml` and reports imports that cross the local boundary.

## Run It

From this directory:

```bash
cargo run --manifest-path .polint/rules/go-import-boundaries/Cargo.toml -- check --profile fast --format json --fail-on none
```

## What It Finds

`handler.go` imports `net/http`, and the example config forbids that import for
the local Go file:

```go
import "net/http"
```

The expected finding is `local/go-import-boundaries`. A real fix would move the
HTTP dependency behind an allowed package or update the boundary if the import is
intentional.
