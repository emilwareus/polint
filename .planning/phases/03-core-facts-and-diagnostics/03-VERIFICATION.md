---
phase: 03-core-facts-and-diagnostics
verified: 2026-04-28T12:11:01Z
status: passed
score: 11/11 must-haves verified
overrides_applied: 0
deferred:
  - truth: "Full TEST-01 coverage beyond Phase 3 core, diagnostics, discovery, runner, and CLI determinism tests"
    addressed_in: "Phases 4-8 and 10"
    evidence: "REQUIREMENTS.md keeps TEST-01 in progress for later Go/TS extraction, rule logic, cache, and release-hardening coverage."
  - truth: "Full TEST-03 snapshot coverage including SARIF-like CI output and broader rule snapshots"
    addressed_in: "Phase 8 and Phase 10"
    evidence: "Phase 3 intentionally covers human and JSON diagnostic snapshots while DIAG-03 and SARIF-like hardening remain mapped to Phase 8."
  - truth: "Full TEST-04 property coverage beyond spans, diagnostic sorting, and discovery include/exclude decisions"
    addressed_in: "Phase 7"
    evidence: "REQUIREMENTS.md keeps TEST-04 in progress for later cache/performance property scope."
---

# Phase 3: Core Facts and Diagnostics Verification Report

**Phase Goal:** Add stable IDs, analysis DB, rule runner, deterministic diagnostics, and SDK-facing primitives.
**Verified:** 2026-04-28T12:11:01Z
**Status:** passed
**Re-verification:** Yes - includes post-review fixes from `03-REVIEW.md`

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Core defines stable typed IDs and models for files, spans, functions, imports, branches, tests, coverage placeholders, and JS/TS fact families. | VERIFIED | `crates/polint-core/src/lib.rs` defines all Phase 3 fact structs and `AnalysisDb` accessors; `analysis_db_exposes_all_phase3_fact_families` passes. |
| 2 | File, function, import, and branch IDs are deterministic by insertion order. | VERIFIED | `analysis_db_assigns_deterministic_ids_and_preserves_shared_source` passes and verifies assigned IDs plus `Arc<str>` source sharing. |
| 3 | Span conversion handles UTF-8, newlines, empty ranges, clamping, and monotonic ranges. | VERIFIED | `span_from_byte_range_handles_utf8_newlines_and_empty_ranges`, `line_col_counts_utf8_boundaries`, and `span_from_byte_range_is_monotonic_for_char_boundaries` pass. |
| 4 | The rule registry exposes capabilities and runner filtering/severity overrides. | VERIFIED | `registry_exposes_capability_declarations` and `run_rules_filters_enabled_patterns_and_applies_severity_override` pass. |
| 5 | Rule errors and panics become controlled internal diagnostics, including metadata panics. | VERIFIED | `run_rules_contains_rule_errors_and_panics` and post-review `run_rules_contains_meta_panics` pass; `Rule::meta()` is inside `catch_unwind`. |
| 6 | Rule execution output is deterministic for sequential and parallel execution. | VERIFIED | `run_rules_parallel_matches_sequential` passes and `run_rules` collects Rayon results in input order before dedupe. |
| 7 | Diagnostics support severity, labels, help, evidence, suggestions, fixes, stable fingerprints, and constructor/fluent helper APIs. | VERIFIED | `diagnostic_builders_cover_labels_suggestions_fixes_evidence_and_help` passes and `Diagnostic` documents constructor-based usage. |
| 8 | Diagnostic fingerprinting, sorting, and dedupe are deterministic and dedupe by global stable fingerprint. | VERIFIED | `fingerprint_includes_rule_file_full_range_and_message`, `sort_diagnostics_is_input_order_independent`, `dedupe_diagnostics_collapses_same_fingerprint_after_sorting`, and post-review `dedupe_diagnostics_removes_non_adjacent_duplicate_fingerprints` pass. |
| 9 | Older serialized diagnostics without additive Phase 3 fields remain readable. | VERIFIED | Post-review `diagnostic_deserializes_missing_phase3_fields_with_computed_fingerprint` passes and recomputes missing fingerprints. |
| 10 | Human and JSON diagnostic rendering are covered by stable snapshots. | VERIFIED | `render_human_snapshot_includes_contract_fields`, `render_json_snapshot_is_stable`, and `render_empty_human_output_is_stable` pass. |
| 11 | File discovery and CLI JSON output are deterministic. | VERIFIED | `discovery_order_is_root_relative_and_stable_with_nested_files`, `load_analysis_files_preserves_discovery_order_in_file_ids`, `discovery_include_exclude_decision_is_stable`, and `check_json_output_is_deterministic_across_repeated_runs` pass. |

**Score:** 11/11 truths verified

### Deferred Items

