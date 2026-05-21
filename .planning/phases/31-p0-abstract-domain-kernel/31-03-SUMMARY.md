---
phase: 31-p0-abstract-domain-kernel
plan: 03
subsystem: analysis
tags: [rust, abstract-interpretation, provider, metadata, cache-identity]

requires:
  - phase: 31-p0-abstract-domain-kernel
    provides: Private abstract-domain contracts, solver, transfer, and result cursors from Plans 31-01 and 31-02.
  - phase: 30-direct-call-facts
    provides: Private direct and unresolved call facts used by conservative domain transfer.
provides:
  - Stored private domain observation and event rows with status, precision, stable keys, and metadata.
  - Private `polint.abstract_domains` provider running after calls and before metrics.
  - Abstract-domain provider parameter, output, and layer-cache identity vocabulary.
affects: [phase-31, phase-32, phase-33, analysis-kernel, abstract-domains]

tech-stack:
  added: []
  patterns:
    - Crate-private fact rows with AnalysisDb replacement and metadata refresh.
    - Provider output digests over normalized rows plus upstream/lifecycle/future absent inputs.

key-files:
  created:
    - crates/polint/src/analysis/domains/facts.rs
    - crates/polint/src/analysis/domains/store.rs
    - crates/polint/src/analysis/domains/provider.rs
    - crates/polint/src/analysis/domains/cache_key.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/analysis/domains/mod.rs
    - crates/polint/src/analysis/domains/lattice.rs
    - crates/polint/src/analysis/domains/solver.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs

key-decisions:
  - "Keep domain facts, provider, store, and cache identity crate-private with no SDK, runner, CLI, README, or docs/facts promotion."
  - "Normalize domain facts into observation rows and event rows with explicit status and precision labels, including top/unknown/setup/budget cases."
  - "Make abstract-domain cache identity include provider policy, MIR, CFG, calls, symbol graph, module topology, syntax, lifecycle/config, and absent extension/model/toolchain slots."

patterns-established:
  - "DomainOutput::from_results converts local solver cursors into deterministic store rows."
  - "polint.abstract_domains provider output digest uses normalized row stable keys/status/precision/value plus upstream digest components."

requirements-completed: [SAE-INT-01]

duration: 16 min
completed: 2026-05-21
---

# Phase 31 Plan 03: Domain Facts Store Metadata Provider Cache Identity Summary

**Private abstract-domain facts persisted in AnalysisDb with provider metadata and deterministic cache identity**

## Performance

- **Duration:** 16 min
- **Started:** 2026-05-21T11:24:06Z
- **Completed:** 2026-05-21T11:39:58Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments

- Added dense run-local domain IDs plus private domain observation/event fact rows.
- Added deterministic `DomainOutput` normalization, `DomainStore` indexes, `AnalysisDb::replace_abstract_domain_facts`, accessors, and metadata refresh under `polint.abstract_domains`.
- Added provider parameter digest, provider output digest, manifest schema/order, kernel run-report wiring, and `LayerKey::abstract_domains_layer_key`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Store domain observation facts with metadata** - `9c83155` (test), `c9c8b7f` (feat)
2. **Task 2: Wire provider and cache identity** - `5619d23` (test), `881f202` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/domains/facts.rs` - Domain slots, locations, values, statuses, precision labels, observation facts, and event facts.
- `crates/polint/src/analysis/domains/store.rs` - Domain output normalization, result materialization, store indexes, and storage tests.
- `crates/polint/src/analysis/domains/provider.rs` - Private provider execution, output digest, manifest/order/layer-key tests.
- `crates/polint/src/analysis/domains/cache_key.rs` - Domain policy/provider parameter digest.
- `crates/polint/src/core/mod.rs` - AnalysisDb storage, accessors, and metadata mapping for domain facts.
- `crates/polint/src/analysis_kernel/provider.rs` - `polint.abstract_domains` manifest after calls and before metrics.
- `crates/polint/src/analysis_kernel/mod.rs` - Kernel execution and run-report wiring for the new provider.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - `LayerKind::AbstractDomains` and layer key identity.

## Decisions Made

- Domain internals remain private implementation detail; no rule-author SDK, CLI, docs, README, or public JSON surface was promoted.
- Domain observations and precision-loss events are stored separately so later debug/eval work can inspect normalized rows without treating diagnostic events as slot values.
- Cache identity includes explicit absent future extension, model, and toolchain slots now, before those inputs exist.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Removed stale dead-code lint expectations after provider wiring**
- **Found during:** Task 2 (Wire provider and cache identity)
- **Issue:** Once the abstract-domain provider consumed the solver and lattice modules, two earlier `dead_code` expectations became unfulfilled lint expectations.
- **Fix:** Removed the stale module-level expectations and the now-unused solver `BlockState` helper.
- **Files modified:** `crates/polint/src/analysis/domains/lattice.rs`, `crates/polint/src/analysis/domains/solver.rs`
- **Verification:** `cargo test -p polint --lib abstract_domains_provider --locked`
- **Committed in:** `881f202`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The fix was caused by this provider wiring and removed obsolete warning scaffolding only. No public surface or architectural scope changed.

## Issues Encountered

None beyond the auto-fixed stale lint expectations listed above.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib abstract_domains_provider --locked` - passed
- `cargo test -p polint --lib abstract_domains_layer_key --locked` - passed
- `cargo test -p polint --lib abstract_domain_fact_storage --locked` - passed
- `cargo test -p polint --lib abstract_domain_fact_metadata --locked` - passed
- `cargo fmt --all -- --check` - passed

## Known Stubs

None. Stub scan hits were false positives in existing test fixture source strings and a test name containing "placeholder".

## Next Phase Readiness

Ready for Plan 31-04 to add validation, debug, eval observation, and fixture proof over the stored abstract-domain provider rows.

## Self-Check: PASSED

- Created files exist on disk.
- Task commits exist in git history.

---
*Phase: 31-p0-abstract-domain-kernel*
*Completed: 2026-05-21*
