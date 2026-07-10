---
phase: 64-store-foundation-and-boundary-proof
iteration: 2
findings_in_scope:
  - WR-01
  - WR-02
fixed: 2
skipped: 0
status: all_fixed
---

# Phase 64 Code Review Fix Report

## Fixed Findings

### WR-01 — Serialize fresh-store migrations under the writer lease

- Commit: `b3b4823e` (`fix(64): serialize fresh store migrations`)
- Writer initialization now acquires `BEGIN IMMEDIATE` before re-reading
  `user_version`, applying migrations, validating the resulting schema, and
  committing.
- The migration runner consumes the already-held transaction instead of opening
  a nested deferred transaction.
- A deterministic absent-store fixture prepares two version-zero connections,
  holds the first initialization lease, verifies the second returns the typed
  busy outcome within the 250 ms policy bound, then proves one valid marker and
  an intact database after release.

### WR-02 — Require WAL negotiation to return WAL

- Commit: `e9c11275` (`fix(64): require successful WAL negotiation`)
- Writer policy now validates the value returned by
  `PRAGMA journal_mode = WAL` case-insensitively.
- A successful non-WAL result becomes the private typed `Policy` error and maps
  to the existing controlled `OpenFailed` store status.
- Focused tests cover mixed-case WAL acceptance and non-WAL rejection.

## Verification

- `cargo test -p polint --lib analysis_kernel::store::migrations::tests --locked -- --test-threads=1` — 9 passed.
- `cargo test -p polint --lib analysis_kernel::store::tests::writer_contention --locked -- --test-threads=1` — 2 passed.
- `cargo test -p polint --lib analysis_kernel::store::tests::connection_policy --locked -- --test-threads=1` — 5 passed.
- `cargo test -p polint --lib analysis_kernel::store --locked -- --test-threads=1` — 23 passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store --locked -- --test-threads=1` — 3 passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` — passed in both atomic commit hooks.

No public API, CLI output, store activation default, timeout, or schema surface
changed.

## Auto Re-review

Iteration 2 re-reviewed the original 15-file scope at standard depth. Both
warnings are resolved, no new blocker/critical/warning/informational findings
were introduced, and `64-REVIEW.md` now records `status: clean`.
