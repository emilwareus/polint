---
phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls
plan: 03
subsystem: analysis
tags: [rust, oxc, typescript, javascript, direct-bindings, module-graph]

requires:
  - phase: 45-01
    provides: private JS/TS inventory rows and deterministic stable keys
  - phase: 45-02
    provides: private JS/TS scope and binding rows
provides:
  - Private TS direct binding fact and unresolved reason model
  - Static local, alias, namespace-member, imported, re-exported, and CommonJS direct binding rows
  - Module-graph-backed direct binding join over ImportFact, ResolvedImportFact, and ModuleNodeId
  - Deterministic direct binding store indexes and cache input contract for Plan 04
affects: [phase-45, js-ts-analysis, direct-binding, semantic-graph]

tech-stack:
  added: []
  patterns: [private normalized fact rows, module-graph identity joins, explicit unresolved reasons]

key-files:
  created:
    - crates/polint/src/ts/binding/mod.rs
    - crates/polint/src/ts/binding/facts.rs
    - crates/polint/src/ts/binding/direct.rs
    - crates/polint/src/ts/binding/store.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/ts/mod.rs
    - crates/polint/src/ts/scope/extract.rs

key-decisions:
  - "Kept TS direct binding facts crate-private under ts::binding with no SDK, runner, CLI, or crate-root public exposure."
  - "Consumed existing module graph identities instead of constructing an oxc_resolver resolver inside ts::binding."
  - "Resolved only directly provable static bindings; token/property/prototype/this cases remain named unresolved reasons."
  - "Added an explicit TS direct binding provider parameter digest contract for Plan 04 rather than introducing a standalone cached provider in Plan 03."

patterns-established:
  - "Direct binding rows reference inventory/scope/module graph IDs and stable keys instead of copying module path payloads."
  - "TsDirectBindingStore indexes by callsite, target function, unresolved reason, module node, and stable key."
  - "Module-mediated tests model ESM named/default/namespace imports, re-export aliases, CommonJS require members, TypeScript path aliases, and external package unresolved status."

requirements-advanced: [JS-03]
requirements-completed: []

duration: 25 min
completed: 2026-05-31
---

# Phase 45 Plan 03: JS/TS Direct Binding Facts Summary

**Private direct-binding rows for static JS/TS local and module-mediated calls**

## Performance

- **Duration:** 25 min
- **Started:** 2026-05-31T18:46:00Z
- **Completed:** 2026-05-31T19:11:49Z
- **Tasks:** 4
- **Files modified:** 6

## Accomplishments

- Added crate-private direct binding IDs, fact rows, status/reason vocabulary, and binding-kind classification.
- Implemented local direct binding for `f()`, direct aliases, and static `ns.f()` members, with computed/token-flow boundaries left unresolved.
- Added module-mediated resolution through existing `ImportFact`, `ResolvedImportFact`, and `ModuleNodeId` facts for ESM named/default/namespace imports, re-exports, CommonJS require members, TypeScript path aliases, and external packages.
- Added deterministic `TsDirectBindingStore` normalization and indexes plus a locked D-12 provider parameter digest contract for Plan 04.

## Task Commits

1. **Task 1: Define direct binding fact and unresolved reason model** - `ff29d15a` (`feat`)
2. **Task 2: Resolve same-file and local alias direct calls** - `818875d1` (`feat`)
3. **Task 3: Resolve ESM/CommonJS/tsconfig module-mediated direct calls** - `89b4a079` (`feat`)
4. **Task 4: Add normalized binding store and cache input contract** - `c72d0ed7` (`feat`)

## Files Created/Modified

- `crates/polint/src/analysis/ids.rs` - Added crate-private dense ID for TS direct binding rows.
- `crates/polint/src/ts/mod.rs` - Registered `pub(crate) mod binding`.
- `crates/polint/src/ts/binding/facts.rs` - Added direct binding fact model and unresolved reason vocabulary.
- `crates/polint/src/ts/binding/direct.rs` - Added local and module-mediated direct binding resolution.
- `crates/polint/src/ts/binding/store.rs` - Added normalized output, lookup indexes, and TS direct binding digest contract.
- `crates/polint/src/ts/scope/extract.rs` - Preserved default export and static CommonJS require-member rows needed by direct binding.

## Decisions Made

- No public API promotion: all new direct binding types and helpers remain `pub(crate)`.
- The binding layer consumes module graph rows and resolved import IDs; it does not resolve files or instantiate `oxc_resolver`.
- Default imports require an explicit default-export scope row; Plan 03 does not guess from “only function in module”.
- JS-03 is advanced but not complete until Plan 04 projects these rows into semantic graph `CopyEdge` and `CallConstraint` constraints.

## Deviations from Plan

- The plan verify command `cargo test -p polint --lib ts::binding::direct_modules module_graph::ts` is not valid Cargo syntax because Cargo accepts only one test filter. It was split into `cargo test -p polint --lib ts::binding::direct_modules` and `cargo test -p polint --lib module_graph::ts`.
- `crates/polint/src/ts/scope/extract.rs` was updated during Task 3 to preserve static CommonJS require-member and default-export rows; this was needed so direct binding could consume normalized scope facts rather than infer them ad hoc.

## Issues Encountered

- Initial static grep matched the word `resolver` inside an internal allow reason because the acceptance pattern included `solver`. The wording was tightened so the validation signal stays quiet.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib ts::binding::facts` - passed
- `cargo test -p polint --lib ts::binding::direct_local` - passed
- `cargo test -p polint --lib ts::binding::direct_modules` - passed
- `cargo test -p polint --lib ts::binding` - passed
- `cargo test -p polint --lib module_graph::ts` - passed
- `cargo test -p polint --lib ts::scope::extract` - passed
- `rg -n "Resolver::new|oxc_resolver::Resolver" crates/polint/src/ts/binding` - no matches
- `rg -n "fixed-point|token propagation|token-set|solver" crates/polint/src/ts/binding` - no matches

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 45-04 can now project `ts::binding` direct binding rows into the Phase 44 semantic graph as `CopyEdge` and `CallConstraint` constraints, using the D-12 digest contract added in `ts::binding::store`.

## Self-Check: PASSED

All direct binding tasks and acceptance checks passed.

---
*Phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls*
*Completed: 2026-05-31*
