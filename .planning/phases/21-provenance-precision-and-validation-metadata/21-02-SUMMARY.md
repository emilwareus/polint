---
phase: 21-provenance-precision-and-validation-metadata
plan: 02
subsystem: analysis-kernel
tags: [rust, analysisdb, metadata, provenance, module-graph, symbol-graph, metrics]

requires:
  - phase: 21-provenance-precision-and-validation-metadata
    provides: Crate-private metadata sidecar for source and syntax fact families
provides:
  - Derived-provider metadata for module graph, symbol graph, and metrics fact families
  - Shared precision/confidence mapping helpers for resolution and symbol precision/status fields
  - Deterministic crate-private missing metadata report for current kernel fact families
affects: [analysis-kernel, module-graph, symbol-graph, metrics, validation]

tech-stack:
  added: []
  patterns:
    - Derived replace boundaries clear and rewrite sidecar metadata for their fact families
    - Missing metadata reports sort by fact-family label and run-local id

key-files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/metrics.rs

key-decisions:
  - "Derived provider metadata uses hard-coded manifest IDs polint.module_graph, polint.symbol_graph, and polint.metrics."
  - "Symbol, definition, and reference metadata stable keys reuse the existing symbol graph stable_key fields exactly."
  - "The missing metadata report stays crate-private and test-facing, with a debug assertion keeping the invariant live inside the kernel."

patterns-established:
  - "Replace-style derived providers remove stale sidecar rows before recording current metadata rows."
  - "Metric metadata stable keys derive from existing source/function metadata stable keys when present, with deterministic path/span fallbacks."

requirements-completed: [SAE-FND-02]

duration: 14m
completed: 2026-05-17
---

# Phase 21 Plan 02: Derived Provider Metadata Coverage Summary

**Module graph, symbol graph, and metrics facts now receive internal metadata, with deterministic gap detection across all current fact families**

## Performance

- **Duration:** 14m
- **Started:** 2026-05-17T07:12:18Z
- **Completed:** 2026-05-17T07:26:41Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added metadata attachment at `AnalysisDb` replace boundaries for resolved imports, module nodes/edges, symbols, definitions, references, file metrics, function metrics, and complexity metrics.
- Added shared precision/confidence mapping for `ResolutionPrecision`, `ResolutionStatus`, and `SymbolPrecision` while preserving the original family-specific fields.
- Added `MissingFactMeta` and a crate-private `AnalysisDb::missing_fact_metadata` scanner that covers every current kernel-produced fact vector.

## Task Commits

1. **Task 1: Attach metadata to module and symbol graph facts** - `1f34579` (test), `0772749` (feat)
2. **Task 2: Attach metadata to derived metrics facts** - `4821ee0` (test), `dfc6e4c` (feat)
3. **Task 3: Add deterministic missing-metadata detection** - `c8f3caf` (test), `341685b` (feat)

_Note: All tasks followed TDD with red test commits before implementation commits._

## Files Created/Modified

- `crates/polint/src/analysis_kernel/metadata.rs` - Added mapping helpers, `MissingFactMeta`, and test-only metadata removal support.
- `crates/polint/src/analysis_kernel/mod.rs` - Added test-only kernel passthrough for missing metadata reports.
- `crates/polint/src/core/mod.rs` - Records derived-provider metadata and scans current fact vectors for missing sidecar rows.
- `crates/polint/src/module_graph/mod.rs` - Added module graph metadata coverage tests.
- `crates/polint/src/symbol_graph/mod.rs` - Added symbol graph metadata coverage tests.
- `crates/polint/src/metrics.rs` - Added metrics metadata trigger/default/stable-key tests.

## Decisions Made

- Derived providers use the Phase 20 provider IDs directly as producer/layer IDs to avoid widening public provider APIs.
- Replace boundaries remove stale metadata for the replaced families before recording fresh rows, keeping sidecar identity aligned with current vectors.
- `AnalysisKernel::run` keeps a debug-only missing-metadata invariant check rather than exposing an SDK, runner, or CLI inspection surface.

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None. Stub scan matches were existing/test fixture strings and a metadata formatter expression, not introduced placeholders or unwired UI/data behavior.

## Issues Encountered

- Parallel Cargo invocations briefly waited on package/artifact locks during verification. Sequential reruns passed.
- Two GSD state helper commands did not match this repository's `STATE.md` headings, so the metric/session rows were applied manually after the tool updated progress and decisions.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib module_graph_metadata --locked`
- `cargo test -p polint --lib symbol_graph_metadata --locked`
- `cargo test -p polint --lib metrics_metadata --locked`
- `cargo test -p polint --lib derive_requested_metrics --locked`
- `cargo test -p polint --lib missing_fact_metadata --locked`
- `cargo test -p polint --lib analysis_kernel --locked`
- `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- `cargo fmt --all -- --check`

## Next Phase Readiness

Plan 21-03 can build validation/debug inspection on top of complete sidecar coverage for the current source, syntax, derived graph, symbol, and metrics fact families.

## Self-Check: PASSED

- Found summary file: `.planning/phases/21-provenance-precision-and-validation-metadata/21-02-SUMMARY.md`
- Found key files: `crates/polint/src/analysis_kernel/metadata.rs`, `crates/polint/src/core/mod.rs`, `crates/polint/src/metrics.rs`
- Found task commits: `1f34579`, `0772749`, `4821ee0`, `dfc6e4c`, `c8f3caf`, `341685b`

---
*Phase: 21-provenance-precision-and-validation-metadata*
*Completed: 2026-05-17*
