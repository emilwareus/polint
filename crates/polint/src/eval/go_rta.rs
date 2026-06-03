//! Go RTA acceptance gate (GO-05 success criteria 2, 3, 4 — D-14, D-15, D-16).
//!
//! For each checked-in `tests/eval-fixtures/go-rta/*` (and the polyglot canary) this
//! crate-private gate runs the kernel over the fixture repo, rebuilds the closed Go
//! RTA snapshot ([`GoRtaInputs::from_db`]) plus the points-to `CopyEdge` constraints,
//! drives them through the SAME engine the `polint.solver` provider uses
//! ([`SolverEngine::run_to_solver_output`]), and asserts on the resulting
//! [`SolverOutput`]:
//!
//! - **iteration-cap** (D-14): a deliberately-tight `[solver.go]
//!   max_candidates_per_callsite = 1` in the fixture's own `.polint.toml` cannot admit
//!   the three candidate callees (`Circle`/`Square`/`Triangle.Area`) of the single
//!   `s.Area()` interface invoke, so resolving that callsite latches
//!   [`BudgetStatus::BudgetExceeded`] — the GO-05 criterion-2 proof that runaway
//!   dispatch fan-out is SIGNALLED, not silently dropped.
//! - **interface-dispatch** (D-15): the instantiated-type FILTER (RTA, not CHA) — the
//!   instantiated receiver's method resolves; a declared-but-not-instantiated type's
//!   method does not.
//! - **address-taken** (D-15): a func-value indirect call resolves via the
//!   address-taken set + signature match.
//! - **polyglot Go+TS canary** (D-16): Go RTA edges resolve AND there is no
//!   cross-language interference. The TS half genuinely feeds the shared solver core a
//!   CopyEdge constraint (an aliased local call); the language-agnostic points-to closure
//!   propagates it INTRA-TS, but NO derived edge crosses the Go<->TS boundary and the
//!   TS-specific `TsTokensPolicy` (still a stub) contributes no `call_constraint` edge —
//!   so the two halves never reach across the language boundary through the shared core.
//!
//! The RTA edges are sourced from the kernel-built `AnalysisDb` (`solver_derived_edges`
//! /`GoRtaInputs`), NOT through the call-graph projection — Phase 52 (GRAPH-05) wires
//! solver edges into `refined_calls`; until then the self-contained engine drive here
//! is the always-runnable proof. A two-run byte-stability check guards determinism.
//!
//! All types stay `pub(crate)`; no `ALLOWED_PRELUDE` entry is added (D-17).

#![cfg(test)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::semantic_graph::facts::NodeKind;
use crate::analysis::solver::budget::{BudgetStatus, SolverBudget};
use crate::analysis::solver::engine::SolverEngine;
use crate::analysis::solver::go_rta::GoRtaInputs;
use crate::analysis::solver::policy::{GoRtaPolicy, TsTokensPolicy};
use crate::analysis::solver::store::SolverOutput;
use crate::analysis_kernel::KernelOutput;
use crate::config::load_config;
use crate::core::AnalysisDb;
use crate::eval::fixtures::load_native_fixture;
use crate::eval::observed::{copy_fixture_repo_for_test, run_kernel_for_repo_for_test};

fn go_rta_fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/eval-fixtures/go-rta")
        .join(name)
}

fn polyglot_fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/eval-fixtures/polyglot-canary")
        .join(name)
}

/// Runs the kernel over a fixture repo and returns the raw [`KernelOutput`].
fn run_fixture_kernel(fixture_dir: &Path) -> KernelOutput {
    let fixture = load_native_fixture(fixture_dir).expect("fixture loads");
    let temp = copy_fixture_repo_for_test(&fixture).expect("copy fixture repo");
    run_kernel_for_repo_for_test(temp.path()).expect("kernel runs over fixture repo")
}

