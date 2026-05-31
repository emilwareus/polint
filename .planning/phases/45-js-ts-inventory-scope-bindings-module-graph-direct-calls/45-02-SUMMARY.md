---
phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls
plan: 02
subsystem: analysis
tags: [rust, oxc, typescript, javascript, scope, bindings]

requires:
  - phase: 45-01
    provides: private JS/TS inventory rows and deterministic stable keys
provides:
  - Private TS/JS scope and binding fact model
  - Oxc semantic/scoping extraction for lexical scopes and bindings
  - Deterministic scope/binding store with lookup indexes
  - Boundary tests keeping token/property/prototype/this behavior unresolved
affects: [phase-45, js-ts-analysis, direct-binding, semantic-graph]

tech-stack:
  added: []
  patterns: [private Oxc semantic extraction, explicit dynamic-boundary statuses]

key-files:
  created:
    - crates/polint/src/ts/scope/mod.rs
    - crates/polint/src/ts/scope/facts.rs
    - crates/polint/src/ts/scope/extract.rs
    - crates/polint/src/ts/scope/store.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/ts/mod.rs

key-decisions:
  - "Kept TS scope/binding facts crate-private under ts::scope with no SDK, runner, or CLI exposure."
  - "Used Oxc SemanticBuilder/Scoping for scope and symbol rows; AST fallback rows are limited to import/export specifier shape, direct aliases, and destructuring gaps."
  - "Recorded parameter callbacks, computed members, prototype dispatch, and this-dependent calls as unsupported dynamic boundary rows instead of implementing later solver behavior."

patterns-established:
  - "Scope and binding dense IDs are assigned after stable-key sort in TsScopeOutput::normalized()."
  - "TsScopeStore indexes by file, stable key, binding name, scope/name, kind, module/imported name, and exported name."
  - "Boundary rows use TsBindingStatus::UnsupportedDynamic with named reasons."

requirements-completed: [JS-02]

duration: 14 min
completed: 2026-05-31
---

# Phase 45 Plan 02: JS/TS Scope And Binding Summary

**Private Oxc semantic scope and binding layer for JS/TS lexical declarations, imports, aliases, and dynamic-boundary statuses**

## Performance

- **Duration:** 14 min
- **Started:** 2026-05-31T18:32:32Z
- **Completed:** 2026-05-31T18:45:55Z
- **Tasks:** 4
- **Files modified:** 6

## Accomplishments

- Added crate-private TS scope and binding IDs, fact rows, kind enums, import/export shape enum, and binding statuses.
- Implemented Oxc `SemanticBuilder`/`Scoping` extraction for scopes and semantic symbols, with documented AST fallbacks for import/export aliases, local aliases, and destructuring rows.
- Added deterministic scope store indexes needed by the next direct-binding plan.
- Added boundary tests proving callback parameters, computed property calls, prototype dispatch, and `this` calls remain unsupported dynamic rows.

## Task Commits

1. **Task 1: Define scope and binding fact model** - `29a60d83` (`feat`)
2. **Task 2: Extract scopes and bindings from Oxc semantic data** - `31de109a` (`feat`)
3. **Task 3: Add normalized scope store and binding lookup indexes** - `93aa3a3e` (`feat`)
4. **Task 4: Preserve direct-binding boundary with unresolved statuses** - `6daa4734` (`test`)

## Files Created/Modified

- `crates/polint/src/analysis/ids.rs` - Added crate-private dense IDs for TS scope and binding rows.
- `crates/polint/src/ts/mod.rs` - Registered `pub(crate) mod scope`.
- `crates/polint/src/ts/scope/facts.rs` - Added scope/binding fact model and status vocabulary.
- `crates/polint/src/ts/scope/extract.rs` - Added Oxc semantic extraction, documented AST fallbacks, and dynamic boundary rows.
- `crates/polint/src/ts/scope/store.rs` - Added normalized output and deterministic lookup indexes.
- `crates/polint/src/ts/scope/mod.rs` - Added module wiring and direct-binding boundary tests.

## Decisions Made

- No public API promotion: scope and binding rows remain private implementation detail for Phase 45.
- AST fallbacks are deliberately narrow: they cover rows Oxc scoping does not expose directly as separate normalized policy rows, not alternate parsing.
- Direct binding remains narrow: no token propagation, property flow, prototype modeling, or receiver/`this` solving was added.

## Deviations from Plan

None - plan executed as written.

## Issues Encountered

- Pre-commit clippy flagged a manual flatten loop and a wide helper signature. The loop was refactored; the import-row helper carries a targeted `#[expect(clippy::too_many_arguments)]` because it mirrors normalized binding fact dimensions.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib ts::scope::facts` - passed
- `cargo test -p polint --lib ts::scope::extract` - passed
- `cargo test -p polint --lib ts::scope` - passed
- `cargo test -p polint --lib ts::scope::direct_binding_boundary` - passed
- `rg -n "token-set propagation|prototype walk|object allocation solving" crates/polint/src/ts/scope` - no matches

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 45-03 can bridge `ts::inventory` and `ts::scope` into direct local/import binding rows. The lookup store now has scope/name and module/imported-name helpers for that work.

## Self-Check: PASSED

All scope/binding tasks and acceptance checks passed.

---
*Phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls*
*Completed: 2026-05-31*
