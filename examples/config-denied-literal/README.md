# Config Denied Literal Example

Minimal TypeScript fixture for `local/no-denied-literals`.

This directory is self-contained: the local rule implementation lives at
`.polint/rules/no-denied-literals/src/main.rs`.

Run it from this directory:

```bash
cargo run --manifest-path .polint/rules/no-denied-literals/Cargo.toml -- check --profile fast --format json --fail-on none
```

`query.ts` intentionally contains `legacy-testid`, and the example config denies
that literal text. This models a repo-local policy where the forbidden value
comes from configuration rather than from a hard-coded rule.
