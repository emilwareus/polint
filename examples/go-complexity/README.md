# Go Complexity Example

Minimal Go fixture for `local/go-cyclomatic-complexity`.

This directory is self-contained: the local rule implementation lives at
`.polint/rules/go-complexity/src/main.rs`.

Run it from this directory:

```bash
cargo run --manifest-path .polint/rules/go-complexity/Cargo.toml -- check --profile fast --format json --fail-on none
```

`router.go` intentionally has two branches while the example config sets
`max = 1`, so the example runner emits one complexity diagnostic.
