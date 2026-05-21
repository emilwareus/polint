---
phase: 31-p0-abstract-domain-kernel
plan: 05
subsystem: eval
tags: [rust, abstract-interpretation, eval-fixtures, public-api, cli-tests]

requires:
  - phase: 31-p0-abstract-domain-kernel
    provides: abstract-domain provider ordering, validation, and debug JSON shape
provides:
  - Internal eval observation for abstract-domain facts, events, counts, and index counts
  - Native mixed Go/TS abstract-domain eval fixture covering P0 slots and uncertainty statuses
  - Public no-leak proof that abstract-domain internals stay out of CLI, SDK, runner, README, and docs/facts surfaces
affects: [eval, analysis-domains, analysis-kernel, cli-public-boundary]

tech-stack:
  added: []
  patterns: [TDD eval fixture expansion, private provider debug normalization, public boundary scans]

key-files:
  created:
    - tests/eval-fixtures/abstract-domains/core/expected.polint-eval.toml
    - tests/eval-fixtures/abstract-domains/core/repo/.polint.toml
    - tests/eval-fixtures/abstract-domains/core/repo/domain.go
    - tests/eval-fixtures/abstract-domains/core/repo/go.mod
    - tests/eval-fixtures/abstract-domains/core/repo/web/package.json
    - tests/eval-fixtures/abstract-domains/core/repo/web/tsconfig.json
    - tests/eval-fixtures/abstract-domains/core/repo/web/src/domain.ts
  modified:
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/mod.rs
    - crates/polint/src/analysis/domains/store.rs
    - crates/polint/src/analysis/domains/provider.rs
    - crates/polint/tests/cli.rs

key-decisions:
  - "Abstract-domain facts remain internal eval/debug evidence, not SDK or CLI contract."
  - "Deterministic top and budget fixture rows use private test-only solver policies rather than changing production solver defaults."
  - "Transient domain place IDs are retained in stable keys but not exposed as invalid indexed references."

patterns-established:
  - "Eval debug normalization emits compact abstract-domain fact rows plus count/index invariants."
  - "Public boundary tests use external temp repos that import only polint::sdk::prelude::* and register through polint::runner::run_cli."

requirements-completed: [SAE-INT-01]

duration: 43min
completed: 2026-05-21
---

# Phase 31 Plan 05: Domain Eval Fixtures And Public Boundary Proof Summary

**Internal abstract-domain eval fixtures with deterministic top/unknown/budget evidence and public CLI/SDK no-leak proof**

## Performance

- **Duration:** 43 min
- **Started:** 2026-05-21T12:03:59Z
- **Completed:** 2026-05-21T12:47:26Z
- **Tasks:** 3
- **Files modified:** 19

## Accomplishments

- Added abstract-domain fact families, fixture area parsing, debug-row normalization, and unknown-like metric handling.
- Added a mixed Go/TS native eval fixture for P0 slots, top/unknown/unsupported/budget evidence, count/index invariants, and determinism.
- Added a public no-leak CLI test proving abstract-domain internals do not appear in check JSON, inspect/test output, CLI help, public SDK/runner sources, README, or docs/facts.
- Fixed validation leaks so propagated abstract-domain uncertainty does not surface as public `polint/internal` diagnostics.

## Task Commits

1. **Task 1: Add abstract-domain eval observation**
   - `a86e278` test(31-05): add failing abstract-domain eval rows
   - `4f34467` feat(31-05): observe abstract-domain eval rows
2. **Task 2: Add native abstract-domain fixture**
   - `27d02d8` test(31-05): add failing abstract-domain fixture tests
   - `dfad141` feat(31-05): add abstract-domain eval fixture
3. **Task 3: Prove public no-leak boundary**
   - `525d7c7` test(31-05): add failing abstract-domain no-leak test
   - `6be15eb` fix(31-05): keep abstract-domain internals private

Additional verification fixes:
- `3748d4b` fix(31-05): align provider order assertions
- `34c7669` fix(31-05): sanitize propagated unresolved-call domain rows

## Files Created/Modified

- `crates/polint/src/eval/model.rs` - Added abstract-domain fixture area and fact-family vocabulary.
- `crates/polint/src/eval/observed.rs` - Normalizes abstract-domain debug JSON into compact eval facts and invariants.
- `crates/polint/src/eval/fixtures.rs` - Adds the abstract-domain fixture runner and deterministic test-policy observation path.
- `crates/polint/src/eval/mod.rs` - Adds focused eval tests for parsing, normalization, and uncertainty metrics.
- `crates/polint/src/eval/{matcher,metrics,report}.rs` - Accounts for top and budget-exceeded statuses.
- `crates/polint/src/analysis/domains/{provider,store}.rs` - Prevents transient domain state from producing invalid public validation diagnostics.
- `crates/polint/src/analysis_kernel/{mod.rs,incremental/run_report.rs}` - Updates provider-order expectations for `polint.abstract_domains`.
- `crates/polint/tests/cli.rs` - Adds external public-boundary no-leak proof.
- `tests/eval-fixtures/abstract-domains/core/` - New mixed Go/TS native fixture and expected eval manifest.

