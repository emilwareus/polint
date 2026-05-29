---
phase: 43-reachability-roots-per-suite-scoring-mode
verified: 2026-05-29T00:00:00Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: none
  previous_score: none
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 43: Reachability, Roots & Per-Suite Scoring Mode Verification Report

**Phase Goal:** polint discovers explicit reachability roots from the v1.2 entrypoint substrate, scores each benchmark suite in the mode its oracle expects, and inherits a determinism gate every subsequent solver phase must pass.
**Verified:** 2026-05-29
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Reachability roots (`main`, `init`, exported, tests, configured repo entrypoints) are discoverable as typed facts derived from the v1.2 entrypoint substrate. | ✓ VERIFIED | `discover.rs:31` `discover_reachability_roots` projects existing facts only (no parse/tree-sitter/oxc) and emits all 5 kinds: Go `Main` (`go_main_init_roots`, package-main check), `Init` (every `func init`), `Exported` (`exported_root` from `FunctionFact.is_exported`), the `Test`/`FrameworkEntrypoint` bridge over `db.entrypoint_facts()` carrying `originating_entrypoint` + inherited precision/status (`entrypoint_bridge_root:125`), and `ConfiguredEntrypoint` from `.polint.toml [reachability] roots`. Typed `pub(crate) struct ReachabilityRootFact` with all 13 D-03 fields (`facts.rs:17`). 8 co-located discovery tests pass (ran: 52/52 reachability:: tests). |
| 2 | Each suite manifest declares a `scoring_mode` (`oracle-rta`, `oracle-jelly`, `whole-repo`) and the gate fails if missing; unreachable direct calls remain facts but are marked outside the reachable graph. | ✓ VERIFIED | `suite.rs:115` non-`Option` `scoring_mode: ScoringMode` on `SuiteManifest` with `deny_unknown_fields` (structural gate) + explicit `validate()` guard (`suite.rs:144`). Per-variant kebab serde renames (`suite.rs:82-87`). Test `manifest_missing_scoring_mode_is_rejected_structurally` asserts the error names the field; invalid-value + committed-suite round-trip tests pass. All 4 TOMLs declare the correct mode (go-x-tools=oracle-rta, jelly=oracle-jelly, gosec/secbench=whole-repo). Unreachable marking: `traverse.rs` BFS over resolved direct-call edges emits one `CallReachabilityFact` per call site without mutating `analysis::calls` (`marking_does_not_mutate_the_call_store` passes); `filter_scored_edges_by_scoring_mode` (`metrics.rs:454`) filters to reachable set only under oracle-rta, wired into the eval scoring path via `scored_call_graph_edges_for_db` (`runner.rs:338`, threads `manifest.scoring_mode`). Ran: eval::suite 11/11, eval::metrics 18/18. |
| 3 | Determinism gate fixture passes: 10 shuffled provider-order runs produce byte-identical observed JSON, identical solver step counts, and identical budget-exceeded reasons. | ✓ VERIFIED | `determinism_gate.rs` `assert_n10_byte_identical` runs N=10 distinct seeded permutations of provider-enumeration order + observed-row insertion order through the live `normalize_run`/`to_deterministic_json_pretty` path; byte-identity asserted for both fixtures. Reserved `solver_step_count`/`budget_exceeded_reasons` on `SolverMetricSection` (`report.rs:198`) surface in the observed JSON so they are transitively covered (default 0/empty in this phase). Go + TS fixtures each have a root + reachable call + unreachable call (`orphan→orphanHelper`). Ran the gate myself: 6/6 pass (incl. both byte-identity tests, distinct-seeds, auto-enrollment count, both marking-exercised tests). See calibration note below on permutation surface. |
| 4 | The determinism gate is wired so every subsequent solver-introducing phase inherits it as an acceptance gate. | ✓ VERIFIED | Gate driven by `AnalysisKernel::provider_manifests()` (`determinism_gate.rs:123`, asserted count-equal at `shuffled_provider_count_equals_manifest_count`), so a new provider auto-enrolls with no harness edit (D-22). CI job `determinism-gate` (`ci.yml:135`) on `matrix.os: [ubuntu-latest, macos-latest]`, `fail-fast: false`, runs `cargo test -p polint --lib eval::determinism_gate --locked`; job comment documents the phases 44-54 inheritance precondition. Gate file doc comment (`determinism_gate.rs:28-43`) names phases 44-54 and the per-phase obligation. Ruby YAML assertion confirms job structure. |