/// Reads the fixture's own `[solver.go]` config so the gate drives the solver under
/// the SAME budget the kernel would — this is what makes the iteration-cap fixture's
/// tight `max_candidates_per_callsite = 1` actually bite (D-14).
fn solver_budget_for_fixture(fixture_dir: &Path) -> SolverBudget {
    let fixture = load_native_fixture(fixture_dir).expect("fixture loads");
    let loaded = load_config(&fixture.repo_dir).expect("config loads");
    SolverBudget {
        go: loaded.config.solver.to_go_sub_budget(),
        ..SolverBudget::default()
    }
}

/// Drives the unified solver engine over the fixture-built db exactly as
/// `derive_solver_with_cache_stats` does (the points-to CopyEdge closure via step-1
/// `derive_edges` + the Go RTA policy + the TS stub), returning the merged, normalized
/// [`SolverOutput`] so the gate can read the run-level `budget_status` and the derived
/// edges. Like production, it does NOT register `PointsToPolicy` (FINDING 3): the
/// points-to edges come from the step-1 CopyEdge closure, and registering the Andersen
/// fold here would re-run a discarded solve whose budget status would pollute the run.
fn solver_output_for_db(db: &AnalysisDb, budget: SolverBudget) -> SolverOutput {
    let constraints = db.semantic_constraints().to_vec();
    let go_rta_inputs = GoRtaInputs::from_db(db);
    let engine = SolverEngine::new(
        vec![
            Box::new(GoRtaPolicy::new(go_rta_inputs)),
            Box::new(TsTokensPolicy),
        ],
        budget,
    );
    engine.run_to_solver_output(&constraints)
}

/// `qualified -> SemanticNodeId` for the fixture's Go functions, so the gate can
/// assert WHICH concrete method an interface invoke resolved to (e.g. `(…Dog).Speak`
/// vs `(…Cat).Speak`, which share the bare name `Speak`).
fn function_nodes(db: &AnalysisDb) -> BTreeMap<String, SemanticNodeId> {
    GoRtaInputs::from_db(db).function_node
}

/// Two engine runs over the same fixture-built db produce byte-identical serialized
/// derived edges (the determinism discipline; dense `id` is `#[serde(skip)]` so this
/// captures endpoints/status/precision/stable_key/provenance).
fn assert_solver_output_byte_stable(db: &AnalysisDb, budget: SolverBudget) {
    let first = solver_output_for_db(db, budget);
    let second = solver_output_for_db(db, budget);
    let first_json = serde_json::to_string(&first.derived_edges).expect("serialize first");
    let second_json = serde_json::to_string(&second.derived_edges).expect("serialize second");
    assert_eq!(
        first_json, second_json,
        "solver derived-edge JSON diverged across two engine runs"
    );
    assert_eq!(
        first.budget_status, second.budget_status,
        "solver budget status diverged across two engine runs"
    );
}

#[test]
fn iteration_cap_fixture_latches_budget_exceeded() {
    // D-14 / GO-05 criterion 2: the fixture's own tight
    // `[solver.go] max_candidates_per_callsite = 1` cannot admit the three candidate
    // callees (Circle/Square/Triangle.Area) of the single `s.Area()` interface invoke,
    // so resolving that callsite latches BudgetExceeded rather than silently dropping
    // the over-cap candidates. This proves runaway dispatch fan-out is SIGNALLED and
    // that the fixture's own config threads into the GoRtaSubBudget end to end.
    let dir = go_rta_fixture_dir("iteration-cap");
    let output = run_fixture_kernel(&dir);
    let budget = solver_budget_for_fixture(&dir);
    assert_eq!(
        budget.go.max_candidates_per_callsite, 1,
        "the fixture's `[solver.go] max_candidates_per_callsite` must thread through (1)"
    );

    let solver_output = solver_output_for_db(&output.db, budget);
    assert_eq!(
        solver_output.budget_status,
        BudgetStatus::BudgetExceeded,
        "the tight per-callsite candidate cap must latch BudgetExceeded (D-14): {:#?}",
        solver_output.derived_edges
    );
    assert_solver_output_byte_stable(&output.db, budget);
}

