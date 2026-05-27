---
phase: 30-direct-call-facts
verified: 2026-05-21T10:28:12Z
status: passed
score: 12/12 must-haves verified
overrides_applied: 0
---

# Phase 30: Direct Call Facts Verification Report

**Phase Goal:** Add direct call-site, target, unresolved-call, direct/static resolution, call indexes, and debug snapshots.
**Verified:** 2026-05-21T10:28:12Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Fixtures cover direct functions, methods, constructors, member calls, function values as unresolved/unknown, unsupported dynamic calls, and precise statuses. | VERIFIED | `tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml` expects `CallSite`, `CallTarget`, and `UnresolvedCall` rows plus direct/reference/import/member/constructor/unresolved/setup-missing statuses; fixture source covers Go and TS/JS taxonomy. Focused fixture test passed. |
| 2 | Direct call facts consume semantic references where available. | VERIFIED | `resolve_direct_call_targets` indexes `references()` and `symbols()`, requires precise resolved references, and emits `CallTargetFact` rows with direct/import/constructor/static/member algorithms. |
| 3 | Public `CallGraph<'_>` remains unsupported until promotion gates justify it. | VERIFIED | `analysis_plan::support_for("call_graph")` returns unsupported with roadmap docs; `CallGraph<'_>` is an inert reserved SDK view with `_db` only. CLI capability test passed and rule did not execute. |
| 4 | Debug snapshots are internal or preview-gated. | VERIFIED | `CallDebugReport` is in `analysis_kernel::debug` test-only debug path and serializes stable keys, relative path/span, counts, and index counts only; no public docs or CLI surface exists. |
| 5 | Crate-private direct-call fact contracts exist for sites, targets, unresolved evidence, IDs, stable keys, statuses, algorithms, reasons, and provenance. | VERIFIED | `analysis::calls` is registered as `pub(crate)`; `CallSiteFact`, `CallTargetFact`, `UnresolvedCallFact`, and `CallTargetId` are crate-private with separate dense IDs and stable keys. |
| 6 | Call storage exposes deterministic D-10 indexes for callers, sites, targets, outgoing/incoming function/symbol, and unresolved reason/status. | VERIFIED | `CallStore` uses `BTreeMap` indexes for all required dimensions and rejects target/unresolved rows whose sites are missing. |
| 7 | The private `polint.calls` provider runs after CFG and before metrics with deterministic output and layer-key identity. | VERIFIED | Kernel invokes `derive_calls_with_cache_stats` after `polint.cfg` and before metrics; manifest declares `calls-facts-1`, outputs, and provider-order fixture includes `provider_order.8 = "polint.calls"`. |
| 8 | Validation rejects malformed call rows deterministically. | VERIFIED | `validate_calls` checks dangling file/function/body/op/place/symbol/site references, duplicate stable keys, invalid spans, contradictory statuses, missing reasons, target identity on unresolved rows, and exact precision violations. |
| 9 | Every MIR call operation becomes a normalized call-site fact without parser AST dependency. | VERIFIED | `extract_call_sites` iterates `db.mir_operations()`, selects `MirOperationKind::Call`, maps MIR/place evidence, builds deterministic stable keys, and tests deterministic ordering. |
| 10 | Function-value, dynamic, setup-missing, and unsupported call shapes produce first-class unresolved evidence. | VERIFIED | `derive_unresolved_calls` emits specific reasons including `FunctionValue`, `DynamicProperty`, `Eval`, `CallApplyBind`, `DynamicImport`, `Reflection`, `GoroutineBoundary`, `SetupMissing`, and `FrameworkDispatch`. |
| 11 | Only direct/binding/static resolution emits resolved targets in Phase 30; refined providers remain deferred. | VERIFIED | Direct resolver emits from precise semantic references only; tests prove non-direct Go/TS cases do not emit direct targets. Later refined providers are explicitly Phase 37. |
| 12 | Internal eval observes direct-call rows, counts unknown-like statuses, and proves cold/warm/no-cache determinism plus index coverage. | VERIFIED | `call_facts_for_test` reads debug `calls` rows and count/index sections; direct-calls fixture runner compares cold/warm/no-cache output and expected invariants name all D-10 indexes. |