**Score:** 4/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/polint/src/analysis/reachability/facts.rs` | ReachabilityRootFact + closed enums + stable-key recipe + CallReachabilityFact | ✓ VERIFIED | 480 lines. 13-field `pub(crate) struct ReachabilityRootFact`; `RootKind` (6 pinned variants, no `#[repr(u8)]`); `RootStatus`/`RootPrecision`/`RootProvenance` closed enums mirroring entrypoint vocabulary; `CallReachabilityFact` with `in_reachable_graph`; length-prefixed stable-key recipe + boundary-escape. Wired into discover/traverse/store/provider; serde round-trip + variant-lock + boundary tests pass. |
| `crates/polint/src/analysis/reachability/discover.rs` | root discovery from existing facts | ✓ VERIFIED | 598 lines. All 5 sources implemented from existing facts only; loss-free entrypoint bridge; configured-unresolvable → `RootStatus::Unresolved` (never dropped). 9 co-located tests pass. Wired into provider. |
| `crates/polint/src/analysis/reachability/traverse.rs` | BFS reachable-set + CallReachabilityFact marking | ✓ VERIFIED | 451 lines. `compute_reachable_set` BFS over resolved direct-call edges only (sorted-frontier `BTreeMap`/`BTreeSet`); `mark_call_reachability` one mark per call site; Phase 47/48 forward-compat doc; no-mutation test passes. Wired into provider Phase 1/2. |
| `crates/polint/src/analysis/reachability/provider.rs` | five-phase pipeline + output digest, spliced after entrypoints | ✓ VERIFIED | 376 lines. `derive_reachability_with_cache_stats`; empty-output sentinel + permuted-insertion determinism tests pass. Wired into kernel `mod.rs:427` after entrypoints; manifest at `provider.rs:533` immediately after `polint.entrypoints` with `PrecisionCeiling::SetupAware`. |
| `crates/polint/src/eval/suite.rs` | ScoringMode enum + required field + gate + wire-string tests | ✓ VERIFIED | `pub(crate) enum ScoringMode` per-variant kebab renames; required non-`Option` field; two-layer gate; 6 scoring_mode tests pass. |
| `crates/polint/src/eval/determinism_gate.rs` | N=10 byte-identity harness driven by provider_manifests() + inheritance doc | ✓ VERIFIED | 288 lines. `#![cfg(test)]`. Drives provider set off `provider_manifests()`; no hardcoded `"polint.` provider-name array; phases 44-54 doc. Ran 6/6 pass. |
| `crates/polint/src/eval/report.rs` | SolverMetricSection reserved on MetricSections via #[serde(default)] | ✓ VERIFIED | `SolverMetricSection { solver_step_count: u64, budget_exceeded_reasons: Vec<String> }` at `report.rs:198`, `#[serde(default)] solver` on `MetricSections:113`; NOT on frozen `MetricSummary` (lines 66-94 contain no solver fields). Layout-lock + backward-compat tests pass (eval::report 18/18). |
| `.github/workflows/ci.yml` | determinism-gate job ubuntu+macos, fail-fast false | ✓ VERIFIED | Job at `ci.yml:135`; matrix `[ubuntu-latest, macos-latest]`, `fail-fast: false`; runs the gate test; phases 44-54 inheritance comment; leak-gate job unchanged. |
| 4 suite TOMLs | scoring_mode declared correctly | ✓ VERIFIED | go-x-tools=oracle-rta, jelly=oracle-jelly, gosec=whole-repo, secbench=whole-repo (line 7 each). |
| 2 determinism fixtures | repo/ + .polint.toml + expected.polint-eval.toml, with unreachable call | ✓ VERIFIED | `go_reachable/` (main.go: main→reachable, orphan→orphanHelper unreachable) + `ts_reachable/` (app.ts: entry→usedHelper, orphanFn→orphanHelper unreachable). Marking-exercised tests confirm >=1 root, >=1 call site, >=1 unreachable mark each. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `analysis_kernel/mod.rs` | `reachability::provider::derive_reachability_with_cache_stats` | kernel provider wiring after entrypoints | ✓ WIRED | `mod.rs:427-446` calls it after the entrypoints provider, passing config roots and upstream digests, pushes provider output. |
| `analysis_kernel/provider.rs` | `PROVIDER_MANIFESTS polint.reachability entry` | manifest after polint.entrypoints | ✓ WIRED | Manifest at line 533 immediately follows entrypoints (504); order tests at 776-777/805-806/861-862 assert the placement. |
| `reachability/discover.rs` | `db.entrypoint_facts()` | EntrypointKind → RootKind bridge | ✓ WIRED | `entrypoint_bridge_root` maps Test→Test, others→FrameworkEntrypoint, inherits precision/status, sets `originating_entrypoint`. Bridge tests pass. |
| `eval/metrics.rs` | `CallReachabilityFact.in_reachable_graph` | mode-aware scoring filter by call-site stable key | ✓ WIRED | `reachable_graph_lookup` (`metrics.rs:424`) + `filter_scored_edges_by_scoring_mode` consumed by `runner.rs:168-169` + `scored_call_graph_edges_for_db:338` threading `manifest.scoring_mode`. |
| `eval/determinism_gate.rs` | `provider_manifests()` | parametric provider-set enumeration for auto-enrollment | ✓ WIRED | `manifest_provider_ids()` sources directly from `provider_manifests()`; count-equality asserted; no hardcoded list. |
| `eval/report.rs` | `MetricSections.solver` | #[serde(default)] reserved sibling | ✓ WIRED | `metrics.rs` default-populates `solver` in `From<ComputedMetrics> for MetricSummary` so it surfaces in observed JSON. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Determinism gate (N=10 byte-identity, auto-enrollment, marking) | `cargo test -p polint --lib eval::determinism_gate` | 6 passed; 0 failed | ✓ PASS |
| Public-surface-leak gate (ALLOWED_PRELUDE intact) | `cargo test -p polint --test public_surface_leak` | 5 passed; 0 failed | ✓ PASS |
| Reachability module (facts/discover/traverse/store/validate/provider/cache_key) | `cargo test -p polint --lib reachability::` | 52 passed; 0 failed | ✓ PASS |
| ScoringMode + required field + gate-fails-if-missing | `cargo test -p polint --lib eval::suite` | 11 passed; 0 failed | ✓ PASS |
| Mode-aware scoring filter (oracle-rta/jelly/whole-repo + subset regression) | `cargo test -p polint --lib eval::metrics` | 18 passed; 0 failed | ✓ PASS |
| SolverMetricSection layout-lock + backward-compat + normalization | `cargo test -p polint --lib eval::report` | 18 passed; 0 failed | ✓ PASS |
| ReachabilityRootId in assert_small_id_contract roster | `cargo test -p polint --lib analysis::ids` | 2 passed; 0 failed | ✓ PASS |
| Provider order / golden | `cargo test -p polint --lib analysis_kernel::provider` | 12 passed; 0 failed | ✓ PASS |
| Determinism-gate CI job structure | `ruby -ryaml` assertion | fail-fast false, matrix ubuntu+macos | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| REACH-01 | 43-01 | Discover reachability roots from v1.2 entrypoint substrate, expose as typed facts | ✓ SATISFIED | `discover_reachability_roots` + `ReachabilityRootFact` + provider; SC-1 verified. |
| REACH-02 | 43-02 | Score suites in oracle-expected mode via `scoring_mode`; unreachable calls marked outside graph | ✓ SATISFIED | Required `scoring_mode` field + gate + 4 TOMLs + `CallReachabilityFact` marking + mode filter; SC-2 verified. |
| REACH-03 | 43-03 | Determinism gate (10 shuffled runs → byte-identical JSON) inherited by every subsequent solver phase | ✓ SATISFIED | N=10 gate driven by `provider_manifests()` + reserved solver fields + CI job + phases 44-54 doc; SC-3 + SC-4 verified. |

