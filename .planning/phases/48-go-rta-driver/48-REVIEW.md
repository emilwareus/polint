---
phase: 48-go-rta-driver
reviewed: 2026-06-02T22:05:00Z
depth: standard
files_reviewed: 17
files_reviewed_list:
  - crates/polint/go-sidecar/polint-go-frontend/internal/semantic/emit.go
  - crates/polint/src/analysis/solver/go_rta/dispatch.rs
  - crates/polint/src/analysis/solver/go_rta/fixpoint.rs
  - crates/polint/src/analysis/solver/go_rta/inputs.rs
  - crates/polint/src/analysis/solver/go_rta/mod.rs
  - crates/polint/src/analysis/solver/engine.rs
  - crates/polint/src/analysis/solver/policy.rs
  - crates/polint/src/analysis/solver/provider.rs
  - crates/polint/src/analysis/solver/budget.rs
  - crates/polint/src/analysis/solver/cache_key.rs
  - crates/polint/src/config/mod.rs
  - crates/polint/src/go/semantic/facts.rs
  - crates/polint/src/go/semantic/lower.rs
  - crates/polint/src/go/semantic/store.rs
  - crates/polint/src/go/semantic/validate.rs
  - crates/polint/src/eval/go_rta.rs
  - crates/polint/src/eval/determinism_gate.rs
findings:
  critical: 1
  warning: 6
  info: 4
  total: 11
status: issues_found
---

# Phase 48: Code Review Report

**Reviewed:** 2026-06-02T22:05:00Z
**Depth:** standard
**Files Reviewed:** 17
**Status:** issues_found

## Summary

Phase 48 extends the Go sidecar to harvest RTA SSA signals (`MakeInterface` instantiated
types, `MakeClosure`/function-value address-taken set, dynamic-dispatch discriminants)
and implements the `analysis::solver::go_rta` Rapid Type Analysis driver over the unified
solver. The work is generally careful and well-documented: determinism discipline
(`BTreeMap`/`BTreeSet` accumulation, dense IDs only after stable-key sort, stable keys from
official Go identity) is consistently applied, the instantiated-type filter that
distinguishes RTA from CHA is correctly implemented, honesty rules (never-exact precision,
honest-unresolved, weakest-trust combine) hold, and `pub(crate)` discipline is clean with
no public-surface leak. The Go sidecar harvest is defensively coded against nil/malformed
SSA (no unguarded panics).

However, there is **one BLOCKER**: the RTA fixpoint reuses the cross-domain
`budget.max_outer_iterations` (default **64**, sized for "number of policy drains") as its
**per-callsite-resolution worklist-step cap**, and that step counter accumulates across
*all* rounds while *every* reachable callsite is re-resolved *every* round. On any
real-world Go repo with more than a few dozen dynamic-dispatch callsite-visits across the
reachability fixpoint, this silently truncates resolution mid-round (dropping real edges)
and latches a spurious `BudgetExceeded`. The checked-in fixtures are all tiny / single-round,
so the gate does not catch it.

The remaining findings are WARNING/INFO: a sidecar recall gap (closure/anonymous-function
bodies are never walked, despite docstrings claiming "the whole reachable program"), a
few honesty/robustness gaps in lowering and digest coverage, and minor quality items.

## Critical Issues

### CR-01: RTA fixpoint uses the policy-count cap (`max_outer_iterations` = 64) as a per-callsite worklist-step cap; real repos falsely latch BudgetExceeded and drop real edges

**File:** `crates/polint/src/analysis/solver/go_rta/fixpoint.rs:101-106` (cap), `:74` (counter), `:94-132` (per-round full re-scan)

**Issue:**
The fixpoint's worklist-step cap is the cross-domain `budget.max_outer_iterations`:

```rust
solver_step += 1;
if solver_step > budget.max_outer_iterations as u64 {
    // Edges resolved before the cap keep their honest status (R1).
    return finish(edges_by_key, true);   // <-- aborts the WHOLE fixpoint mid-round
}
```

Two facts make this wrong on real input:

