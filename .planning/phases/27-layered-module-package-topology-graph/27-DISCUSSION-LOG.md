# Phase 27: Layered Module/Package/Topology Graph - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-19
**Phase:** 27-Layered Module/Package/Topology Graph
**Areas discussed:** Provider Boundary, Root And Package Model, Dependency Layers, Import-To-Package Classification, Topology Overlays And Public Surface
**Mode:** `--auto`

---

## Provider Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Deepen internal module graph provider | Extend `polint.module_graph` with layered topology facts while keeping public behavior stable. | yes |
| Create public topology/query surface now | Promote new SDK/CLI graph contracts during this phase. | |
| Separate every topology layer into standalone providers immediately | Split provider boundaries before fact model and cache contracts are proven. | |

**User's choice:** Auto-selected recommended default: deepen the internal module graph provider.
**Notes:** This carries forward Phases 20, 24, 25, and 26 public-boundary decisions.

---

## Root And Package Model

| Option | Description | Selected |
|--------|-------------|----------|
| Layered internal facts | Add workspace root, package/project, and source-set facts with stable IDs and precision/status metadata. | yes |
| Overload existing ModuleNode labels | Encode richer topology only through existing labels/kinds. | |
| Wait for later language-specific phases | Defer topology modeling despite Phase 27 success criteria. | |

**User's choice:** Auto-selected recommended default: layered internal facts.
**Notes:** Root detection must be deterministic and fail closed; Go monorepo behavior follows the project Go lifecycle contract.

---

## Dependency Layers

| Option | Description | Selected |
|--------|-------------|----------|
| Separate declared/resolved/usage layers | Keep manifests, lockfile/tool selections, and imports as distinct evidence layers. | yes |
| Collapse into one DependsOn edge | Treat every dependency relation as the same edge kind. | |
| Only model imports for now | Ignore declared and selected package dependencies. | |

**User's choice:** Auto-selected recommended default: separate declared requirements, resolved edges, and import usage.
**Notes:** This matches `research/module-graph/FINAL-REPORT.md` and avoids false precision.

---

## Import-To-Package Classification

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit classified import-to-package facts | Emit per-edge kind/status/precision for source, test, generated, vendor, external, unresolved, setup-missing, dynamic, unsupported, ambiguous, and undeclared cases. | yes |
| Infer classification only at query time | Delay classification until later query views. | |
| Only classify local vs external | Keep the classification too coarse for Phase 27 success criteria. | |

**User's choice:** Auto-selected recommended default: explicit classified import-to-package facts.
**Notes:** Existing `ResolvedImportFact` and public SDK views must remain compatible.

---

## Topology Overlays And Public Surface

| Option | Description | Selected |
|--------|-------------|----------|
| Internal overlays and no public promotion | Model overlays privately for future ownership/layer/deploy-unit work and avoid new public SDK/CLI contracts. | yes |
| Public SDK views immediately | Promote `Packages`, `Dependencies`, `SourceSets`, or `RepoTopology` now. | |
| Defer overlays entirely | Skip overlay shape despite the phase goal naming overlays. | |

**User's choice:** Auto-selected recommended default: internal overlays and no public promotion.
**Notes:** Real extension activation and public topology ergonomics remain later-phase work.

---

## the agent's Discretion

- The planner may choose exact submodule names and sequencing.
- The planner may decide whether topology remains under existing `module_graph` files or moves into narrower internal submodules.
- The planner may defer non-Go and non-TS/JS ecosystems, exact dynamic package-manager behavior, and public topology surfaces.

## Deferred Ideas

- Python, Java/JVM, Cargo, Maven/Gradle, Nx/Turborepo, Pants/Bazel, and broader monorepo/task graph support.
- Real extension-provider activation and extension-aware merge/quarantine.
- Public topology SDK views, topology CLI commands, and broad advanced query builders.
