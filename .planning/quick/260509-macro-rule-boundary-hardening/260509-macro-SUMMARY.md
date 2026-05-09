# Quick Task 260509-macro Summary

## Completed

- Hardened `#[polint::rule]` parsing so the first parameter must be `&mut RuleCtx<'_>`.
- Hardened `#[polint::rule]` parsing so rule functions must be plain non-generic sync functions and return `RuleResult` or `RuleResult<()>`.
- Rejected qualified fact-view paths unless they are canonical `polint::sdk::facts::*` or `polint::sdk::prelude::*` paths.
- Rejected missing or non-placeholder lifetimes on `RuleCtx` and fact-view parameters.
- Updated generated rule scaffolds to use `_ctx` so fresh rules do not warn on an unused context.
- Updated static capability docs and `AGENTS.md` with the stricter rule-authoring boundary.
- Fixed a rustdoc link that still referenced now-private rule internals.

## Verification

- `cargo test -p polint-macros --locked`
- `cargo test -p polint --test cli rule_macro_rejects_non_canonical_qualified_fact_view_paths --locked`
- `cargo fmt --all -- --check`
- `cargo check --workspace --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked`
- `git diff --check`
- `cargo install --path crates/polint --locked --force`
- Installed `polint check --format json --fail-on none` against every checked-in example.
- Installed `polint init`, `polint new-rule ts no-inline-colors`, `polint explain plan --format json`, and `polint check --format json --fail-on none` in a generated temp repo.
