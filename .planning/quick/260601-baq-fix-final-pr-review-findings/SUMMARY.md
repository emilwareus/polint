---
quick_id: 260601-baq
slug: fix-final-pr-review-findings
status: complete
completed: 2026-06-01
---

# Summary: Fix Final PR Review Findings

## Completed

- Prevented arbitrary TS member calls like `obj.f()` from resolving to same-file
  functions by property name alone.
- Preserved lexical scope for AST fallback alias and destructuring rows.
- Kept explicit object-literal member aliases, such as `const ns = { f }; ns.f()`,
  as the supported local member-call shape.
- Added regression tests for arbitrary member calls, block-scoped alias leakage,
  and non-function alias shadowing.
- Updated the Phase 45 roadmap progress table to `5/5 Complete`.

## Verification

- `cargo test -p polint direct_local -- --nocapture`
- `cargo test -p polint direct_ -- --nocapture`
- `cargo test -p polint`
- `cargo fmt --all --check`
- `git diff --check`
- `cargo clippy -p polint --all-targets -- -D warnings`
