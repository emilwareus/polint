---
quick_id: 260601-e11
slug: fix-deep-pr-review-findings
status: complete
completed: 2026-06-01
---

# Summary: Fix Deep PR Review Findings

## Completed

- Made TS direct-binding resolution choose the nearest visible lexical binding before classifying imports, aliases, functions, or unsupported dynamic boundary rows.
- Made parameter callback boundary extraction scope-aware and tied boundary rows to the actual call scope.
- Resolved local function-expression calls through their lexical binding even when Oxc symbol spans do not overlap the function-expression span.
- Made module import resolution depend on explicit export rows, with support for local export aliases and CommonJS export assignments.
- Tightened local export alias resolution to visible local functions or overlapping export-assignment functions, avoiding file-wide nested-function fallbacks.
- Replaced direct-binding store indexes over per-file dense inventory IDs with stable-key indexes to avoid cross-file collisions.
- Added focused regressions for import shadowing, scoped callback boundaries, function-expression bindings, local export aliases, nested non-visible exports, and stable-key store indexes.

## Verification

- `cargo test -p polint direct_ -- --nocapture`
- `cargo test -p polint`
- `cargo test -p polint --test public_surface_leak`
- `cargo test -p polint --doc`
- `cargo fmt --all --check`
- `git diff --check`
- `cargo clippy -p polint --all-targets -- -D warnings`
