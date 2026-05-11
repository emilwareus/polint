---
phase: 12-resolved-imports-and-module-relationships
plan: "01"
subsystem: core-analysis
tags: [rust, sdk, capabilities, module-graph, resolved-imports]

requires:
  - phase: 11-capability-driven-analysis-plan
    provides: deterministic AnalysisPlan, capability support diagnostics, and typed macro capability derivation
provides:
  - Core resolved import and module graph fact records with stable ID newtypes
  - SDK typed fact views for resolved imports and module relationship graph queries
  - SDK prelude exports for the relationship fact model and views
  - Macro capability derivation for ResolvedImports and ModuleGraphFacts
  - Deterministic unsupported planner rows for unresolved provider wiring
affects: [12-02, module-graph-provider, sdk, macros, analysis-plan]

tech-stack:
  added: []
  patterns:
    - public relationship facts use polint-owned IDs and status enums
    - SDK fact views borrow AnalysisDb and expose narrow query methods
    - contract-only capabilities remain known but unsupported until providers populate facts

key-files:
  created:
    - .planning/phases/12-resolved-imports-and-module-relationships/12-01-SUMMARY.md
  modified:
    - crates/polint/src/core/mod.rs
    - crates/polint/src/sdk/facts.rs
    - crates/polint/src/sdk/mod.rs
    - crates/polint-macros/src/lib.rs
    - crates/polint/src/analysis_plan.rs

key-decisions:
  - "Resolved imports and module graph are known capabilities but stay Unsupported until Plan 12-02 wires the provider."
  - "ModuleGraphFacts::reachable_from uses deterministic breadth-first traversal over Resolved and External edges only."
  - "Public relationship facts expose polint-owned IDs and status enums, not resolver outputs or graph internals."

patterns-established:
  - "replace_module_graph_facts assigns stable IDs from vector positions before storing relationship facts."
  - "ResolvedImports and ModuleGraphFacts follow the existing FactView build pattern."
  - "Capability derivation accepts unqualified SDK view names and canonical polint::sdk::facts paths."

requirements-completed: [MOD-01, MOD-04]

duration: 11m 4s
completed: 2026-05-11
---

# Phase 12 Plan 01: Resolved Import And Module Graph Contract Summary

**Core and SDK contract for resolved imports and module graph facts, with macro-derived capabilities blocked until provider wiring.**

## Performance

- **Duration:** 11m 4s
- **Started:** 2026-05-11T14:15:24Z
- **Completed:** 2026-05-11T14:26:28Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `ResolvedImportFact`, `ModuleNode`, `ModuleEdge`, their ID newtypes, setup-aware status/precision/reason enums, `AnalysisDb` storage/accessors, and `resolved_imports`/`module_graph` capabilities.
- Added `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` SDK views with import lookup, unresolved filtering, node/edge lookup, dependency status, and deterministic reachability queries.
- Exported the new fact model through `polint::sdk::prelude::*` without exposing `Capabilities`.
- Mapped the new typed views in `polint-macros` and kept the analysis planner returning deterministic `Unsupported` support rows until Plan 12-02 connects the provider.

## Task Commits

1. **Task 1 RED: core relationship contract tests** - `409ceca` (test)
2. **Task 1 GREEN: core relationship facts and storage** - `b06d2bc` (feat)
3. **Task 2 RED: SDK view and prelude tests** - `672b3ae` (test)
4. **Task 2 GREEN: SDK typed views and exports** - `5a6775c` (feat)
5. **Task 3 RED: capability mapping and planner tests** - `334929f` (test)
6. **Task 3 GREEN: macro mapping and unsupported planner rows** - `c2117d6` (feat)

## Files Created/Modified

- `crates/polint/src/core/mod.rs` - Added relationship fact types, enums, storage/accessors, capability flags, and core contract tests.
- `crates/polint/src/sdk/facts.rs` - Added `ResolvedImports` and `ModuleGraphFacts` borrowed fact views plus query tests.
- `crates/polint/src/sdk/mod.rs` - Added curated prelude exports for relationship facts and views.
- `crates/polint-macros/src/lib.rs` - Added view-to-capability mappings for `ResolvedImports<'_>` and `ModuleGraphFacts<'_>`.
- `crates/polint/src/analysis_plan.rs` - Added known unsupported support rows for `resolved_imports` and `module_graph`.

## Decisions Made

- `resolved_imports` and `module_graph` are recognized capability names now, but rules requesting them remain blocked by `polint/capability` diagnostics until provider support is wired in Plan 12-02.
- Relationship SDK views expose borrowed slices/iterators where possible; `reachable_from` returns an owned `Vec<ModuleNodeId>` because it computes a graph traversal.
- Public facts use only normalized polint IDs and enums. No raw resolver output, `petgraph`, AST nodes, or mutable database access was added.

## Verification

- `cargo test -p polint --lib module_relationship_core_contract --locked`
- `cargo test -p polint --lib module_graph_sdk_views --locked`
- `cargo test -p polint-macros --lib capability_for_type_maps_supported_fact_views --locked`
- `cargo test -p polint --lib analysis_plan_recognizes_module_relationship_capabilities --locked`
- `cargo test -p polint --lib sdk_prelude_exports_rule_authoring_surface --locked`
- `cargo test -p polint-macros --lib --locked`
- `cargo fmt --all -- --check`
- Structural `rg` checks for required public names, storage/accessors, capability rows, macro mappings, prelude exports, and absence of `pub use crate::core::Capabilities`.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Task 2 GREEN initially needed an explicit borrow when passing `graph.edges()[1]` into `dependency_status(&ModuleEdge)`. The test call was corrected before the implementation commit.
- Parallel targeted test runs briefly waited on Cargo file locks. All final verification commands were run successfully.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None. Stub-pattern scan found no functional placeholders in the touched files.

## Next Phase Readiness

Plan 12-02 can now add the project-wide module graph provider and switch `resolved_imports`/`module_graph` support from deterministic `Unsupported` rows to real provider-backed support when facts are populated before rule execution.

## Self-Check: PASSED

- Found `.planning/phases/12-resolved-imports-and-module-relationships/12-01-SUMMARY.md`.
- Found key modified files for core facts, SDK views, SDK exports, macro mapping, and analysis planning.
- Found task commits `409ceca`, `b06d2bc`, `672b3ae`, `5a6775c`, `334929f`, and `c2117d6`.

---
*Phase: 12-resolved-imports-and-module-relationships*
*Completed: 2026-05-11*
