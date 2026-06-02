# Phase 47: Unified Solver Core & Derived-Edge Provenance - Pattern Map

**Mapped:** 2026-06-02
**Files analyzed:** 14 (new + modified)
**Analogs found:** 13 / 14 (1 partial: `polint explain` consumption)

This is a private Rust static-analysis library phase (crate `polint`). There is NO
frontend/UI. Every analog below lives under `crates/polint/src/`. All new types are
`pub(crate)`; the Phase 42 public-surface-leak gate must stay green (D-16).

The single most important framing for the planner: **the solver core is a generalize-
and-fold, not a from-scratch build.** `analysis::points_to::solver` already implements
the deterministic `VecDeque` worklist + budget + fixpoint + determinism test. Phase 47
lifts that shape into a unified `analysis::solver` and folds points-to in as the first
`SolverPolicy` impl (D-03) — points-to fixtures must stay byte-identical.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `analysis/solver/mod.rs` (new) | module-root + naming-collision doc | n/a | `analysis/semantic_graph/mod.rs` | exact |
| `analysis/solver/engine.rs` (new) | service (worklist fixpoint) | batch / fixpoint | `analysis/points_to/solver.rs` | exact (the engine being folded) |
| `analysis/solver/budget.rs` (new) | model (closed-enum + struct) | transform | `analysis/points_to/facts.rs` (`PointsToBudget`/`PointsToBudgetStatus`) | exact |
| `analysis/solver/policy.rs` (new) | trait scaffolding + 1 impl + 2 stubs | event-driven | `analysis/semantic_graph/constraints.rs` (closed-enum discipline) | role-match |
| `analysis/solver/provenance.rs` (new) | model (fact with stable-key total order) | transform | `analysis/points_to/facts.rs` (`PointsToConstraintFact`) + `analysis/stable_key.rs` | exact |
| `analysis/solver/facts.rs` (new) | model (derived-edge fact family) | CRUD | `analysis/semantic_graph/facts.rs` + `constraints.rs` (`ConstraintFact`) | exact |
| `analysis/solver/store.rs` (new) | store (output + dense-ID-after-sort) | CRUD | `analysis/semantic_graph/store.rs` (`SemanticGraphOutput`/`SemanticGraphStore`) | exact |
| `analysis/solver/provider.rs` (new) | provider (derive + digest) | request-response | `analysis/semantic_graph/provider.rs` | exact |
| `analysis/solver/cache_key.rs` (new) | config (parameter digest) | transform | `analysis/semantic_graph/cache_key.rs` | exact |
| `analysis/solver/validate.rs` (new) | validation pass | transform | `analysis/semantic_graph/validate.rs` | exact |
| `analysis/mod.rs` (modify) | module registration | n/a | itself (`pub(crate) mod semantic_graph;` line) | exact |
| `analysis/ids.rs` (modify) | id newtypes | n/a | itself (`SemanticConstraintId` block, lines 170-178) | exact |
| `analysis_kernel/provider.rs` (modify) | provider manifest + ~7 snapshot sites | n/a | `polint.semantic_graph` manifest entry (lines 657-687) | exact |
| `analysis_kernel/mod.rs` (modify) | provider dispatch wiring | request-response | semantic_graph dispatch block (lines 542-572) | exact |
| `cli/mod.rs` (modify) | private plumbing for `explain` | request-response | `explain(...)` (lines 1331-1370) | partial — see notes |
| `analysis_kernel/metadata.rs` (modify) | `FactFamily` enum entries | n/a | `FactFamily::PointsToSet` (line 87) | exact |

---

## Pattern Assignments

### `analysis/solver/engine.rs` (service, batch/fixpoint) — THE FOLD TARGET

**Analog:** `crates/polint/src/analysis/points_to/solver.rs` (the entire file; 455 lines).
This IS the engine being folded in (D-03). The unified core owns the worklist/budget/
policy abstraction; the points-to fixpoint becomes its first `SolverPolicy` impl. Per
D-03/Discretion, the planner may physically relocate this engine into `solver/` OR
invoke it in place as a registered sub-domain — provided points-to fixtures stay
byte-identical.

