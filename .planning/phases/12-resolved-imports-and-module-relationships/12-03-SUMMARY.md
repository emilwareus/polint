---
phase: 12-resolved-imports-and-module-relationships
plan: "03"
subsystem: core-analysis
tags: [rust, module-graph, resolved-imports, typescript, oxc-resolver]

requires:
  - phase: 12-02
    provides: project-wide module graph provider, deterministic builder, conservative TS/Go resolver boundaries, and supported relationship capabilities
provides:
  - Project-aware TS/JS import resolution through one oxc_resolver context per provider run
  - Local file, external package, unresolved, setup-missing, and dynamic TS/JS resolution classifications
  - TS/JS project module nodes from nearest package.json or tsconfig.json with contains and dependency edges
  - Dynamic import syntax facts for string and non-string import expressions
  - Repeated-run determinism proof for TS/JS resolved imports, module nodes, module edges, and reachability ordering
affects: [12-04, module-graph-provider, ts-adapter, sdk-relationship-views]

tech-stack:
  added: []
  patterns:
    - one crate-private resolver context per module graph provider run
    - resolver absolute paths normalized lexically before mapping back to AnalysisDb files
    - dynamic import expressions preserved through a crate-private sentinel and explicit Dynamic status

key-files:
  created:
    - .planning/phases/12-resolved-imports-and-module-relationships/12-03-SUMMARY.md
  modified:
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/module_graph/model.rs
    - crates/polint/src/module_graph/ts.rs
    - crates/polint/src/module_graph/paths.rs
    - crates/polint/src/module_graph/query.rs
    - crates/polint/src/module_graph/go.rs
    - crates/polint/src/ts/adapter.rs
    - crates/polint/src/ts/mod.rs
    - crates/polint/src/ts/tests.rs

key-decisions:
  - "TS/JS resolution uses one oxc_resolver Resolver per provider run with fixed options, tsconfig auto-discovery, and lexical path identity mapping."
  - "TS/JS project module labels prefer package.json name and fall back to the repo-relative module root."
  - "Non-string dynamic import expressions use a crate-private `<dynamic>` sentinel and resolve to Dynamic/DynamicExpression rather than External or Unresolved."
  - "Package-style TS/JS specifiers that do not map to analyzed repo files become external dependency nodes."

patterns-established:
  - "TsResolverContext owns resolver setup and the normalized absolute path to FileId index."
  - "ResolverInput borrows the TS resolver context instead of constructing resolver state per import."
  - "TS adapter import extraction handles static imports, CommonJS require calls, and dynamic import expressions through the same ImportFact path."

requirements-completed: [MOD-02, MOD-04]

duration: 17 min
completed: 2026-05-11
---

# Phase 12 Plan 03: TS Resolver And Dynamic Imports Summary

**TS/JS project-aware import resolution with oxc_resolver, project module nodes, dynamic import facts, and deterministic graph output**

## Performance

- **Duration:** 17 min
- **Started:** 2026-05-11T15:37:00Z
- **Completed:** 2026-05-11T15:53:56Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Replaced the conservative TS/JS resolver with `oxc_resolver` using fixed extension, alias, condition, main-field, exports, imports, builtin, tsconfig, and symlink options.
- Added `TsResolverContext` construction once per provider run, with normalized absolute path mapping back to analyzed `FileId`s.
- Added TS/JS project module-root discovery from `package.json` and `tsconfig.json`, including `Contains` edges to local files and `DependsOn` edges to external dependencies.
- Extended TS adapter import extraction to include `await import("./lazy")` and non-string dynamic import expressions.
- Added determinism tests for repeated TS/JS provider runs and deterministic reachability ordering.

## Task Commits

1. **Task 1 RED: TS resolver graph behavior tests** - `e8b6349` (test)
2. **Task 1 GREEN: project-aware TS import resolution** - `d15c573` (feat)
3. **Task 2 RED: dynamic import tests** - `72d882e` (test)
4. **Task 2 GREEN: dynamic TS imports to module graph** - `09e3eb3` (feat)
5. **Task 3: deterministic TS module graph output tests** - `0715ada` (test)

_Note: Task 3's RED tests passed immediately because Task 1 and the existing query helper already satisfied the determinism behavior; it was committed as coverage-only proof._

## Files Created/Modified

