---
phase: 63-ground-truth-and-performance-baseline
reviewed: 2026-07-09T00:00:00Z
depth: standard
files_reviewed: 20
files_reviewed_list:
  - crates/polint/Cargo.toml
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
  - research/evaluation-harness/baselines/persisted-graph-accuracy.json
  - research/evaluation-harness/suites/BENCHMARK-SUITE.md
  - research/evaluation-harness/suites/grafana-grafana-scale.toml
  - research/evaluation-harness/suites/gohugoio-hugo-scale.toml
  - research/evaluation-harness/suites/excalidraw-excalidraw-scale.toml
  - research/evaluation-harness/suites/devloupe-monorepo-local.toml
findings:
  blocker: 0
  high: 3
  medium: 1
  low: 7
  total: 11
status: high
---

# Phase 63: Code Review Report (Ground Truth and Performance Baseline)

**Reviewed:** 2026-07-09
**Depth:** standard
**Files Reviewed:** 20
**Status:** issues_found — highest severity `high`
**Nature:** No source was modified (review is read-only).

## Summary

Phase 63 lands a `pub(crate)` benchmark/eval harness under `eval::bench`
(measurement substrate, curve telemetry, markdown/JSON report, whole-repo runner,
sweep, store-disabled + persisted-graph baselines, and a regression-budget gate),
plus scale-suite manifests and three committed baseline JSON files.

Verified good: the `getrusage` FFI is sound (`std::mem::zeroed::<libc::rusage>()`
is valid POD, `RUSAGE_SELF` read-only, non-zero return guarded, negatives clamped)
and the single `#[allow(unsafe_code)]` is correctly scoped while the crate keeps
`unsafe_code = "deny"`; deterministic-JSON emission sorts and avoids float sort
keys (byte-stability tests are meaningful); the gate's zero-denominator guard and
`ratio > budget` (exactly-at-budget = Pass) are correct; every new item is
`pub(crate)` with `unreachable_pub = "deny"` enforcing it; git refs are passed
positionally (no shell); and directory recursion skips symlinks. The two
intentional environment stubs are honestly labeled and env-gated as documented.

No security vulnerabilities or crashes were found. The substantive findings are
**measurement-validity, cross-platform correctness, and gate-robustness** issues:
the harness records a process-wide absolute peak RSS (and discards the
run-attributable delta it computes), the OS unit normalization is wrong on the
BSDs, and the committed baselines are so small the ratio gate that will consume
them in Phase 64+ is noise-dominated. These are latent (the gate is not wired to
run against these baselines yet), so no blocker — but they undermine the point of
a "performance baseline" phase and should be fixed before the Phase 64 gate goes
live.

## High

### HI-01: Absolute process-wide peak RSS is recorded/gated; the confound-correcting delta is computed then discarded

**File:** `crates/polint/src/eval/bench/measure.rs:23-41,59-96`; `crates/polint/src/eval/bench/runner.rs:104`; `crates/polint/src/eval/bench/curve.rs:67`
**Issue:** `peak_rss_bytes()` reads `getrusage(RUSAGE_SELF).ru_maxrss` — a
**process-wide, monotonic, whole-lifetime** high-water mark, not the RSS of the
analyzed run. `TimedRun::measure` recognizes this and computes
`peak_rss_delta_bytes = peak.saturating_sub(baseline_peak)`, but `cold_then_warm`
discards the delta and returns only the absolute `peak_rss_bytes`, and
`run_repo_perf_point` records `timing.peak_rss_bytes` (absolute) into
`CurvePoint.peak_rss_bytes`, which is what `StoreDisabledBaseline` and
`gate::evaluate_regression_budget` compare on. A grep confirms
`peak_rss_delta_bytes` is **never read** anywhere — it is dead. Two consequences:
(a) any measurement taken inside a larger process (the `cargo test` regenerator,
or a real CLI that already allocated) records a peak reflecting unrelated
allocations, so the committed 47 MB is "peak RSS of the test binary at that
moment," not of `polint check`; and (b) `cold_then_warm` taking
`max(warm.peak, cold.peak)` is tautological — the mark is monotonic, so warm is
always ≥ cold — meaning peak can never be attributed to a specific run.
**Fix:** Propagate and record the delta, and gate on it:
```rust
// measure.rs — carry the run-attributable delta through ColdWarm
pub(crate) struct ColdWarm {
    pub(crate) cold_ms: u64,
    pub(crate) warm_ms: u64,
    pub(crate) peak_rss_bytes: u64,        // absolute (reporting only)
    pub(crate) peak_rss_delta_bytes: u64,  // run-attributable
}
peak_rss_delta_bytes: warm.peak_rss_delta_bytes.max(cold.peak_rss_delta_bytes),
```
For a trustworthy absolute, measure a child `polint` process via
`getrusage(RUSAGE_CHILDREN)` after fork/exec rather than `RUSAGE_SELF`.

