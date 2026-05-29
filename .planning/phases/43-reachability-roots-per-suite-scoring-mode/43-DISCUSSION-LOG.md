# Phase 43: Reachability, Roots & Per-Suite Scoring Mode - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-29
**Phase:** 43-reachability-roots-per-suite-scoring-mode
**Mode:** `--auto` (autonomous; Claude selected the recommended option for every gray area, no interactive prompts)
**Areas discussed:** Reachability-roots module & fact shape, Root discovery sources & per-language semantics, Per-suite scoring mode + reachable-graph marking, Determinism gate design & cross-phase inheritance

---

## Reachability-Roots Module & Fact Shape (REACH-01)

| Option | Description | Selected |
|--------|-------------|----------|
| New `analysis::reachability` module + `polint.reachability` provider | Fresh `pub(crate)` module; closed `RootKind` enum; compose v1.2 IDs by reference | ✓ |
| Extend `analysis::entrypoints` | Add root kinds onto the existing entrypoint fact family | |
| Reuse `analysis::domains` reachability domain | Repurpose the block-level `polint.domain.reachability` abstract domain | |

**Auto-selected:** New `analysis::reachability` module.
**Notes:** The existing `polint.domain.reachability` is block-level (in-body) reachability — a different concept from whole-program reachability-from-roots. Mandatory naming-collision guard (D-02). Closed `RootKind` enum mirrors the Phase 42 `IdentityCategory` byte-stability discipline.

---

## Root Discovery Sources & Per-Language Semantics (REACH-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Compose existing facts (entrypoints + Go funcs/packages + TS/JS exports + configured roots) | Discover from facts already produced; honest status/precision labels | ✓ |
| Re-parse sources for roots | Independent root extraction pass over Go/TS ASTs | |
| Entrypoint-substrate only | Use only Phase 35 entrypoints, skip main/init/exported | |

**Auto-selected:** Compose existing facts.
**Notes:** Go `main`/`init`/exported from existing function+package facts; TS/JS `exported` from symbol+module-graph; `Test`/`FrameworkEntrypoint` bridged from Phase 35; `ConfiguredEntrypoint` from a minimal new `.polint.toml` input. TS/JS `main`/`init` intentionally not synthesized (deferred to Phase 45 / configured roots). No new parsing.

---

## Per-Suite Scoring Mode + Reachable-Graph Marking (REACH-02)

| Option | Description | Selected |
|--------|-------------|----------|
| Required `scoring_mode` enum on `SuiteManifest` + separate marking fact family | Non-`Option` field (structural + explicit validation); `CallReachabilityFact` keyed by call-site stable key; compute reachable set over direct-call edges | ✓ |
| Optional `scoring_mode` with default | Field defaults to `whole-repo` when absent | |
| Mutate `analysis::calls` in place to add a reachable flag | Add `in_reachable_graph` directly onto call facts | |

**Auto-selected:** Required field + separate marking fact family.
**Notes:** `deny_unknown_fields` + non-`Option` makes "gate fails if missing" structural; an explicit negative test is added on top. Mode semantics: `oracle-rta` filters by reachable set, `oracle-jelly` does not filter, `whole-repo` scores everything. Marking composes over call facts by stable key (Phase 42 pattern), never mutates them. Pre-solver edge set = direct calls; later phases swap in solver edges behind the same contract.

---

## Determinism Gate Design & Cross-Phase Inheritance (REACH-03)

| Option | Description | Selected |
|--------|-------------|----------|
| Parametric harness driven by `provider_manifests()` | 10 seeded permutations; byte-identical observed JSON; auto-enrolls future providers; reserve solver step/budget JSON fields now | ✓ |
| Hand-maintained provider list per phase | Explicit list of providers to shuffle, edited each phase | |
| Per-provider determinism tests only | Keep the existing per-provider shuffle tests; no milestone-wide gate | |

**Auto-selected:** Parametric `provider_manifests()`-driven harness.
**Notes:** Inheritance is the whole point of REACH-03 — driving the gate off the manifest means Phases 44–54 are covered for free. Reserve `solver_step_count`/`budget_exceeded_reasons` (defaulted) so byte-identity holds across the milestone. Fast CI on Linux + macOS, both pass independently. Per-phase obligation documented in the gate file (D-25).

---

## Claude's Discretion

- Internal file layout of `analysis::reachability/`.
- Whether `RootKind` reuses entrypoint precision/status enums or defines parallel ones.
- Exact `.polint.toml` configured-roots schema.
- Precise provider-order permutation plumbing (the 10-shuffle byte-identical contract is fixed).
- Whether marking is a standalone fact family or a reachable-set index by stable key.
- Plan slicing into ~3 plans (roots+discovery+provider / traversal+marking+scoring-mode / determinism gate+fixtures+CI).

## Deferred Ideas

- Reachability fixpoint over solver-derived edges → Phase 48 (GO-05).
- Shared semantic graph + constraint vocabulary → Phase 44.
- JS/TS inventory/scope/module-graph (richer TS/JS entry notion) → Phase 45.
- Go semantic frontend + sidecar (full module import-path qualification) → Phase 46.
- Unified solver core + `DerivedEdgeProvenance` (inherits this determinism gate) → Phase 47.
- `solver_step_count`/`budget_exceeded_reasons` population → Phase 47+.
- Consolidated unknown taxonomy + `polint inspect unknowns --format json` → Phase 52.
- Per-suite precision floors, F-score β=0.5, polyglot canary, final leak gate → Phase 54.
- Public SDK promotion of any v1.3 type → out of v1.3.
