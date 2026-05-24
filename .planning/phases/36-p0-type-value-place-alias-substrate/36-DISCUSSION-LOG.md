# Phase 36: P0 Type/Value/Place/Alias Substrate - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-05-24
**Phase:** 36-p0-type-value-place-alias-substrate
**Areas discussed:** Fact family boundary, Type and narrowing scope, Value/allocation/access-path model, Points-to and alias strategy, Provider/cache/extension integration, Validation/eval/public boundary
**Mode:** `--auto` selected all gray areas and chose recommended defaults without user prompts.

---

## Fact Family Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Layered private substrate | Add explicit type, narrowed type, value, allocation, access-path, points-to, and alias facts while extending existing places. | X |
| Single alias pass | Focus implementation around one alias/points-to pass and derive other precision later. | |
| Public view first | Promote SDK query views immediately and backfill internals. | |

**User's choice:** Auto-selected layered private substrate.
**Notes:** This matches the roadmap and research recommendation: type/value/place/narrowing facts come before expensive points-to and public SDK promotion.

---

## Type and Narrowing Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Language-owned normalized facts | Normalize Go and TS/JS type/narrowing evidence into polint-owned rows with explicit precision and setup states. | X |
| Raw tool output | Surface Go/TS tool outputs directly as the internal/public model. | |
| Syntax-only type hints | Keep only cheap syntax labels and defer narrowing. | |

**User's choice:** Auto-selected language-owned normalized facts.
**Notes:** Official tooling is allowed where it is the compatibility authority, but its output must cross a polint validation/cache/provenance boundary.

---

## Value, Allocation, and Access Paths

| Option | Description | Selected |
|--------|-------------|----------|
| Stable tokens and explicit access paths | Model function/class/module/object allocations and access paths as first-class facts over existing places. | X |
| Embed everything in PlaceFact | Keep projections only inside place rows and avoid separate access-path/value facts. | |
| Solver-only objects | Let points-to constraints invent object identity without separate allocation facts. | |

**User's choice:** Auto-selected stable tokens and explicit access paths.
**Notes:** Existing `PlaceFact` remains the identity anchor, but later data-flow and evidence phases need directly referenceable allocation/access-path facts.

---

## Points-To and Alias Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Optional bounded provider plus alias query stack | Use cheap definitive alias answers first, then budgeted points-to only when needed. | X |
| Mandatory whole-repo points-to | Run global points-to for every baseline check. | |
| Alias as primary graph | Store an alias graph as the source of truth. | |

**User's choice:** Auto-selected optional bounded provider plus alias query stack.
**Notes:** Alias answers must include `NoAlias`, `MayAlias`, `MustAlias`, `PartialAlias`, and `Unknown`, with evidence and honest budget/setup statuses.

---

## Provider, Cache, and Extension Integration

| Option | Description | Selected |
|--------|-------------|----------|
| Native provider with validated extension facts | Add a provider with explicit manifest/cache identity and let Phase 34 extensions add validated precision facts. | X |
| Extension-only precision | Rely on repo-local extensions for most facts. | |
| Native-only precision | Exclude extension-provided type/value/alias facts until public promotion. | |

**User's choice:** Auto-selected native provider with validated extension facts.
**Notes:** Native facts remain authoritative; extensions can add precision but cannot delete native facts or bypass validation/precision ceilings/quarantine.

---

## Validation, Evaluation, and Public Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Full private validation/eval/no-leak proof | Validate references, precision, statuses, cache, eval fixtures, and public no-leak behavior before later promotion. | X |
| Minimal unit tests only | Add facts with narrow unit tests and defer eval/no-leak proof. | |
| Public docs now | Document public SDK usage in this phase. | |

**User's choice:** Auto-selected full private validation/eval/no-leak proof.
**Notes:** Phase 36 must preserve existing CLI/SDK behavior and defer public `Types`, `Values`, and `Aliases` views to Phase 41.

---

## The Agent's Discretion

- Exact module layout and plan split.
- Exact provider placement if extension-aware merge constraints require a split.
- How much official Go/TypeScript tooling is activated immediately versus represented as validated digest/input hooks.
- Whether points-to solver output is stored eagerly, query-scoped, or a hybrid, as long as baseline whole-repo solving remains optional.

## Deferred Ideas

- Refined call graph providers: Phase 37.
- Data-flow propagation and source/sink/sanitizer/barrier modeling: Phase 38.
- Slicing, evidence bundles, and path rendering: Phase 39.
- Benchmark promotion gates: Phase 40.
- Public typed SDK views and agent ergonomics: Phase 41.
