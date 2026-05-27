---
phase: 30-direct-call-facts
plan: 05
subsystem: analysis
tags: [rust, analysis-kernel, call-facts, direct-targets, unresolved-calls]

requires:
  - phase: 30-direct-call-facts
    provides: "Plans 30-01 through 30-04 call fact contracts, provider slot, validation/debug scaffolding, MIR call-site extraction, and unresolved-call evidence"
  - phase: 26-semantic-index-deepening
    provides: "semantic symbols, references, imports, and resolution rows consumed by direct target resolution"
  - phase: 27-layered-module-package-topology-graph
    provides: "import-to-package topology rows used as import-binding evidence"
provides:
  - "Crate-private direct call target resolver for lexical, import-binding, constructor, and static/member calls backed by precise semantic references"
  - "Provider output with populated CallTargetFact rows, resolved-site status updates, and explicit unresolved rows for missing semantic references"
  - "Validation coverage for malformed unresolved target identities and digest proof for target algorithm changes"
affects: [analysis, analysis-kernel, direct-calls, summaries, future-refined-call-providers]

tech-stack:
  added: []
  patterns: ["semantic-reference-only direct target resolution", "explicit unresolved rows for non-direct call classes", "target stable keys include provider/schema/model identity"]

key-files:
  created:
    - crates/polint/src/analysis/calls/direct.rs
  modified:
    - crates/polint/src/analysis/calls/mod.rs
    - crates/polint/src/analysis/calls/facts.rs
    - crates/polint/src/analysis/calls/provider.rs
    - crates/polint/src/analysis/calls/unresolved.rs
    - crates/polint/src/analysis/calls/validate.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis_kernel/debug.rs
    - crates/polint/src/analysis_kernel/validation.rs

key-decisions:
  - "Direct targets are emitted only from precise resolved ReferenceFact evidence; dynamic/interface/function-token/framework/value-flow cases remain unresolved or unsupported."
  - "Native direct target rows use NativeDirect provenance and SetupAware precision under the private polint.calls provider."
  - "Provider-derived unresolved rows are filtered off call sites that have a resolved direct target, so precise evidence wins over dynamic-shape uncertainty."

patterns-established:
  - "CallTargetFact stable keys include the call-site stable key, algorithm, target stable key, provider id, schema id, and absent model identity."
  - "Unbound direct-shaped call sites become MissingSemanticReference unresolved rows instead of silent omissions."

requirements-completed: [SAE-SEM-05]

duration: 14 min
completed: 2026-05-21
---

# Phase 30 Plan 05: Direct Call Target Resolution Summary

**Semantic-reference-backed direct, import-binding, and static/member call targets with honest unresolved rows for deferred call graph tiers**

## Performance

- **Duration:** 14 min
- **Started:** 2026-05-21T08:48:20Z
- **Completed:** 2026-05-21T09:02:20Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Added `resolve_direct_call_targets(db, sites)` under private `analysis::calls::direct`, consuming `references()`, `symbols()`, `resolved_imports()`, `semantic_imports()`, and `import_to_package_edges()`.
- Wired the calls provider to publish resolved `CallTargetFact` rows after call-site extraction and before unresolved derivation.
- Preserved explicit unresolved rows for Go function values/interface evidence and TS/JS dynamic/eval/call-apply-bind/framework evidence, without implementing refined call graph algorithms.
- Tightened target validation for unsupported/unresolved target rows that incorrectly carry concrete target identity.

## Task Commits

1. **Task 1 RED:** `ce9cc98` test(30-05): add failing tests for direct call targets
2. **Task 1 GREEN:** `8196b0f` feat(30-05): resolve precise direct call targets
3. **Task 2 RED:** `68f2c06` test(30-05): add failing non-direct call coverage
4. **Task 2 GREEN:** `6ebe74a` fix(30-05): preserve unresolved rows for unbound calls
5. **Task 3 RED:** `fd8ac1e` test(30-05): add failing call target validation test
6. **Task 3 GREEN:** `1dd0208` fix(30-05): validate unresolved call target identity

