---
phase: 63-ground-truth-and-performance-baseline
reviewed: 2026-07-09T00:00:00Z
depth: deep
review_type: advisory-non-blocking
files_reviewed: 13
files_reviewed_list:
  - crates/polint/src/eval/bench/mod.rs
  - crates/polint/src/eval/bench/measure.rs
  - crates/polint/src/eval/bench/curve.rs
  - crates/polint/src/eval/bench/gate.rs
  - crates/polint/src/eval/bench/report.rs
  - crates/polint/src/eval/bench/runner.rs
  - crates/polint/src/eval/bench/sweep.rs
  - crates/polint/src/eval/baseline.rs
  - crates/polint/src/eval/markdown.rs
  - crates/polint/src/eval/external/mod.rs
  - crates/polint/Cargo.toml
  - research/evaluation-harness/baselines/persisted-graph-accuracy.json
  - research/evaluation-harness/baselines/store-disabled-check.json
findings:
  blocker: 0
  high: 0
  medium: 3
  low: 5
  total: 8
status: medium
---

# Phase 63: Code Review Report (Ground Truth and Performance Baseline)

**Reviewed:** 2026-07-09
**Depth:** deep (cross-file; build + clippy verification)
**Files Reviewed:** 13
**Status:** issues_found — highest severity `medium`
**Nature:** ADVISORY / non-blocking. No source was modified.

## Summary

Phase 63 lands a `pub(crate)` benchmark harness under `eval::bench` (measurement
substrate, curve telemetry, markdown/JSON report, whole-repo runner, sweep,
store-disabled + persisted-graph baselines, and a regression-budget gate), plus
scale-suite manifests and three committed baseline JSON files.

Verified good:

- **`getrusage` FFI soundness** — `std::mem::zeroed::<libc::rusage>()` is valid
  (all-integer POD), `RUSAGE_SELF` is read-only, the non-zero return is guarded,
  and negatives are clamped with `.max(0)`. The single `#[allow(unsafe_code)]` is
  correctly scoped and greppable; the crate keeps `unsafe_code = "deny"` otherwise.
- **Determinism** (`write_curve_series`, `write_graph_accuracy_baseline`) — sort +
  `serde_json::to_string_pretty` with no maps and no float in any `Ord` sort key;
  byte-stability tests are meaningful. Holds.
- **Divide-by-zero guard + ratio math** in `gate.rs` — zero denominator is an
  explicit Fail, and the `ratio > budget` comparison correctly treats
  exactly-at-budget as Pass.
- **Public surface** — every new item is `pub(crate)`; no fully-`pub` item was
  added, and `unreachable_pub = "deny"` structurally enforces this. The harness
  stays crate-internal as required.
- **Lint table** — the hand-rolled `[lints.rust]`/`[lints.clippy]` in
  `crates/polint/Cargo.toml` **does** mirror the workspace table verbatim
  (`unsafe_code` `forbid`→`deny`, `unreachable_pub`, and the same 6 clippy lints);
  the "mirrored verbatim" claim checks out.
- **Serde defaults** on `BaselineThresholds` give backward-compatible
  deserialization (verified by test).
- `cargo build -p polint --lib` (forced recompile) and `cargo clippy -p polint`
  (`--lib` and `--tests`) are both **clean** — no warnings, no dead_code, despite
  `gate.rs`/`measure.rs` being test-only-consumed but not `#[cfg(test)]`-gated.

The findings below are correctness-portability and baseline-honesty concerns, not
build breakers.

## Medium

### MD-01: `ru_maxrss` unit normalization is wrong on the BSDs (silent 1024× under-report)

**File:** `crates/polint/src/eval/bench/measure.rs:35-42`
**Issue:** Normalization branches on `cfg!(target_os = "linux")` (KiB → ×1024)
and treats **everything else** as already-bytes, with the comment
"macOS and other BSDs already report bytes." That is factually incorrect: only
Darwin/iOS report `ru_maxrss` in bytes. FreeBSD, OpenBSD, NetBSD and DragonFly
report it in **kilobytes** (same as Linux). On any BSD host the recorded
`peak_rss_bytes` is under-reported by 1024×, and a `store-disabled-*.json`
baseline regenerated there would be silently wrong.

Mitigating: within a single gate the error cancels in the measured/baseline
*ratio* (both sides same OS), so `evaluate_regression_budget` is unaffected — but
the absolute values in `benchmark-curves.json` and the committed baselines are
not.

**Fix:** Restrict the bytes case to Darwin and treat the other BSDs like Linux:
```rust
if cfg!(any(target_os = "macos", target_os = "ios")) {
    raw // Darwin reports bytes
} else {
    // Linux and the BSDs (FreeBSD/OpenBSD/NetBSD/DragonFly) report kilobytes
    raw.saturating_mul(1024)
}
```
And correct the doc comment accordingly.

### MD-02: Store-disabled baselines are measured on a 2-file toy repo — the "scale/latency" budget is inside measurement noise

**File:** `research/evaluation-harness/baselines/store-disabled-check.json:6-8`,
`research/evaluation-harness/baselines/store-disabled-review.json:6-8`
(generator: `crates/polint/src/eval/baseline.rs:697-760`)
**Issue:** Both committed baselines are `repo_id: "polint-tiny-fixture"` with
`cold_wall_clock_ms == warm_wall_clock_ms == 20`. The locked budgets
(`+20%` RSS, `+25%` cold — `DEFAULT_MAX_COLD_WALL_CLOCK_RATIO = 1.25`) against a
20 ms baseline yield a 25 ms cold threshold. At millisecond resolution on a
2-file fixture that whole window is jitter: the gate this phase builds for the
Phase 64+ *scale/latency* outcome gates will either false-Fail on ordinary
scheduling noise or be meaningless. A 2-file repo also carries no scale signal at
all, so nothing here exercises the "curves vs size" objective the phase claims.

