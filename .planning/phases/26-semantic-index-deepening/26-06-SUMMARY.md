---
phase: 26-semantic-index-deepening
plan: 06
subsystem: analysis-kernel
tags: [rust, semantic-index, eval, fixtures, public-boundary]

requires:
  - phase: 26-01
    provides: Internal semantic fact contracts, AnalysisDb storage, and metadata rows
  - phase: 26-04
    provides: Test-only semantic debug JSON with semantic row arrays
  - phase: 26-05
    provides: Semantic-aware symbol graph layer cache persistence and warm restore
provides:
  - Internal eval observation and matching for semantic rows and unknown semantic statuses
  - Native semantic-index eval fixture covering imports, exports, aliases, generated rows, unknowns, references, and cache reuse
  - Public compatibility proof for check, inspect rule, and test JSON no-leak behavior
affects: [semantic-index, eval-harness, symbol-reference-docs, public-cli-compatibility]

tech-stack:
  added: []
  patterns:
    - Semantic debug rows are normalized into internal eval ObservedFact rows with payload evidence
    - Native semantic-index fixtures use tagged cold/warm layer-cache invariants for symbol graph reuse
    - Public no-leak tests exercise external temp-repo SDK rules across check, inspect, and test

key-files:
  created:
    - tests/eval-fixtures/semantic-index/core/expected.polint-eval.toml
    - tests/eval-fixtures/semantic-index/core/repo/.polint.toml
    - tests/eval-fixtures/semantic-index/core/repo/src/app.ts
    - tests/eval-fixtures/semantic-index/core/repo/src/lib.ts
    - tests/eval-fixtures/semantic-index/core/repo/go.mod
    - tests/eval-fixtures/semantic-index/core/repo/service.go
  modified:
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/matcher.rs
    - crates/polint/src/eval/metrics.rs
    - crates/polint/src/eval/report.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/tests/cli.rs
    - docs/facts/symbols-and-references.md

key-decisions:
  - "Keep semantic eval support crate-private/test-facing; no public eval CLI, SDK view, or generic semantic graph API was added."
  - "Represent semantic unknown statuses explicitly in eval reports so ambiguous, unresolved, dynamic, external, cycle, generated, setup-missing, and unsupported rows count as unknown evidence."
  - "Document only existing Symbols<'_> and References<'_> behavior; scopes/import closure/resolution-step rows remain internal."

patterns-established:
  - "Semantic eval fixtures can use area = \"semantic-index\" and run cold/warm symbol graph cache proof through run_semantic_index_core_fixture_for_test."
  - "Public semantic no-leak tests should cover check, inspect rule, and polint test JSON together for temp-repo SDK consumers."

requirements-completed: [SAE-SEM-01]

duration: 17min
completed: 2026-05-19
---

# Phase 26 Plan 06: Semantic Index Evaluation And Public Boundary Summary

**Internal semantic eval rows, native semantic-index fixture coverage, and public no-leak proof for existing symbol/reference surfaces**

## Performance

- **Duration:** 17 min
- **Started:** 2026-05-19T08:18:36Z
- **Completed:** 2026-05-19T08:35:23Z
- **Tasks:** 3
- **Files modified:** 14

## Accomplishments

- Extended the crate-private eval harness to observe semantic debug rows, carry semantic payload evidence, and classify semantic unknown statuses in matcher/metrics/report output.
- Added `semantic-index-core`, a native eval fixture with TS/JS and Go source covering dynamic imports, CommonJS, aliases, exports, generated rows, ambiguous/unresolved references, shadowing, and symbol graph cold/warm cache reuse.
- Added a public compatibility test proving external rules using `polint::sdk::prelude::*`, `Symbols<'_>`, `References<'_>`, and `polint::runner::run_cli` still work across `check`, `inspect rule`, and `test` JSON without leaking semantic internals.
- Updated public symbol/reference docs only for the existing supported fact views and their precision/status limits.

## Task Commits

Each task was committed atomically:

1. **Task 1: Teach internal eval to observe semantic rows** - `ec34414` (test), `d904bcf` (feat)
2. **Task 2: Add semantic-index native fixture coverage** - `c060e07` (test), `fd56ceb` (feat)
3. **Task 3: Prove public compatibility and update supported fact docs** - `bd1fa64` (test), `4a7dfae` (docs)
4. **Formatting cleanup** - `36ec26b` (refactor)

_Note: TDD tasks have separate red test and green implementation commits._

## Files Created/Modified

