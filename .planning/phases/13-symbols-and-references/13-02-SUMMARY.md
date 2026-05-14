---
phase: 13-symbols-and-references
plan: 02
subsystem: symbol-graph
tags: [rust, symbols, references, stable-ids, deterministic-output]

requires:
  - phase: 13-symbols-and-references
    provides: stable core symbol/reference fact contract and SDK views from Plan 13-01
  - phase: 07-cache-and-performance
    provides: deterministic cache::stable_hash helper
  - phase: 12-resolved-imports-and-module-relationships
    provides: internal query and AnalysisDb indexing patterns
provides:
  - stable semantic ID helpers for symbols, definitions, and references
  - deterministic SymbolGraphBuilder with collision diagnostics and status/precision preservation
  - crate-private symbol graph query helpers for provider tests
affects:
  - 13-symbols-and-references
  - 14-direct-and-resolved-call-graph-facts
  - symbol_graph
  - language_providers
  - sdk

tech-stack:
  added: []
  patterns:
    - length-prefixed stable key encoding before cache::stable_hash
    - BTree-backed fact staging for deterministic provider output
    - crate-private provider test helpers over AnalysisDb indexes

key-files:
  created:
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/symbol_graph/stable_id.rs
    - crates/polint/src/symbol_graph/model.rs
    - crates/polint/src/symbol_graph/query.rs
    - .planning/phases/13-symbols-and-references/13-02-SUMMARY.md
  modified:
    - crates/polint/src/lib.rs

key-decisions:
  - "Stable symbol/reference IDs are derived from length-prefixed semantic stable keys using cache::stable_hash, never storage position or randomized hashing."
  - "SymbolGraphBuilder stages facts in BTree-backed maps and emits deterministic polint/internal diagnostics for ID collisions or incompatible duplicate stable keys."
  - "Internal query helpers remain crate-private and delegate to AnalysisDb/SDK-compatible semantics instead of widening the public API."

patterns-established:
  - "Stable key hashing: language providers submit normalized semantic key parts that are length-prefixed before hashing and preserved as debug-safe stable_key strings."
  - "Builder-owned ordering: providers can insert drafts in any order while finish() returns deterministic symbol, definition, reference, and diagnostic vectors."
  - "Internal-only graph queries: symbol graph convenience helpers support provider tests without exporting a public graph database surface."

requirements-completed: [SYM-04]

duration: 8h 46m
completed: 2026-05-13
---

# Phase 13 Plan 02: Symbol Graph Builder Foundation Summary

**Stable semantic symbol/reference IDs with deterministic builder output and crate-private provider query helpers**

## Performance

- **Duration:** 8h 46m
- **Started:** 2026-05-12T20:27:07Z
- **Completed:** 2026-05-13T05:13:28Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added stable ID key types and helpers for `SymbolId`, `DefinitionId`, and `ReferenceId` using length-prefixed semantic keys and `cache::stable_hash`.
- Added `SymbolGraphBuilder` and draft types that preserve language-provided precision/status values, sort output deterministically, and report deterministic `polint/internal` collision diagnostics.
- Added crate-private symbol graph query helpers over `AnalysisDb` for provider tests without widening the public API.

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement stable key hashing for symbol facts** - `c849f8f` (test), `d4482b5` (feat), `3d3683b` (refactor)
2. **Task 2: Implement deterministic SymbolGraphBuilder** - `0fe32f6` (test), `9522ae4` (feat)
3. **Task 3: Add internal query helpers for provider tests** - `51c8ef0` (test), `809ed74` (feat)

**Plan metadata:** committed after summary self-check.

## Files Created/Modified

- `crates/polint/src/lib.rs` - Registers the crate-private `symbol_graph` module.
- `crates/polint/src/symbol_graph/mod.rs` - Defines the crate-private module boundary for model, query, and stable ID internals.
- `crates/polint/src/symbol_graph/stable_id.rs` - Implements stable key builders, length-prefixed encoding, and ID conversion helpers for symbols, definitions, and references.
- `crates/polint/src/symbol_graph/model.rs` - Implements `SymbolGraphBuilder`, draft structs, deterministic output ordering, and collision diagnostics.
- `crates/polint/src/symbol_graph/query.rs` - Implements crate-private deterministic helpers for symbol, definition, and reference lookups in provider tests.
- `.planning/phases/13-symbols-and-references/13-02-SUMMARY.md` - Captures execution output and verification results.

## Decisions Made

- Stable IDs are generated from semantic stable keys through `cache::stable_hash`; no vector position, parser object identity, pointer address, or randomized hasher participates in IDs.
- Stable key strings are debug-safe normalized metadata only, with tests covering length-prefix boundary behavior and excluding raw source snippets.
- Builder collision diagnostics are emitted as deterministic `polint/internal` diagnostics while preserving sorted fact output.
- Query helpers stay `pub(crate)` and compare against SDK view semantics in tests rather than creating a public graph query API.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- A Task 1 RED assertion initially used the wrong byte length for `src/button.ts`; the implementation corrected the expected length while preserving the length-prefix security test intent.
- Stable-key-first sorting made length-prefixed names less useful for provider assertions, so the builder now sorts deterministically by file/span/kind/name with the stable key as a tie-breaker.

## Verification

- `cargo test -p polint --lib symbol_graph_stable_ids --locked` passed: 8 tests.
- `cargo test -p polint --lib symbol_graph_builder --locked` passed: 3 tests.
- `cargo test -p polint --lib symbol_graph_query --locked` passed: 2 tests.
- `cargo fmt --all -- --check` passed.
- Acceptance scans confirmed no randomized/pointer/index ID sources, required stable ID helpers, BTree-backed builder staging, crate-private query helpers, and no public `symbol_graph` export.

## Known Stubs

None. Stub-pattern scanning only matched a `{}` literal inside a unit-test source fixture.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 13-03 can populate TypeScript/JavaScript symbols through the builder and verify provider behavior with the crate-private query helpers. Plan 13-04 can reuse the same stable ID and deterministic output contracts for Go.

---
*Phase: 13-symbols-and-references*
*Completed: 2026-05-13*

## Self-Check: PASSED

- Verified created files exist: `crates/polint/src/symbol_graph/mod.rs`, `crates/polint/src/symbol_graph/stable_id.rs`, `crates/polint/src/symbol_graph/model.rs`, `crates/polint/src/symbol_graph/query.rs`, `.planning/phases/13-symbols-and-references/13-02-SUMMARY.md`
- Verified task commits exist: `c849f8f`, `d4482b5`, `3d3683b`, `0fe32f6`, `9522ae4`, `51c8ef0`, `809ed74`