1. **Wrong constant.** `max_outer_iterations` defaults to **64** and is documented in
   `budget.rs:97-100` as bounding the *number of policy drains* ("A single fixpoint drain
   is one outer iteration today; the cap keeps future multi-policy rounds bounded"). The
   engine (`engine.rs:88-100`) correctly uses it to bound a `VecDeque` of *policy indices*
   (length = 3). Reusing the same `64` to bound *Go callsite resolutions* mixes two
   incompatible scales. The Go sub-budget already has its own knobs (`max_rta_rounds = 32`,
   `max_candidates_per_callsite = 128`, `address_taken_threshold = 256`) but **no
   per-step knob**, so the fixpoint borrows the cross-domain one.

2. **The counter is inflated by full per-round re-scans.** `solver_step` is declared once
   (`:74`) and never reset. Each round (`:94-132`) rebuilds `reachable_snapshot` from the
   *entire* reachable set and re-resolves *every* callsite of *every* reachable caller —
   there is no "frontier" of newly-reachable callers and no per-callsite "already resolved"
   marker. So total steps ≈ Σ_rounds (reachable_callers × their_dynamic_callsites), which
   grows super-linearly. A modest graph (e.g. ~15 reachable dynamic callsite-visits over 5
   convergence rounds = 75 visits) exceeds 64 and aborts.

The production path is affected: `analysis_kernel/mod.rs:591-594` builds
`SolverBudget { go: ..., ..default() }`, leaving `max_outer_iterations = 64`. When the cap
trips, line 105 does `return finish(edges_by_key, true)` — abandoning every callsite not yet
visited this round and every subsequent round, so **real reachable call edges are silently
dropped**, AND `provider.rs:107-109` emits a `budget_exceeded_diagnostic()` warning claiming
"some transitive edges were truncated." The result is incorrect (incomplete) RTA output plus
a misleading honest-looking budget signal on perfectly normal repos.

Why tests don't catch it: every checked-in fixture is tiny. `tests/eval-fixtures/go-rta/
iteration-cap` deliberately trips the *candidate* cap (`max_candidates_per_callsite = 1`) on a
single callsite, `interface-dispatch`/`address-taken` are single-round, the determinism
`go_rta` fixture is single-edge, and the unit tests in `fixpoint.rs` use 1-2 callsites. None
approach 64 callsite-visits, so the mis-scaled cap is invisible to the suite.

**Fix:**
Give the Go RTA fixpoint its own honest step knob (sized like the points-to `max_steps`
default of 10,000, not the policy-count 64), and/or only resolve the callsites of
*newly-reachable* callers each round so the step count reflects real work. Minimal change —
add `max_rta_worklist_steps` (or reuse the existing `budget.max_steps`) and stop re-scanning
the whole reachable set every round:

```rust
// budget.rs — add a Go-scaled step knob (default ~10_000), like points_to max_steps.
pub(crate) struct GoRtaSubBudget {
    pub(crate) address_taken_threshold: usize,
    pub(crate) max_candidates_per_callsite: usize,
    pub(crate) max_rta_rounds: usize,
    pub(crate) max_worklist_steps: usize, // NEW, default 10_000
}

// fixpoint.rs — bound by the Go-scaled knob, and resolve only the FRONTIER each round.
if solver_step > budget.go.max_worklist_steps as u64 {
    return finish(edges_by_key, true);
}
// ...
// Round N>1 should iterate `newly_reachable` from the prior round, not the full set,
// so a converged caller's callsites are not re-resolved every round.
```

Add a fixture (or unit test) with > 64 dynamic callsite-visits across ≥ 2 rounds that
asserts `WithinBudget` and that all expected edges resolve, so the cap scale is pinned.
Remember to fold any new budget knob into `cache_key.rs::budget_parts` +
`provider.rs::solver_output_digest` + the locked digest tests (D-15).

## Warnings

### WR-01: Sidecar never walks closure / anonymous-function bodies, so RTA signals inside `func(){...}` literals are not harvested — contradicting the "whole reachable program" docstrings

**File:** `crates/polint/go-sidecar/polint-go-frontend/internal/semantic/emit.go:498-515` (`ssaFunctions`), `:169-181` (`emitSSAPackage`)

**Issue:**
`ssaFunctions` collects only top-level `pkg.Members` functions and method values
(`Prog.MethodValue`). In `go/ssa`, anonymous functions (closures, `func(){...}` literals,
bound method-value thunks) live in `parent.AnonFuncs`, which is never iterated. As a result,
`emitInstantiatedTypes` / `emitCallsites` / `emitAddressTaken` never walk the `Blocks` of any
closure body, so a `*ssa.MakeInterface`, a dynamic callsite, or a function-value operand that
appears *inside* a closure is invisible to the harvest. This under-approximates the RTA
rapid-type / address-taken / dispatch sets and can make interface dispatch MISS real targets
that are only instantiated inside a closure.

This directly contradicts the claims in `fixpoint.rs:14-20` ("the sidecar built SSA over
`ssautil.AllPackages` and harvested `MakeInterface`/`MakeClosure` over the reachable
program … the WHOLE reachable program's rapid-type / address-taken sets") and `emit.go:341,
388` ("in the reachable SSA program"). Under-approximation is the honest direction (no
fabricated edges), but the docstrings overstate coverage, which is itself a truthfulness
issue per `AGENTS.md` ("Heuristic rules must say they are heuristic and must not claim exact
coverage"). `TestEmitHarvestsRTASignals` does not exercise this — its instantiated type comes
from `main`'s body (`call(t)`), not from the `apply(func(){ t.M() })` closure.

**Fix:**
Recurse into `fn.AnonFuncs` when collecting functions to walk (closures can nest, so do it
transitively), e.g.:

```go
func collectWithAnon(fn *ssa.Function, out *[]*ssa.Function) {
    if fn == nil {
        return
    }
    *out = append(*out, fn)
    for _, anon := range fn.AnonFuncs {
        collectWithAnon(anon, out)
    }
}
```

and feed each top-level/method function through it before the `sort.Slice`. If closure
harvesting is intentionally deferred, soften the docstrings to say "top-level and method
bodies (closure bodies are not yet walked)" so the coverage claim stays honest.

### WR-02: `lower_go_semantic` does not validate that the new RTA-signal rows carry their required identity fields; the discriminant guard only runs at store time and only for dynamic_dispatch

**File:** `crates/polint/src/go/semantic/lower.rs:159-191`

**Issue:**
`lower_address_taken` / `lower_instantiated_type` blindly copy `row.function` / `row.type_name`
with no non-empty check, and `lower_dynamic_dispatch` copies `caller` with no check. Unlike
`lower_callsite` (which routes through `lower_optional_file_span` and rejects repo-escaping
paths), these rows are accepted as-is. The only downstream guards are
`validate.rs::validate_dynamic_dispatch` (discriminant + non-empty `callsite_stable_key`) and
`reject_empty_stable_key`. There is no validation that `address_taken.function` or
`instantiated_type.type_name` is non-empty. A malformed sidecar row with an empty
`type_name` but a non-empty `stable_key` would be stored, then in `inputs.rs:169`
`normalize_type("")` yields `""`, which could spuriously intersect a method-set keyed on an
empty type name. The real sidecar guards against empty identities (`emit.go:373, 404`), so this
is robustness-against-a-misbehaving-frontend rather than a live bug, but the asymmetry with the
other rows is a defensive gap given the sidecar is an external process.

**Fix:**
Reject empty identity fields in `validate.rs` for the harvest rows, mirroring the
dynamic-dispatch guard:

```rust
for instantiated_type in &output.instantiated_types {
    reject_empty_stable_key("instantiated_type", &instantiated_type.stable_key)?;
    if instantiated_type.type_name.is_empty() {
        return Err(invalid_fact("instantiated_type fact has empty type_name".into()));
    }
}
// same for address_taken.function
```

### WR-03: Fallback stable-key recipe in `row_stable_key` cannot distinguish RTA-signal rows, so an absent sidecar stable_key collides distinct facts

**File:** `crates/polint/src/go/semantic/lower.rs:242-258`

**Issue:**
`row_stable_key` falls back to a key built from `(go_kind, package, name=qualified, file,
caller, message)` when `row.stable_key` is empty. For the new `address_taken` /
`instantiated_type` rows the discriminating identity lives in `row.function` /
`row.type_name`, neither of which is in the fallback parts. Two distinct `address_taken`
rows (different `function`, both with empty `qualified`/`caller`) would produce the SAME
fallback key and then be silently deduped by `store.rs` set-dedup (kept-first), dropping a
real set member. The live sidecar always emits a stable_key (`emit.go:413, 382, 337`), so the
fallback is currently dead for these kinds, but it is a latent collision if the protocol ever
emits these rows without a key (or an older frontend is run).

**Fix:**
Either route these kinds through a kind-specific fallback that includes `function` /
`type_name`, or make a missing stable_key on a harvest row a hard lowering error (the rows are
machine-generated and should always carry one):

```rust
"address_taken" | "instantiated_type" | "dynamic_dispatch" if row.stable_key.is_empty() => {
    return Err(GoSemanticLowerError::InvalidPath(format!(
        "Go semantic {} row is missing a stable_key", row.kind
    )));
}
```

### WR-04: Reachability-root → `qualified` mapping relies on span-CONTAINMENT and can mis-map when a file has same-named functions with nested spans

**File:** `crates/polint/src/analysis/solver/go_rta/inputs.rs:265-317`

**Issue:**
`matching_core_function` (and the reverse `qualified_for_function_id`) match a Go semantic
function to a core `FunctionFact` by `file + language + name + span-CONTAINMENT`
(`function.span.start_byte <= span.start_byte <= function.span.end_byte`). Containment, not
equality, is required to bridge the SSA point-span vs. tree-sitter declaration-span gap (well
documented, and methods are receiver-qualified so `(A).Read` vs `(B).Read` disambiguate).
However, `find` returns the FIRST match, and containment is not a unique relation: if two
core functions in the same file share a `name` and one declaration span nests inside the
other's byte range (e.g. an outer function whose span encloses an inner same-named construct,
or overlapping spans from a parse anomaly), the wrong node can be chosen. The
`first`-match-wins over a linear scan also makes the chosen target order-sensitive to
`db.functions()` ordering. In practice Go forbids same-name top-level declarations and method
names are receiver-qualified, so this is low-likelihood, but it is an unguarded correctness
assumption on a containment join.

**Fix:**
Prefer an exact span match when one exists and only fall back to containment when the semantic
span is a zero-width point (the documented SSA case), and assert at most one match:

```rust
// Prefer exact-equal span; fall back to point-in-declaration only for zero-width spans.
let exact = db.functions().iter().find(|f|
    f.file == file && f.language == Language::Go && f.name == name
        && f.span.start_byte == span.start_byte && f.span.end_byte == span.end_byte);
exact.or_else(|| /* point-containment fallback, asserting uniqueness */ )
```

### WR-05: Eval acceptance gate re-drives the solver standalone instead of asserting the kernel-persisted `solver_derived_edges`, so a provider-wiring regression would pass

**File:** `crates/polint/src/eval/go_rta.rs:83-95, 138, 156, 209`

**Issue:**
Every acceptance test builds a fresh `SolverEngine` via `solver_output_for_db(&output.db, …)`
and asserts on *that* output, rather than on `output.db.solver_derived_edges()` (the rows the
`polint.solver` provider actually persisted during the kernel run). The module docstring even
acknowledges this ("The RTA edges are sourced from the kernel-built `AnalysisDb` … NOT through
the call-graph projection"). The risk: if the provider wiring in `analysis_kernel` regressed
(e.g. stopped passing `solver.go` config, stopped registering `GoRtaPolicy`, or failed to
persist), these tests would still pass because they recompute the solver themselves from the
db's input facts. The gate proves the *algorithm* is correct, not that the *pipeline* delivers
it. (`iteration_cap_fixture_latches_budget_exceeded` does assert the config threads through, so
that one path is covered, but the persisted-edge path is not asserted anywhere here.)

**Fix:**
Add at least one assertion against `output.db.solver_derived_edges()` so the persisted output
(post-provider, post-store) is what is checked, e.g. in
`interface_dispatch_fixture_proves_instantiated_type_filter`:

```rust
assert!(
    output.db.solver_derived_edges().iter().any(|e| e.target == dog_speak),
    "the kernel-PERSISTED solver edges must include the resolved (Dog).Speak edge"
);
```

### WR-06: `solver_output_digest` omits the run-level `budget_status` from the output digest, so a budget-truncated run can share a cache digest with a complete run

**File:** `crates/polint/src/analysis/solver/provider.rs:144-204`

**Issue:**
`solver_output_digest` folds the budget *knobs* and the per-edge rows, but never folds
`output.budget_status`. Two runs over the same inputs that produce the same *surviving* edge
set but differ in whether the budget was exhausted (`WithinBudget` vs `BudgetExceeded`) would
compute the **same** output digest. Because the budget status drives a diagnostic
(`provider.rs:107-109`) and is the honest "this result is incomplete" signal, a cache layer
keyed on the output digest could serve a `BudgetExceeded` (truncated) result under a digest
that a later `WithinBudget` run also produces, or vice versa — losing the truncation signal on
a cache hit. The edges themselves carry per-edge `BudgetExceeded` status only when an edge's
own derivation was truncated; a fixpoint that aborted *before reaching* an edge leaves no
per-edge trace, so the run-level status is the only carrier and it is not in the digest.

**Fix:**
Add the run-level status to the digest parts:

```rust
parts.push(format!("budget_status={}", output.budget_status.as_str()));
```

(and extend the digest tests to lock it).

## Info

### IN-01: `qualified_for_node` reverse lookup is an O(reachable × functions) linear scan per resolved edge

**File:** `crates/polint/src/analysis/solver/go_rta/fixpoint.rs:162-171`

**Issue:** For every resolved edge, `qualified_for_node` does a linear `find` over the whole
`function_node` map to recover the callee's `qualified`. Combined with the per-round full
re-scan (CR-01), this is quadratic-ish work. Performance is out of v1 review scope, but it is
worth noting it compounds CR-01: a reverse `SemanticNodeId -> qualified` index built once in
`GoRtaInputs::from_db` would remove the per-edge scan and make the step accounting cheaper.

**Fix:** Precompute `node_to_qualified: BTreeMap<SemanticNodeId, String>` in `from_db` and look
up directly.

### IN-02: `lower.rs` match has a redundant catch-all arm

**File:** `crates/polint/src/go/semantic/lower.rs:67-68`

**Issue:** `"receiver_type" | "unsupported" | "type_fact" => {}` followed by `_ => {}` — both
arms are no-ops, so the explicit list documents intent but is functionally identical to the
catch-all. Harmless, but a reader may assume the named arm does something. Consider a comment
("known-ignored kinds") to make the intent explicit, since the protocol's `allowed_kinds`
already gates unknown kinds upstream.

**Fix:** Add an explanatory comment, or drop the named arm and rely on `_ => {}`.

### IN-03: `address_taken_threshold` is checked once per round, not against the actual candidate fan-out

**File:** `crates/polint/src/analysis/solver/go_rta/fixpoint.rs:87-90`

**Issue:** The address-taken threshold latches `BudgetExceeded` when `address_taken.len() >
threshold`. Since `address_taken` is immutable across the fixpoint (seeded once), this check is
loop-invariant and could be hoisted out of the round loop for clarity (it produces the same
result every round). Functionally correct; just redundant work and slightly misleading placement
(it reads as if the set could grow during the loop, which it cannot per the module docs).

**Fix:** Hoist the threshold check above the `loop` (evaluate once before iterating).

### IN-04: Sidecar `versionInts` silently coerces an unparseable Go version segment to 0, which can mis-order synthetic `go.work` versions

**File:** `crates/polint/go-sidecar/polint-go-frontend/internal/semantic/emit.go:858-869`

**Issue:** `versionInts` maps any non-numeric segment to `0` (e.g. a pre-release/`rc` suffix
like `1.24rc1` → `[1, 24, 0]` after `Atoi` fails on `"24rc1"`... actually `Atoi("24rc1")`
fails and yields `0`, so `1.24rc1` → `[1, 0]`). `compareGoVersion` would then under-order such a
version and `syntheticGoWorkVersion` might pick a lower `go` directive than intended for the
synthetic workspace. This only affects synthetic-`go.work` generation for multi-module repos
with non-standard version strings; the impact is a possibly-too-low `go` line, which Go tooling
usually tolerates. Worth hardening if pre-release Go directives are in scope.

**Fix:** On `Atoi` failure, strip a trailing non-numeric suffix and re-parse the numeric prefix,
or treat the segment as the max of parsed digits, rather than collapsing to 0.

---

_Reviewed: 2026-06-02T22:05:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
