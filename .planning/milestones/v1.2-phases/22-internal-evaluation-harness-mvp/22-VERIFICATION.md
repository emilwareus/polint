---
phase: 22-internal-evaluation-harness-mvp
verified: 2026-05-17T18:14:46Z
status: passed
score: 8/8 must-haves verified
overrides_applied: 0
---

# Phase 22: Internal Evaluation Harness MVP Verification Report

**Phase Goal:** Add a hidden/internal evaluation model with deterministic expected/observed JSON, generic matchers, metrics, and native fixtures.
**Verified:** 2026-05-17T18:14:46Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Internal evaluation data represents expected/observed diagnostics, facts, graph edges, paths, invariants, and runtime budgets. | VERIFIED | `crates/polint/src/eval/model.rs` defines all expected/observed item variants and runtime budget shapes; `cargo test -p polint eval_ --locked` passed 52 eval tests. |
| 2 | Expected/observed report serialization and output hashing are deterministic after normalization. | VERIFIED | `crates/polint/src/eval/report.rs` sorts cases/items/matches and hashes canonical JSON via `stable_hash`; commit `ae4b708` fixed REVIEW WR-01 with total-order serialized tie-breakers and a regression test. |
| 3 | Output hashes exclude transient runtime duration and machine-local path/timestamp data. | VERIFIED | `deterministic_output_hash` clears runtime durations before hashing; tests cover timestamp/path marker exclusion and duration-insensitive hashes. |
| 4 | Generic matchers cover exact, tolerant, partial, forbidden, trap, unknown, graph/path, invariant, and runtime-budget outcomes. | VERIFIED | `crates/polint/src/eval/matcher.rs` implements `match_case`, `MatchOutcome`, partial graph/path `Unconfirmed`, and runtime pass/fail from `budget_passed`. |
| 5 | Metrics include confusion counts, precision/recall/F-scores, trap/unknown counts, graph/path counts, runtime budget counts, and accepted/rejected fact status counts. | VERIFIED | `crates/polint/src/eval/metrics.rs` computes and maps `ComputedMetrics` into report `MetricSummary`; eval metric tests passed. |
| 6 | Native fixtures run from fixture-owned repos and at least one fixture consumes real `AnalysisKernel::run` output. | VERIFIED | `crates/polint/src/eval/observed.rs` copies fixture repos, runs `AnalysisKernel::run`, observes diagnostics/provider order/metadata facts, and rejects symlink escape. |
| 7 | Harness fixtures cover kernel, provenance, cache, and extension invariants. | VERIFIED | Four fixture manifests exist under `tests/eval-fixtures/{kernel,provenance,cache,extension}`; `eval_native_fixture_suite_covers_required_categories` passed. |
| 8 | The harness remains hidden/internal with no public CLI, SDK, runner, schema, or public check JSON leak. | VERIFIED | `crates/polint/src/lib.rs` has `pub(crate) mod eval;`; `eval_harness_stays_internal` proves `polint eval` is unrecognized and public JSON has no eval markers. |

