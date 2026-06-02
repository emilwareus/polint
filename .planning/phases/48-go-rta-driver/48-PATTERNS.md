# Phase 48: Go RTA Driver - Pattern Map

**Mapped:** 2026-06-02
**Files analyzed:** 18 (new + modified)
**Analogs found:** 18 / 18 (every file has an in-repo analog; this is an extension phase, not greenfield)

> **Read order for the planner:** every excerpt below is an *exact in-repo pattern to copy*, not an abstraction. Visibility is `pub(crate)` everywhere (D-01/D-17); the public-surface-leak gate (`crates/polint/tests/public_surface_leak.rs`) and the determinism gate (auto-enrolled via `provider_manifests()`) MUST stay green. Composition-over-rewrite is the governing rule (D-02): points-to derived-edge output stays byte-identical.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/polint/src/analysis/solver/go_rta/mod.rs` *(new)* | module (driver root) | event-driven (worklist fixpoint) | `crates/polint/src/analysis/solver/mod.rs` + `engine.rs` | role-match |
| `crates/polint/src/analysis/solver/go_rta/fixpoint.rs` *(new)* | solver/policy | event-driven (fixpoint) | `crates/polint/src/analysis/points_to/solver.rs` (`Solver` + `solve`) | exact (deterministic VecDeque worklist) |
| `crates/polint/src/analysis/solver/go_rta/dispatch.rs` *(new)* | solver (dispatch resolver) | transform (callsite -> callees) | `engine.rs::derive_edges` (worst-trust per-source closure) | role-match |
| `crates/polint/src/analysis/solver/go_rta/instantiated_types.rs` *(new)* | solver (rapid-type set) | transform | `engine.rs::derive_edges` adjacency-accumulation block (BTree sets) | role-match |
| `crates/polint/src/analysis/solver/go_rta/address_taken.rs` *(new)* | solver (address-taken set) | transform | `engine.rs::derive_edges` adjacency block | role-match |
| `crates/polint/src/analysis/solver/policy.rs` *(modify: replace `GoRtaPolicy` stub; extend `PolicyOutcome`)* | solver/policy | event-driven | `PointsToPolicy` (the one real impl in the same file) | exact |
| `crates/polint/src/analysis/solver/budget.rs` *(modify: add `GoRtaSubBudget`)* | config/model | n/a | `PointsToSubBudget` (same file) | exact |
| `crates/polint/src/analysis/solver/engine.rs` *(modify: route Go RTA + points-to into one `SolverOutput`)* | service (orchestration) | event-driven | `SolverEngine::run` + `derive_edges` (same file) | exact |
| `crates/polint/src/analysis/solver/facts.rs` *(reuse `DerivedEdgeFact`; RTA edges are this family)* | model (fact) | n/a | `DerivedEdgeFact` (same file) | exact (no new edge family — D-04) |
| `crates/polint/src/analysis/solver/provenance.rs` *(reuse `DerivedEdgeProvenance`; RTA edges attach this)* | model (provenance) | n/a | `DerivedEdgeProvenance` (same file) | exact |
| `crates/polint/src/analysis/solver/provider.rs` *(modify: drive engine, digest Go sub-budget + Go-fact digests)* | provider (whole-repo) | request-response | `derive_solver_with_cache_stats` (same file) | exact |
| `crates/polint/src/analysis/solver/cache_key.rs` *(modify: add Go sub-budget + algo-version to digest)* | config (cache key) | n/a | `solver_provider_parameter_digest` + `budget_parts` (same file) | exact |
| `crates/polint/src/analysis/solver/store.rs` *(reuse `SolverOutput::normalized`; no shape change)* | store | n/a | `SolverOutput` / `normalized` (same file) | exact |
| `crates/polint/src/config/mod.rs` *(modify: add `[solver]` table + `go` sub-table)* | config | n/a | `ReachabilityConfig` (`[reachability]`, same file) | exact |
| `crates/polint/src/analysis/ids.rs` *(modify: add Go RTA / new Go-frontend fact IDs)* | model (id newtypes) | n/a | `DerivedEdgeId` / `GoSemanticFunctionId` family | exact |
| `crates/polint/src/go/semantic/facts.rs` *(modify: add address-taken / instantiated-type / dispatch-detail facts)* | model (fact) | n/a | `GoSemanticMethodSetFact` / `GoSemanticCallsiteFact` (same file) | role-match |
| `crates/polint/src/go/semantic/lower.rs` *(modify: lower the new RTA-signal rows)* | service (lowering) | transform (NDJSON row -> fact) | `lower_method_set` / `lower_callsite` (same file) | exact |
| `crates/polint/go-sidecar/.../internal/semantic/emit.go` *(modify: harvest MakeInterface / MakeClosure / dispatch detail)* | service (SSA emitter) | transform (SSA instr -> Row) | `emitCallsites` / `emitMethodSets` (same file) | exact |
| `tests/eval-fixtures/...` *(new: iteration-cap, x/tools RTA native, polyglot Go+TS canary)* | test (fixture) | n/a | `determinism/go_reachable/` + `framework-entrypoints/mixed-go-ts/` | role-match |

---

## Pattern Assignments

### `analysis::solver::go_rta/fixpoint.rs` (solver, event-driven fixpoint) — Plan 2

**Analog:** `crates/polint/src/analysis/points_to/solver.rs` (the proven deterministic VecDeque worklist + budget fixpoint). This is the structural template the engine module docs explicitly point at: *"the worklist drain mirrors the proven `points_to::solver` shape."*

**Worklist + budget shape to copy** (`points_to/solver.rs:43-91`):
```rust
struct Solver<'a> {
    constraints: &'a [PointsToConstraintFact],
    budget: PointsToBudget,
    // ... BTreeMap/BTreeSet-keyed accumulators for determinism ...
    queue: VecDeque<(PtVarId, BTreeSet<ObjectTokenId>)>,
    steps: usize,
    budget_exceeded: bool,
}