#[test]
fn interface_dispatch_fixture_proves_instantiated_type_filter() {
    // D-15 / GO-05 criterion 4: Dog is instantiated → main -> (Dog).Speak resolves;
    // Cat is declared + in its method-set but NEVER instantiated → no edge targets
    // (Cat).Speak. This is the RTA discriminant (instantiated-type filter), not CHA.
    let dir = go_rta_fixture_dir("interface-dispatch");
    let output = run_fixture_kernel(&dir);
    let budget = solver_budget_for_fixture(&dir);
    let solver_output = solver_output_for_db(&output.db, budget);
    let nodes = function_nodes(&output.db);

    let dog_speak = nodes
        .iter()
        .find(|(qualified, _)| qualified.contains("Dog") && qualified.ends_with(".Speak"))
        .map(|(_, node)| *node)
        .expect("(Dog).Speak must have a semantic node");
    let cat_speak = nodes
        .iter()
        .find(|(qualified, _)| qualified.contains("Cat") && qualified.ends_with(".Speak"))
        .map(|(_, node)| *node)
        .expect("(Cat).Speak must have a semantic node");

    // The instantiated receiver's method is resolved.
    assert!(
        solver_output
            .derived_edges
            .iter()
            .any(|edge| edge.target == dog_speak),
        "RTA must resolve the interface invoke to the instantiated (Dog).Speak: {:#?}",
        solver_output.derived_edges
    );
    // The NON-instantiated receiver's method is excluded (the RTA filter, not CHA).
    assert!(
        !solver_output
            .derived_edges
            .iter()
            .any(|edge| edge.target == cat_speak),
        "RTA must NOT resolve to the non-instantiated (Cat).Speak (instantiated-type filter)"
    );
    // Every resolved RTA edge is honest: Present + never-exact precision.
    for edge in &solver_output.derived_edges {
        assert!(
            edge.honors_precision_ceiling(),
            "RTA derived edges must never claim exact precision (D-08): {edge:#?}"
        );
    }
    assert_eq!(solver_output.budget_status, BudgetStatus::WithinBudget);

    // WR-05: assert the KERNEL-PERSISTED edges (the rows the `polint.solver` provider
    // actually stored during the kernel run via SolverEngine + GoRtaPolicy + store),
    // not only the standalone recompute above. This catches a provider-wiring
    // regression (e.g. the provider stops threading `solver.go` config, stops
    // registering GoRtaPolicy, or fails to persist) that the recompute would mask.
    let persisted = output.db.solver_derived_edges();
    assert!(
        persisted.iter().any(|edge| edge.target == dog_speak),
        "the kernel-PERSISTED solver edges must include the resolved (Dog).Speak edge: {persisted:#?}"
    );
    assert!(
        !persisted.iter().any(|edge| edge.target == cat_speak),
        "the kernel-PERSISTED solver edges must exclude the non-instantiated (Cat).Speak"
    );

    assert_solver_output_byte_stable(&output.db, budget);
}