**Score:** 12/12 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/polint/src/analysis/calls/facts.rs` | Private call fact contracts and vocabulary | VERIFIED | Exists, substantive, crate-private structs/enums found. |
| `crates/polint/src/analysis/calls/store.rs` | Normalized call output and deterministic indexes | VERIFIED | Exists, substantive, uses `BTreeMap`, validates dangling sites, exposes all D-10 accessors. |
| `crates/polint/src/core/mod.rs` | AnalysisDb call storage, replacement, accessors, metadata | VERIFIED | `replace_call_facts`, call row storage, index helpers, and metadata refresh exist. Warning remains for dead-code accessor methods, but tests pass and this is not a goal blocker. |
| `crates/polint/src/analysis/calls/provider.rs` | Private provider and output digest | VERIFIED | Extracts sites, resolves targets, derives unresolved rows, stores output, hashes stable payloads. |
| `crates/polint/src/analysis/calls/cache_key.rs` | Calls layer-key and parameter vocabulary | VERIFIED | Provider parameters include schema and direct-call tier labels; layer-key hook verified. |
| `crates/polint/src/analysis/calls/validate.rs` | Call fact validation | VERIFIED | Malformed rows and precision-ceiling checks implemented and tested. |
| `crates/polint/src/analysis_kernel/debug.rs` | Test-only call debug rows/counts | VERIFIED | `CallDebugReport` includes sites, targets, unresolved, `index_counts`, and counts by language/kind/algorithm/status/reason/provider. |
| `crates/polint/src/analysis/calls/extract.rs` | MIR call operation extraction | VERIFIED | Consumes MIR operations and places, no parser AST dependency. |
| `crates/polint/src/analysis/calls/unresolved.rs` | Unresolved-call evidence derivation | VERIFIED | Consumes call shape and `UnsupportedDomain::Calls` evidence. |
| `crates/polint/src/analysis/calls/direct.rs` | Direct target resolver | VERIFIED | Consumes semantic references/symbols and semantic import evidence; emits only precise direct target rows. |
| `crates/polint/src/eval/observed.rs` | Call debug JSON to eval observation | VERIFIED | `call_facts_for_test` maps sites, targets, unresolved, counts, and index counts. |
| `crates/polint/src/eval/metrics.rs` | Unknown-like status accounting | VERIFIED | Mechanical artifact grep missed literal `setup_missing`, but source accounts for `ObservedStatus::SetupMissing` with unresolved/unsupported unknown-like statuses and tests prove behavior. |
| `crates/polint/src/eval/fixtures.rs` | Direct-calls native fixture runner | VERIFIED | `run_direct_calls_core_fixture_for_test` runs cold/warm/no-cache and emits determinism invariant. |
| `tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml` | Expected direct-call fact, count, and index coverage | VERIFIED | Contains required fact families, statuses, algorithms, determinism, count, and D-10 index invariants. |
| `crates/polint/tests/cli.rs` | Public no-leak and unsupported capability tests | VERIFIED | `direct_calls_internals_stay_private` and `call_graph_capability_remains_unsupported` passed. |
| `crates/polint/src/analysis_plan.rs` | Reserved unsupported `call_graph` planning | VERIFIED | `call_graph` remains unsupported with roadmap docs path. |
| `crates/polint/src/sdk/facts.rs` | Reserved inert `CallGraph<'_>` view | VERIFIED | `CallGraph<'_>` has `_db` only and no query methods. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `analysis_kernel::run` | `analysis::calls::provider` | `derive_calls_with_cache_stats` after CFG before metrics | WIRED | Kernel calls provider after `polint.cfg` output and records `polint.calls` provider output. |
| `analysis::calls::provider` | `extract`, `direct`, `unresolved`, `CallStore` | provider data flow | WIRED | Provider extracts sites, resolves targets, filters unresolved rows for resolved sites, normalizes, digests, and stores. |
| `analysis::calls::direct` | semantic references/symbols/imports | direct target evidence | WIRED | Resolver consumes `references()`, `symbols()`, and semantic imports; exact import-to-package lookup is not required for the Phase 30 roadmap contract because semantic reference evidence drives direct targets. |
| `analysis_kernel::validation` | `analysis::calls::validate` | validation hook | WIRED | `validate_fact_metadata` calls `validate_calls`. |
| `analysis_kernel::debug` | call store rows/indexes | safe debug snapshots | WIRED | Debug reads call rows and emits rows/counts/index counts. |
| `eval::observed` | debug calls JSON | call fact observation | WIRED | `call_facts_for_test` reads `calls.sites`, `calls.targets`, `calls.unresolved`, counts, and index counts. |
| CLI tests | SDK and analysis plan | public no-leak/unsupported proof | WIRED | Integration tests scan public surfaces and exercise unsupported `CallGraph<'_>` request. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `analysis::calls::provider` | `CallOutput { sites, targets, unresolved }` | `extract_call_sites(db)`, `resolve_direct_call_targets(db, &sites)`, `derive_unresolved_calls(db, &sites)` | Yes | FLOWING |
| `analysis::calls::extract` | `CallSiteFact` rows | Stored semantic MIR bodies/operations/places | Yes | FLOWING |
| `analysis::calls::direct` | `CallTargetFact` rows | Semantic `ReferenceFact` and `SymbolFact` evidence plus semantic import labels | Yes | FLOWING |
| `analysis::calls::unresolved` | `UnresolvedCallFact` rows | Call-site callee shapes and `UnsupportedDomain::Calls` rows | Yes | FLOWING |
| `analysis_kernel::debug` | `CallDebugReport` | Stored call rows and `CallStore` indexes | Yes | FLOWING |
| `eval::observed` | Eval `ObservedItem`s | Test-only debug `calls` JSON | Yes | FLOWING |
| `eval::fixtures` | Direct-call fixture observed rows | Real fixture repo analyzed by `AnalysisKernel` with symbols/references/imports/module graph requested | Yes | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Call internals and unit tests work | `cargo test -p polint --lib analysis::calls --locked` | 31 passed | PASS |
| Direct-call fixture covers taxonomy, statuses, counts, indexes, determinism | `cargo test -p polint --lib eval::fixtures::direct_calls_core --locked` | 5 passed | PASS |
| Public direct-call internals do not leak | `cargo test -p polint --test cli direct_calls_internals_stay_private --locked` | 1 passed; existing dead-code warning only | PASS |
| Public `call_graph` remains unsupported | `cargo test -p polint --test cli call_graph_capability_remains_unsupported --locked` | 1 passed; existing dead-code warning only | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| SAE-SEM-05 | 30-01 through 30-08 | polint records direct call-site, direct target, and unresolved-call facts with call indexes and debug snapshots while keeping public whole-program call graph views unsupported. | SATISFIED | Private call fact contracts, provider, indexes, validation/debug, direct target resolver, unresolved rows, eval fixture, and no-leak/call_graph unsupported tests are present and passing. |

No orphaned Phase 30 requirements found. Later Phase 37 covers refined call graph providers, and Phase 41 covers public SDK promotion; those are intentionally outside Phase 30.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `crates/polint/src/analysis/calls/provider.rs` | 478 | `"placeholder"` in a test fixture helper | Info | Test-only fallback label; does not flow to production output. |
| `crates/polint/tests/cli.rs` | multiple | `TODO` literals in CLI fixture tests | Info | Existing test data for rule/baseline behavior, not direct-call stubs. |
| `crates/polint/src/core/mod.rs` | 1020+ | dead-code warning for call-store accessor methods | Info | Current phase intentionally builds private substrate for later consumers; focused and full regression gates passed. |

### Human Verification Required

None. This phase is internal Rust analysis/eval behavior with deterministic tests and source-level public-boundary checks; no visual, external-service, or manual UX behavior is required.

### Gaps Summary

No blocking gaps found. The phase delivers private direct-call facts, direct target resolution from precise semantic references, explicit unresolved evidence, deterministic indexes and debug/eval snapshots, native fixture coverage, and public no-leak/unsupported `CallGraph<'_>` proof.

---

_Verified: 2026-05-21T10:28:12Z_
_Verifier: Codex (gsd-verifier)_