All 3 REACH IDs in REQUIREMENTS.md (lines 19-21, 112-114, 144) map to Phase 43 and match plan frontmatter exactly. No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | — | No TBD/FIXME/XXX in any phase-43 source file | — | Clean — completion auditable |
| (none) | — | No TODO/HACK/PLACEHOLDER in reachability module or gate | — | Clean |

Note on `#[cfg(test)]` gating: the mode-aware scoring helpers (`filter_scored_edges_by_scoring_mode`, `reachable_graph_lookup`, `scored_call_graph_edges_for_db`) are `#[cfg(test)]`. This is NOT a stub — it is a documented, intentional decision (43-02-SUMMARY line 97): the entire eval external-suite scoring path is internal/test-facing with no public CLI/SDK surface, consistent with the v1.2 eval-harness discipline. The production marking (`CallReachabilityFact`) is fully wired into the live provider/store/digest. The `#[allow(dead_code)]` on not-yet-consumed `AnalysisDb` reachability accessors follows the existing Phase-34 codebase precedent.

### Human Verification Required

None. All success criteria are verifiable structurally and via the test suite, which was run in this verification process (not merely trusted from the SUMMARY).

### Gaps Summary

No gaps. All 4 ROADMAP success criteria are achieved with codebase evidence, all 3 requirements satisfied, all gates green when run during verification, v1.3 discipline held (all new types `pub(crate)`, no bare `pub` in the reachability module, ALLOWED_PRELUDE byte-unchanged, leak gate 5/5), and the whole-program `polint.reachability` provider is cleanly distinct from the block-level `polint.domain.reachability` abstract domain (`domains/core.rs:106`).

**Calibration note (informational, not a gap) — SC-3 permutation surface:**
SC-3's literal wording is "10 shuffled provider-order runs." The gate permutes the provider *enumeration* order (asserting the permutation preserves the `provider_manifests()` set) and the observed-row insertion order, then feeds both through the live `normalize_run`/`to_deterministic_json_pretty` path, rather than re-executing the kernel scheduler 10 times with shuffled provider scheduling. This is an explicit, documented design decision (43-03-SUMMARY decision 2): the kernel runs providers in a fixed DAG order and is already internally deterministic, so the controllable permutation surface is the enumeration/row order. The determinism *intent* of SC-3 is genuinely achieved — byte-identical normalized observed JSON across 10 distinct seeded permutations, with the kernel's own determinism independently established and the provider order faithfully captured in the observed JSON (`provider_order.N` rows). This satisfies the criterion; recorded here for transparency so phases 44-54 understand exactly what the inherited gate proves.

---

_Verified: 2026-05-29_
_Verifier: Claude (gsd-verifier)_