#[test]
fn alias_dispatch_fixture_resolves_dispatch_through_a_type_alias() {
    // FIX 3: a value of a type ALIAS (`type AliasDog = Dog`) converted to an interface must
    // dispatch to the UNDERLYING type's method. go/types reports the instantiated_type and
    // AliasDog's method-set under the alias spelling (`...AliasDog`), while (Dog).Speak's
    // receiver is the underlying `...Dog`; without the sidecar's `types.Unalias`
    // canonicalization, the instantiated-type ⋈ method-set ⋈ receiver join misses and the
    // `s.Speak()` edge is silently dropped. After the fix the edge resolves to (Dog).Speak.
    // Cat (declared, never instantiated) must still be excluded — proving the alias edge is
    // the RTA instantiated-type filter at work, not a coarse CHA fallback.
    let dir = go_rta_fixture_dir("alias-dispatch");
    let output = run_fixture_kernel(&dir);
    let budget = solver_budget_for_fixture(&dir);
    let solver_output = solver_output_for_db(&output.db, budget);
    let nodes = function_nodes(&output.db);

    let dog_speak = nodes
        .iter()
        .find(|(qualified, _)| qualified.contains("Dog") && qualified.ends_with(".Speak"))
        .map(|(_, node)| *node)
        .expect("(Dog).Speak must have a semantic node");
    let cat_speak = nodes
        .iter()
        .find(|(qualified, _)| qualified.contains("Cat") && qualified.ends_with(".Speak"))
        .map(|(_, node)| *node)
        .expect("(Cat).Speak must have a semantic node");

    // The interface invoke of the alias-spelled value resolves to the underlying
    // (Dog).Speak (the join succeeds because the sidecar canonicalized the alias).
    assert!(
        solver_output
            .derived_edges
            .iter()
            .any(|edge| edge.target == dog_speak),
        "RTA must resolve the alias-spelled interface invoke to (Dog).Speak: {:#?}",
        solver_output.derived_edges
    );
    // The non-instantiated implementer is still excluded (instantiated-type filter, not CHA).
    assert!(
        !solver_output
            .derived_edges
            .iter()
            .any(|edge| edge.target == cat_speak),
        "RTA must NOT resolve to the non-instantiated (Cat).Speak (instantiated-type filter)"
    );
    for edge in &solver_output.derived_edges {
        assert!(
            edge.honors_precision_ceiling(),
            "RTA derived edges must never claim exact precision (D-08): {edge:#?}"
        );
    }
    assert_eq!(solver_output.budget_status, BudgetStatus::WithinBudget);

    // The KERNEL-PERSISTED edges (the rows the provider actually stored) must also carry the
    // resolved dispatch — catches a provider-wiring regression the recompute masks.
    let persisted = output.db.solver_derived_edges();
    assert!(
        persisted.iter().any(|edge| edge.target == dog_speak),
        "the kernel-PERSISTED solver edges must include the (Dog).Speak edge: {persisted:#?}"
    );
    assert!(
        !persisted.iter().any(|edge| edge.target == cat_speak),
        "the kernel-PERSISTED solver edges must exclude the non-instantiated (Cat).Speak"
    );

    assert_solver_output_byte_stable(&output.db, budget);
}

#[test]
fn generic_dispatch_fixture_resolves_method_on_instantiated_generic_type() {
    // FINDING A: an interface satisfied by a method on a GENERIC type, dispatched via the
    // interface, must resolve to the INSTANTIATED type's method. x/tools records the
    // instantiated named type `Box[int]` in the SSA runtime-type set with a real method
    // value `(Box[int]).Speak`, while the syntactic declaration is `Box[T any]`. The
    // frontend previously keyed the method-set by the GENERIC name only, so the Rust
    // resolver's `method_sets.get("Box[int]")` missed and the edge was lost. The fix emits
    // the method-set AND the concrete method keyed by the instantiated identity, so the
    // invoke of `Speak` resolves to the (Box[int]).Speak node.
    let dir = go_rta_fixture_dir("generic-dispatch");
    let output = run_fixture_kernel(&dir);
    let budget = solver_budget_for_fixture(&dir);
    let solver_output = solver_output_for_db(&output.db, budget);
    let nodes = function_nodes(&output.db);

    let box_speak = nodes
        .iter()
        .find(|(qualified, _)| qualified.contains("Box[int]") && qualified.ends_with(".Speak"))
        .map(|(_, node)| *node)
        .unwrap_or_else(|| {
            panic!("(Box[int]).Speak must have a semantic node; got nodes {nodes:#?}")
        });

    // The interface invoke resolves to the instantiated generic type's method.
    assert!(
        solver_output
            .derived_edges
            .iter()
            .any(|edge| edge.target == box_speak),
        "RTA must resolve the interface invoke to the instantiated (Box[int]).Speak: {:#?}",
        solver_output.derived_edges
    );
    for edge in &solver_output.derived_edges {
        assert!(
            edge.honors_precision_ceiling(),
            "RTA derived edges must never claim exact precision (D-08): {edge:#?}"
        );
    }
    assert_eq!(solver_output.budget_status, BudgetStatus::WithinBudget);

    // The KERNEL-PERSISTED edges (the rows the provider actually stored) must also carry
    // the resolved dispatch — catches a provider-wiring regression the recompute masks.
    let persisted = output.db.solver_derived_edges();
    assert!(
        persisted.iter().any(|edge| edge.target == box_speak),
        "the kernel-PERSISTED solver edges must include the (Box[int]).Speak edge: {persisted:#?}"
    );

    assert_solver_output_byte_stable(&output.db, budget);
}