**Score:** 8/8 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/polint/src/eval/model.rs` | Canonical internal expected/observed model | VERIFIED | Contains `ExpectedItem`, `ObservedItem`, `AssertionMode`, `ObservedStatus`, and all Phase 22 item structs. |
| `crates/polint/src/eval/report.rs` | Deterministic report serialization and hashing | VERIFIED | Normalization uses stable semantic keys plus serialized tie-breakers; `ae4b708` regression test passes. |
| `crates/polint/src/eval/matcher.rs` | Generic matcher engine | VERIFIED | Pure matcher over normalized items; no provider execution in matcher. |
| `crates/polint/src/eval/metrics.rs` | Unified metric aggregation | VERIFIED | Counts TP/FP/FN/TN, traps, unknowns, graph/path, runtime, and fact statuses. |
| `crates/polint/src/eval/fixtures.rs` | Native fixture loader/runner | VERIFIED | Loads manifests, validates paths, gates synthetic observed rows to extension fixtures, runs suite coverage. |
| `crates/polint/src/eval/observed.rs` | Real-kernel observed item collection | VERIFIED | Uses `AnalysisKernel::run`, provider manifests, metadata debug JSON, relative paths, and runtime budget rows. |
| `tests/eval-fixtures/` | Native fixtures for required areas | VERIFIED | Contains passing kernel, provenance, cache, and extension manifests and tiny repos. |
| `crates/polint/tests/cli.rs` | Public-boundary proof | VERIFIED | `eval_harness_stays_internal` asserts no `polint eval` support and no public eval JSON leak. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `eval/report.rs` | `cache::stable_hash` | Hashes canonical JSON | VERIFIED | Direct `stable_hash(&[canonical_json.as_str()])` call. |
| `eval/model.rs` | serde | `Serialize`/`Deserialize`, snake_case, deny unknown fields where manifests need strictness | VERIFIED | Model and manifest structs derive serde. |
| `eval/fixtures.rs` | `eval/matcher.rs` and `eval/metrics.rs` | Fixture runner calls `match_case` and `compute_metrics` | VERIFIED | Fixture runs build `EvaluationRun` from observed rows, matches, and metrics. |
| `eval/observed.rs` | `analysis_kernel` | Real kernel fixture observation | VERIFIED | Calls `AnalysisKernel::run`, `provider_manifests`, and `metadata_debug_json_for_test`. |
| `eval/fixtures.rs` | cache determinism invariant | Cold/warm/no-cache comparisons gate `cache.current_determinism` | VERIFIED | Invariant emitted only after normalized JSON and hash equality pass. |
| `cli.rs` | public CLI/SDK/runner boundary | Behavioral and structural assertions | VERIFIED | `polint eval` rejected; SDK/runner/CLI surface scanned for eval markers. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `eval/observed.rs` | `observed` rows | `AnalysisKernel::run` output, provider manifests, metadata debug JSON | Yes | FLOWING |
| `eval/fixtures.rs` | `EvaluationRun` | Fixture manifest expected rows + observed kernel/synthetic extension rows | Yes | FLOWING |
| `eval/report.rs` | canonical JSON/hash | Normalized `EvaluationRun` clone | Yes | FLOWING |
| `eval/matcher.rs` | `MatchSummary` rows | Expected/observed item comparisons | Yes | FLOWING |
| `eval/metrics.rs` | `MetricSummary` | `MatchSummary` outcomes | Yes | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Review fix keeps eval report normalization total-order deterministic | `cargo test -p polint --lib eval_report --locked` | 5 passed | PASS |
| Required native fixture categories pass | `cargo test -p polint --lib eval_native_fixture_suite_covers_required_categories --locked` | 1 passed | PASS |
| Public eval surface remains absent | `cargo test -p polint --test cli eval_harness_stays_internal --locked` | 1 passed | PASS |
| Full eval-focused suite | `cargo test -p polint eval_ --locked` | 52 lib eval tests + CLI boundary test passed | PASS |
| Formatting | `cargo fmt --all -- --check` | passed | PASS |

Orchestrator also reported these post-fix gates passed: `cargo test -p polint --lib eval_report --locked`, `cargo test -p polint eval_ --locked`, `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`, and `cargo test --workspace --all-features --locked`.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| SAE-FND-03 | Phase 22 plans 01-06 | polint has an internal evaluation harness MVP with deterministic expected/observed JSON, matchers, metrics, and native fixtures for kernel, provenance, cache, and extension invariants. | SATISFIED | Internal eval modules exist and remain crate-private; fixture suite covers all required areas; focused and orchestrator gates passed. |

No orphaned Phase 22 requirements were found.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `crates/polint/tests/cli.rs` | multiple | `TODO` fixture literals | Info | Existing policy-test sample data, not Phase 22 harness stubs. |
| `tests/eval-fixtures/**/.polint.toml` | multiple | `exclude = []` | Info | Intentional minimal fixture config. |

No blocker anti-patterns, TODO/FIXME placeholders, hollow returns, or user-visible stubs were found in Phase 22 eval implementation files.

### Human Verification Required

None. This phase is internal/test-facing and fully checkable through source inspection and automated tests.

### Gaps Summary

No gaps found. Phase 22 satisfies the roadmap success criteria and SAE-FND-03. The post-review fix commit `ae4b708` is present at HEAD and resolves REVIEW WR-01 by making report item ordering total-order deterministic and adding the regression test that now passes.

---

_Verified: 2026-05-17T18:14:46Z_
_Verifier: Claude (gsd-verifier)_
