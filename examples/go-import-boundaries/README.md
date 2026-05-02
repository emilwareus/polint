# Go Import Boundaries Example

Minimal Go fixture for `local/go-import-boundaries`.

This directory is self-contained: the local rule implementation lives at
`.polint/rules/go-import-boundaries/src/main.rs`.

Run it from this directory:

```bash
cargo run --manifest-path .polint/rules/go-import-boundaries/Cargo.toml -- check --profile fast --format json --fail-on none
```

`handler.go` imports `net/http`, and the example config forbids that import for
the local Go file. This models a project-specific architecture boundary.