**Fix:** Regenerate the store-disabled baselines against a representative
scale checkout (or, if a fixture must be used for portability, record the
baseline at a size where cold wall-clock is well above the ms-jitter floor and
cold ≠ warm), and/or apply an absolute floor alongside the ratio (e.g.
`max(baseline * 1.25, baseline + N_ms)`) so a 20 ms baseline cannot make the
budget noise-dominated.

### MD-03: Committed persisted-graph accuracy baseline is an all-null / all-zero stub

**File:** `research/evaluation-harness/baselines/persisted-graph-accuracy.json:6-22`
**Issue:** Both rows have `recall: null`, `precision: null`, and
`graph_edges_expected/observed/unknown_count` all `0`. The `reference` string is
honest ("regenerated with `POLINT_WRITE_GRAPH_BENCH` when the gated benchmark
clones are present"), and `GraphAccuracyRow` intentionally allows `null` — so the
labeling is honest — but the committed artifact contains **zero** accuracy data.
As a "pre-store accuracy reference" it cannot detect any accuracy regression, and
the guarding test (`committed_persisted_graph_accuracy_baseline_has_both_suites`,
`report.rs:404-457`) only asserts structure/keys, never that the values are
measured. It is a placeholder masquerading as a baseline.

**Fix:** Either populate the baseline with real measured recall/precision (run the
gated benchmark once and commit the result), or make the honesty explicit at the
type level — e.g. a `measured: bool` / `status: "unmeasured"` field the report and
any future gate must branch on — so a downstream consumer can't mistake a null
stub for a real reference. At minimum, tighten the test to assert measured values
once clones are expected.

## Low

### LW-01: `diagnostics_digest` parity marker is recorded but never enforced

**File:** `crates/polint/src/eval/bench/gate.rs:44-70`,
`crates/polint/src/eval/baseline.rs:51-52`
**Issue:** `StoreDisabledBaseline.diagnostics_digest` is documented as *the*
parity marker ("the store must not change the diagnostics polint emits"), but
`evaluate_regression_budget` compares only `peak_rss_bytes` and
`cold_wall_clock_ms`. Nothing in this phase compares digests, so the parity
invariant the field exists for is unguarded. (Docs defer it to "a later run" —
acceptable for Phase 63, but worth tracking so it is not forgotten.)
**Fix:** Add a digest-equality check to the gate (Fail on mismatch) when the
measured run's digest is available, or file the enforcement as an explicit Phase
64 task.

### LW-02: Gate `Warn` verdict is unreachable

**File:** `crates/polint/src/eval/bench/gate.rs:48-56, 96-116`
**Issue:** `ratio_budget_check` only ever returns `Pass` or `Fail`. The struct doc
describes `.max()` aggregation with "Fail dominates Warn dominates Pass" and
`is_blocking` is documented against Warn semantics, but no code path in this gate
produces `Warn`. Dead conceptual surface that can mislead a future maintainer into
thinking a soft-warn tier exists here.
**Fix:** Either drop the Warn language from these docs, or introduce a genuine
warn band (e.g. Warn when the ratio is within some fraction of the budget) so the
`.max()`/`is_blocking` design is actually exercised.

### LW-03: Cold-run error is silently swallowed in the perf runner

**File:** `crates/polint/src/eval/bench/runner.rs:75-80`
**Issue:** `cold_then_warm` runs the closure twice and the closure overwrites
`last` each call, so only the **warm** (2nd) run's `Result` is inspected. If the
cold run returns `Err` but the warm run returns `Ok`, the cold error is discarded
and `cold_wall_clock_ms` reports the wall-clock of a run that actually failed.
Test-only code, but it can produce a misleading "successful" measurement.
**Fix:** Capture both results (e.g. `Vec<Result<..>>` or a small struct) and
propagate the first error, or assert the cold run succeeded before trusting
`cold_ms`.

### LW-04: `libc` pinned with a literal version instead of `libc.workspace = true`

**File:** `crates/polint/Cargo.toml:22`
**Issue:** Every other dependency in this manifest uses `.workspace = true`, but
`libc = "0.2"` is pinned inline. This diverges from the workspace-dep convention
and bypasses workspace version unification (a future workspace `libc` pin could
silently differ from this crate's).
**Fix:** Add `libc` to `[workspace.dependencies]` and use `libc.workspace = true`
here, matching the rest of the manifest.

### LW-05: `diff_hunk_lines` can over-count a degenerate range by 1

**File:** `crates/polint/src/eval/bench/runner.rs:57-63`
**Issue:** `u64::from(end.saturating_sub(*start) + 1)` assumes every
`new_line_ranges` entry is a non-empty inclusive `(start, end)` with `end >=
start`. For a degenerate/zero-width or inverted range the `saturating_sub` floors
to 0 and the `+ 1` still counts one line, inflating the hunk-line total.
Defensive-only given how `changeset_for_ref` builds ranges, but the invariant is
unchecked.
**Fix:** If empty ranges are possible, guard them (skip when `end < start`);
otherwise add a comment asserting the inclusive-non-empty invariant this relies
on.

---

_Reviewed: 2026-07-09_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep — advisory / non-blocking_