## Files Created/Modified

- `crates/polint/src/analysis/calls/direct.rs` - Direct target resolver plus focused direct and non-direct tests.
- `crates/polint/src/analysis/calls/mod.rs` - Registered the private direct resolver module.
- `crates/polint/src/analysis/calls/facts.rs` - Added `MethodDirect` edge kind and `NativeDirect` provenance.
- `crates/polint/src/analysis/calls/provider.rs` - Publishes resolved targets, updates resolved site status, filters redundant unresolved rows, and hashes algorithm changes.
- `crates/polint/src/analysis/calls/unresolved.rs` - Emits `MissingSemanticReference` rows for unbound identifier/constructor call sites.
- `crates/polint/src/analysis/calls/validate.rs` - Rejects unresolved/unsupported target rows with concrete target identity.
- `crates/polint/src/core/mod.rs` - Added metadata label support for `MethodDirect`.
- `crates/polint/src/analysis_kernel/debug.rs` - Added debug label support for `MethodDirect` and `NativeDirect`.
- `crates/polint/src/analysis_kernel/validation.rs` - Added kernel validation regression for contradictory unresolved target identity.

## Decisions Made

- Kept all call target work crate-private; no SDK, runner, CLI, README, docs, or public `CallGraph<'_>` surface was promoted.
- Treated semantic references as the only authority for resolved Phase 30 targets. Import/topology rows classify an already-resolved reference as `ImportBinding`; they do not independently invent targets.
- Left Go CHA/RTA/VTA, TS/JS function-token flow, points-to, summaries, framework recognizers, and repo models as deferred unresolved/unsupported evidence.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added missing direct target vocabulary**
- **Found during:** Task 1
- **Issue:** The plan required `NativeDirect` provenance and method-direct edge labeling, but existing call fact enums only had generic `Native` provenance and `Method`.
- **Fix:** Added `CallProvenance::NativeDirect` and `CallEdgeKind::MethodDirect` with metadata/debug label support.
- **Files modified:** `crates/polint/src/analysis/calls/facts.rs`, `crates/polint/src/core/mod.rs`, `crates/polint/src/analysis_kernel/debug.rs`
- **Verification:** `cargo test -p polint --lib analysis::calls::direct --locked`
- **Committed in:** `8196b0f`

**2. [Rule 2 - Missing Critical] Prevented silent omissions for unbound direct-shaped calls**
- **Found during:** Task 2
- **Issue:** Identifier and constructor call sites without semantic references were not resolved, but also did not produce unresolved rows.
- **Fix:** Emitted `MissingSemanticReference` unresolved rows for those direct-shaped sites.
- **Files modified:** `crates/polint/src/analysis/calls/unresolved.rs`
- **Verification:** `cargo test -p polint --lib analysis::calls::unresolved --locked`
- **Committed in:** `6ebe74a`

---

**Total deviations:** 2 auto-fixed (Rule 2).
**Impact on plan:** Both changes were required to satisfy the plan's truthfulness and target identity requirements. No public surface or refined provider was added.

## Issues Encountered

- Existing store and validation scaffolding already covered most D-10 indexes and malformed target rows from prior plans, so Task 3 focused on the remaining contradictory unresolved-target identity gap and provider digest algorithm coverage.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib analysis::calls::direct --locked`
- `cargo test -p polint --lib analysis::calls::unresolved --locked`
- `cargo test -p polint --lib analysis_kernel::validation::calls --locked`
- `cargo test -p polint --lib calls_provider --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None.

## Threat Flags

None - semantic-reference-to-target and unresolved-dynamic-evidence trust boundaries were covered by the plan threat model.

## Next Phase Readiness

Phase 30 can now add eval/no-leak/final proof plans on top of populated direct target rows while public whole-program call graph views remain unsupported.

## Self-Check: PASSED

- Verified created/modified key files exist.
- Verified all task commit hashes exist in git history.
- Verified summary file exists on disk.

---
*Phase: 30-direct-call-facts*
*Completed: 2026-05-21*
