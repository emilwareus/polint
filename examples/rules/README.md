# Example Rules

This crate contains example policy rules and a small runner for the checked-in
example fixtures.

These rules are not bundled into the shipped `polint` CLI. They live under
`examples/` so rule authors can inspect real SDK rule code without turning those
policies into product defaults.

Run an example fixture from that fixture directory:

```bash
cargo run --manifest-path ../rules/Cargo.toml -- check --profile fast --format json --fail-on none
```

Run the example rule tests:

```bash
cargo test -p polint-example-rules
```
