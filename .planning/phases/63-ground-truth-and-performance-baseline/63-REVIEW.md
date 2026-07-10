---
phase: 63-ground-truth-and-performance-baseline
reviewed: 2026-07-09T21:10:00Z
depth: standard
re_review: true
pass: 4
prior_findings: { high: 0, medium: 0, low: 1, total: 1 }
files_reviewed: 13
files_reviewed_list:
  - crates/polint/Cargo.toml
  - Cargo.toml
  - crates/polint/src/eval/mod.rs
  - crates/polint/src/eval/baseline.rs
  - crates/polint/src/eval/markdown.rs
  - crates/polint/src/eval/external/mod.rs
  - crates/polint/src/eval/bench/mod.rs
  - crates/polint/src/eval/bench/measure.rs
  - crates/polint/src/eval/bench/curve.rs
  - crates/polint/src/eval/bench/runner.rs
  - crates/polint/src/eval/bench/report.rs
  - crates/polint/src/eval/bench/sweep.rs
  - crates/polint/src/eval/bench/gate.rs
  - research/evaluation-harness/baselines/store-disabled-check.json
  - research/evaluation-harness/baselines/store-disabled-review.json
findings:
  blocker: 0
  high: 0
  medium: 0
  low: 0
  total: 0
status: clean
---

# Phase 63: Code Review Report (Fourth Pass — Sweep Isolation Fix Verification)

**Reviewed:** 2026-07-09
**Depth:** standard
**Files Reviewed:** 13 (+2 committed baseline JSONs)
**Status:** clean — no issues of any severity remain
**Nature:** No source was modified (review is read-only).

## Summary

Fourth review pass, scoped to confirm the sole residual from pass 3 (LW-10: the
benchmark sweep measured points with the non-isolated `run_repo_perf_point`, so
LW-09's new "gate metric" delta column surfaced order-confounded values into the
sweep report) is resolved by commit `8727c144`, plus a final holistic pass over
the phase-63 harness.

**LW-10 is resolved, and the fix introduced no new defect.** The three prior
settled items (the intentional graph-accuracy `null` stub, the documented
check-scoped digest LW-08, and the env-gated regenerable fixture-magnitude
baselines) were not re-examined per the review scope. No new findings.

## Confirmed Fixed

### LW-10 — sweep now measures every point through the isolated child-process path (RESOLVED)

`run_benchmark_sweep` (`sweep.rs:68-73`) now drives `run_sweep_with` with a
measurer that calls `run_repo_perf_point_isolated` (`sweep.rs:70-72`), the same
dedicated-child-process methodology (`runner.rs:169-211`) used to regenerate the
committed gate baselines (`baseline.rs:776-777`). Each curve point therefore gets
its own unsaturated `RUSAGE_SELF` high-water mark, so the sweep's
`peak_rss_delta_bytes` column is now run-attributable and order-independent — the
exact HI-01R confound that LW-10 flagged is eliminated at the source rather than
papered over with a disclaimer. The chosen fix is the stronger of the two options
proposed in pass 3.

Verified against every failure mode called out in the assignment:

- **Isolated path used consistently.** Production entry point (`sweep.rs:71`) and
  the real-fixture integration test (`sweep.rs:314-315`) both use
  `run_repo_perf_point_isolated`. The regenerator (`baseline.rs:776-777`) uses the
  same path, so the sweep's methodology matches the committed baselines it frames
  its report against. The module/function docs (`sweep.rs:8-11,55-65`) now
  accurately describe the dedicated-child-process measurement.
- **Determinism test still valid.** `sweep_assembles_multipoint_series_and_writes_deterministic_artifacts`
  (`sweep.rs:233-277`) still injects the deterministic `fixed_point` measurer via
  the `run_sweep_with` seam (`sweep.rs:252,271`), correctly isolating the
  byte-identical-emission assertion from inherently volatile timing/RSS capture.
  The seam is the right design: the production isolated path is instead covered by
  the real-fixture test and the absent-checkout entry-point test.
- **No dead import.** `use ...runner::run_repo_perf_point_isolated` (`sweep.rs:24`)
  is used at lines 71 and 315. The remaining `run_repo_perf_point` mentions in
  `sweep.rs` are intra-doc-link references only (`sweep.rs:56,67`), not a `use`.
  The non-isolated `run_repo_perf_point` remains live (the in-child measurement
  entry `runner.rs:404` plus its two direct tests `runner.rs:443,500`), so it is
  not orphaned by the switch.
- **Absent-clone skip intact.** `committed_sweep_targets` still gates each target
  on `repo_root.exists()` (`sweep.rs:90`), and `run_sweep_with` skips (not fails)
  a repo whose baseline point errors and skips just the point for an unreachable
  review ref (`sweep.rs:116-141`). `sweep_entry_point_skips_absent_checkouts_without_failing`
  (`sweep.rs:340-356`) confirms the real entry point succeeds and writes both
  artifacts with all clones absent.
- **Still `#[cfg(test)]` / `pub(crate)`.** `sweep.rs` is `#![cfg(test)]` (line 19)
  and `run_benchmark_sweep` is `pub(crate)` (line 68). No public/SDK/CLI surface
  is introduced.

## Holistic Pass

Re-traced the surrounding harness — measurement substrate (`measure.rs`),
curve/report emission (`curve.rs`, `report.rs`, `markdown.rs`), the regression
gate (`gate.rs`), and the baseline types/regenerator (`baseline.rs`). No new
correctness, security, or quality defects. Prior-pass strengths hold: loud
failure propagation in the child-process path, deterministic sorted JSON with
`deny_unknown_fields`, table-cell escaping of `|`/CR/LF against injection, the
zero-baseline-denominator Fail (not divide-by-zero), the HI-03 absolute noise
floors, and the widen-before-`+1` guard on hunk-line counting. Committed
store-disabled baselines still load/validate with run-attributable deltas
(~38.7 MiB check, ~35.3 MiB review) that clear the 16 MiB floor, keeping the
Phase 64 gate ceiling meaningful.

All reviewed files meet quality standards. No issues found.

---

_Reviewed: 2026-07-09_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard (fourth pass)_