- `crates/polint/src/eval/model.rs` - Added semantic fact family vocabulary, `semantic-index` fixture area, observed payloads, and semantic status variants.
- `crates/polint/src/eval/observed.rs` - Harvests semantic debug arrays into observed fact rows with producer, precision, status, path/span payload evidence.
- `crates/polint/src/eval/matcher.rs`, `metrics.rs`, `report.rs` - Match, count, and serialize semantic unknown/resolved statuses deterministically.
- `crates/polint/src/eval/fixtures.rs` - Adds the semantic-index core fixture runner and focused fixture tests.
- `tests/eval-fixtures/semantic-index/core/` - Adds the TS/JS and Go fixture repo plus expected semantic/cache rows.
- `crates/polint/tests/cli.rs` - Adds `semantic_index_internals_stay_private` public no-leak integration coverage.
- `docs/facts/symbols-and-references.md` - Clarifies bounded precision/status evidence for existing public symbol/reference facts.

## Decisions Made

- Kept all semantic row observation and fixture machinery internal to eval/test code; no public semantic command, schema, SDK view, or graph API was promoted.
- Treated `Resolved` as a distinct eval status label while counting it as present facts; unknown-like semantic statuses count through the existing unknown metric path.
- Used partial stable-key assertions in the semantic-index fixture where provider stable keys intentionally include deterministic but verbose semantic identity material.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added semantic status accounting outside the originally listed eval files**
- **Found during:** Task 1 (Teach internal eval to observe semantic rows)
- **Issue:** Adding semantic statuses to `ObservedStatus` required metrics and report serialization updates, otherwise the crate would not compile and unknown-rate accounting would be incomplete.
- **Fix:** Updated eval metrics and report status handling for `Resolved`, `Unresolved`, `Ambiguous`, `Dynamic`, `External`, `Cycle`, and `Generated`.
- **Files modified:** `crates/polint/src/eval/metrics.rs`, `crates/polint/src/eval/report.rs`
- **Verification:** `cargo test -p polint --lib eval::semantic_rows --locked`
- **Committed in:** `d904bcf`

**2. [Rule 3 - Blocking] Added a semantic-index fixture runner and area**
- **Found during:** Task 2 (Add semantic-index native fixture coverage)
- **Issue:** The plan required `area = "semantic-index"` plus cold/warm cache invariants, but the fixture model did not have that area or a runner that combined semantic observations with tagged layer-cache counters.
- **Fix:** Added `FixtureArea::SemanticIndex` and `run_semantic_index_core_fixture_for_test`.
- **Files modified:** `crates/polint/src/eval/model.rs`, `crates/polint/src/eval/fixtures.rs`
- **Verification:** `cargo test -p polint --lib eval::fixtures::semantic_index_core --locked`
- **Committed in:** `fd56ceb`

---

**Total deviations:** 2 auto-fixed (1 missing critical, 1 blocking)
**Impact on plan:** Both were required to make the planned eval fixture and semantic status assertions executable. No public semantic surface was added.

## Issues Encountered

- The TS dynamic import fixture had to place `import(dynamicPath)` at top level because the current TS semantic collector observes top-level expression/variable initializers for dynamic import rows. The fixture still covers dynamic import behavior without changing provider architecture.
- CLI test runs emit a pre-existing `dead_code` warning for `SemanticIndexBuilder::add_generated_symbol`; verification passed and the warning was not introduced by this plan.

## Known Stubs

None.

## Threat Flags

None - no network endpoints, auth paths, or public schema/export surfaces were introduced. The planned public JSON trust boundary is covered by `semantic_index_internals_stay_private`.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib eval::semantic_rows --locked`
- `cargo test -p polint --lib eval::fixtures::semantic_index_core --locked`
- `cargo test -p polint --test cli semantic_index_internals_stay_private --locked`
- `cargo test -p polint --test cli inspect_rule_manifest_json_is_stable_for_local_rules --locked`
- `cargo test -p polint --test cli polint_test_runs_temp_repo_fixtures --locked`
- `cargo fmt --all -- --check`

## Next Phase Readiness

Phase 26 now has semantic fact contracts, language-owned semantic row producers, validation/debug output, cache persistence, internal fixture coverage, and public-boundary no-leak proof. This closes `SAE-SEM-01` and leaves Phase 27 free to build topology/module/package graph work without widening semantic-index public APIs.

## Self-Check: PASSED

- Created summary and semantic-index fixture files exist.
- Task commits found: `ec34414`, `d904bcf`, `c060e07`, `fd56ceb`, `bd1fa64`, `4a7dfae`, `36ec26b`.