**Deterministic `VecDeque` worklist** (`solver.rs` lines 43-91): the worklist is
`queue: VecDeque<(PtVarId, BTreeSet<ObjectTokenId>)>`; all accumulation is `BTreeMap`/
`BTreeSet`-ordered. The drain loop is the exact shape the unified core copies:
```rust
fn solve(&mut self) -> PointsToSolveResult {
    self.initialize();
    while let Some((var, delta)) = self.queue.pop_front() {
        if !self.step_budget_ok() {
            break;
        }
        self.propagate_copy(var, &delta);
        self.propagate_load(var, &delta);
        // ... one propagation pass per constraint kind
    }
    self.to_result()
}
```

**Budget step-counter + exhaustion latch** (`solver.rs` lines 211-252): `add_all` checks
`self.budget_exceeded` first, then enforces `max_objects_per_var`; `step_budget_ok`
enforces `max_steps`. This is the template for the unified `SolverBudget` enforcement and
the bounded-outer-iteration cap (D-11).

**Status/precision projection from budget outcome** (`solver.rs` lines 254-269): the
`to_result` mapping `budget_exceeded → BudgetExceeded / Unknown` is the honest-precision
pattern (D-06). The derived-edge equivalent must reject `FactPrecision::Exact`.

**Determinism + budget-exhaustion unit tests** (`solver.rs` lines 308-453): two locked
tests — `solver_handles_core_constraint_vocabulary_deterministically` (asserts
`first == second`) and `solver_reports_budget_exhaustion_as_unknown_budget_status`. The
unified core ships analogous tests; the points-to versions must keep passing unchanged.

---

### `analysis/solver/budget.rs` (model) — generalize the points-to budget (D-05)

**Analog:** `crates/polint/src/analysis/points_to/facts.rs` lines 12-27 and 95-100.

**`PointsToBudget` struct + `Default`** (lines 12-27) — the knobs `SolverBudget`
generalizes (`max_steps`, `max_objects_per_var`, `max_dynamic_vars`):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PointsToBudget {
    pub(crate) max_steps: usize,
    pub(crate) max_objects_per_var: usize,
    pub(crate) max_dynamic_vars: usize,
}
impl Default for PointsToBudget { /* 10_000 / 64 / 512 */ }
```

**`PointsToBudgetStatus` closed enum** (lines 95-100) — the shape `BudgetStatus`
generalizes. Note the derived `Ord` + `#[repr(u8)]`-free byte-stability via pinned order:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum PointsToBudgetStatus {
    WithinBudget,
    BudgetExceeded,
    NotRun,
}
```
D-05 decision (planner's choice): alias or wrap `PointsToBudget`/`PointsToBudgetStatus`
as a sub-domain projection of the unified `SolverBudget`/`BudgetStatus`. Either is fine
provided points-to fixtures stay byte-identical and the unified budget carries the
cross-domain knobs (`max_steps`, bounded outer-iteration cap) plus a per-sub-domain
channel.

---

### `analysis/solver/policy.rs` (trait + 1 real impl + 2 honest stubs) (D-07)

**Analog (discipline, not shape):** `crates/polint/src/analysis/semantic_graph/constraints.rs`
lines 67-73 — `ConstraintKind::ModelEdge` is the canonical **honest-emptiness / reserved-
but-stubbed** precedent the Go/TS policies follow:
```rust
/// Reserved adaptation-model edge. No producer exists until Phase 49
/// (ADAPT-01); `build_semantic_graph` emits ZERO of these (honest emptiness,
/// D-11). The variant is fieldless because no model-edge payload is defined yet.
ModelEdge,
```
Phase 47 ships exactly ONE real `SolverPolicy` impl (the points-to sub-domain, D-03). The
Go (`go_rta`, Phase 48) and TS (`ts_tokens`, Phase 49) policies are documented stubs —
do NOT fake a driver. Mirror the doc-comment style above: name the reserving phase and
state the emptiness is intentional.

**Naming-collision guard (D-04)** — copy the top-of-module doc-comment pattern from
`crates/polint/src/analysis/semantic_graph/mod.rs` lines 13-39 verbatim in spirit:
distinguish the unified `analysis::solver` core (consumes GRAPH-02 `ConstraintKind`,
emits derived edges with provenance) from the points-to sub-domain's internal
`PointsToConstraintKind`/`PtVarId` language. The unified core sits *above* points-to.

---

### `analysis/solver/provenance.rs` (model, total-ordered) (D-08, D-09)

**Analog (fact shape):** `crates/polint/src/analysis/points_to/facts.rs` lines 7-14
(`PointsToConstraintFact { id, kind, status, precision, stable_key }`) and
`semantic_graph/constraints.rs` lines 178-188 (`ConstraintFact` with `#[serde(skip)]` on
the dense `id`).

