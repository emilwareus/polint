# Quick Task 260520-h6j Summary

**Date:** 2026-05-20
**Status:** Complete

## Outcome

Fixed local Phase 28 semantic MIR review findings and added broad regression coverage for TS/JS MIR edge cases.

## Iterations

### Iteration 1

- Refactored oversized helper signatures with `PlaceInsert`, `UnsupportedDraftInput`, and `SpanCheck`.
- Removed redundant clones caught by local clippy.
- Expanded TS/JS lowering to recurse through common nested expression and statement forms instead of silently dropping evidence.
- Added structured unsupported rows for conservative constructs including switch, try/catch/finally, throw, do-while, for-in/of, optional chaining, dynamic import, private fields, tagged templates, function/class expressions, and JSX edge cases.

### Iteration 2

- Tightened place-reference conversion for Go and TS/JS call arguments and unsupported affected places so missing keys are all-or-none instead of silently filtered.
- Added debug assertions for missing drafted place keys before conversion.
- Added assignment target support for TS wrapper expressions and private-field targets.

### Iteration 3

- Fixed `for (existingTarget of/in source)` lowering so existing identifier/member targets get explicit write operations.
- Lowered computed object literal keys so nested call evidence in keys is retained.
- Lowered call callee expressions so computed callees like `api[helper(k)](...)` retain nested key/callee evidence.
- Added focused regression tests for existing for-in/of targets, computed object keys, computed callees, JSX children, optional/private/dynamic import, control statements, and nested expression calls.

## Verification

- `cargo test -p polint analysis::mir::lower_ts --lib --all-features --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
- `git diff --check`

All local checks passed. The slow `cargo_install_smoke` test remains ignored by default as before.

## Review Verdict

No remaining local blockers found after the final review pass. The TS/JS MIR lowerer remains intentionally conservative: semantics not modeled by the private MIR are emitted as structured unsupported evidence rather than claimed as exact behavior.
