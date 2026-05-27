---
phase: 28-private-semantic-mir-and-place-identity
plan: 07
subsystem: static-analysis-engine
tags: [rust, semantic-mir, public-api, cli, sdk, no-leak]

requires:
  - phase: 28-private-semantic-mir-and-place-identity
    provides: private semantic MIR contracts, provider wiring, eval fixtures, and prior boundary proofs from plans 28-01 through 28-06
provides:
  - public CLI/SDK/docs no-leak proof for private semantic MIR and place internals
  - external temp-repo compatibility proof using only polint::sdk::prelude::* and polint::runner::run_cli
  - mixed Go plus TS/JS public check, inspect rule, and polint test JSON regression coverage
  - deterministic private MIR ID offsetting before cross-language merge validation
affects: [phase-28, phase-29-cfg, phase-30-direct-calls, public-api-boundary]

tech-stack:
  added: []
  patterns: [external temp-repo compatibility tests, exact forbidden-marker no-leak scans, private ID remapping before merged validation]

key-files:
  created: []
  modified:
    - crates/polint/tests/cli.rs
    - crates/polint/src/analysis/provider.rs
    - docs/CAPABILITY-FULFILLMENT-RESEARCH.md
    - docs/RULE-AUTHORING-PLATFORM-REVIEW.md
    - docs/roadmap/02_ENTRY_2_CFG_FACTS.md
    - docs/roadmap/05_ENTRY_5_DIRECT_CALL_GRAPH.md

key-decisions:
  - "Keep semantic MIR/place internals out of public check JSON, inspect JSON, polint test JSON, CLI help, SDK, runner, crate-root public exports, README, and docs."
  - "Use an external temp-repo rule that requests only supported public fact views to prove existing rule-author workflows remain compatible."
  - "Offset private MIR/place/unsupported IDs per language output before merge so validation does not cross-wire Go and TS/JS run-local IDs."

patterns-established:
  - "Public-boundary integration tests should assert both machine-output no-leak behavior and source-surface no-leak behavior."
  - "Cross-language private analysis outputs must disambiguate run-local IDs before merged store normalization and validation."

requirements-completed: [SAE-SEM-03]

duration: 11 min
completed: 2026-05-20
---

# Phase 28 Plan 07: Public Boundary Proof Summary

**Semantic MIR/place internals remain private while public rule-author SDK workflows still run through check, inspect, and test JSON.**

## Performance

- **Duration:** 11 min
- **Started:** 2026-05-20T09:12:41Z
- **Completed:** 2026-05-20T09:23:59Z
- **Tasks:** 1
- **Files modified:** 6

## Accomplishments

- Added `semantic_mir_internals_stay_private` in `crates/polint/tests/cli.rs`.
- Built a mixed Go plus TS/JS temp repo whose local rule imports only `polint::sdk::prelude::*`, registers through `polint::runner::run_cli`, requests `ResolvedImports<'_>`, `ModuleGraphFacts<'_>`, `Symbols<'_>`, and `References<'_>`, and emits a diagnostic.
- Asserted `polint check --format json`, `polint inspect rule --format json`, `polint test --format json`, CLI help, SDK, runner, crate-root public section, README, and docs exclude exact semantic MIR/place/CFG/call/dataflow forbidden markers.
- Fixed private semantic MIR provider merging so Go and TS/JS run-local IDs are offset before normalization and validation.
- Reworded public docs to avoid advertising exact reserved CFG and call-graph query spellings before promotion gates.

## Task Commits

1. **Task 1 RED:** `163bebe` test - failing semantic MIR public-boundary test skeleton.
2. **Task 1 GREEN:** `d5bf18e` feat - semantic MIR public no-leak proof, merged-ID fix, and public doc wording cleanup.

## Files Created/Modified

- `crates/polint/tests/cli.rs` - Adds the public no-leak integration test, temp-repo fixture, forbidden-marker assertions, CLI help scans, and source-surface scans.
- `crates/polint/src/analysis/provider.rs` - Offsets private MIR body/place/operation/unsupported IDs and references before merging language outputs.
- `docs/CAPABILITY-FULFILLMENT-RESEARCH.md` - Removes exact reserved CFG/call-graph query spellings from public research wording.
- `docs/RULE-AUTHORING-PLATFORM-REVIEW.md` - Rewords reserved call-relationship fact-view mention without exact unsupported query spelling.
- `docs/roadmap/02_ENTRY_2_CFG_FACTS.md` - Rewords future CFG query examples without exact unsupported query spelling.
- `docs/roadmap/05_ENTRY_5_DIRECT_CALL_GRAPH.md` - Rewords future call-graph query examples without exact unsupported query spelling.