fn solve(&mut self) -> PointsToSolveResult {
    self.initialize();
    while let Some((var, delta)) = self.queue.pop_front() {
        if !self.step_budget_ok() {   // <-- honest break, never unbounded
            break;
        }
        self.propagate_copy(var, &delta);
        // ... other propagation steps ...
    }
    self.to_result()
}
```

**Map the points-to shape onto RTA** (planner): the RTA worklist is `reachable functions x newly-instantiated types`; each pop expands dispatch (D-06: CHA filtered by the instantiated-type set). Use `BTreeMap<.., BTreeSet<..>>` accumulators (reachable set, instantiated-type set, address-taken set, per-callsite candidate set) keyed by **stable identity / dense Go-fact id**, never run-local discovery order — this is what keeps the 10-shuffle determinism gate green (D-17). Latch `budget_exceeded` on the round cap / per-callsite candidate cap, exactly as `points_to` latches it.

**Worst-trust closure + per-source budget discipline to copy** (`engine.rs::derive_edges:171-340`) — the RTA dispatch resolver in `dispatch.rs` must reuse this exactly:
- `BTreeMap`/`BTreeSet`-ordered adjacency accumulation (`engine.rs:177-198`),
- a **global monotonic** `solver_step` counter that is never reset (`engine.rs:202-207`, review finding #R3),
- a **per-source** budget counter reset per start so one runaway source never starves the others (`engine.rs:210-215`, review finding #3),
- worst-of-two `weakest_status`/`weakest_precision` helpers (`engine.rs:377-418`) — RTA edges inherit the WEAKEST status across the adopted derivation (D-09),
- edges fully derived before the cap keep their honest status; exhaustion costs only the edges never reached (`engine.rs:282-326`, review finding #R1).

---

### `analysis::solver::go_rta/mod.rs` (module root) — Plan 2

**Analog:** `crates/polint/src/analysis/solver/mod.rs` (the D-04 naming-collision doc + `pub(crate) mod` list).

**Top-of-module doc to copy** (D-01 mandates the naming-collision guard; `solver/mod.rs:14-32`):
```rust
//! ## D-04 naming-collision guard (MANDATORY)
//!
//! The unified `analysis::solver` core sits **above** the points-to sub-domain.
//! Do not conflate the two vocabularies:
//! - The unified core consumes ... `ConstraintKind` ... emits derived edges with provenance.
//! - The `points_to` sub-domain keeps its own internal language ... unchanged.
```
Write the analogous guard distinguishing **the unified solver's derived-edge vocabulary (`DerivedEdgeFact`/`ConstraintKind`) from the Go-frontend fact vocabulary (`GoSemanticMethodSetFact`/the new address-taken & instantiated-type facts)** (D-01). Declare submodules `pub(crate) mod` exactly as `solver/mod.rs:52-60` does. Internal file layout (`fixpoint.rs`/`dispatch.rs`/`instantiated_types.rs`/`address_taken.rs`/`budget.rs`) is Claude's discretion per CONTEXT.

---

### `analysis::solver::policy.rs` (replace `GoRtaPolicy` stub; extend `PolicyOutcome`) — Plan 2

**Analog:** `PointsToPolicy` in the same file — the *one real impl* to mirror.

**Stub being replaced** (`policy.rs:106-121`):
```rust
pub(crate) struct GoRtaPolicy;
impl SolverPolicy for GoRtaPolicy {
    fn id(&self) -> &'static str { "go_rta" }
    fn solve(&self, _budget: &SolverBudget) -> PolicyOutcome { PolicyOutcome::empty() }
}
```

**Real-impl shape to copy from `PointsToPolicy`** (`policy.rs:77-104`) — own a closed snapshot of inputs, delegate to the fixpoint, project the budget status:
```rust
pub(crate) struct PointsToPolicy { constraints: Vec<PointsToConstraintFact> }
impl SolverPolicy for PointsToPolicy {
    fn id(&self) -> &'static str { "points_to" }
    fn solve(&self, budget: &SolverBudget) -> PolicyOutcome {
        let result = solve_points_to(&self.constraints, budget.points_to_budget());
        let budget_status = BudgetStatus::from_points_to(result.budget_status);
        PolicyOutcome { points_to: Some(result), budget_status, steps: 0 }
    }
}
```
`GoRtaPolicy` becomes the symmetric impl: it owns a closed snapshot of Go RTA inputs (reachability roots + the new Go-frontend facts + the `CallConstraint` callsites), runs the `go_rta::fixpoint`, and returns derived edges + `BudgetStatus`.

**`PolicyOutcome` extension to make (D-03)** — today (`policy.rs:31-52`) the struct carries only `points_to: Option<PointsToSolveResult>` + `budget_status` + `steps`, and `empty()` zeroes all three. Add a derived-edge channel (e.g. `derived_edges: Vec<DerivedEdgeFact>` or a `SolverOutput` fragment — exact shape is Claude's discretion). **Keep the `points_to` field and `empty()` byte-identical** so the Phase 47 fold stays unchanged and the two remaining stubs (`TsTokensPolicy`) keep returning `PolicyOutcome::empty()`. Mirror the `go_and_ts_stubs_derive_nothing` test (`policy.rs:144-159`) — `TsTokensPolicy` must still assert empty; add a positive test that `GoRtaPolicy` now derives edges.

---

### `analysis::solver::budget.rs` (add `GoRtaSubBudget`) — Plan 2

**Analog:** `PointsToSubBudget` in the same file — the exact structural template (D-11).

**Copy this shape** (`budget.rs:27-42`):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PointsToSubBudget {
    pub(crate) max_objects_per_var: usize,
    pub(crate) max_dynamic_vars: usize,
}
impl Default for PointsToSubBudget {
    fn default() -> Self { Self { max_objects_per_var: 64, max_dynamic_vars: 512 } }
}
```
Add `GoRtaSubBudget { address_taken_threshold, max_candidates_per_callsite, max_rta_rounds }` (exact knob names = planner's discretion; the **address-taken threshold is the roadmap-named default**, D-10/D-11). Hang it on `SolverBudget` as `pub(crate) go: GoRtaSubBudget` beside `points_to` (`budget.rs:50-55`).

**CRITICAL — keep `SolverBudget::default()` byte-identical** (`budget.rs:57-70`): `max_steps: 10_000`, `max_outer_iterations: 64`, `points_to: PointsToSubBudget::default()`. The locked test `solver_budget_default_matches_points_to_defaults` (`budget.rs:130-142`) and the cache-key locked test (below) both pin these exact bytes — adding the `go` field must not perturb the existing fields' digest contribution unless the planner also updates the locked recipe in `cache_key.rs`. Reuse the unified `BudgetStatus::BudgetExceeded` (`budget.rs:94-103`) — D-13 says *no new enum*; the 3-variant exhaustive test (`budget.rs:155-171`) must stay green.

---

### `analysis::solver::engine.rs` (route Go RTA + points-to into one `SolverOutput`) — Plan 2

**Analog:** `SolverEngine::run` + `derive_edges` in the same file. This is the **reserved seam** D-02 mandates routing through.

**The reserved-seam doc that authorizes this change** (`engine.rs:18-28`):
```rust
//! **Reserved multi-policy orchestration (Phase 47 scope — intentional).**
//! ... The [`SolverEngine`] + [`SolverPolicy`] multi-policy layer is the
//! reserved seam Phases 48/49 extend: when the Go RTA and TS token drivers register
//! as policies, production will route through the engine so multiple sub-domains
//! converge under one budget.
```

**Engine drain loop to extend** (`engine.rs:84-129`) — keep the deterministic registration-order `VecDeque<usize>` drain, the `max_outer_iterations` cap, the worst-case `budget_status` combine, and the monotonic `steps` fold:
```rust
let mut queue: VecDeque<usize> = (0..self.policies.len()).collect();
while let Some(index) = queue.pop_front() {
    steps += 1;
    if steps > self.budget.max_outer_iterations as u64 { budget_exceeded = true; break; }
    let outcome = self.policies[index].solve(&self.budget);
    if outcome.budget_status == BudgetStatus::BudgetExceeded { budget_exceeded = true; }
    steps = steps.saturating_add(outcome.steps);
    records.push(PolicyRunRecord { policy_id: policy.id(), outcome });
}
```

**Composition contract (D-02):** the engine must aggregate the points-to `CopyEdge` derivation (`derive_edges`, still byte-identical) **and** the Go RTA policy's `DerivedEdgeFact`s into one `SolverOutput` under one `SolverBudget`. The cleanest composition (engine aggregates per-policy `SolverOutput`s vs. a thin orchestration wrapper) is Claude's discretion, provided: (a) points-to output stays byte-identical, (b) the engine owns the single-fixpoint-per-run / bounded-outer-iteration contract, (c) the determinism gate stays green. The acceptance test to preserve verbatim: `points_to_via_engine_equals_solve_points_to` (`engine.rs:475-496`) and `derive_edges_is_shuffle_stable` (`engine.rs:928-944`). Run the merged edges through `SolverOutput::normalized()` (dense IDs only after the stable-key sort) before returning.

---

### `analysis::solver::facts.rs` + `provenance.rs` (RTA edges reuse these — D-04) — Plan 2

**Analog:** `DerivedEdgeFact` / `DerivedEdgeProvenance` in those files. **Do NOT mint a parallel Go edge fact family** (D-04). A resolved Go call edge is `caller-fn-node -> callee-fn-node` as a `DerivedEdgeFact`.

**Edge fact to reuse** (`facts.rs:34-58`) — `source`/`target` are `SemanticNodeId`s, status/precision reuse the shared `PointsToStatus`/`PointsToPrecision` vocabulary, dense `id` is `#[serde(skip)]`:
```rust
pub(crate) struct DerivedEdgeFact {
    #[serde(skip)] pub(crate) id: DerivedEdgeId,
    pub(crate) source: SemanticNodeId,
    pub(crate) target: SemanticNodeId,
    pub(crate) status: PointsToStatus,
    pub(crate) precision: PointsToPrecision,
    pub(crate) stable_key: String,
    pub(crate) provenance: DerivedEdgeProvenance,
}
```

**Precision ceiling to honor (D-08)** (`facts.rs:74-86`) — `derived_edge_precision_ceiling` NEVER returns `FactPrecision::Exact`; an RTA-resolved interface edge claims at most `SetupAware`/`Heuristic`. The locked test `derived_edge_precision_ceiling_never_returns_exact` (`facts.rs:109-126`) covers this — RTA edges go through the same `honors_precision_ceiling` gate in the store.

**Provenance to attach (D-04/D-09)** (`provenance.rs:98-114`) — build from the contributing fact stable keys (callsite + method-set + instantiated-type), the producing `ConstraintKind` (`CallConstraint`, from `constraints.rs:67-69`), and the solver step:
```rust
DerivedEdgeProvenance::new(
    contributing_facts /* callsite + method-set + instantiated-type stable keys */,
    &ConstraintKind::CallConstraint { callsite },
    solver_step,
)
```
`new` sorts + dedups contributing facts by `stable_key`, so provenance is byte-stable regardless of discovery order (`provenance.rs:103-108`). The **deletion-invalidation property (D-09)** must extend to RTA edges: the stable_key embeds the witness (via `stable_key_fragment`, `provenance.rs:126-133`), so deleting a contributing instantiated-type/method-set/callsite fact must not reproduce the same derived edge. Mirror `deleting_any_contributing_fact_invalidates_the_derived_edge` (`provenance.rs:233-283`) for RTA.

---

### `analysis::solver::provider.rs` (drive engine; digest Go sub-budget + Go-fact digests) — Plan 2

**Analog:** `derive_solver_with_cache_stats` in the same file. The slot already exists; Phase 48 makes the body drive registered policies through the engine instead of calling only the free `derive_edges`.

**Current body to extend** (`provider.rs:53-113`):
```rust
let constraints = db.semantic_constraints().to_vec();
let output = derive_edges(&constraints, &budget);          // <-- D-02: route through SolverEngine::run
// ... output_digest, validate_derived_edges, detect_solver_summary_cycle ...
if output.budget_status == BudgetStatus::BudgetExceeded {
    diagnostics.push(budget_exceeded_diagnostic());        // <-- reuse for RTA round-cap (D-13/D-14)
}
match db.replace_solver_facts(output) { Ok(()) => ..., Err(error) => ... output_digest: None }
```

**Budget-exhaustion diagnostic to reuse (D-13)** (`provider.rs:179-189`) — the RTA round-cap / per-callsite explosion latches `BudgetStatus::BudgetExceeded` and surfaces THIS diagnostic, never a silent drop:
```rust
Diagnostic::warning("polint/internal", "<workspace>", TextRange::point(1, 1),
    "Solver budget exceeded; ...")
    .with_evidence("provider", SOLVER_PROVIDER_ID)
    .with_evidence("budget_status", BudgetStatus::BudgetExceeded.as_str())
```

**Output digest to extend (D-12/D-15)** (`provider.rs:125-174`) — the digest already folds the `SolverBudget` knobs explicitly (`budget.max_steps`, `max_outer_iterations`, `points_to.*`). Add the new `budget.go.*` parts AND the consumed Go-frontend fact digests (address-taken / instantiated-type / dispatch facts) as new upstream-digest parameters so a Go-knob or Go-fact change invalidates downstream (forward-compatible with CACHE-01/02). The slot-order test `provider_manifests_list_solver_between_semantic_graph_and_refined_calls` (`provider.rs:417-426`) must stay green — Phase 48 reuses the existing `polint.solver` slot, so the ~7 `provider_order_for_test()` snapshots in `analysis_kernel/provider.rs` do not change unless the new Go-frontend fact provider wiring touches ordering (see `analysis_kernel/provider.rs:854-911`).

---

### `analysis::solver::cache_key.rs` (add Go sub-budget + algo-version to digest) — Plan 2

**Analog:** `solver_provider_parameter_digest` + `budget_parts` in the same file.

**`budget_parts` to extend** (`cache_key.rs:56-72`) — add the `go` sub-budget knobs alongside the points-to knobs:
```rust
fn budget_parts(budget: &SolverBudget) -> Vec<String> {
    vec![
        format!("budget.max_steps={}", budget.max_steps),
        format!("budget.max_outer_iterations={}", budget.max_outer_iterations),
        format!("budget.points_to.max_objects_per_var={}", budget.points_to.max_objects_per_var),
        format!("budget.points_to.max_dynamic_vars={}", budget.points_to.max_dynamic_vars),
        // ADD: format!("budget.go.address_taken_threshold={}", budget.go.address_taken_threshold), etc.
    ]
}
```

**Algorithm-version list to extend** (`cache_key.rs:33-49`) — add an RTA algo-version string (e.g. `"go_rta_fixpoint_v1"`) to the `parts` list. **This is a deliberate trip-wire:** the locked test `solver_provider_parameter_digest_locks_parts_list` (`cache_key.rs:86-110`) reconstructs the *exact* parts list including `budget.max_steps=10000` / `budget.points_to.max_objects_per_var=64`, so the planner MUST update that locked test (and `algorithm_version_bump_invalidates_the_pre_bump_digest`) in the same edit. Add a `budget.go.*` change-invalidation assertion mirroring `budget_change_invalidates_the_parameter_digest` (`cache_key.rs:136-166`).

---

### `config/mod.rs` (add `[solver]` table + `go` sub-table) — Plan 2

**Analog:** `ReachabilityConfig` (the `[reachability]` table) in the same file — the exact precedent the `[solver]` table sits beside (D-10).

**Config-table pattern to copy** (`config/mod.rs:45-49`, registered at `:36`):
```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ReachabilityConfig {
    #[serde(default)]
    pub(crate) roots: Vec<String>,
}
// registered on PolintConfig (config/mod.rs:35-36):
//     #[serde(default)]
//     pub(crate) reachability: ReachabilityConfig,
```
Add `SolverConfig { go: SolverGoConfig }` with `SolverGoConfig` exposing the `solver_config.go.*` knobs (address-taken threshold, RTA iteration/dispatch caps). Register `#[serde(default)] pub(crate) solver: SolverConfig` on `PolintConfig`. Keep all fields `#[serde(default)]` so absence falls back to `GoRtaSubBudget::default()` (D-11). This is `.polint.toml` config surface, NOT SDK promotion (CONVENTIONS / D-10). Config values map into `SolverBudget.go` in the provider wiring; the config digest already participates in the solver output digest (`provider.rs:138` `config={}`).

---

### `analysis::ids.rs` (add Go RTA / new Go-frontend fact IDs) — Plans 1 & 2

**Analog:** the dense-id newtype block in the same file (`DerivedEdgeId` at `:186-189`, the `GoSemantic*Id` family lives in `go/semantic/facts.rs:5-16`).

**Newtype pattern to copy** (`ids.rs:186-189`) — `#[serde(skip)]`-bearing fact ids get `Default`:
```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DerivedEdgeId(pub(crate) u64);
```
Add ids for the new Go-frontend facts (address-taken, instantiated-type, dispatch-detail) following the `GoSemanticCallsiteId` shape (`go/semantic/facts.rs:9-16` — no `Default`, no serde, since those ids are not `#[serde(skip)]` payload fields). Every new id MUST be added to the `assert_small_id_contract` test list (`ids.rs:217-269`) or the locked test breaks.

---

### `go/semantic/facts.rs` (add address-taken / instantiated-type / dispatch-detail facts) — Plan 1

**Analog:** `GoSemanticMethodSetFact` (type -> methods, the method-set input) and `GoSemanticCallsiteFact` in the same file. **This is the single most load-bearing change (D-05):** RTA without an instantiated-type set is just CHA and will not lift recall.

**Fact-struct pattern to copy** (`facts.rs:74-82`) — every fact carries `id` + `stable_key` (length-prefixed, from official Go identity) + package coords + payload:
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticMethodSetFact {
    pub(crate) id: GoSemanticMethodSetId,
    pub(crate) stable_key: String,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) type_name: String,
    pub(crate) methods: Vec<String>,
}
```

**Dispatch-detail gap to fill** (`facts.rs:59-72`) — `GoSemanticCallsiteFact` carries `caller`/`static_callee`/`status` but **no interface/method discriminant**. D-05 needs, for each `UnresolvedDynamic` callsite, the interface type + invoked method name (or func-value signature) for method-set matching. The `GoSemanticCallStatus` enum already separates the target set (`facts.rs:25-30`): `UnresolvedDynamic` is exactly what RTA resolves.

Add (exact shapes = planner/researcher discretion, grounded in x/tools RTA, D-05; keep minimal & honest):
1. an **address-taken-function fact** (from `*ssa.MakeClosure`, function-valued globals/params, method-value refs),
2. an **instantiated-runtime-type fact** (the "rapid type" set from `*ssa.MakeInterface` + planner-confirmed alloc families),
3. **dynamic-callsite dispatch detail** (interface type + method name per `UnresolvedDynamic` callsite).

New facts stay `pub(crate)`, length-prefixed/stable-keyed from official Go identities (Phase 46 D-12/D-13), validated, and participate in the cache key. Emit `Unsupported`/unresolved rows rather than fabricating matchable identities (Phase 46 D-15). Add a corresponding `GoSemantic*Id` newtype in `go/semantic/facts.rs` (mirror `GoSemanticMethodSetId`, `:12-13`).

---

### `go/semantic/lower.rs` (lower the new RTA-signal rows) — Plan 1

**Analog:** `lower_method_set` / `lower_callsite` (+ the `match row.kind.as_str()` dispatch) in the same file.

**Row-kind dispatch to extend** (`lower.rs:49-61`):
```rust
for row in &output.rows {
    match row.kind.as_str() {
        "package" => lowered.packages.push(lower_package(row, &files)?),
        "function" | "method" | "init_function" => lowered.functions.push(lower_function(row, &files)?),
        "callsite" => lowered.callsites.push(lower_callsite(row, &files)?),
        "method_set" => lowered.method_sets.push(lower_method_set(row)),
        "package_error" => lowered.package_errors.push(lower_package_error(row)),
        "receiver_type" | "unsupported" | "type_fact" => {}   // <-- intentionally ignored rows
        _ => {}
    }
}
```
Add arms for the new sidecar row kinds (e.g. `"address_taken"`, `"instantiated_type"`, `"dynamic_dispatch"`). Mirror the `lower_*` builders: assign `Id(0)` (dense IDs come from `normalized()`), build the `stable_key` via `row_stable_key`, clone the payload from the raw frame. The `lower_callsite` status-mapping pattern (`lower.rs:127-130`) is the template for mapping the new dispatch-detail enum:
```rust
status: match row.status.as_str() {
    "resolved_static" => GoSemanticCallStatus::ResolvedStatic,
    "unsupported" => GoSemanticCallStatus::Unsupported,
    _ => GoSemanticCallStatus::UnresolvedDynamic,
},
```
End with `Ok(lowered.normalized())` (`lower.rs:63`). The new facts must also be threaded through `go/semantic/{store.rs,provider.rs,cache_key.rs,validate.rs}` following the existing method-set wiring (CONTEXT canonical-refs).

---

### `go-sidecar/.../internal/semantic/emit.go` (harvest MakeInterface / MakeClosure / dispatch detail) — Plan 1

**Analog:** `emitCallsites` / `emitMethodSets` in the same file. **The high-cost SSA build is already done** — `ssautil.AllPackages` + `prog.Build()` (`emit.go:99-100`) and the `fn.Blocks`/`block.Instrs` walk (`emit.go:247-248`) are in hand; the new emission is an additive walk over data already loaded.

**SSA instruction-walk pattern to copy** (`emit.go:243-289`) — already iterates every instruction; type-switches on `ssa.CallInstruction`:
```go
func (e *emitter) emitCallsites(pkg *ssa.Package, fn *ssa.Function) {
    for _, block := range fn.Blocks {
        for _, instr := range block.Instrs {
            call, ok := instr.(ssa.CallInstruction)
            if !ok { continue }
            common := call.Common()
            row := Row{ "kind": "callsite", "package_id": packageID(pkg), "caller": fn.String() }
            if common != nil && common.StaticCallee() != nil {
                row["static_callee"] = common.StaticCallee().String()
                row["status"] = "resolved_static"
            } else {
                row["status"] = "unresolved_dynamic"
                row["reason"] = "dynamic or interface dispatch deferred to Phase 48"  // <-- Phase 48 now fills this
            }
            row["stable_key"] = stableKey(stableParts...)
            e.add(row)
        }
    }
}
```

**Row + stable_key emission to copy** (`emit.go:145-154`, `:231-239`) — every emitted Row sets `kind` + `package_id` + `package_path` + a `stable_key` built from official identity via `stableKey(...)`:
```go
e.add(Row{
    "kind":         "receiver_type",
    "package_id":   packageID(pkg),
    "package_path": packagePath(pkg),
    "method":       fn.String(),
    "receiver":     fn.Signature.Recv().Type().String(),
    "stable_key":   stableKey(packageID(pkg), "recv", fn.String(), fn.Signature.Recv().Type().String()),
})
```
Add, in the same instruction walk: extend the type-switch to also match `*ssa.MakeInterface` (emit an `"instantiated_type"` row keyed on the concrete `X.Type()` identity), `*ssa.MakeClosure` / function-valued operands (emit `"address_taken"` rows), and enrich the `unresolved_dynamic` branch with the interface type + invoked method name (the `"dynamic_dispatch"` detail). Keep `stableKey(...)` keyed on `go/types`/`ssa.Function` identity, never run-local order (Phase 46 D-12/D-13). Bump `SchemaVersion` (`emit.go:22`) since the row vocabulary grows (cache-input discipline). Emit `unsupported` rows for SSA shapes that lack stable identity rather than fabricating one (`emit.go:187-196` precedent, Phase 46 D-15).

---

### `tests/eval-fixtures/...` (iteration-cap, x/tools RTA native, polyglot Go+TS canary) — Plan 3

**Structural precedent:** every native fixture is `tests/eval-fixtures/<family>/<case>/` with an `expected.polint-eval.toml` (`schema_version = "polint-eval-fixture-1"`) + a `repo/` subtree (`.polint.toml`, `go.mod`, `*.go`, and — for polyglot — a `web/` TS subtree with `package.json`/`tsconfig.json`).

**1. Iteration-cap fixture (D-14 — the GO-05 success-criterion-2 proof).** Precedent: `tests/eval-fixtures/determinism/go_reachable/`. Build a `repo/` whose interface-dispatch graph is large/cyclic enough to exceed a deliberately-tight RTA cap, and assert `BudgetExceeded` is emitted (observable in solver output / `solver_step_count` / `budget_exceeded_reasons`, reserved by Phase 43 D-23) rather than dropped. The determinism manifest style (`determinism/go_reachable/expected.polint-eval.toml`) — minimal, order-independent, `[repo] path = "repo"` + `[budget] max_runtime_ms = 120000` — is the template; the iteration-cap manifest adds an invariant asserting the budget signal.

**2. x/tools RTA native fixtures (D-15).** Reuse the existing `go-x-tools-rta-callgraph` suite (`scoring_mode = "oracle-rta"`) and its adapter `eval::external::go_x_tools_callgraph` (`go_x_tools_callgraph.rs:24-90`). The adapter already projects kernel edges to the `go.rta.call_graph.{graph_kind}` oracle keys (`go_x_tools_callgraph.rs:104-120`) and emits `case_count`/`expected_edge_count` native metrics (`:70-90`). Add `repo/` fixtures exercising interface dispatch, method values/closures (address-taken), and dynamic calls; assert RTA produces the expected reachable-only edges. **`oracle-rta` scoring filters to reachable-from-roots edges (Phase 43 D-17)** — get the reachable seed/marking wiring right (D-07: seed from `ReachabilityRootFact.target_function`) or the suite silently misreads recall.

**3. Polyglot Go+TS canary (D-16 — does NOT exist yet, must be created here).** Precedent: `tests/eval-fixtures/framework-entrypoints/mixed-go-ts/` (a `repo/` with `go.mod` + `main.go` AND `package.json` + `*.ts`/`tsconfig.json` side by side). The `expected.polint-eval.toml` uses `[[expected]] invariant = { name = "...", value = "...", mode = "exact" }` rows (`mixed-go-ts/expected.polint-eval.toml:22-105`). The canary must show **Go edges resolved AND TS behavior unchanged** (the TS policy is still a stub) — i.e. no cross-language interference through the shared solver core. Phase 54 (BENCH-01) later promotes this to a hard gate; Phase 48 only *adds* it and proves non-regression.

---

## Shared Patterns

### Determinism: dense IDs after stable-key sort + BTree accumulation
**Source:** `crates/polint/src/analysis/solver/store.rs:42-50` (`SolverOutput::normalized`) + `engine.rs:177-198` (BTree adjacency).
**Apply to:** every new `go_rta` accumulator, the merged engine output, and the new Go-frontend facts.
```rust
pub(crate) fn normalized(mut self) -> Self {
    self.derived_edges.sort_by(|l, r| (l.stable_key.as_str(), l.id).cmp(&(r.stable_key.as_str(), r.id)));
    for (index, edge) in self.derived_edges.iter_mut().enumerate() {
        edge.id = DerivedEdgeId(index as u64);   // dense IDs ONLY after the stable-key sort
    }
    self
}
```
This is the single mechanism that keeps the 10-shuffle determinism gate green when the new derivation lands (D-17). Use `BTreeMap`/`BTreeSet` for every accumulator; never `HashMap`.

### Honest status/precision + no edge flooding
**Source:** `engine.rs:377-418` (`weakest_status`/`weakest_precision` + rank functions) + `facts.rs:74-86` (precision ceiling).
**Apply to:** every RTA-derived edge.
- Unresolved-after-RTA dispatch stays an honest unresolved/`Unknown` signal — never a fabricated edge (D-08).
- Where an edge is justified by multiple contributing facts, status/precision = the WEAKEST across the adopted derivation (D-09); provenance lists that derivation.
- Derived edges reject `FactPrecision::Exact` (the ceiling, asserted in `facts.rs:110-126`).

### Stable keys from official identity (never run-local order)
**Source:** `crates/polint/src/analysis/reachability/facts.rs:235-257` (`compute_reachability_root_stable_key` + `escape_field`) and `emit.go` `stableKey(...)`.
**Apply to:** every new Go-frontend fact and every RTA edge.
```rust
format!("reachability_root|{}|{}|{}|{}|{}..{}", kind.as_str(), language_label(language),
        escape_field(function_identity), file_id.0, span.start_byte, span.end_byte)
