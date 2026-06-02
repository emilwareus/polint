---
phase: 46-go-semantic-frontend-sidecar
plan: 03
subsystem: go-analysis
tags: [rust, go, semantic-graph, identity, lowering]
requires:
  - phase: 46-go-semantic-frontend-sidecar
    provides: 46-01 semantic sidecar rows and 46-02 Rust protocol/client
provides:
  - Private Go semantic fact families, validation, normalization, and lifecycle digest helper
  - Lowering from validated sidecar rows to stable-keyed private facts
  - Semantic-graph projection for matched Go callsites and static callees
  - Go identity package import-path preference when semantic package rows are available
affects: [go-semantic, semantic-graph, identity, go-rta-eval]
tech-stack:
  added: []
  patterns: [private-fact-store, stable-key-normalization, matched-core-id-projection]
key-files:
  created:
    - crates/polint/src/go/semantic/facts.rs
    - crates/polint/src/go/semantic/store.rs
    - crates/polint/src/go/semantic/lower.rs
    - crates/polint/src/go/semantic/validate.rs
    - crates/polint/src/go/semantic/cache_key.rs
  modified:
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis/semantic_graph/build.rs
    - crates/polint/src/analysis/identity/provider.rs
    - crates/polint/src/analysis/identity/render/go_relstring.rs
    - crates/polint/src/eval/external/go_x_tools_callgraph.rs
    - crates/polint/src/go/semantic/protocol.rs
    - crates/polint/src/go/semantic/mod.rs
key-decisions:
  - Project Go semantic graph evidence only when private semantic facts match existing core function/callsite identities.
  - Keep dynamic/interface calls unresolved for Phase 48; do not fabricate solver-derived call edges.
  - Prefer semantic package import paths for identity while preserving package-name and path fallbacks.
patterns-established:
  - Private Go semantic stores normalize by stable key before assigning dense IDs.
  - Sidecar file paths must validate as repo-relative before facts are accepted.
requirements-completed: [GO-02]
duration: 70min
completed: 2026-06-01
---

# Phase 46 Plan 03 Summary

**Private Go semantic facts now lower into stable graph constraints and Go identity import paths**

## Performance

- **Duration:** 70 min
- **Completed:** 2026-06-01T12:34:19Z
- **Tasks:** 4
- **Files modified:** 12

## Accomplishments

- Added private package/function/callsite/method-set/package-error fact families plus normalized store validation.
- Added `lower_go_semantic` to convert decoded sidecar rows into facts with repo-relative path validation and package-error preservation.
- Extended semantic graph building with Go semantic projection for matched core callsites; static callees emit target evidence and dynamic calls remain unresolved.
- Updated Go identity to prefer semantic package import paths when available, with existing fallbacks preserved.

## Task Commits

1. **Plan 03 implementation** - `2423c0c1` (feat)

## Files Created/Modified

- `crates/polint/src/go/semantic/facts.rs` - Private Go semantic fact structs and status vocabulary.
- `crates/polint/src/go/semantic/store.rs` - Normalized private store.
- `crates/polint/src/go/semantic/lower.rs` - Protocol row lowering and path checks.
- `crates/polint/src/go/semantic/validate.rs` - Duplicate key and repo-escaping path validation.
- `crates/polint/src/go/semantic/cache_key.rs` - Lifecycle digest helper for semantic inputs.
- `crates/polint/src/analysis/semantic_graph/build.rs` - Go semantic projection into graph constraints.
- `crates/polint/src/analysis/identity/provider.rs` - Go package import-path preference from semantic packages.

## Decisions Made

- Projection is intentionally conservative: facts must match existing core IDs by file/span/name before they affect the graph.
- The RTA benchmark adapter still emits bare oracle names because x/tools `WANT:` expectations are bare-name based.

## Deviations from Plan

- The implementation keeps provider/kernel wiring deferred; `replace_go_semantic_facts` exists and is covered, but the sidecar client is not invoked by the main kernel path yet. This preserves the Plan 03 private-lowering boundary without introducing a partially cached provider.
- Type and method-set facts are stored privately but not projected into `TypeConstraint` yet; there is no existing type-substrate bridge to reference without fabricating `TypeFactId`s.

## Issues Encountered

- Cargo accepts only one test filter per invocation, so multi-filter verification was run as separate focused commands.
- Clippy flagged future-facing private storage as dead code; fields now have narrow accessors/reasons where provider wiring is intentionally later.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 04 can add the explicit cache key, diagnostics taxonomy, fixture coverage, determinism gates, and public API leak checks around the now-private semantic fact path.

---
*Phase: 46-go-semantic-frontend-sidecar*
*Completed: 2026-06-01*
