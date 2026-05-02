# TypeScript Complexity Example

Minimal TypeScript fixture for `local/ts-cyclomatic-complexity`.

This directory is self-contained: the local rule implementation lives at
`.polint/rules/ts-complexity/src/main.rs`.

Run it from this directory:

```bash
cargo run --manifest-path .polint/rules/ts-complexity/Cargo.toml -- check --profile fast --format json --fail-on none
```

`label.ts` intentionally has two branches while the example config sets
`max = 1`, so the example runner emits one complexity diagnostic.