**Analog (total-order recipe):** `crates/polint/src/analysis/stable_key.rs` lines 16-18
+ `analysis_kernel/metadata.rs` lines 370-384 (`stable_key_from_parts` — length-prefixed,
label-sorted, family-tagged, backslash-normalized). This is the recipe for ordering
contributing fact IDs by stable key:
```rust
pub(crate) fn semantic_stable_key(family: FactFamily, parts: &[(&str, String)]) -> StableFactKey {
    StableFactKey(stable_key_from_parts(family, parts))
}
```

**`DerivedEdgeProvenance` three roadmap-named fields** (D-08):
1. contributing fact IDs **totally ordered by stable ID** (sort by stable key, the
   Phase 42 dedup total-order rule — composition over duplication; reference existing
   stable identities, do not mint parallel ones),
2. the **constraint kind** that produced the edge — reuse
   `ConstraintKind::as_str()` from `semantic_graph/constraints.rs` lines 84-94 (it
   already returns a stable snake_case label), and
3. the **solver step** — a monotonic `u64` worklist step counter (sourced from the
   `steps` field in the folded engine, `points_to/solver.rs` line 55 / `step_budget_ok`).

**Stable-key emission concretely** — follow the points-to `to_result` call
(`points_to/solver.rs` lines 280-287):
```rust
stable_key: stable_key_from_parts(
    FactFamily::PointsToSet,
    &[("variable", variable.0.to_string()), ("budget", format!("{budget_status:?}"))],
),
```

---

### `analysis/solver/facts.rs` (model, derived-edge fact family)

**Analog:** `crates/polint/src/analysis/semantic_graph/constraints.rs` lines 178-188
(`ConstraintFact`) — mirror `{ #[serde(skip)] id, kind, status, precision, stable_key }`,
reusing the shared `PointsToStatus`/`PointsToPrecision` vocabulary (lines 1-5 imports)
rather than inventing redundant status/precision enums. The dense `id` carries
`#[serde(skip)]` so it never enters the digest (D-06); serde restores via `Default` (see
`ids.rs` lines 170-178). Derived edges reject `FactPrecision::Exact` (D-06).

---

### `analysis/solver/store.rs` (store, dense-IDs-after-sort)

**Analog:** `crates/polint/src/analysis/semantic_graph/store.rs` — `SemanticGraphOutput`
(line 19), `normalized()` (line 35, stable-key sort then assign dense IDs),
`SemanticGraphStore::from_output` (line 108, builds deterministic kind indexes +
referentially validates), and the shuffle-stability tests (`normalized_is_shuffle_stable`
line 320, `normalized_constraints_are_shuffle_stable` line 460). The `PROVIDER_ID` const
pattern is line 10 (`SEMANTIC_GRAPH_PROVIDER_ID`); add `SOLVER_PROVIDER_ID = "polint.solver"`.

---

### `analysis/solver/provider.rs` (provider, derive + output digest)

**Analog:** `crates/polint/src/analysis/semantic_graph/provider.rs` (the entire file).

**Run entry + 7-phase pipeline** (lines 23-108): `derive_*_with_cache_stats` returns a
`*ProviderRunOutput { diagnostics, cache_stats, output_digest }`; on store error returns
`output_digest: None` so a cache layer never records a hit for un-persisted state.

**Output digest over stable KEYS, never dense IDs** (lines 127-234): `*_output_digest`
folds (a) `provider_id`/`provider_version`/`schema`/`parameter` digests, (b) every
consumed upstream provider output digest, (c) per-row stable-key parts, then `parts.sort()`
+ `Digest::from_parts`. **D-15 — the solver digest additionally folds the SolverBudget**
(budgets participate so a budget change invalidates downstream). At minimum digest
`polint.semantic_graph` + the points-to source families (`points_to_constraints` /
`points_to_sets` from `polint.type_value_alias`).

