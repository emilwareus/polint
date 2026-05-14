---
phase: 13-symbols-and-references
plan: 01
subsystem: sdk
tags: [rust, sdk, symbols, references, capabilities, macros]

requires:
  - phase: 11-capability-driven-analysis-plan
    provides: capability planning and setup diagnostics
  - phase: 12-resolved-imports-and-module-relationships
    provides: typed fact-view and module graph patterns
provides:
  - stable core fact contract for symbols, definitions, and references
  - borrowed SDK views for Symbols<'_> and References<'_>
  - macro-derived capability mapping for symbol/reference views
  - planner recognition for unsupported symbol/reference capabilities
  - documentation target for symbol/reference capability diagnostics
affects:
  - 14-direct-and-resolved-call-graph-facts
  - 15-cfg-facts
  - symbol_graph
  - sdk
  - macros

tech-stack:
  added: []
  patterns:
    - public typed fact views backed by private AnalysisDb indexes
    - provider-owned stable IDs preserved by replace_symbol_graph_facts
    - unsupported capability rows documented before provider promotion

key-files:
  created:
    - docs/facts/symbols-and-references.md
    - .planning/phases/13-symbols-and-references/13-01-SUMMARY.md
  modified:
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis_plan.rs
    - crates/polint/src/sdk/facts.rs
    - crates/polint/src/sdk/mod.rs
    - crates/polint-macros/src/lib.rs
    - docs/facts/README.md

key-decisions:
  - "References capability internally enables symbols so reference consumers always get symbol identity."
  - "Symbol/reference provider names are recognized but remain Unsupported until provider plans promote support."
  - "Definitions are queried through the Symbols view rather than a separate Definitions view."

patterns-established:
  - "Provider-owned IDs: AnalysisDb preserves SymbolId, DefinitionId, and ReferenceId values from fact providers instead of renumbering from vector indexes."
  - "Borrowed views: SDK fact views expose filtered iterators and slices over private AnalysisDb indexes without exposing adapter internals."
  - "Capability honesty: unsupported symbol/reference capabilities are recognized and documented, but rules do not execute with unavailable facts."

requirements-completed: [SYM-01, SYM-04]

duration: 17min
completed: 2026-05-12
---

# Phase 13 Plan 01: Symbol and Reference Contract Summary

**Stable symbol/reference fact contract with borrowed SDK views and macro-derived capabilities**

## Performance

- **Duration:** 17 min
- **Started:** 2026-05-12T20:07:40Z
- **Completed:** 2026-05-12T20:24:17Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Added stable core `SymbolFact`, `DefinitionFact`, and `ReferenceFact` contracts with provider-owned IDs, precision/status enums, storage vectors, and deterministic BTree-backed lookup indexes.
- Added `Symbols<'_>` and `References<'_>` SDK views plus prelude exports for normalized polint-owned facts and enums.
- Extended macro capability derivation so canonical `Symbols<'_>` and `References<'_>` parameters request the correct capabilities, with `references` also enabling `symbols`.
- Added planner recognition and documentation for symbol/reference capabilities while providers remain unsupported.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add core symbol/reference fact contract** - `2d699f2` (test), `159d391` (feat), `fe423bb` (refactor)
2. **Task 2: Add typed SDK views for symbols and references** - `6c12df8` (test), `7d4cc12` (feat)
3. **Task 3: Map SDK views to capabilities through the macro** - `de3b8b9` (test), `9604127` (feat)
4. **Deviation support: Add missing capability docs target** - `2ec4d45` (docs)

**Plan metadata:** committed after summary self-check.

## Files Created/Modified

- `crates/polint/src/core/mod.rs` - Added symbol/reference IDs, fact structs, precision/status enums, AnalysisDb storage/indexes, lookup helpers, and symbol/reference capability flags.
- `crates/polint/src/analysis_plan.rs` - Recognizes `symbols` and `references` capabilities with deterministic unsupported diagnostics and docs links.
- `crates/polint/src/sdk/facts.rs` - Added borrowed `Symbols<'_>` and `References<'_>` fact views with deterministic query methods.
- `crates/polint/src/sdk/mod.rs` - Exported symbol/reference facts, enums, IDs, and views through the curated SDK prelude.
- `crates/polint-macros/src/lib.rs` - Mapped canonical symbol/reference view parameters to macro-derived capabilities.
- `docs/facts/symbols-and-references.md` - Documented fact fields, supported query surface, precision/status semantics, and current provider limits.
- `docs/facts/README.md` - Linked the new symbol/reference fact documentation.
- `.planning/phases/13-symbols-and-references/13-01-SUMMARY.md` - Captures execution output and verification results.

## Decisions Made

- `Capabilities::references()` sets both `references` and `symbols`, because references need symbol identity for target and candidate relationships.
- `Definitions<'_>` was not added as a separate public view; definitions are available through `Symbols<'_>::definition` and `Symbols<'_>::definitions`.
- Symbol/reference capabilities are real planner names now, but their support status stays `Unsupported` until later provider plans populate facts.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added missing capability docs target**
- **Found during:** Post-verification after Task 3
- **Issue:** Planner diagnostics referenced `docs/facts/symbols-and-references.md`, but that documentation target did not exist.
- **Fix:** Added the symbol/reference facts documentation and linked it from the facts README.
- **Files modified:** `docs/facts/symbols-and-references.md`, `docs/facts/README.md`
- **Verification:** `test -f docs/facts/symbols-and-references.md && rg "Symbols<'_>|References<'_>|SymbolPrecision|SymbolResolutionStatus" docs/facts/symbols-and-references.md`; `cargo fmt --all -- --check`
- **Committed in:** `2ec4d45`

---

**Total deviations:** 1 auto-fixed (Rule 2)
**Impact on plan:** The docs target was required for truthful capability diagnostics. No feature scope was expanded.

## Issues Encountered

- `cargo fmt --all -- --check` found formatting drift after the core implementation; `fe423bb` records the formatting-only cleanup.
- The Task 3 core planner test already passed during RED because Task 1 had implemented the required `references` implies `symbols` behavior. The macro test still failed as expected before the Task 3 implementation.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

The stable core/SDK/macro contract is ready for language-provider work. Later Phase 13 plans can populate symbol and reference facts without widening the public rule-authoring API or exposing parser internals.

---
*Phase: 13-symbols-and-references*
*Completed: 2026-05-12*

## Self-Check: PASSED

- Verified created files exist: `docs/facts/symbols-and-references.md`, `.planning/phases/13-symbols-and-references/13-01-SUMMARY.md`
- Verified task and deviation commits exist: `2d699f2`, `159d391`, `fe423bb`, `6c12df8`, `7d4cc12`, `de3b8b9`, `9604127`, `2ec4d45`
