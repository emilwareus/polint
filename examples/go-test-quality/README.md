# Go Test Quality Example

Minimal Go test fixture for one local heuristic test-quality policy:

- `local/go-test-quality`

This directory is self-contained: the local rule implementation lives at
`.polint/rules/go-test-quality/src/main.rs`.

Run it from this directory:

```bash
cargo run --manifest-path .polint/rules/go-test-quality/Cargo.toml -- check --profile fast --format json --fail-on none
```

`payment_test.go` intentionally calls production-looking code without an
assertion. The same local rule also computes a tiny heuristic maintainability
score; the threshold is set to `0` so the fixture emits both diagnostics from
one rule.
