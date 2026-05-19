# Quick Task 260519-qdf: Fix Second Phase 27 Topology Review Findings

**Date:** 2026-05-19
**Status:** Complete

## Scope

Fix the five follow-up review findings from the Phase 27 module topology deep review:

1. Make module graph topology cache keys notice source-less TS/JS workspace member manifests.
2. Preserve uncertainty when duplicate semantic import rows share the same `(file, path)`.
3. Avoid invalid `ExactLockfile` precision on generic repository overlay rows.
4. Mark malformed or unsupported `package.json` manifests as unsupported topology evidence.
5. Parse both `bundleDependencies` and `bundledDependencies`.

## Tasks

### 1. Patch topology identity and fact derivation

- Files: `crates/polint/src/analysis_kernel/incremental/keys.rs`, `crates/polint/src/module_graph/mod.rs`, `crates/polint/src/module_graph/ts.rs`, `crates/polint/src/module_graph/formats/package_json.rs`
- Action: apply targeted fixes without widening public SDK or CLI surfaces.
- Verify: focused unit/cache tests for each bug.
- Done: complete.

### 2. Add regression coverage

- Files: same modules' existing test sections.
- Action: add focused tests that fail under the reviewed behavior.
- Verify: run the named tests and full workspace test suite.
- Done: complete.

### 3. Final review and PR update

- Files: changed Rust files and GSD artifacts.
- Action: run formatting, clippy, one more static review, update summary/state, commit, and push the existing PR branch.
- Verify: clean worktree and PR branch pushed.
- Done: complete.