#[test]
fn static_reachability_fixture_resolves_dispatch_in_statically_reached_function() {
    // FINDING 1 / standard RTA: `main` (a root) makes a DIRECT (static) call to the
    // UNEXPORTED helper `runDispatch` (NOT a root). The interface invoke `s.Speak()`
    // lives inside runDispatch. RTA reachability must be the fixpoint closure over BOTH
    // static-call edges AND resolved-dynamic-dispatch edges from roots: the static
    // main -> runDispatch edge must bring runDispatch into the worklist so its dispatch
    // resolves to (Dog).Speak. If reachability only grew along dynamic edges, runDispatch
    // never enters the worklist and the real `runDispatch -> (Dog).Speak` edge is
    // silently dropped while the run still reports WithinBudget.
    let dir = go_rta_fixture_dir("static-reachability");
    let output = run_fixture_kernel(&dir);
    let budget = solver_budget_for_fixture(&dir);
    let solver_output = solver_output_for_db(&output.db, budget);
    let nodes = function_nodes(&output.db);

    let run_dispatch = nodes
        .iter()
        .find(|(qualified, _)| qualified.ends_with(".runDispatch"))
        .map(|(_, node)| *node)
        .expect("runDispatch must have a semantic node");
    let dog_speak = nodes
        .iter()
        .find(|(qualified, _)| qualified.contains("Dog") && qualified.ends_with(".Speak"))
        .map(|(_, node)| *node)
        .expect("(Dog).Speak must have a semantic node");

    // The dispatch inside the statically-reached helper resolves: runDispatch -> (Dog).Speak.
    assert!(
        solver_output
            .derived_edges
            .iter()
            .any(|edge| edge.source == run_dispatch && edge.target == dog_speak),
        "RTA must resolve the interface invoke inside the STATICALLY-reached runDispatch \
         to (Dog).Speak (reachability must close over static-call edges): {:#?}",
        solver_output.derived_edges
    );
    assert_eq!(
        solver_output.budget_status,
        BudgetStatus::WithinBudget,
        "a finite static-then-dynamic chain must converge WithinBudget"
    );

    // The KERNEL-PERSISTED edges (the rows the provider actually stored) must also carry
    // the resolved dispatch — catches a provider-wiring regression the recompute masks.
    let persisted = output.db.solver_derived_edges();
    assert!(
        persisted
            .iter()
            .any(|edge| edge.source == run_dispatch && edge.target == dog_speak),
        "the kernel-PERSISTED solver edges must include runDispatch -> (Dog).Speak: {persisted:#?}"
    );

    assert_solver_output_byte_stable(&output.db, budget);
}