```
Length-prefixed `stable_key_from_parts` (used by `ContributingFact::from_parts`, `provenance.rs:58-62`) keeps the family folded into the key and the boundary `|`-escaped.

### Provider cache-key digests budget + upstream + version
**Source:** `cache_key.rs:33-72` + `provider.rs:125-174`.
**Apply to:** the solver provider's Go-knob + Go-fact digest additions (D-12).
The Go sub-budget rides the existing `budget_parts`; the new Go-frontend fact digests join the consumed-upstream-digest list. A Go-knob or Go-fact change must invalidate downstream (forward-compatible with CACHE-01/02). The locked parts-list test is the intended trip-wire — update it in the same edit.

### Closed-taxonomy enum discipline (pinned order + exhaustive test)
**Source:** `budget.rs:91-103` (`BudgetStatus`) / `constraints.rs:43-94` (`ConstraintKind`) / `reachability/facts.rs:48-71` (`RootKind`).
**Apply to:** any new closed enum (e.g. a dispatch-detail kind on the new Go facts).
Pinned declaration order + derived `Ord` + serde `rename_all = "snake_case"` + an `as_str()` label + a `*_has_exactly_N_variants` compile-time-exhaustive test (`constraints.rs:195-238`). No `#[repr(u8)]`.

---

## No Analog Found

