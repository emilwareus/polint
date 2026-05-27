# Roadmap: polint

## Milestones

- [x] **v1.0 MVP** - repo-local static analysis framework for Go and TypeScript/JavaScript, shipped 2026-05-02. Archive: [v1.0 roadmap](milestones/v1.0-ROADMAP.md).
- [x] **v1.1 Capability Fulfillment** - capability planning, resolved imports/module graph, and symbol/reference foundations for Go and TS/JS.
- [x] **v1.2 Static Analysis Engine Implementation** - private, validated, cache-aware, agent-extensible analysis engine substrate; 22 phases and 136 plans shipped 2026-05-27. Archive: [v1.2 roadmap](milestones/v1.2-ROADMAP.md).

## Current Status

v1.2 is complete and archived. There are no active milestone phases in this roadmap. Start the next milestone with `/gsd-new-milestone` so a fresh `.planning/REQUIREMENTS.md` and phase roadmap can be defined.

## Archived Phase Summary

<details>
<summary>v1.2 Static Analysis Engine Implementation (Phases 20-41) - shipped 2026-05-27</summary>

| Phase | Name | Plans | Completed |
|-------|------|-------|-----------|
| 20 | Private Analysis Kernel Facade | 2/2 | 2026-05-16 |
| 21 | Provenance, Precision, and Validation Metadata | 4/4 | 2026-05-17 |
| 22 | Internal Evaluation Harness MVP | 6/6 | 2026-05-17 |
| 23 | Input Snapshots and Cache-Key Vocabulary | 5/5 | 2026-05-18 |
| 24 | Persistent Layer Cache for Existing Cheap Facts | 5/5 | 2026-05-18 |
| 25 | Rule Manifest, Inspect, and Test Skeleton | 4/4 | 2026-05-18 |
| 26 | Semantic Index Deepening | 6/6 | 2026-05-19 |
| 27 | Layered Module/Package/Topology Graph | 7/7 | 2026-05-19 |
| 28 | Private Semantic MIR and Place Identity | 7/7 | 2026-05-20 |
| 29 | Local CFG and Control Dependence | 6/6 | 2026-05-20 |
| 30 | Direct Call Facts | 8/8 | 2026-05-21 |
| 31 | P0 Abstract-Domain Kernel | 5/5 | 2026-05-21 |
| 32 | Summary Kernel and Direct Summaries | 7/7 | 2026-05-21 |
| 33 | Demand Queries and Summary SCC Cache | 7/7 | 2026-05-24 |
| 34 | Rust Extension/Provider Sink | 6/6 | 2026-05-23 |
| 35 | Framework Entrypoints and Trust Boundaries | 8/8 | 2026-05-24 |
| 36 | P0 Type/Value/Place/Alias Substrate | 7/7 | 2026-05-24 |
| 37 | Refined Call Graph Providers | 6/6 | 2026-05-25 |
| 38 | Local Plus Summary-Projected Data Flow | 10/10 | 2026-05-25 |
| 39 | Slicing, Paths, and Evidence Bundles | 7/7 | 2026-05-25 |
| 40 | External Benchmark Adapters and Promotion Gates | 8/8 | 2026-05-26 |
| 41 | Public SDK Query Views and Agent Ergonomics | 5/5 | 2026-05-26 |

</details>
