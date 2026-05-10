# Quick Task 260510-dzr: Implement reusable derived metric signals for rules

**Date:** 2026-05-10
**Status:** In progress

## Goal

Add first-class reusable metric signals that rules can request through typed SDK
fact views, so many rules can consume shared file/function/complexity evidence
without depending on each other or internal polint modules.

## Tasks

1. Add public metric fact structs and typed views.
   - Files: `crates/polint/src/core/mod.rs`, `crates/polint/src/sdk/facts.rs`,
     `crates/polint/src/sdk/mod.rs`
   - Verify: unit tests prove derived metrics are deterministic and exposed from
     the prelude.

2. Wire static capability derivation.
   - Files: `crates/polint/src/core/mod.rs`,
     `crates/polint/src/analysis_plan.rs`, `crates/polint-macros/src/lib.rs`
   - Verify: macro tests and analysis-plan tests show metric view parameters map
     to supported capabilities.

3. Document and demonstrate realistic composition.
   - Files: `examples/code-quality-metrics/**`, `docs/facts/**`, `README.md`,
     `crates/polint/src/cli/skill.rs`
   - Verify: integration tests run an outside-style rule pack with metric views
     through `polint check --format json`.

## Test Plan

- `cargo test -p polint-macros`
- `cargo test -p polint`
- Example CLI runs from `examples/code-quality-metrics`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