**Manifest-slot assertion test** (lines 564-573) — copy
`provider_manifests_list_semantic_graph_between_type_value_alias_and_refined_calls` to add
`provider_manifests_list_solver_between_semantic_graph_and_refined_calls`.

---

### `analysis/solver/cache_key.rs` (config, parameter digest)

**Analog:** `crates/polint/src/analysis/semantic_graph/cache_key.rs` (entire file). Define
`SOLVER_SCHEMA_LABEL` (line 4 pattern), a `*_provider_parameter_digest()` listing the
frozen algorithm-version strings (lines 49-67), and the locked "parts list" trip-wire test
(lines 73-95) + the "algorithm-version bump invalidates" test (lines 97-116). Add the
SolverBudget knobs to the parameter parts so a budget-default change is captured (D-15).

---

### `analysis/solver/validate.rs` (validation pass)

**Analog:** `crates/polint/src/analysis/semantic_graph/validate.rs` (entire file head,
lines 1-30). Emit an evidence-bearing `Diagnostic` per problem (duplicate stable keys,
dangling endpoints, non-contiguous dense IDs, precision-ceiling violations) rather than
silently dropping. **D-12 — add the cycle-detection check here or in a fixture**: prove no
solver↔summary loop is admitted (summaries are an input, never re-fed into the same
fixpoint). The precision-ceiling rejection of the exact tier (validate.rs doc lines 18-21)
is the template for "derived edges reject `FactPrecision::Exact`."

---

### `analysis/mod.rs` (modify — module registration)

**Analog:** itself. Add `pub(crate) mod solver;` to the alphabetized list (between
`slicing` and `stable_key`, lines 27-29). Note the file-level
`#![cfg_attr(not(test), expect(dead_code, ...))]` at the top — solver types introduced
before full provider integration may need the same dead-code allowance.

---

### `analysis/ids.rs` (modify — id newtypes)

**Analog:** itself, lines 160-178 (`SemanticNodeId`/`SemanticEdgeId`/`SemanticConstraintId`).
Add solver / derived-edge / provenance dense-ID newtypes. Any ID whose owning fact carries
`#[serde(skip)]` on its `id` field MUST derive `Default` (the lines 160-178 block shows the
`Default`-bearing variant and explains why). Register each new newtype in the
`assert_small_id_contract` list (lines 204-258).

---

### `analysis_kernel/provider.rs` (modify — manifest + ~7 snapshot sites) — THE CHORE

**Analog:** the `polint.semantic_graph` manifest entry, lines 657-687. Add a `polint.solver`
`ProviderManifest` AFTER it and BEFORE `polint.refined_calls` (lines 688-711). Slot
confirmed by D-13; default unless the DAG dictates otherwise.

**SCHEMA const** — copy lines 220-223 (`SEMANTIC_GRAPH_SCHEMA`) to a `SOLVER_SCHEMA`
referencing `crate::analysis::solver::cache_key::SOLVER_SCHEMA_LABEL`.

**~7 provider-order / snapshot sites that MUST be updated** (memory:
`polint-kernel-provider-snapshot-sites` — run full `cargo test -p polint`):

| # | Site | Line(s) | What to change |
|---|------|---------|----------------|
| 1 | `provider_order_matches_behavior_preserving_kernel_sequence` | 827-855 | insert `"polint.solver"` after `"polint.semantic_graph"` (line 848) |
| 2 | `symbol_graph_manifest_declares_semantic_outputs_without_reordering_providers` | 858-913 | same order vec (line 879) |
| 3 | `module_graph_manifest_declares_base_topology_outputs_without_reordering_providers` | 915-943 | same order vec (line 937) |
| 4 | `provider_order_report_for_test` snapshot (`ProviderOrderRow` list) | 1349-1385 | insert a `ProviderOrderRow { id: "polint.solver", ... }` between semantic_graph (1350) and refined_calls (1366) |
| 5 | semantic_graph's own slot-assertion test in `semantic_graph/provider.rs` | 564-573 | now `tva + 2 == "polint.solver"`, `tva + 3 == "polint.refined_calls"` — update this neighbor assertion |
| 6 | `provider_manifests_cover_existing_kernel_providers` in `analysis_kernel/mod.rs` | ~1990 | add `polint.solver` to the expected id set |
| 7 | any further `provider_order_for_test()` / required-metadata loops (e.g. lines 811-824, 1473+) | — | run the full suite; the metadata loop auto-covers, the ordered vecs do not |