## Decisions Made

- Kept abstract-domain facts private: eval can consume debug evidence, but no public SDK fact view or CLI output contract was introduced.
- Used test-only solver policies in the fixture runner to force top and budget rows deterministically without changing production defaults.
- Used a TS-only public-boundary temp repo for Task 3 because Task 2 already proves mixed Go/TS abstract-domain behavior; the boundary test’s job is API privacy.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added deterministic test-policy domain observations**
- **Found during:** Task 2
- **Issue:** The production deterministic solver policy does not reliably emit top and budget-exceeded rows in a small fixture.
- **Fix:** Added private test-only solver-policy observation runs for low widening fuel and zero iteration budget.
- **Files modified:** `crates/polint/src/eval/fixtures.rs`
- **Verification:** `cargo test -p polint --lib eval::fixtures::abstract_domains_core --locked`
- **Committed in:** `dfad141`

**2. [Rule 1 - Bug] Prevented dangling transient place references from leaking as public diagnostics**
- **Found during:** Task 3 public no-leak verification
- **Issue:** Abstract-domain block snapshots could carry transient place IDs not present in persisted MIR places, producing `polint/internal` diagnostics in public CLI JSON.
- **Fix:** Provider-backed domain output filters place references against persisted MIR places while retaining stable-key identity.
- **Files modified:** `crates/polint/src/analysis/domains/store.rs`, `crates/polint/src/analysis/domains/provider.rs`, `crates/polint/src/eval/fixtures.rs`
- **Verification:** `cargo test -p polint --test cli abstract_domain_internals_stay_private --locked`
- **Committed in:** `6be15eb`

**3. [Rule 1 - Bug] Updated stale provider-order assertions**
- **Found during:** Overall verification
- **Issue:** Kernel run-report tests still expected the pre-abstract-domain provider list.
- **Fix:** Added `polint.abstract_domains` to manifest-order expectations between calls and metrics.
- **Files modified:** `crates/polint/src/analysis_kernel/mod.rs`, `crates/polint/src/analysis_kernel/incremental/run_report.rs`
- **Verification:** Focused provider-order tests and full `cargo test -p polint --all-targets --locked`
- **Committed in:** `3748d4b`

**4. [Rule 1 - Bug] Sanitized propagated unresolved-call top reasons**
- **Found during:** Overall verification
- **Issue:** Block/function snapshots could retain an `unresolved_call` reason without a MIR operation reference, causing public validation diagnostics.
- **Fix:** Operation-less snapshots now downgrade propagated unresolved-call top reasons to generic unknown evidence.
- **Files modified:** `crates/polint/src/analysis/domains/store.rs`
- **Verification:** `cargo test -p polint --test cli syntax_cache_ignores_unrelated_rule_edits --locked`, `cargo test -p polint --test cli checked_in_examples_are_runnable_cli_fixtures --locked`, and full all-targets.
- **Committed in:** `34c7669`

---

**Total deviations:** 4 auto-fixed (3 Rule 1 bugs, 1 Rule 2 missing critical)
**Impact on plan:** All fixes were required for deterministic eval proof and public-boundary correctness. No public API expansion was introduced.

## Known Stubs

None. Stub scan only found intentional TODO strings in existing CLI test fixtures.

## Issues Encountered

- Full verification initially failed on stale provider-order expectations and abstract-domain validation diagnostics in public CLI tests; both were fixed and reverified.

## Verification

- `cargo test -p polint --lib eval::abstract_domain_rows --locked`
- `cargo test -p polint --lib eval::fixtures::abstract_domains_core --locked`
- `cargo test -p polint --test cli abstract_domain_internals_stay_private --locked`
- `cargo test -p polint --test cli direct_calls_internals_stay_private --locked`
- `cargo test -p polint --test cli cfg_public_no_leak --locked`
- `cargo test -p polint --test cli semantic_mir_internals_stay_private --locked`
- `cargo test -p polint --lib analysis::domains --locked`
- `cargo test -p polint --all-targets --locked`
- `cargo fmt --all -- --check`

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 31 now has internal eval coverage and a public privacy proof for abstract-domain internals. Later extension or SDK-promotion phases can use this summary to distinguish private debug/eval evidence from any future public fact-view design.

## Self-Check: PASSED

- Summary file exists.
- Key created files exist.
- All task and auto-fix commits are present in git history.

---
*Phase: 31-p0-abstract-domain-kernel*
*Completed: 2026-05-21*
