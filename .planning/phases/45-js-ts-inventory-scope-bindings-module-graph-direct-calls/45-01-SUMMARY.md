---
phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls
plan: 01
subsystem: analysis
tags: [rust, oxc, typescript, javascript, inventory, spans]

requires:
  - phase: 42
    provides: identity substrate and Jelly span renderer discipline
provides:
  - Private TS/JS inventory fact model for functions and callsites
  - Oxc-based inventory extraction for required JS/TS function forms
  - Oxc-based inventory extraction for required JS/TS callsite forms
  - Deterministic TS inventory normalization and lookup store
affects: [phase-45, js-ts-analysis, semantic-graph, jelly-eval]

tech-stack:
  added: []
  patterns: [private Oxc AST inventory, stable-key-before-dense-id normalization]

key-files:
  created:
    - crates/polint/src/ts/inventory/mod.rs
    - crates/polint/src/ts/inventory/facts.rs
    - crates/polint/src/ts/inventory/extract.rs
    - crates/polint/src/ts/inventory/store.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/ts/mod.rs

key-decisions:
  - "Kept TS inventory crate-private under ts::inventory; no SDK, runner, CLI, or public fact-view export was added."
  - "Used Oxc parser/semantic AST nodes for inventory traversal and crate Span byte conversion; no Jelly string rendering is done in inventory."
  - "Did not add persistent AnalysisDb storage in Plan 01; later Phase 45 plans can consume private TsInventoryOutput/TsInventoryStore directly before deciding whether DB/cache persistence is needed."

patterns-established:
  - "Stable keys include file, Oxc byte span, syntactic kind, lexical parent key, display name, and status where relevant."
  - "Dense TS inventory IDs are assigned only after stable-key sort in TsInventoryOutput::normalized()."
  - "Dynamic/unsupported callsite forms carry explicit TsInventoryStatus rows instead of fabricated targets."

requirements-completed: [JS-01]

duration: 20 min
completed: 2026-05-31
---

# Phase 45 Plan 01: JS/TS Inventory Summary

**Private Oxc-based TS/JS inventory layer for function and callsite facts with deterministic stable keys and dense IDs**

## Performance

- **Duration:** 20 min
- **Started:** 2026-05-31T18:05:48Z
- **Completed:** 2026-05-31T18:26:00Z
- **Tasks:** 4
- **Files modified:** 6

## Accomplishments

- Added crate-private TS inventory ID newtypes plus function/callsite fact rows and closed kind/status enums.
- Implemented Oxc AST inventory extraction for declarations, function expressions, arrows, methods, constructors, accessors, class static blocks, calls, new expressions, tagged templates, optional calls, dynamic imports, and require calls.
- Added deterministic normalization and store indexes by file, stable key, function kind, and callsite kind.
- Added focused inventory tests covering required function forms, callsite forms, unresolved non-string dynamic import behavior, span sanity, stable-key ordering, and dense ID assignment.

## Task Commits

1. **Task 1: Add private inventory fact model and IDs** - `8f160ca4` (`feat`)
2. **Task 2: Implement Oxc inventory extraction for function forms** - `bfb4c24b` (`feat`)
3. **Task 3: Implement Oxc inventory extraction for callsite forms** - `9d3ced46` (`feat`)
4. **Task 4: Normalize/store inventory output and bridge to existing DB lifecycle** - `b0626568` (`feat`)

## Files Created/Modified

- `crates/polint/src/analysis/ids.rs` - Added crate-private dense ID newtypes for TS inventory function and callsite rows.
- `crates/polint/src/ts/mod.rs` - Registered `pub(crate) mod inventory`.
- `crates/polint/src/ts/inventory/facts.rs` - Added private function/callsite fact model and kind/status enums.
- `crates/polint/src/ts/inventory/extract.rs` - Added Oxc parser/semantic AST inventory extraction and stable-key construction.
- `crates/polint/src/ts/inventory/store.rs` - Added normalized output and deterministic lookup store.
- `crates/polint/src/ts/inventory/mod.rs` - Added inventory modules and focused tests.

## Decisions Made

- No public API promotion: inventory remains private implementation detail for Phase 45 consumers.
- No persistent `AnalysisDb` storage yet: this slice exposes normalized private output/store and leaves DB/cache lifecycle decisions to later binding/graph integration once the consumption path is concrete.
- Stable-key contents favor source truth over generated labels: file identity, Oxc byte span, syntactic kind, lexical parent key, display name, and status are used; benchmark/Jelly expected labels are not referenced.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Split invalid multi-filter Cargo command**
- **Found during:** Task 1 verification
- **Issue:** `cargo test -p polint --lib ts::inventory::facts analysis::ids` is not valid Cargo syntax because Cargo accepts only one test-name filter before `--`.
- **Fix:** Ran the equivalent focused checks as separate commands.
- **Files modified:** None
- **Verification:** `cargo test -p polint --lib ts::inventory::facts` and `cargo test -p polint --lib analysis::ids` passed.
- **Committed in:** N/A

---

**Total deviations:** 1 auto-fixed (blocking command syntax)
**Impact on plan:** Verification coverage was preserved; no implementation scope changed.

## Issues Encountered

- The staged private model initially triggered `dead_code` in the pre-commit lint before extraction/store consumers existed. The inventory modules carry scoped internal `dead_code` allowances with reasons until later Phase 45 plans wire the rows into downstream consumers.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib ts::inventory::facts` - passed
- `cargo test -p polint --lib analysis::ids` - passed
- `cargo test -p polint --lib ts::inventory::extract_function_forms` - passed
- `cargo test -p polint --lib ts::inventory::extract_callsite_forms` - passed
- `cargo test -p polint --lib ts::inventory` - passed

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 45-02 can build the private TS scope/binding layer on top of Oxc semantic/scoping data and can consume `TsInventoryOutput`/`TsInventoryStore` without public API changes. The remaining known follow-up is to remove or narrow temporary internal dead-code allowances once later plans make the inventory rows part of the active graph pipeline.

## Self-Check: PASSED

All plan acceptance criteria were verified. The only command deviation was Cargo filter syntax, handled by equivalent focused test invocations.

---
*Phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls*
*Completed: 2026-05-31*
