# Phase 41 Code Review

Status: PASS
Reviewer: Codex inline review
Date: 2026-05-26

## Scope

Reviewed production changes from phase 41 against `origin/main...HEAD`, focusing on:

- Public SDK query helpers in `crates/polint/src/sdk/facts.rs`
- Agent JSON CLI commands in `crates/polint/src/cli/mod.rs`
- Analysis plan capability support wiring in `crates/polint/src/analysis_plan.rs`
- External-rule CLI tests and generated skill/documentation updates

## Findings

### Fixed During Review

- `facts sample --cap <reserved>` accepted reserved public capabilities such as `dataflow` and returned an empty success report even though `facts list` advertised `sampling: false`.
  - Fix: `facts_sample` now rejects unknown capabilities before analysis and rejects reserved/non-sampling capabilities with a docs pointer.
  - Regression: `facts_sample_requires_or_applies_bounded_limit` now asserts `dataflow` sampling fails with `docs/facts/data-flow.md`.
  - Commit: `97b520c fix(41): reject sampling reserved fact views`

### Remaining Findings

No unresolved correctness, security, or public API contract findings found in the reviewed scope.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test -p polint --test cli facts_sample_requires_or_applies_bounded_limit --locked -- --exact`
- `cargo test -p polint --test cli --locked -- --test-threads=1`
- `cargo doc -p polint --all-features --no-deps --locked`

