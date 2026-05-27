---
phase: 27-layered-module-package-topology-graph
plan: 05
subsystem: analysis-kernel
tags: [rust, module-graph, module-topology, semantic-imports, layer-cache, validation]

requires:
  - phase: 27-layered-module-package-topology-graph
    provides: Base module graph topology facts, cache identity inputs, and package/source-set modeling from Plans 27-01 through 27-04.
  - phase: 26-semantic-index-deepening
    provides: Semantic import rows consumed after symbol graph execution.
provides:
  - Semantic-aware import-to-package classification with explicit uncertainty states.
  - Crate-private polint.module_topology provider scheduled after symbol graph and before metrics.
  - Module topology cache keys, payload restore, run-report cache stats, and output digests.
  - Topology referential integrity, path, precision-ceiling, stable-key, and cache-restore validation.
affects: [analysis-kernel, module-graph, topology-facts, layer-cache, eval-provider-order]

tech-stack:
  added: []
  patterns:
    - Post-symbol derived provider for topology facts that require semantic rows.
    - Normalized module topology cache payloads restored through AnalysisDb replacement APIs.
    - Topology validation diagnostics with family, stable_key, field, and reason evidence.

key-files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/module_graph/model.rs
    - crates/polint/src/module_graph/topology.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/observed.rs
    - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml

key-decisions:
  - "Add semantic-aware import-to-package facts in a crate-private polint.module_topology provider instead of widening public module graph contracts."
  - "Run module topology after polint.symbol_graph so semantic import rows are available without creating a provider cycle."
  - "Reject duplicate cached import-to-package stable keys before restore so stale or conflicting topology payloads are recomputed."

patterns-established:
  - "Derived provider rows record cache stats and output digests in KernelRunReport like other cache-backed kernel stages."
  - "Topology fact validation fails closed on malformed refs, paths, precision claims, and stable-key conflicts."

requirements-completed: [SAE-SEM-02]

duration: 23min
completed: 2026-05-19
---

# Phase 27 Plan 05: Module Topology Provider Summary

**Semantic-aware import-to-package topology with a post-symbol provider, cache-backed payloads, and fail-closed validation**

## Performance

- **Duration:** 23 min
- **Started:** 2026-05-19T12:55:03Z
- **Completed:** 2026-05-19T13:18:16Z
- **Tasks:** 3
- **Files modified:** 12

## Accomplishments

- Added import-to-package derivation that joins syntax imports, resolved imports, semantic import rows, source sets, workspace packages, and external package requirements.
- Added the crate-private `polint.module_topology` provider, scheduled after `polint.symbol_graph`, with cache keys, payload restore, provider output metadata, and deterministic provider-order fixture updates.
- Added topology validation for references, repo-relative paths, precision ceilings, `Undeclared` consistency, stable-key conflicts, and cached payload duplicate-key rejection.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add semantic-aware import-to-package classification** - `cc5b951` (test), `cc37dfc` (feat)
2. **Task 2: Add polint.module_topology provider, kernel order, and cache** - `0e9fbf5` (test), `d7c318f` (feat)
3. **Task 3: Validate topology referential integrity and precision ceilings** - `bb7b07a` (test), `fa717b7` (feat)

## Files Created/Modified

- `crates/polint/src/module_graph/mod.rs` - Added semantic import-to-package derivation, module topology cache derivation, payload restore, and cache payload validation.
- `crates/polint/src/module_graph/topology.rs` - Extended import-to-package topology rows with bridge fields, stable keys, explicit statuses, context, precision, and provenance.
- `crates/polint/src/module_graph/model.rs` - Added the module topology cache payload model.
- `crates/polint/src/core/mod.rs` - Updated core fact storage/replacement behavior for import-to-package topology rows.
- `crates/polint/src/analysis_kernel/provider.rs` - Added the crate-private `polint.module_topology` provider manifest.
- `crates/polint/src/analysis_kernel/mod.rs` - Wired module topology into the kernel after symbol graph and recorded provider output metadata.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Added module topology layer key construction over topology, import, semantic, config, lifecycle, module, and symbol inputs.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Carried provider output metadata and cache stats for the new provider row.
- `crates/polint/src/analysis_kernel/validation.rs` - Added topology referential integrity, path, precision, `Undeclared`, and stable-key validation.
- `crates/polint/src/eval/fixtures.rs` - Updated provider-order fixture handling for the new provider.
- `crates/polint/src/eval/observed.rs` - Updated observed provider-order invariants.
- `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml` - Updated expected provider order to include `polint.module_topology`.

## Verification

- `cargo test -p polint --lib module_graph::import_to_package --locked`
- `cargo test -p polint --lib module_topology_layer_cache --locked`
- `cargo test -p polint --lib topology_stable_key_conflicts --locked`
- `cargo test -p polint --lib module_topology_layer_cache_rejects_duplicate_stable_keys --locked`
- `cargo test -p polint --lib analysis_kernel::validation::topology --locked`
- `cargo test -p polint --lib analysis_kernel::validation --locked`
- `cargo test -p polint --lib provider_order --locked`
- `cargo test -p polint --lib kernel_run_report_module_topology_row_carries_layer_cache_stats --locked`
- `cargo test -p polint --lib module_topology_layer_key_changes_on_import_topology_module_symbol_and_semantic_inputs --locked`
- `cargo fmt --all -- --check`

## Decisions Made

- Kept `polint.module_topology` crate-private and internal to the analysis kernel, preserving public API discipline.
- Used a separate post-symbol provider rather than changing the base module graph pass, because semantic rows are not available until after symbol graph execution.
- Rejected duplicate cached import-to-package stable keys before restore, preventing cache reuse when payload identity is ambiguous.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The literal acceptance filter `analysis_kernel::validation::topology_stable_key_conflicts` selected zero tests, so verification used `topology_stable_key_conflicts` to run the intended test and also ran the broader `analysis_kernel::validation::topology` target.
- `rustfmt --check` reported one formatting-only diff after Task 3 implementation; `cargo fmt --all` resolved it before final verification.

## Known Stubs

None. Stub-pattern matches in touched files were pre-existing test fixture snippets or placeholder wording in test names, not plan-introduced runtime stubs.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 27-06 can verify that the new topology provider remains crate-private and does not leak through public JSON, SDK, runner, or docs surfaces.

## Self-Check: PASSED

- Summary file exists at `.planning/phases/27-layered-module-package-topology-graph/27-05-SUMMARY.md`.
- Task commits found: `cc5b951`, `cc37dfc`, `0e9fbf5`, `d7c318f`, `bb7b07a`, `fa717b7`.
- Modified files listed above exist in the working tree.

---
*Phase: 27-layered-module-package-topology-graph*
*Completed: 2026-05-19*