> Treat "~7" as a floor, not a ceiling. The reliable procedure: make the manifest +
> dispatch change, run `cargo test -p polint`, and update every failing ordered-vec /
> snapshot assertion until green. Sites 1-4 are hand-maintained ordered vecs (must edit);
> the determinism gate and required-metadata loops auto-enroll (no edit).

---

### `analysis_kernel/mod.rs` (modify — provider dispatch wiring)

**Analog:** the semantic_graph dispatch block, lines 542-572. Add a parallel block AFTER
it that calls `crate::analysis::solver::provider::derive_*_with_cache_stats(&mut db, ...,
Self::provider_manifest("polint.solver"), <upstream digests incl. semantic_graph_output_digest>)`,
then `provider_outputs.push(Self::provider_output_for_with_optional_digest("polint.solver",
...))`. Thread the SolverBudget into the call so D-15 (budget in cache key) holds. The
`semantic_graph_output_digest` produced at line 565 is the primary upstream digest the
solver consumes.

---

### `cli/mod.rs` (modify — `explain` private plumbing) (D-10) — PARTIAL ANALOG

**Analog (surface only):** `explain(...)` lines 1331-1370. **Important caveat for the
planner:** the existing `explain` is rule-capability-planning oriented (it discovers local
rule hosts and emits an `ExplainReport` of rule rows + capabilities) — it is NOT currently
a fact/edge inspector. D-10 says provenance is consumable by the existing `explain`
*surface* and extends *existing private plumbing* — it is explicitly NOT a new public CLI
surface (the only new public CLI surface in v1.3 is `polint inspect unknowns`, Phase 52).
The planner should wire `DerivedEdgeProvenance` into the private/internal plumbing reached
from this command (contributing facts + constraint kind + solver step), keeping all
provenance types `pub(crate)` and adding nothing to the public JSON schema. The
`ExplainReport`/`ExplainRuleRow` structs (lines 1574-1584) are the existing serializable
shapes; do not promote new public fields. This is the only file with a partial analog —
flag for the planner to confirm the exact private seam before implementing.

---

### `analysis_kernel/metadata.rs` (modify — `FactFamily` entries)

**Analog:** `FactFamily` enum, lines 6-115; `PointsToSet` entry line 87 and its `label()`
arm line 190. Add a `FactFamily` variant per new solver fact family (derived edge,
provenance) so `stable_key_from_parts` can family-tag them. Each variant needs a `label()`
arm. Reserved-but-unproduced families use the `#[expect(dead_code, reason = "...")]`
pattern (lines 89-92) naming the phase.

---

## Shared Patterns

### Closed-enum byte-stability (every new solver enum: `BudgetStatus`, etc.)
**Source:** `analysis/semantic_graph/constraints.rs` lines 41-43 (derive set + pinned
order, no `#[repr(u8)]`); `analysis/points_to/facts.rs` lines 95-100.
**Apply to:** `budget.rs`, `policy.rs`, any solver enum.
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
// pinned declaration order drives derived Ord + serde => byte-stable
```
Lock variant count + order with a `*_sorts_in_pinned_declaration_order` test
(constraints.rs lines 241-287) and an exhaustive-match count test (lines 195-238).

### Dense-IDs-after-sort + `#[serde(skip)]` on dense `id`
**Source:** `analysis/ids.rs` lines 155-178; `semantic_graph/constraints.rs` lines 178-188;
`semantic_graph/store.rs` `normalized()` line 35.
**Apply to:** all solver fact families + store. Dense IDs are a post-normalize read concern,
assigned only after stable-key sort; they never enter the output digest.