None. Phase 48 is an **extension phase**: every file to be created or modified has a direct in-repo analog (the solver core, the Go frontend, the config table, the fixture tree all exist). The only genuinely *new* artifact with no exact predecessor is the **polyglot Go+TS canary fixture (D-16)**, but its structure is fully precedented by `tests/eval-fixtures/framework-entrypoints/mixed-go-ts/` (mixed Go+TS `repo/` + invariant-style `expected.polint-eval.toml`), so the planner should copy that layout rather than fall back to RESEARCH.md.

---

## Metadata

**Analog search scope:**
- `crates/polint/src/analysis/solver/` (all 9 modules: budget, cache_key, engine, facts, policy, provenance, provider, store, mod)
- `crates/polint/src/analysis/points_to/solver.rs` (proven worklist)
- `crates/polint/src/analysis/semantic_graph/constraints.rs` (constraint vocabulary)
- `crates/polint/src/analysis/reachability/facts.rs` (roots + marking)
- `crates/polint/src/go/semantic/{facts.rs,lower.rs}` + `go-sidecar/.../emit.go`
- `crates/polint/src/config/mod.rs`, `crates/polint/src/analysis/ids.rs`
- `crates/polint/src/analysis_kernel/provider.rs` (provider slot + order snapshots)
- `crates/polint/src/eval/external/go_x_tools_callgraph.rs` + `research/evaluation-harness/suites/go-x-tools-rta-callgraph.toml`
- `tests/eval-fixtures/` tree (determinism, framework-entrypoints/mixed-go-ts precedents)

**Files scanned:** 18 source/config/fixture files read in full or in targeted ranges.
**Pattern extraction date:** 2026-06-02
**Project conventions applied:** CLAUDE.md (`pub(crate)` default, no SDK promotion, `.polint.toml`-only config, cache-digest participation) + `.agents/skills/rust-best-practices/SKILL.md` (BTree determinism, `Result` over panic, exhaustive-match enum tests).
