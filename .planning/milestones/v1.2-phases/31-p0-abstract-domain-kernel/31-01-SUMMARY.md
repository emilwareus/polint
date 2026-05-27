---
phase: 31-p0-abstract-domain-kernel
plan: 01
subsystem: analysis
tags: [rust, abstract-interpretation, lattice, domains, deterministic-state]

requires:
  - phase: 28-private-semantic-mir-and-place-identity
    provides: Private semantic MIR and PlaceId handles consumed by later domain solving.
  - phase: 29-local-cfg-and-control-dependence
    provides: Private CFG/control-flow substrate for later local solving.
provides:
  - Crate-private abstract domain contracts with top reasons, widening hooks, and stable digest parts.
  - P0 core domain slots for reachability, nilness/nullishness, truthiness, constants, strings, and initializedness.
  - Deterministic ProductState and CoreDomains containers over BTreeMap-backed place slots.
affects: [phase-31, phase-32, phase-33, analysis-kernel, abstract-domains]

tech-stack:
  added: []
  patterns:
    - Crate-private domain traits and product state under analysis::domains.
    - BTreeMap/BTreeSet-backed deterministic state and digest parts.

key-files:
  created:
    - crates/polint/src/analysis/domains/mod.rs
    - crates/polint/src/analysis/domains/lattice.rs
    - crates/polint/src/analysis/domains/core.rs
    - crates/polint/src/analysis/domains/state.rs
  modified:
    - crates/polint/src/analysis/mod.rs

key-decisions:
  - "Keep all abstract-domain contracts and P0 slots crate-private under analysis::domains with no SDK, runner, CLI, README, or docs/facts promotion."
  - "Represent top/unknown causes as private TopReason labels that participate in stable digest parts."
  - "Use BTreeMap/BTreeSet ordering for deterministic product state and literal-set digest behavior."

patterns-established:
  - "AbstractDomain: bottom/top/leq/join/join_into/widen/stable_digest_parts as the crate-private lattice contract."
  - "ProductState: CoreDomains plus private zero-sized extension marker, with value-only bounded reductions."

requirements-completed: [SAE-INT-01]

duration: 8 min
completed: 2026-05-21
---

# Phase 31 Plan 01: Private Domain Contracts And Law Tests Summary

**Crate-private P0 abstract-domain vocabulary with law-tested lattice contracts, finite core domains, and deterministic product state**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-21T10:55:09Z
- **Completed:** 2026-05-21T11:03:43Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `analysis::domains` as a crate-private module only.
- Defined `AbstractDomain`, `Changed`, `TopReason`, `WidenSite`, `WidenFuel`, and deterministic digest-part helpers.
- Implemented six P0 domain slots: reachability, nilness/nullishness, truthiness, constants, strings, and initializedness.
- Added deterministic `CoreDomains` and `ProductState` with BTreeMap-backed place slots, product join/widening, stable digest parts, and bounded value-only reductions.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add private lattice contracts** - `e2f2198` (test), `5c02b21` (feat)
2. **Task 2: Implement P0 core domain slots** - `d75bb97` (test), `2c06933` (feat)
3. **Task 3: Add deterministic product state** - `26048c9` (test), `e82f72b` (feat)

**Plan verification formatting:** `cb9ad95` (style)

## Files Created/Modified

- `crates/polint/src/analysis/mod.rs` - Registers the private `domains` module.
- `crates/polint/src/analysis/domains/mod.rs` - Registers lattice, core, and state modules.
- `crates/polint/src/analysis/domains/lattice.rs` - Private lattice contract, top reasons, widening hooks, and lattice tests.
- `crates/polint/src/analysis/domains/core.rs` - P0 finite domains, capped literal sets, widening behavior, and law-focused tests.
- `crates/polint/src/analysis/domains/state.rs` - Deterministic product state, BTreeMap place slots, joins, widening, digest parts, and reductions.

## Decisions Made

- Domain internals remain crate-private and are not exported through `lib.rs`, SDK, runner, CLI, README, or `docs/facts`.
- Top reasons are part of private domain values and stable digest parts so future diagnostics can explain precision loss without claiming exact coverage.
- Literal sets are capped and deterministic; widening over cap produces `TopReason::Widened`.
- Extension slots are represented only by a private zero-sized marker with an empty digest component in this plan.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed finite-domain join match coverage**
- **Found during:** Task 2 (Implement P0 core domain slots)
- **Issue:** Initial finite-domain `join` implementations had guarded equality arms that did not satisfy Rust exhaustiveness checking, and the lattice sample test had a redundant top pattern.
- **Fix:** Added explicit same-variant arms for finite domains and tightened the lattice sample top-reason comparison.
- **Files modified:** `crates/polint/src/analysis/domains/core.rs`, `crates/polint/src/analysis/domains/lattice.rs`
- **Verification:** `cargo test -p polint --lib analysis::domains::core --locked`
- **Committed in:** `2c06933`

**2. [Rule 3 - Blocking] Applied rustfmt after plan-level formatting check**
- **Found during:** Final verification
- **Issue:** `cargo fmt --all -- --check` reported formatting diffs in `core.rs` and `state.rs`.
- **Fix:** Ran `cargo fmt --all` and committed the mechanical formatting output.
- **Files modified:** `crates/polint/src/analysis/domains/core.rs`, `crates/polint/src/analysis/domains/state.rs`
- **Verification:** `cargo fmt --all -- --check`
- **Committed in:** `cb9ad95`

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking)
**Impact on plan:** Both fixes were scoped to correctness and verification. No public surface or architectural scope changed.

## Issues Encountered

None beyond the auto-fixed implementation and formatting issues listed above.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib analysis::domains --locked` - passed
- `cargo fmt --all -- --check` - passed
- `rg -n "pub mod domains|pub use crate::analysis::domains|Nilness&lt;'_|Constants&lt;|Truthiness&lt;'" crates/polint/src/lib.rs crates/polint/src/sdk crates/polint/src/runner README.md docs/facts || true` - no matches

## Known Stubs

None. Stub scan found no placeholder data or unwired UI/data surfaces. One regex hit was a false positive inside a `format!` string in `state.rs`.

## Next Phase Readiness

Ready for Plan 31-02 to build transfer and solver behavior over these private domain contracts.

## Self-Check: PASSED

- Created files exist on disk.
- Task and verification commits exist in git history.

---
*Phase: 31-p0-abstract-domain-kernel*
*Completed: 2026-05-21*