| # | Item | Addressed In | Evidence |
|---|------|--------------|----------|
| 1 | Full TEST-01 coverage beyond Phase 3 core/diagnostic/discovery scope | Phases 4-8 and 10 | Requirement traceability keeps TEST-01 in progress and later phases own Go/TS extraction, rule logic, cache, and release hardening. |
| 2 | SARIF-like production snapshots and CI output hardening | Phase 8 | DIAG-03 is still pending; Phase 3 only verifies human and JSON snapshots. |
| 3 | Cache/performance property coverage | Phase 7 | TEST-04 remains in progress for later cache/performance invariants. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/polint-core/src/lib.rs` | Stable facts, spans, rule registry, runner containment, deterministic runner output | VERIFIED | Contains Phase 3 fact model, `AnalysisDb`, `RuleCtx`, `RuleRegistry`, `run_rules`, and focused unit/property tests. |
| `crates/polint-diagnostics/src/lib.rs` | Diagnostic contract, rendering, sorting, dedupe, and serde compatibility | VERIFIED | Contains builders, full-range fingerprints, global fingerprint dedupe, human/JSON snapshots, and compatibility deserialization. |
| `crates/polint-fs/src/lib.rs` | Deterministic discovery output and include/exclude decisions | VERIFIED | Sorts normalized relative paths after filtering and has unit/property coverage. |
| `crates/polint-cli/tests/cli.rs` | Repeated CLI JSON determinism integration coverage | VERIFIED | Contains `check_json_output_is_deterministic_across_repeated_runs`, which passed. |
| `.planning/phases/03-core-facts-and-diagnostics/*-SUMMARY.md` | Plan execution records | VERIFIED | All three Phase 3 summaries exist and have `Self-Check: PASSED`. |
| `.planning/phases/03-core-facts-and-diagnostics/03-REVIEW.md` | Advisory code review | VERIFIED | Review found 3 warnings and no critical findings. |
| `.planning/phases/03-core-facts-and-diagnostics/03-REVIEW-FIX.md` | Review fix audit trail | VERIFIED | Records fixes for all 3 warnings with passing targeted tests. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Formatting is clean | `cargo fmt -- --check` | Exit 0 | PASS |
| Clippy is warning-clean | `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 | PASS |
| Workspace tests pass | `cargo test --workspace` | Exit 0 | PASS |
| Schema drift gate | `node /Users/emilwareus/.codex/get-shit-done/bin/gsd-tools.cjs verify schema-drift 03` | `drift_detected: false` | PASS |
| Code review findings fixed | `03-REVIEW-FIX.md` plus targeted tests | 3/3 warnings fixed | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| FS-02 | `03-03-PLAN.md` | File discovery output is deterministic | SATISFIED | Discovery sorting, include/exclude property coverage, and repeated CLI JSON determinism tests pass. |
| CORE-01 | `03-01-PLAN.md`, `03-03-PLAN.md` | Stable IDs and fact models for core analysis data | SATISFIED | Fact structs/accessors and deterministic ID tests pass. |
| CORE-02 | `03-01-PLAN.md`, `03-REVIEW-FIX.md` | Rule registry, capabilities, dedupe, panic containment, deterministic sorting | SATISFIED | Registry, runner, panic/error/meta-panic containment, parallel equivalence, and dedupe tests pass. |
| DIAG-01 | `03-02-PLAN.md`, `03-REVIEW-FIX.md` | Diagnostics support severity, labels, suggestions/fixes, evidence, stable fingerprints, and human output | SATISFIED | Builder tests, fingerprint tests, human/JSON snapshots, and serde compatibility tests pass. |
| TEST-01 | `03-01-PLAN.md`, `03-02-PLAN.md`, `03-03-PLAN.md` | Unit tests cover Phase 3 core, diagnostics, discovery, runner, and CLI determinism scope | SATISFIED FOR PHASE 3 SCOPE | Workspace tests pass; requirement remains in progress for broader later-phase coverage. |
| TEST-03 | `03-02-PLAN.md` | Snapshot tests cover Phase 3 human and JSON diagnostics | SATISFIED FOR PHASE 3 SCOPE | Human and JSON inline snapshots pass; SARIF-like snapshots remain later. |
| TEST-04 | `03-01-PLAN.md`, `03-02-PLAN.md`, `03-03-PLAN.md` | Property tests cover Phase 3 spans, diagnostic sorting, and discovery decisions | SATISFIED FOR PHASE 3 SCOPE | Proptest coverage passes for span monotonicity, sorting determinism, and include/exclude stability. |

### Human Verification Required

None.

### Gaps Summary

No Phase 3 gaps found. The phase goal is achieved on `main`: shared core facts, deterministic in-run IDs, span conversion, rule runner containment, diagnostic identity/rendering, dedupe, serde compatibility, deterministic discovery, and repeated CLI JSON determinism are verified. Broad cross-phase test requirements remain explicitly in progress for later phases.

---

_Verified: 2026-04-28T12:11:01Z_
_Verifier: Codex (inline GSD verifier)_
