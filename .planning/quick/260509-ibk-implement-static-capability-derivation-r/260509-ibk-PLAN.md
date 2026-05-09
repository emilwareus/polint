# Quick Task 260509-ibk: Implement static capability derivation rule authoring API

**Date:** 2026-05-09
**Mode:** quick local execution

## Goal

Replace the normal handwritten `capabilities()` rule-authoring path with macro-derived capabilities from typed fact-view parameters. This branch is pre-release and intentionally does not preserve backwards compatibility where a cleaner API is possible.

## Tasks

1. Add typed fact-view API and a proc macro that derives rule capabilities from rule function parameters.
2. Route generated rules through the existing `AnalysisPlan` runtime while removing broad fact access from the normal `RuleCtx` surface.
3. Rewrite examples and `polint new-rule` scaffolds to use macro-style analyzable rules.
4. Add realistic tests proving derived capabilities appear in `polint explain plan`, generated scaffolds compile, and invalid macro shapes fail.
5. Update docs to describe the new user experience and pre-release compatibility posture.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
- Targeted CLI tests for `new-rule`, checked-in examples, explain-plan, and invalid macro usage.
