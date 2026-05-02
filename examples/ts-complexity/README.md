# TypeScript Complexity Example

Minimal TypeScript fixture for `examples/ts-cyclomatic-complexity`.

Run it from this directory:

```bash
cargo run --manifest-path ../rules/Cargo.toml -- check --profile fast --format json --fail-on none
```

`label.ts` intentionally has two branches while the example config sets
`max = 1`, so the example runner emits one complexity diagnostic.