#[test]
fn address_taken_fixture_resolves_func_value_by_signature() {
    // D-15: the func() indirect call in main resolves via the address-taken set +
    // signature match. The SINGLE func-value callsite is `funcs[0]()` (a func()); it fans
    // out to BOTH same-signature address-taken functions `handler` AND `other`. `noise`
    // (func(int)) is ALSO address-taken (in a never-invoked []func(int) slice), so it is in
    // the candidate universe — but its signature does not match the func() callsite, and
    // there is NO func(int) callsite, so it must NEVER be a resolved target. This is the
    // non-vacuous signature-filter proof: a same-set-but-wrong-signature candidate is
    // EXCLUDED, not flooded in (the previous test only checked targets were address-taken,
    // which `noise` is — so a regression resolving func() to noise would have passed).
    let dir = go_rta_fixture_dir("address-taken");
    let output = run_fixture_kernel(&dir);
    let budget = solver_budget_for_fixture(&dir);
    let solver_output = solver_output_for_db(&output.db, budget);
    let nodes = function_nodes(&output.db);

    let node_for = |suffix: &str| -> SemanticNodeId {
        nodes
            .iter()
            .find(|(qualified, _)| qualified.ends_with(suffix))
            .map(|(_, node)| *node)
            .unwrap_or_else(|| panic!("{suffix} must have a semantic node; got {nodes:#?}"))
    };
    let handler = node_for(".handler");
    let other = node_for(".other");
    let noise = node_for(".noise");

    let targets: std::collections::BTreeSet<SemanticNodeId> = solver_output
        .derived_edges
        .iter()
        .filter(|edge| edge.provenance.constraint_kind == "call_constraint")
        .map(|edge| edge.target)
        .collect();

    // The func() callsite fans out to BOTH same-signature address-taken funcs.
    assert!(
        targets.contains(&handler),
        "RTA must resolve the func() indirect call to the address-taken `handler`: {:#?}",
        solver_output.derived_edges
    );
    assert!(
        targets.contains(&other),
        "RTA must ALSO resolve the func() indirect call to the same-signature `other` \
         (fans out to BOTH func() address-taken funcs): {:#?}",
        solver_output.derived_edges
    );
    // The signature filter EXCLUDES noise (func(int)): it is address-taken (in the candidate
    // universe) but never a resolved target — no func(int) callsite, and its signature does
    // not match the func() callsite.
    assert!(
        !targets.contains(&noise),
        "RTA must NOT resolve any callsite to `noise` (func(int)) — the signature filter \
         excludes a same-set-but-wrong-signature candidate: {:#?}",
        solver_output.derived_edges
    );
    // `noise` IS in the address-taken set (so the exclusion is a real signature-filter
    // decision, not merely an absent candidate).
    assert!(
        GoRtaInputs::from_db(&output.db)
            .address_taken
            .iter()
            .any(|qualified| qualified.ends_with(".noise")),
        "noise must be genuinely address-taken (a candidate the signature filter excludes)"
    );

    // Honest func-value resolution (no flooding): every RTA-resolved (call_constraint)
    // edge targets an ADDRESS-TAKEN function — never an arbitrary function.
    let address_taken_nodes: std::collections::BTreeSet<SemanticNodeId> =
        GoRtaInputs::from_db(&output.db)
            .address_taken
            .iter()
            .filter_map(|qualified| nodes.get(qualified).copied())
            .collect();
    for edge in solver_output
        .derived_edges
        .iter()
        .filter(|edge| edge.provenance.constraint_kind == "call_constraint")
    {
        assert!(
            address_taken_nodes.contains(&edge.target),
            "every func-value RTA edge must target an address-taken function (no flooding): {edge:#?}"
        );
        assert!(
            edge.honors_precision_ceiling(),
            "RTA derived edges must never claim exact precision (D-08): {edge:#?}"
        );
    }
    assert_solver_output_byte_stable(&output.db, budget);
}