### Provider output digest = stable keys + upstream digests + params (+ budgets for solver)
**Source:** `semantic_graph/provider.rs` lines 127-234.
**Apply to:** `solver/provider.rs`. D-15 adds SolverBudget to the digest parts.

### Honest status/precision (budget exhaustion as a fact, never a silent drop)
**Source:** `points_to/solver.rs` lines 254-269 + `points_to/facts.rs` `PointsToStatus::
BudgetExceeded` (line 82), `PointsToBudgetStatus::BudgetExceeded` (line 98).
**Apply to:** `engine.rs` result projection, `facts.rs`, `validate.rs`. Derived edges
reject the exact precision tier (validate.rs precision-ceiling check).

### Determinism-gate auto-enrollment (NO harness edit)
**Source:** `crates/polint/src/eval/determinism_gate.rs` (lines 1-44 doc, `PERMUTATION_RUNS
= 10` line 62). The shuffled provider set is sourced from
`AnalysisKernel::provider_manifests()`, so `polint.solver` AUTO-ENROLLS once registered
(D-14). The doc explicitly names phases 44-54 including **47** as inheritors. No per-phase
gate edit needed; verification keeps `tests/eval-fixtures/determinism/{go,ts}_reachable`
green on Linux + macOS as a NAMED acceptance criterion.

### Stable-key total-order recipe (provenance contributing-fact ordering)
**Source:** `analysis_kernel/metadata.rs` `stable_key_from_parts` lines 370-384;
`analysis/stable_key.rs` lines 16-18.
**Apply to:** `provenance.rs` (order contributing fact IDs by stable key) and every solver
`stable_key:` field.

### Naming-collision guard via top-of-module doc comment (D-04)
**Source:** `analysis/semantic_graph/mod.rs` lines 13-39 (the D-09 guard distinguishing
`ConstraintKind` from `PointsToConstraintKind`).
**Apply to:** `solver/mod.rs` — distinguish the unified core from the points-to sub-domain.

### Public-surface-leak gate stays green (D-16)
**Source:** `crates/polint/tests/public_surface_leak.rs` (`ALLOWED_PRELUDE` const, line 42).
**Apply to:** ALL new solver types stay `pub(crate)`; do NOT extend `ALLOWED_PRELUDE`. This
is a NAMED acceptance criterion (GRAPH-03 SC5).

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `cli/mod.rs` `explain` provenance seam | private plumbing | request-response | The existing `explain` is a rule-capability planner, not a fact/edge inspector. D-10 extends existing private plumbing but there is no current edge-provenance consumer to copy from. Planner must confirm the exact private seam; keep all types `pub(crate)`, add no public JSON schema field. |

(All other files have exact or strong role-match analogs.)

---

## Fixture Precedents (D-09, D-12)

- **Provenance-deletion property test (D-09):** lives as a unit/property test alongside
  `solver/` (or under `tests/eval-fixtures/provenance/`, which already exists with a
  `metadata` subtree). The proof: re-running the solver without any one contributing fact
  must NOT reproduce the derived edge. Determinism precedent for the assertion shape:
  `points_to/solver.rs` lines 392-395 (`assert_eq!(first, second)`).
- **Cycle-detection fixture (D-12):** `tests/eval-fixtures/` is the native fixture-tree
  precedent (`determinism/`, `semantic-graph/`, `provenance/` already present). The fixture
  demonstrates a constraint set that would create a solver→summary→solver cycle is
  detected/bounded rather than diverging — the concrete mechanism behind "closed input
  set / single-fixpoint-per-run" (D-11).

---

## Metadata

**Analog search scope:** `crates/polint/src/analysis/{points_to,semantic_graph,refined_calls}/`,
`crates/polint/src/analysis/{mod,ids,stable_key}.rs`,
`crates/polint/src/analysis_kernel/{provider,mod,metadata}.rs`,
`crates/polint/src/cli/mod.rs`, `crates/polint/src/eval/determinism_gate.rs`,
`crates/polint/tests/public_surface_leak.rs`, `tests/eval-fixtures/`.
**Files scanned:** ~15 source files read in full or targeted ranges.
**Pattern extraction date:** 2026-06-02
