---
phase: 13-symbols-and-references
plan: 04
subsystem: static-analysis
tags: [rust, oxc, typescript, javascript, symbols, references, module-graph]

requires:
  - phase: 13-symbols-and-references
    provides: Symbol graph derivation pipeline and stable symbol/reference fact model from 13-03
provides:
  - Oxc-backed TS/JS local symbol and definition extraction
  - Oxc-backed resolved and unresolved TS/JS reference extraction
  - Module graph linked TS import alias references with visible uncertainty states
affects: [symbol_graph, typescript_adapter, module_graph, sdk_facts]

tech-stack:
  added: []
  patterns:
    - Borrow SourceFile.source while building Oxc semantic facts
    - Delay cross-file import alias linking until all per-file exports are indexed
    - Use public polint precision/status enums to avoid overstating semantic certainty

key-files:
  created:
    - .planning/phases/13-symbols-and-references/13-04-SUMMARY.md
  modified:
    - crates/polint/src/symbol_graph/ts.rs
    - crates/polint/src/symbol_graph/model.rs

key-decisions:
  - "TS/JS Oxc lexical symbols and references are marked ExactLocal, not ExactSemantic."
  - "Cross-file import alias links use ModuleLinked precision and visible Ambiguous/Unresolved/SetupMissing/Unsupported statuses."
  - "Import aliases are linked after all TS files have emitted exported symbols so source/target file order does not affect results."

patterns-established:
  - "TS symbol extraction normalizes Oxc symbols into SymbolGraphBuilder drafts with stable file/scope/span inputs."
  - "TS reference extraction uses Oxc resolved references for local targets and root unresolved references for visible missing/global names."
  - "Module graph import facts are joined by file, import path, and source literal span before linking alias references."

requirements-completed: [SYM-03, SYM-04]

duration: 25m30s
completed: 2026-05-13
---

# Phase 13 Plan 04: TS/JS Symbols And References Summary

**Oxc-backed TS/JS symbol and reference facts with module-linked import alias resolution**

## Performance

- **Duration:** 25m30s
- **Started:** 2026-05-13T05:40:10Z
- **Completed:** 2026-05-13T06:05:40Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Replaced the TS/JS placeholder symbol provider with Oxc semantic extraction for local symbols, definitions, declaration merging, and stable IDs.
- Added resolved local reference facts plus visible unresolved root references with conservative `ReferenceKind` mapping.
- Linked named, default, and namespace import aliases through existing module graph facts using `ModuleLinked`, `Ambiguous`, and `Unresolved` precision/status states.

## Task Commits

1. **Task 1 RED:** `629db6c` test(13-04): add failing tests for TS local symbols
2. **Task 1 GREEN:** `bac6ce6` feat(13-04): extract TS local symbols
3. **Task 2 RED:** `5c4ca1e` test(13-04): add failing tests for TS references
4. **Task 2 GREEN:** `71745c8` feat(13-04): extract TS references
5. **Task 3 RED:** `301085c` test(13-04): add failing tests for TS import links
6. **Task 3 GREEN:** `bd5a3ec` feat(13-04): link TS import aliases

## Files Created/Modified

- `crates/polint/src/symbol_graph/ts.rs` - TS/JS Oxc semantic symbol, definition, reference, unresolved, and module-linked import alias extraction with tests.
- `crates/polint/src/symbol_graph/model.rs` - Builder helpers for preserving draft spans/kinds when emitting setup-missing and unsupported reference statuses.
- `.planning/phases/13-symbols-and-references/13-04-SUMMARY.md` - Execution summary and verification record.

## Decisions Made

- Used `ExactLocal` for Oxc lexical facts because this plan does not add a type-checker or cross-file semantic engine.
- Used `ModuleLinked` for import alias references that resolve through `ResolvedImportFact.target_node` to a unique exported symbol.
- Kept uncertain module-link outcomes visible as reference facts instead of dropping them.

## Deviations from Plan

None - plan executed as written.

## Issues Encountered

- Task 3 initially selected a source-file import alias symbol in a test fixture where the alias shared the exported target name. The test was tightened to select exported target symbols, then the implementation passed.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib symbol_graph_ts_local_symbols --locked`
- `cargo test -p polint --lib symbol_graph_ts_references --locked`
- `cargo test -p polint --lib symbol_graph_ts_import_links --locked`
- `cargo test -p polint --lib symbol_graph_ts --locked`
- `cargo fmt --all -- --check`

## Next Phase Readiness

Plans 13-05 and 13-06 can build on real TS/JS symbol/reference facts and query views. Remaining precision limits are explicit: no TS compiler sidecar, no exact cross-file member/property resolution, and globals remain unresolved until a supported global model exists.

## Self-Check: PASSED

- Created/modified files exist: `crates/polint/src/symbol_graph/ts.rs`, `crates/polint/src/symbol_graph/model.rs`, `.planning/phases/13-symbols-and-references/13-04-SUMMARY.md`
- Task commits exist: `629db6c`, `bac6ce6`, `5c4ca1e`, `71745c8`, `301085c`, `bd5a3ec`

---
*Phase: 13-symbols-and-references*
*Completed: 2026-05-13*