#[test]
fn polyglot_go_ts_canary_resolves_go_edges_without_ts_interference() {
    // D-16 / GO-05 criterion 3: with the Go RTA driver active and the TS token policy
    // still a stub, the mixed Go+TS fixture shows Go edges resolved AND TS behavior
    // unchanged (no cross-language interference through the shared solver core).
    let dir = polyglot_fixture_dir("go-ts");
    let output = run_fixture_kernel(&dir);
    let budget = solver_budget_for_fixture(&dir);
    let solver_output = solver_output_for_db(&output.db, budget);
    let nodes = function_nodes(&output.db);

    // (a) Go driver active: the instantiated (Dog).Speak interface invoke resolves.
    let dog_speak = nodes
        .iter()
        .find(|(qualified, _)| qualified.contains("Dog") && qualified.ends_with(".Speak"))
        .map(|(_, node)| *node)
        .expect("(Dog).Speak must have a semantic node in the polyglot repo");
    assert!(
        solver_output
            .derived_edges
            .iter()
            .any(|edge| edge.target == dog_speak),
        "the polyglot canary must resolve the Go interface invoke to (Dog).Speak: {:#?}",
        solver_output.derived_edges
    );

    // (b) The TS sources are analyzed AND genuinely feed the shared solver core a
    // solver-relevant constraint that COULD propagate. The cross-language NON-INTERFERENCE
    // proof is then non-vacuous (FINDING F2): no derived edge crosses the Go<->TS language
    // boundary, and the TS-specific token policy (TsTokensPolicy, still a Phase-48 stub)
    // contributes nothing. (The language-agnostic points-to CopyEdge closure DOES propagate
    // the TS CopyEdge INTRA-TS — that is the shared core working uniformly, not cross-
    // language interference; we assert specifically that no Go<->TS edge appears.)
    let ts_function_ids: std::collections::BTreeSet<crate::core::FunctionId> = output
        .db
        .functions()
        .iter()
        .filter(|function| function.language == crate::core::Language::TypeScript)
        .map(|function| function.id)
        .collect();
    assert!(
        !ts_function_ids.is_empty(),
        "the polyglot canary must actually analyze TS functions (the TS half is live)"
    );
    let node_language = |node_id: SemanticNodeId| -> Option<crate::core::Language> {
        output.db.semantic_nodes().iter().find_map(|node| {
            if node.id != node_id {
                return None;
            }
            match node.kind {
                NodeKind::Function(function_id) => output
                    .db
                    .functions()
                    .iter()
                    .find(|function| function.id == function_id)
                    .map(|function| function.language),
                _ => None,
            }
        })
    };
    let ts_nodes: std::collections::BTreeSet<SemanticNodeId> = output
        .db
        .semantic_nodes()
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::Function(function_id) if ts_function_ids.contains(&function_id) => {
                Some(node.id)
            }
            _ => None,
        })
        .collect();

    // NON-VACUITY: the TS half emits >= 1 CopyEdge with a TS function-node endpoint (the
    // `const aliasMake = makeToken` alias), so the constraint surface that COULD leak into
    // the shared solver core genuinely exists.
    let ts_copy_edges = output
        .db
        .semantic_constraints()
        .iter()
        .filter(|constraint| match &constraint.kind {
            crate::analysis::semantic_graph::constraints::ConstraintKind::CopyEdge { dst, src } => {
                ts_nodes.contains(dst) || ts_nodes.contains(src)
            }
            _ => false,
        })
        .count();
    assert!(
        ts_copy_edges >= 1,
        "the polyglot canary's TS half must emit >= 1 CopyEdge with a TS function-node \
         endpoint (so the non-interference assertion is non-vacuous), got {ts_copy_edges}"
    );

    // NO CROSS-LANGUAGE INTERFERENCE: no derived edge connects a Go node to a TS node (in
    // either direction). The shared core may propagate the TS CopyEdge intra-TS, and resolve
    // the Go interface invoke Go-internally, but the two halves never reach across the
    // language boundary through the shared solver core (D-16).
    for edge in &solver_output.derived_edges {
        let source_ts = ts_nodes.contains(&edge.source);
        let target_ts = ts_nodes.contains(&edge.target);
        if source_ts ^ target_ts {
            // Exactly one endpoint is a TS function node — the other must NOT be a Go node.
            let other = if source_ts { edge.target } else { edge.source };
            assert_ne!(
                node_language(other),
                Some(crate::core::Language::Go),
                "no derived edge may cross the Go<->TS boundary (cross-language interference): {edge:#?}"
            );
        }
    }

    // The TS-specific token policy adds nothing: every TS-endpoint derived edge is a
    // language-agnostic `copy_edge` from the shared points-to closure, NEVER a
    // `call_constraint` (the TsTokensPolicy stub resolves no dynamic TS dispatch).
    for edge in solver_output
        .derived_edges
        .iter()
        .filter(|edge| ts_nodes.contains(&edge.source) || ts_nodes.contains(&edge.target))
    {
        assert_eq!(
            edge.provenance.constraint_kind, "copy_edge",
            "a TS-endpoint derived edge must come only from the shared points-to CopyEdge \
             closure, never the TsTokensPolicy stub (which is empty in Phase 48): {edge:#?}"
        );
    }

    assert_solver_output_byte_stable(&output.db, budget);
}