- `crates/polint/src/module_graph/mod.rs` - Threads `TsResolverContext`, seeds TS project module roots, links module contains/dependency edges, and adds TS resolver behavior tests.
- `crates/polint/src/module_graph/model.rs` - Extends `ResolverInput` with a borrowed TS resolver context.
- `crates/polint/src/module_graph/ts.rs` - Implements `oxc_resolver` resolution, local/external/unresolved/setup/dynamic classifications, and TS determinism tests.
- `crates/polint/src/module_graph/paths.rs` - Adds lexical absolute path normalization and repo-relative path mapping helpers.
- `crates/polint/src/module_graph/query.rs` - Adds deterministic reachability ordering coverage.
- `crates/polint/src/module_graph/go.rs` - Updates resolver tests for the extended `ResolverInput`.
- `crates/polint/src/ts/adapter.rs` - Emits ImportFacts for dynamic import expressions and defines the dynamic sentinel.
- `crates/polint/src/ts/mod.rs` - Re-exports the crate-private dynamic sentinel for sibling module graph code.
- `crates/polint/src/ts/tests.rs` - Adds adapter coverage for string and non-string dynamic imports.

## Decisions Made

- Resolver output paths are never exposed publicly; they are normalized and mapped to `FileId`s before becoming relationship facts.
- `symlinks: false` is set on the TS resolver so path identity stays lexical and matches the AnalysisDb file index.
- TS module ownership is derived from nearest `package.json` or `tsconfig.json`; a package name is a clearer module label when available.
- The dynamic import sentinel remains crate-private and is converted into explicit `Dynamic` relationship facts by the provider.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Generalized setup-missing capability messaging**
- **Found during:** Task 1 (Resolve relative imports, tsconfig aliases, and package imports)
- **Issue:** Provider setup-missing support text was Go-specific, but TS resolver setup failures can now also block requested relationship capabilities.
- **Fix:** Changed capability reason/help text to describe language resolver setup broadly.
- **Files modified:** `crates/polint/src/module_graph/mod.rs`
- **Verification:** `cargo test -p polint --lib module_graph_ts_resolution --locked`
- **Committed in:** `d15c573`

**2. [Rule 3 - Blocking] Re-exported the dynamic import sentinel through `ts::mod`**
- **Found during:** Task 2 (Make dynamic TS/JS imports visible to relationship facts)
- **Issue:** `ts::adapter` is a private child module, so sibling `module_graph::ts` could not legally compare against the crate-private sentinel defined in the adapter.
- **Fix:** Added a narrow crate-private re-export from `ts/mod.rs` instead of widening the adapter module.
- **Files modified:** `crates/polint/src/ts/mod.rs`
- **Verification:** `cargo test -p polint --lib dynamic_imports --locked`; `cargo test -p polint --lib module_graph_ts_dynamic_resolution --locked`
- **Committed in:** `09e3eb3`

---

**Total deviations:** 2 auto-fixed (1 missing critical, 1 blocking)
**Impact on plan:** Both fixes were required for correctness after adding TS resolver setup and dynamic import handling. No public API or CLI surface was widened.

## Issues Encountered

- Task 3's new determinism tests passed immediately because the deterministic builder/query behavior was already present after Task 1 and prior Plan 12-02 work. The task was completed as coverage-only proof.
- Parallel verification commands briefly waited on Cargo locks. All final verification commands passed.

## Verification

- `cargo test -p polint --lib module_graph_ts_resolution --locked`
- `cargo test -p polint --lib module_graph_resolver_contracts --locked`
- `cargo test -p polint --lib dynamic_imports --locked`
- `cargo test -p polint --lib module_graph_ts_dynamic_resolution --locked`
- `cargo test -p polint --lib module_graph_ts_determinism --locked`
- `cargo test -p polint --lib module_graph_ts --locked`
- `cargo test -p polint --lib ts --locked`
- `cargo fmt --all -- --check`
- Structural `rg` checks for required `oxc_resolver` APIs, resolver options, one-context threading, module-root edges, dynamic sentinel handling, deterministic tests, and absence of TypeScript semantic analysis in `module_graph::ts`.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None. Stub-pattern scan hits only test fixture literals such as `export const tokens = {};`, not production placeholders or unwired data.

## Next Phase Readiness

Plan 12-04 can add Go package/module resolution on top of the same provider and builder contracts. TS/JS relationship facts now provide resolved local files, external dependencies, unresolved imports, project module roots, dynamic import status, and deterministic graph output.

## Self-Check: PASSED

- Confirmed summary file and all key modified source files exist.
- Confirmed task commits exist: `e8b6349`, `d15c573`, `72d882e`, `09e3eb3`, `0715ada`.

---
*Phase: 12-resolved-imports-and-module-relationships*
*Completed: 2026-05-11*