### HI-02: `ru_maxrss` unit normalization is wrong on the non-Darwin BSDs (silent 1024× under-report)

**File:** `crates/polint/src/eval/bench/measure.rs:35-41`
**Issue:** Normalization branches on `cfg!(target_os = "linux")` (KiB → ×1024) and
treats **everything else** as already-bytes ("macOS and other BSDs already report
bytes"). Only Darwin/iOS report `ru_maxrss` in bytes. FreeBSD, OpenBSD, NetBSD and
DragonFly report it in **kilobytes** (like Linux). On any of those hosts the
recorded `peak_rss_bytes` is under-reported by 1024×, silently corrupting
`benchmark-curves.json` and any baseline regenerated there. (Within a single gate
the error cancels in the ratio, so `evaluate_regression_budget` is unaffected — but
the committed absolute values are not.)
**Fix:** Restrict the bytes case to Darwin:
```rust
if cfg!(any(target_os = "macos", target_os = "ios")) {
    raw // Darwin reports bytes
} else {
    raw.saturating_mul(1024) // Linux + FreeBSD/OpenBSD/NetBSD/DragonFly: kilobytes
}
```
and correct the doc comment.

### HI-03: Ratio-based regression budgets are noise-dominated against the tiny committed baselines

**File:** `crates/polint/src/eval/bench/gate.rs:42-100`; `research/evaluation-harness/baselines/store-disabled-check.json:6-8`; `research/evaluation-harness/baselines/store-disabled-review.json:6-8`
**Issue:** Both committed baselines are `repo_id: "polint-tiny-fixture"` with
`cold_wall_clock_ms == warm_wall_clock_ms == 20` and `peak_rss_bytes ≈ 47 MB`.
`evaluate_regression_budget` applies *ratio* budgets (+25% cold, +20% RSS),
yielding an absolute tolerance of **5 ms** on cold wall-clock and **~9.5 MB** on
peak RSS. Ordinary scheduling jitter on a 20 ms run and the process-wide RSS
confound (HI-01) will routinely exceed those tolerances, so when this gate is
wired in Phase 64+ it will emit false `Fail`/blocking verdicts on runs that did
not regress. That cold == warm == 20 ms in both baselines is itself evidence the
fixture is below the timer's useful resolution, and a 2-file fixture carries no
"curves vs size" scale signal.
**Fix:** Regenerate the store-disabled baseline against a representative scale
checkout (grafana/hugo/excalidraw), or add an absolute floor alongside the ratio
(e.g. exempt the check when `baseline.cold_wall_clock_ms < 200`, or gate on
`max(baseline * ratio, baseline + N_ms)`), and take min-of-N runs for a noise
margin.

## Medium

### MD-01: `deterministic_baseline_json` silently swallows serialization failure

**File:** `crates/polint/src/eval/baseline.rs:332-337`
**Issue:** `serde_json::to_string_pretty(&normalized).unwrap_or_else(|_| "{}".to_string())`
converts a serialization error into a valid-looking but wrong `"{}"`. Any caller
comparing determinism/parity (its stated purpose) would see two `"{}"` compare
equal and conclude the baselines match when serialization actually failed. A
silent wrong result is worse than a surfaced error.
**Fix:** Return `anyhow::Result<String>` and propagate:
```rust
pub(crate) fn deterministic_baseline_json(baseline: &EvalBaseline) -> anyhow::Result<String> {
    let mut normalized = baseline.clone();
    normalized.run = normalize_run(&normalized.run);
    normalized.output_hash = normalized.run.output_hash.clone();
    Ok(serde_json::to_string_pretty(&normalized)?)
}
```

## Low

### LW-01: Cold-run error is silently swallowed in the perf runner

**File:** `crates/polint/src/eval/bench/runner.rs:75-80`
**Issue:** `cold_then_warm` runs the closure twice and the closure overwrites
`last` each call, so only the **warm** run's `Result` is inspected. If the cold
run returns `Err` but warm returns `Ok`, the cold error is discarded and
`cold_wall_clock_ms` reports the wall-clock of a run that failed, yielding a
misleading "successful" measurement.
**Fix:** Capture both results and propagate the first error (or assert the cold run
succeeded before trusting `cold_ms`).

### LW-02: `diagnostics_digest` parity marker is recorded but never enforced

**File:** `crates/polint/src/eval/bench/gate.rs:44-70`; `crates/polint/src/eval/baseline.rs:51-53`
**Issue:** `StoreDisabledBaseline.diagnostics_digest` is documented as *the* parity
marker ("the store must not change the diagnostics polint emits"), but
`evaluate_regression_budget` compares only `peak_rss_bytes` and
`cold_wall_clock_ms`. The parity invariant the field exists for is unguarded in
this phase (docs defer it to "a later run" — acceptable for 63, but track it).
**Fix:** Add a digest-equality check to the gate (Fail on mismatch) when the
measured digest is available, or file the enforcement as an explicit Phase 64 task.

### LW-03: Gate `Warn` verdict is unreachable

**File:** `crates/polint/src/eval/bench/gate.rs:47-67,80-100`
**Issue:** `ratio_budget_check` only returns `Pass` or `Fail`. The struct doc
describes `.max()` aggregation with "Fail dominates Warn dominates Pass" and
`is_blocking` is documented against Warn semantics, but no path here produces
`Warn`. Dead conceptual surface that can mislead a maintainer into thinking a
soft-warn tier exists.
**Fix:** Drop the Warn language from these docs, or introduce a genuine warn band
(e.g. within some fraction of budget) so the `.max()`/`is_blocking` design is
exercised.

### LW-04: Committed persisted-graph accuracy baseline is an all-null / all-zero stub with a structure-only guard test

**File:** `research/evaluation-harness/baselines/persisted-graph-accuracy.json:5-22`; `crates/polint/src/eval/bench/report.rs:423-466`
**Issue:** Both rows have `recall: null`, `precision: null`, and
`graph_edges_*`/`unknown_count` all `0`. The `reference` string is honest and
`GraphAccuracyRow` intentionally allows `null` (this is the plan-anticipated
env-gated stub, so it is not itself a bug). The weakness is the guarding test
`committed_persisted_graph_accuracy_baseline_has_both_suites`, which asserts only
structure/keys and never that values are measured — so a stub can never be
distinguished from a real reference by the suite.
**Fix:** When the gated clones are expected present, tighten the test to assert
non-null measured values; or add an explicit `measured: bool` / `status` field so
a downstream consumer/gate can branch rather than mistake a null stub for a
reference.

### LW-05: `libc` pinned with a literal version instead of `libc.workspace = true`

**File:** `crates/polint/Cargo.toml:24`
**Issue:** Every other dependency uses `.workspace = true`, but `libc = "0.2"` is
pinned inline, diverging from the workspace-dep convention and bypassing version
unification (a future workspace `libc` pin could silently differ here).
**Fix:** Add `libc` to `[workspace.dependencies]` and use `libc.workspace = true`.

### LW-06: `diff_hunk_lines` uses non-saturating `+ 1` on a `u32` and assumes non-empty inclusive ranges

**File:** `crates/polint/src/eval/bench/runner.rs:63-67`
**Issue:** `u64::from(end.saturating_sub(*start) + 1)` performs the `+ 1` in `u32`
before widening (overflow-panic in debug for a `u32::MAX`-wide range), and assumes
every `new_line_ranges` entry (`Vec<(u32,u32)>`) is non-empty with `end >= start`;
a degenerate/inverted range floors the sub to 0 and still counts one line.
Defensive-only given how `changeset_for_ref` builds ranges, but the invariant is
unchecked.
**Fix:** Widen first, then add — `u64::from(end.saturating_sub(*start)) + 1` — and
either skip `end < start` entries or comment the inclusive-non-empty invariant.

### LW-07: `escape_cell` escapes only `|`, not newlines — table injection via repo/dir names

**File:** `crates/polint/src/eval/bench/report.rs:90-92`; `crates/polint/src/eval/markdown.rs:269-271`
**Issue:** `escape_cell` replaces `|` only. `CurvePoint.repo_id` is derived from a
checkout directory's `file_name()` (runner.rs:51-54); a directory name can contain
newlines on Unix, which would break the markdown table structure or inject rows
into the benchmark report. Developer-controlled input, so low, but the escaper is
incomplete for the value it handles.
**Fix:** Also neutralize CR/LF, e.g.
`value.replace('|', "\\|").replace(['\n', '\r'], " ")`.

---

_Reviewed: 2026-07-09_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
