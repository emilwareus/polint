# Quick Task 260509-rul Summary: Remove manual Rule implementation escape hatch

**Date:** 2026-05-09
**Status:** Complete

## What Changed

- Replaced the public `Rule` trait shape with an opaque `Rule` value.
- Updated `#[polint::rule]`, runner, analysis planning, cache keys, CLI paths,
  and bench hooks to pass `Vec<Rule>` instead of `Vec<Arc<dyn Rule>>`.
- Kept rule construction behind generated macro plumbing; normal rule authors
  cannot implement `Rule` manually or construct capabilities through the prelude.
- Made the macro build canonical `polint::sdk::facts::*` views so local
  lookalike fact types cannot become capability sources by name.
- Updated docs, AGENTS.md, and the generated agent skill guidance to state the
  beta fail-forward rule-authoring contract.

## Verification

- `cargo fmt --all -- --check`
- `cargo check --workspace --locked`
- `cargo test --workspace --all-features --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `git diff --check`
- `cargo install --path crates/polint --locked --force`
- `polint --version` reports `polint 0.1.7`
- Ran installed `polint check --format json --fail-on none` for every
  checked-in example.
- Ran installed `polint init`, `polint new-rule ts`, `polint explain plan`, and
  `polint check` against a temp repo under `target/` using the local crate path.

## Notes

An installed smoke test created outside this checkout will resolve the published
crate version from crates.io. Until this branch is published, use a temp repo
under the checkout or rewrite the generated rule pack dependency to a local path
for development verification.
