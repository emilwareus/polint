# Config Denied Literal Example

Minimal TypeScript fixture for `examples/config-query-no-literal`.

Run it from this directory:

```bash
cargo run --manifest-path ../rules/Cargo.toml -- check --profile fast --format json --fail-on none
```

`query.ts` intentionally contains `legacy-testid`, and the example config denies
that literal text. This models a repo-local policy where the forbidden value
comes from configuration rather than from a hard-coded rule.