## Decisions Made

- The boundary proof uses existing supported public fact views only; no semantic MIR/place SDK view, CLI command, runner helper, crate-root export, or public JSON section was added.
- The crate-root scan intentionally checks only the public section before `pub(crate) mod analysis;`, preserving private module declarations while guarding exports.
- Documentation remains forward-looking where appropriate but avoids exact public query spellings that would look like promoted contracts.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Offset private semantic MIR IDs before cross-language merge**
- **Found during:** Task 1 GREEN verification
- **Issue:** The mixed Go plus TS/JS temp repo exposed a public `polint/internal` diagnostic containing `MirOperation` because merged language outputs reused run-local body/place/operation IDs, causing validation to associate a TS operation with the wrong owning body/file.
- **Fix:** Added provider-side ID offsetting for bodies, places, operations, unsupported rows, and their internal references before merged normalization.
- **Files modified:** `crates/polint/src/analysis/provider.rs`
- **Verification:** `cargo test -p polint --test cli semantic_mir_internals_stay_private --locked` and `cargo test -p polint --lib semantic_mir_provider --locked` passed.
- **Committed in:** `d5bf18e`

**2. [Rule 1 - Bug] Reworded public docs that matched no-leak query markers**
- **Found during:** Task 1 acceptance scans
- **Issue:** Public docs contained exact reserved `Cfg<'_>` and `CallGraph<'_>` query spellings, failing the plan's source-surface no-leak criteria.
- **Fix:** Reworded those docs to describe future CFG/call-graph fact-view concepts without exact unsupported query spellings.
- **Files modified:** `docs/CAPABILITY-FULFILLMENT-RESEARCH.md`, `docs/RULE-AUTHORING-PLATFORM-REVIEW.md`, `docs/roadmap/02_ENTRY_2_CFG_FACTS.md`, `docs/roadmap/05_ENTRY_5_DIRECT_CALL_GRAPH.md`
- **Verification:** Public-surface `rg` acceptance scan returned no matches.
- **Committed in:** `d5bf18e`

---

**Total deviations:** 2 auto-fixed (2 Rule 1)
**Impact on plan:** Both fixes directly supported the intended public-boundary proof. No public semantic MIR/place/CFG/call/dataflow surface was added.

## Issues Encountered

- The plan referenced `crates/polint/src/runner.rs`, but the current repository stores the runner at `crates/polint/src/runner/mod.rs`; the actual runner module was scanned.
- Initial GREEN verification exposed the merged-ID validation bug before the no-leak proof passed.

## Verification

- `cargo test -p polint --test cli semantic_mir_internals_stay_private --locked` passed.
- `cargo test -p polint --test cli semantic_index_internals_stay_private --locked` passed.
- `cargo test -p polint --test cli module_topology_internals_stay_private --locked` passed.
- `cargo test -p polint --lib semantic_mir_provider --locked` passed.
- `rg -n "semantic_mir_internals_stay_private|polint\\.semantic_mir|semantic-mir-facts|Mir<'_>|Places<'_>|CallGraph<'_>|DataFlow<'_>" crates/polint/tests/cli.rs` returned expected test assertion matches.
- `rg -n "Mir<'_|Places<'_|Cfg<'_|CallGraph<'_|DataFlow<'_|polint mir|semantic-mir-facts|polint\\.semantic_mir" README.md docs crates/polint/src/sdk crates/polint/src/runner crates/polint/src/cli` returned no matches.
- `cargo fmt --all -- --check` passed.

## Known Stubs

None. Stub scan matches were pre-existing fixture `exclude = []` / `rules = []`, placeholder-documentation examples, format strings, and test fixture empty-string comparisons; none block the plan goal.

## Threat Flags

None. This plan added no network endpoint, auth path, new file-access boundary, schema boundary, public CLI command, SDK view, runner helper, or public JSON surface.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 28 public-boundary proof is complete. Phase 29 can build CFG/control-dependence over the private MIR/place substrate with regression coverage proving Phase 28 internals are not public contracts.

## Self-Check: PASSED

- Verified summary file exists.
- Verified modified source/doc files exist.
- Verified task commits `163bebe` and `d5bf18e` exist in git history.
- Verified no plan-blocking stubs remain in modified files.

---
*Phase: 28-private-semantic-mir-and-place-identity*
*Completed: 2026-05-20*
