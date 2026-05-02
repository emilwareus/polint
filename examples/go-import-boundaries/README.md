# Go Import Boundaries Example

Minimal Go fixture for `examples/go-import-boundaries`.

Run it from this directory:

```bash
polint check --profile fast --format json --fail-on none
```

`handler.go` imports `net/http`, and the example config forbids that import for
the local Go file. This models a project-specific architecture boundary.
