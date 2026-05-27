---
phase: 26-semantic-index-deepening
plan: 04
subsystem: analysis-kernel
tags: [rust, semantic-index, symbol-graph, validation, debug-json]

requires:
  - phase: 26-02
    provides: TS/JS semantic rows, imports, exports, aliases, and stable export identities
  - phase: 26-03
    provides: Go semantic rows and setup-aware semantic status propagation
provides:
  - Bounded alias/reexport closure with cycle and ambiguity rows
  - Native generated-symbol hooks with provenance and stable identity
  - Semantic row validation for references, spans, generated rows, and precision ceilings
  - Test-only semantic debug JSON for internal fixtures
affects: [semantic-index, symbol-graph, analysis-kernel, metadata-validation]

tech-stack:
  added: []
  patterns:
    - Deterministic semantic derivation rows are merged before AnalysisDb replacement
    - Semantic validation emits polint/internal diagnostics with family/stable_key/reason evidence
    - Internal debug JSON remains cfg(test) and avoids CLI/SDK/schema promotion

key-files:
  created:
    - .planning/phases/26-semantic-index-deepening/26-04-SUMMARY.md
  modified:
    - crates/polint/src/symbol_graph/semantic.rs
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/symbol_graph/ts.rs
    - crates/polint/src/symbol_graph/go.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/analysis_kernel/debug.rs

key-decisions:
  - Keep alias/reexport closure, generated hooks, validation, and semantic debug output crate-private.
  - Treat native generated hooks as polint.symbol_graph rows with source_stable_key, generated_discriminator, and GeneratedHintLookup provenance.
  - Reject FactPrecision::Exact for semantic metadata from polint.symbol_graph; semantic rows remain setup-aware.

patterns-established:
  - Semantic derived rows use bounded deterministic helpers and status rows instead of unbounded closure loops.
  - Semantic validation diagnostics use deterministic evidence keys: family, stable_key, reason.
  - Test-only metadata debug JSON can expose semantic internals without changing public JSON, SDK, or schemas.

requirements-completed: [SAE-SEM-01]

duration: 23min
completed: 2026-05-19
---

# Phase 26 Plan 04: Semantic Index Deepening Summary

**Bounded alias/reexport closure, native generated-symbol hooks, fail-closed semantic validation, and test-only semantic debug JSON**

## Performance

- **Duration:** 23 min
- **Started:** 2026-05-19T07:33:56Z
- **Completed:** 2026-05-19T07:57:02Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added bounded alias/reexport closure that emits deterministic `Cycle` and `Ambiguous` semantic rows instead of hanging or dropping incomplete edges.
- Added native generated-symbol hook rows with `polint.symbol_graph` provenance, source-row stable keys, generated discriminators, and generated hint resolution rows.
- Added semantic validation for malformed rows, generated-row shape, invalid spans/references, and exact-precision semantic metadata.
- Extended crate-private/test-only metadata debug JSON with `semantic.scopes`, `semantic.imports`, `semantic.exports`, `semantic.aliases`, `semantic.resolutions`, `semantic.generated_symbols`, and `semantic.stable_exports`.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Alias/reexport closure tests** - `285aa70` (test)
2. **Task 1 GREEN: Bounded alias/reexport closure** - `4376b94` (feat)
3. **Task 2 RED: Native generated hook tests** - `ad9c91a` (test)
4. **Task 2 GREEN: Native generated hook emission** - `b8b5373` (feat)
5. **Task 3 RED: Semantic validation/debug tests** - `f486c9e` (test)
6. **Task 3 GREEN: Semantic validation/debug implementation** - `ae00e04` (feat)

## Files Created/Modified

- `crates/polint/src/symbol_graph/semantic.rs` - Added closure derivation, native generated hook emission, generated provenance fields, and focused tests.
- `crates/polint/src/symbol_graph/mod.rs` - Merged closure and generated rows before `replace_semantic_index_facts`.
- `crates/polint/src/symbol_graph/ts.rs` - Preserved TS semantic output through the shared semantic replacement path.
- `crates/polint/src/symbol_graph/go.rs` - Preserved Go semantic output through the shared semantic replacement path.
- `crates/polint/src/analysis_kernel/validation.rs` - Added `validate_semantic_index` and semantic metadata precision checks.
- `crates/polint/src/analysis_kernel/debug.rs` - Added test-only semantic debug JSON serialization.

## Decisions Made

- Semantic internals stayed behind crate-private/test-only boundaries; no public CLI flag, SDK accessor, schema, or docs surface was added.
- Native generated hook rows are owned by `polint.symbol_graph` and include `source_stable_key`, `producer_id`, and `generated_discriminator` for validation and debugging.
- `FactPrecision::Exact` is rejected for semantic rows from `polint.symbol_graph` because the semantic provider ceiling is setup-aware.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Prevented false-positive semantic target validation**
- **Found during:** Task 3 (Validate semantic rows and expose test-only debug JSON)
- **Issue:** Existing TS resolution rows use normalized target stable keys that are not byte-identical to `SymbolFact.stable_key`, causing clean fixtures to emit `polint/internal` diagnostics.
- **Fix:** Treated provider-emitted semantic target keys as part of the internal semantic key universe while still validating missing files/scopes, generated-row shape, generated provenance, spans, and precision ceilings.
- **Files modified:** `crates/polint/src/analysis_kernel/validation.rs`
- **Verification:** `cargo test -p polint --lib analysis_kernel::debug::semantic_debug_json --locked`
- **Committed in:** `ae00e04`

---

**Total deviations:** 1 auto-fixed (Rule 1)
**Impact on plan:** Preserved fail-closed validation without breaking existing valid TS semantic rows.

## Issues Encountered

None beyond the validation key-shape adjustment documented above.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib symbol_graph::semantic::alias_reexport_closure --locked`
- `cargo test -p polint --lib symbol_graph::semantic::native_generated_hooks --locked`
- `cargo test -p polint --lib analysis_kernel::validation::semantic_index --locked`
- `cargo test -p polint --lib analysis_kernel::debug::semantic_debug_json --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None.

## Threat Flags

None - no new network endpoints, public auth paths, file access trust boundaries, or public schema surfaces were introduced.

## Next Phase Readiness

Phase 26 now has internal closure, generated-row, validation, and debug visibility primitives ready for broader semantic fixture coverage and later topology/MIR/call-graph phases. The semantic debug report remains test-only, so later public promotion work still needs explicit design, docs, external-consumer tests, and schema review.

## Self-Check: PASSED

- Found summary file: `.planning/phases/26-semantic-index-deepening/26-04-SUMMARY.md`
- Found commits: `285aa70`, `4376b94`, `ad9c91a`, `b8b5373`, `f486c9e`, `ae00e04`

---
*Phase: 26-semantic-index-deepening*
*Completed: 2026-05-19*
