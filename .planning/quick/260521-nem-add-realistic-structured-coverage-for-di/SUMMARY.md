# Quick Task 260521-nem: Add realistic structured coverage

## Scope

Added structured fixture coverage for the supported direct-call and abstract-domain language features in PR 35.

## Changes

- Tagged direct-call fixture source with `POLINT-FEATURE` markers for supported Go and TS/JS call shapes.
- Tagged abstract-domain fixture source with `POLINT-FEATURE` markers for supported Go and TS/JS domain-transfer shapes.
- Added feature matrix tests that verify fixture source markers exactly match the supported feature list.
- Added observed-contract tests that verify each supported feature category is represented in eval output.
- Expanded abstract-domain invariant checks to cover every P0 slot, uncertainty status, precision category, provider count, and index family.

## Verification

- `cargo test -p polint --lib eval::fixtures:: --locked`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --locked`
