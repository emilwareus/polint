---
phase: 30-direct-call-facts
reviewed: 2026-05-21T10:19:06Z
depth: standard
files_reviewed: 5
files_reviewed_list:
  - crates/polint/src/analysis/calls/direct.rs
  - crates/polint/src/analysis/calls/provider.rs
  - crates/polint/src/analysis_kernel/provider.rs
  - crates/polint/src/eval/fixtures.rs
  - tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml
findings:
  critical: 0
  warning: 0
  info: 0
  total: 0
status: clean
---

# Phase 30: Code Review Rerun Report

**Reviewed:** 2026-05-21T10:19:06Z
**Depth:** standard
**Files Reviewed:** 5
**Status:** clean

## Summary

Re-reviewed the focused direct-call fix area after the code-review fixes and the stale direct-calls core fixture correction. The direct resolver now preserves distinct algorithms for direct references, import bindings, constructor bindings, static members, and instance members; the calls provider marks resolved sites from resolved targets and includes call output payloads in deterministic digests; the provider manifest keeps `polint.calls` after its semantic and CFG inputs; and the direct-calls core eval fixture now expects `DirectMember` rather than the stale `StaticMember` invariant for the instance-member path.

No correctness regressions, security issues, or stale fixture expectations were found in the reviewed files.

Verification run:

- `cargo test -p polint --locked direct_calls_core` passed: 5 passed, 0 failed.
- `cargo test -p polint --locked analysis::calls::direct::tests` passed: 5 passed, 0 failed.

Both targeted commands emitted the existing Rust dead-code warning for call-store accessor methods in `crates/polint/src/core/mod.rs`; that file was outside this focused review scope and the warning did not affect the targeted test results.

All reviewed files meet quality standards. No issues found.

---

_Reviewed: 2026-05-21T10:19:06Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
