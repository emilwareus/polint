# Quick Task 260512-aop: Fix review findings for baseline and module relationships

**Date:** 2026-05-12
**Status:** In progress

## Goal

Fix the four confirmed review findings before merge:

1. TS path aliases that match but miss their target must stay unresolved, not external.
2. Baseline matching must support moved-path refresh without treating ordinary moved diagnostics as new.
3. TS dynamic imports inside array elements must produce import facts.
4. Setup-missing capability diagnostics must report the resolver family that actually failed.

## Tasks

### 1. Relationship Resolver Correctness

**Files:** `crates/polint/src/module_graph/ts.rs`, `crates/polint/src/module_graph/mod.rs`

**Action:**
- Split `MatchedAliasNotFound` from ordinary `NotFound` in TS resolution.
- Track setup-missing reasons from actual failed resolver facts instead of default Go metadata when there are no Go inputs.

**Verify:**
- Add/adjust unit coverage for matched TS alias misses and TS-only setup errors.

### 2. TS Import Traversal

**Files:** `crates/polint/src/ts/adapter.rs`, `crates/polint/src/ts/tests.rs`

**Action:**
- Extend array-element traversal to include `ImportExpression` and expression-like nested array forms where relevant.

**Verify:**
- Add parser fixture coverage for `[import("./lazy"), import(name)]`.

### 3. Baseline Identity

**Files:** `crates/polint/src/baseline.rs`, `crates/polint/tests/cli.rs`

**Action:**
- Keep exact `rule_id + fingerprint` matching first.
- Add a conservative relocation identity for diagnostics whose default fingerprint changed only because file/range moved.
- Avoid cross-file duplicate suppression by using relocation only when exactly one current diagnostic and one stored entry share that relocation identity.

**Verify:**
- Add unit and CLI coverage for baseline moved-path refresh and duplicate ambiguity.

### 4. Final Checks

**Verify:**
- `cargo fmt --all -- --check`
- Focused Rust tests for changed modules.
- Broader workspace checks if focused tests pass.
