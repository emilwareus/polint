---
quick_id: 260601-e11
slug: fix-deep-pr-review-findings
status: complete
created: 2026-06-01
---

# Quick Task: Fix Deep PR Review Findings

## Goal

Fix the deep PR review findings for Phase 45 TS direct bindings, run all local checks, commit the fixes locally, and do not push.

## Scope

- Make direct binding resolution choose the nearest visible lexical binding before classifying imports, aliases, functions, and unsupported dynamic boundaries.
- Make parameter callback boundaries scope-aware instead of file-name based.
- Link local function-expression bindings to inventory functions without relying on overlapping declaration/function spans.
- Resolve imported targets through explicit exported rows instead of private same-named functions.
- Remove or prevent cross-file dense-id collisions from the direct-binding store contract.
- Add regression tests for each fixed behavior.

## Verification

- `cargo test -p polint direct_ -- --nocapture`
- `cargo test -p polint`
- `cargo fmt --all --check`
- `git diff --check`
- `cargo clippy -p polint --all-targets -- -D warnings`
