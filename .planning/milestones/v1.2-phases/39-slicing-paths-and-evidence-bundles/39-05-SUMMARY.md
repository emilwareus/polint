---
phase: 39-slicing-paths-and-evidence-bundles
plan: 05
subsystem: static-analysis-engine
tags: [rust, diagnostics, evidence, json, sarif]

requires:
  - phase: 39-02-local-evidence-graph
    provides: evidence store and bundle rows
  - phase: 39-03-bounded-paths
    provides: path and ranking rows
  - phase: 39-04-summary-context
    provides: summary expansion and context path metadata
provides:
  - Internal diagnostic evidence bundle references
  - Versioned structured `evidence_v1` JSON payloads
  - SARIF code-flow projection for structured evidence
affects: [phase-39-extension-merge, phase-39-eval]

tech-stack:
  added: []
  patterns: [versioned optional JSON object, bounded renderer, lossy SARIF projection]

key-files:
  created:
    - crates/polint/src/analysis/evidence/render.rs
  modified:
    - crates/polint/src/analysis/evidence/mod.rs
    - crates/polint/src/diagnostics/mod.rs
    - crates/polint/src/analysis/refined_calls/validate.rs

key-decisions:
  - "Structured evidence is optional under `evidence_v1`, preserving existing scalar `evidence` compatibility."
  - "Diagnostic bundle references are internal side-table handles and do not affect stable fingerprints."
  - "SARIF renders evidence as a lossy codeFlow/threadFlow projection with explicit partial/unknown/omitted wording."

patterns-established:
  - "Evidence JSON is bounded by explicit path, edge, unknown, and omitted-region limits."
  - "Evidence JSON includes bundle status, precision, provenance, validation, replay key, path hidden-node counts, edge summary expansion, unknowns, and omitted regions."
  - "Evidence render tests assert deterministic output and avoid absolute roots, raw source bodies, parser object ids, and timestamps."

requirements-completed: [SAE-PREC-04]

duration: 7min
completed: 2026-05-25
---

# Phase 39-05: Diagnostic Evidence Bundles JSON And SARIF Rendering Summary

**Diagnostics can now carry versioned structured evidence and project it to JSON/SARIF**

## Performance

- **Duration:** 7 min
- **Started:** 2026-05-25T14:35:20Z
- **Completed:** 2026-05-25T14:42:35Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added internal diagnostic bundle references and optional `evidence_v1` structured payloads without changing scalar evidence behavior.
- Added deterministic evidence JSON rendering with bounded paths, edges, unknowns, omitted regions, replay keys, path status, precision, provenance, hidden-node counts, and summary expansion keys.
- Added scalar evidence bridge helpers for compatibility renderers.
- Extended SARIF output to include evidence-derived `codeFlows` / `threadFlows` with lossy partial, unknown, opaque, omitted, and budget wording.
- Added tests proving fingerprint stability, scalar evidence compatibility, deterministic/bounded JSON, SARIF code-flow projection, and CLI JSON/SARIF compatibility.

## Task Commits

1. **Tasks 1-3: Diagnostic evidence JSON and SARIF rendering** - `8803ecd` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/evidence/render.rs` - Versioned evidence JSON renderer, scalar bridge, and SARIF message projection.
- `crates/polint/src/analysis/evidence/mod.rs` - Registers evidence rendering module.
- `crates/polint/src/diagnostics/mod.rs` - Adds internal evidence attachment fields and SARIF code-flow rendering.
- `crates/polint/src/analysis/refined_calls/validate.rs` - Updates direct diagnostic initializer for new internal fields.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib diagnostics --locked` - passed
- `cargo test -p polint --lib analysis::evidence::render --locked` - passed
- `cargo test -p polint --test cli sarif_output_includes_ci_fields_and_honors_fail_threshold --locked` - passed
- `cargo test -p polint --test cli check_json_output_is_deterministic_across_repeated_runs --locked` - passed
- `cargo clippy -p polint --lib --locked -- -D warnings` - passed

## Deviations from Plan

- The plan referenced `crates/polint/src/reporting.rs`, but this repository keeps JSON/SARIF rendering in `crates/polint/src/diagnostics/mod.rs`. The implementation followed the actual codebase boundary.

## Issues Encountered

- Direct `Diagnostic` struct initialization in refined-call validation needed the new internal evidence fields populated with `None`.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Wave 6 can add extension evidence merge and validation using the structured renderer and internal bundle bridge.

---
*Phase: 39-slicing-paths-and-evidence-bundles*
*Completed: 2026-05-25*
