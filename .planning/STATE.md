---
gsd_state_version: 1.0
milestone: v1.2
milestone_name: Static Analysis Engine Implementation
status: executing
last_updated: "2026-05-16T19:50:33.390Z"
last_activity: 2026-05-16 -- Phase 20 planning complete
progress:
  total_phases: 22
  completed_phases: 0
  total_plans: 2
  completed_plans: 0
  percent: 0
---

# State: polint

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-16)

**Core value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

**Current focus:** v1.2 Static Analysis Engine Implementation, sourced from `research/ROADMAP.md`.

## Current Status

- **GitHub:** `emilwareus/polint` (public repository name).
- Active branch policy: do not push directly to `main` unless explicitly instructed; create a feature/fix branch before sharing remote work.
- v1.0 MVP was audited, archived, tagged, and closed on 2026-05-02.
- v1.1 Capability Fulfillment completed the capability plan, resolved imports/module graph, and symbols/references foundations.
- Static-analysis engine research completed on 2026-05-16 in `research/ROADMAP.md`.
- v1.2 requirements are defined in `.planning/REQUIREMENTS.md`.
- v1.2 roadmap is defined in `.planning/ROADMAP.md`.
- Each v1.2 research PR maps to one GSD phase, in order, from Phase 20 through Phase 41.
- New broad research is not needed by default. Use the relevant research documents referenced by each phase; do additional research only for a concrete implementation gap.

## Current Position

Milestone: v1.2 Static Analysis Engine Implementation
Status: Ready to execute
Phase: 20
Plan: Not started
Last activity: 2026-05-16 -- Phase 20 planning complete

## Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 20 | Pending | Private analysis kernel facade; requirement SAE-FND-01 |
| 21 | Pending | Provenance, precision, and validation metadata; requirement SAE-FND-02 |
| 22 | Pending | Internal evaluation harness MVP; requirement SAE-FND-03 |
| 23 | Pending | Input snapshots and cache-key vocabulary; requirement SAE-FND-04 |
| 24 | Pending | Persistent layer cache for existing cheap facts; requirement SAE-FND-05 |
| 25 | Pending | Rule manifest, inspect, and test skeleton; requirement SAE-FND-06 |
| 26 | Pending | Semantic index deepening; requirement SAE-SEM-01 |
| 27 | Pending | Layered module/package/topology graph; requirement SAE-SEM-02 |
| 28 | Pending | Private semantic MIR and place identity; requirement SAE-SEM-03 |
| 29 | Pending | Local CFG and control dependence; requirement SAE-SEM-04 |
| 30 | Pending | Direct call facts; requirement SAE-SEM-05 |
| 31 | Pending | P0 abstract-domain kernel; requirement SAE-INT-01 |
| 32 | Pending | Summary kernel and direct summaries; requirement SAE-INT-02 |
| 33 | Pending | Demand queries and summary SCC cache; requirement SAE-INT-03 |
| 34 | Pending | Rust extension/provider sink; requirement SAE-INT-04 |
| 35 | Pending | Framework entrypoints and trust boundaries; requirement SAE-INT-05 |
| 36 | Pending | P0 type/value/place/alias substrate; requirement SAE-PREC-01 |
| 37 | Pending | Refined call graph providers; requirement SAE-PREC-02 |
| 38 | Pending | Local plus summary-projected data flow; requirement SAE-PREC-03 |
| 39 | Pending | Slicing, paths, and evidence bundles; requirement SAE-PREC-04 |
| 40 | Pending | External benchmark adapters and promotion gates; requirement SAE-PROM-01 |
| 41 | Pending | Public SDK query views and agent ergonomics; requirement SAE-PROM-02 |

## Accumulated Context

- Product code and GSD planning documents live together in the repository root on `main`.
- Public API discipline is strict: use `pub(crate)` for internals, promote only curated SDK/runner surfaces, and fix `unreachable_pub` by tightening visibility.
- Rule-author examples and temp-repo tests must consume `polint::sdk::prelude::*` and `polint::runner::run_cli`, not internal modules.
- Capability names must stay honest: unsupported or setup-missing hard capabilities produce capability diagnostics rather than placeholder facts.
- Comment ignores are an engine/reporting concern; individual rules should report the diagnostics they find.
- Go semantic lifecycle must support monorepos without requiring a root `go.mod`; module roots are inferred or configured in `.polint.toml`.
- New analysis modules for v1.2 should stay private until validation and promotion gates justify public SDK or CLI exposure.
- Every new fact family should carry stable IDs, precision/status/provenance, deterministic ordering, cache inputs, validation fixtures, and explicit unknown states.

## Next Action

Discuss and plan Phase 20:

`/gsd-discuss-phase 20`

or plan directly:

`/gsd-plan-phase 20`
