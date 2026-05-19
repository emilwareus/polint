---
phase: 26-semantic-index-deepening
plan: 02
subsystem: analysis-kernel
tags: [rust, semantic-index, symbol-graph, typescript, oxc]

requires:
  - phase: 26-semantic-index-deepening
    provides: Internal semantic row contracts, AnalysisDb storage, and symbol graph manifest outputs from Plan 26-01
provides:
  - TS/JS semantic extraction rows for scopes, imports, exports, aliases, resolution steps, unknowns, and stable export identities
  - SemanticIndexBuilder collection path used by TS/JS extraction
  - Conservative TS/JS dynamic and CommonJS semantic rows
affects: [semantic-index, symbol-graph, ts-js-analysis]

tech-stack:
  added: []
  patterns: [crate-private semantic builder, Oxc-owned TS semantic extraction, explicit uncertainty rows]

key-files:
  created: []
  modified:
    - crates/polint/src/symbol_graph/semantic.rs
    - crates/polint/src/symbol_graph/ts.rs

key-decisions:
  - "Keep TS/JS semantic rows crate-private under symbol_graph::semantic with no SDK, runner, CLI, or crate-root public surface."
  - "Use Oxc scopes and references as the TS/JS semantic source, with conservative rows for unresolved, dynamic, external, and unsupported forms."
  - "Represent TS/JS stable export identities with a native generated discriminator while future plans decide DB/cache publication."

patterns-established:
  - "SemanticIndexBuilder owns per-family vectors and returns deterministic SemanticIndexOutput sorted by stable row keys."
  - "TS/JS semantic extraction uses helper functions for scope_stable_key and reference_scope_stable_key rather than changing public SymbolFact or ReferenceFact fields."
  - "Dynamic import, CommonJS require, module.exports, and exports.name emit internal uncertainty rows instead of exact claims."

requirements-completed: [SAE-SEM-01]

duration: 19 min
completed: 2026-05-19
---

# Phase 26 Plan 02: TS/JS Semantic Index Deepening Summary

**Oxc-backed TS/JS semantic rows for scopes, imports, exports, aliases, resolution steps, and native stable export identities**

## Performance

- **Duration:** 19 min
- **Started:** 2026-05-19T05:57:05Z
- **Completed:** 2026-05-19T06:15:43Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added `SemanticIndexBuilder` and `SemanticIndexOutput` as the shared crate-private collector for scope, import, export, alias, resolution, generated-symbol, and stable-export row families.
- Added `derive_ts_semantic_index` to the TS/JS symbol path, deriving deterministic module/function/class/block/catch/loop/switch/type/namespace scope rows from Oxc data.
- Emitted TS/JS semantic rows for static imports, type-only imports, side effects, exports, reexports, CommonJS patterns, dynamic imports, lexical resolution, import/module lookup, unknown fallback, and native stable exports.

## Task Commits

Each task was committed atomically:

1. **Task 1: Emit TS/JS scopes and declaration ownership** - `9a7901d` (test), `360d19a` (feat)
2. **Task 2: Emit TS/JS imports, exports, reexports, and aliases** - `ac1060a` (test), `e214bd0` (feat)
3. **Task 3: Record TS/JS resolution ladder rows and stable export identities** - `128daae` (test), `a31a371` (feat)

_Note: TDD tasks have separate red test and green implementation commits._

## Files Created/Modified

- `crates/polint/src/symbol_graph/semantic.rs` - Added deterministic semantic output/builder APIs plus TS/JS-specific scope, import, export, and resolution vocabulary.
- `crates/polint/src/symbol_graph/ts.rs` - Added Oxc-backed TS/JS semantic extraction, conservative import/export/CommonJS/dynamic rows, resolution ladder rows, and focused unit tests.

## Decisions Made

- Kept the new semantic rows internal and test-facing; no public SDK, CLI, runner, or crate-root API was added.
- Used Oxc scope ancestry for scope/reference stable keys and supplemented it with recognized TS AST scope rows for type and namespace-like constructs.
- Used conservative statuses for setup-sensitive or dynamic TS/JS semantics: relative/static imports without module graph proof are unresolved, packages are external, dynamic forms are dynamic, and CommonJS exports are unsupported.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added deterministic semantic builder output in Task 1**
- **Found during:** Task 1
- **Issue:** Plan 26-01 had semantic row structs and AnalysisDb storage, but no shared collector API for TS/JS to use.
- **Fix:** Added `SemanticIndexBuilder` and `SemanticIndexOutput` with all required insertion APIs and deterministic per-family sorting.
- **Files modified:** `crates/polint/src/symbol_graph/semantic.rs`
- **Verification:** `cargo test -p polint --lib symbol_graph::ts::semantic_scopes --locked`
- **Committed in:** `360d19a`

---

**Total deviations:** 1 auto-fixed (Rule 2)
**Impact on plan:** The change was required by the plan action and stayed within the named semantic module boundary.

## Issues Encountered

- `docs/facts/module-graph.md` referenced by the plan does not exist; existing TS module graph code and `docs/facts/imports.md` were used for status alignment.

## Known Stubs

None.

## Threat Flags

None - the new surface is crate-private semantic row construction and test coverage only; no public API, network endpoint, file access boundary, or schema boundary was added.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 26-03 can build on concrete TS/JS semantic rows and the builder output shape to wire DB/cache publication, validation, or cross-language semantic closure without adding public API.

## Verification

- `cargo test -p polint --lib symbol_graph::ts::semantic_scopes --locked`
- `cargo test -p polint --lib symbol_graph::ts::semantic_imports_exports --locked`
- `cargo test -p polint --lib symbol_graph::ts::semantic_resolution --locked`
- `cargo fmt --all -- --check`

## Self-Check: PASSED

- Created/modified files exist.
- Task commits found: `9a7901d`, `360d19a`, `ac1060a`, `e214bd0`, `128daae`, `a31a371`.

---
*Phase: 26-semantic-index-deepening*
*Completed: 2026-05-19*
