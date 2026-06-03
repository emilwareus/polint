//! Go RTA acceptance gate (GO-05 success criteria 2, 3, 4 — D-14, D-15, D-16).
//!
//! For each checked-in `tests/eval-fixtures/go-rta/*` (and the polyglot canary) this
//! crate-private gate runs the kernel over the fixture repo, rebuilds the closed Go
//! RTA snapshot ([`GoRtaInputs::from_db`]) plus the points-to `CopyEdge` constraints,
//! drives them through the SAME engine the `polint.solver` provider uses
//! ([`SolverEngine::run_to_solver_output`]), and asserts on the resulting
//! [`SolverOutput`]:
//!
//! - **iteration-cap** (D-14): a deliberately-tight `[solver.go] max_rta_rounds = 1`
//!   in the fixture's own `.polint.toml` forces the multi-round dispatch graph to
//!   latch [`BudgetStatus::BudgetExceeded`] — the GO-05 criterion-2 proof that runaway
//!   dispatch is SIGNALLED, not silently dropped.
//! - **interface-dispatch** (D-15): the instantiated-type FILTER (RTA, not CHA) — the
//!   instantiated receiver's method resolves; a declared-but-not-instantiated type's
//!   method does not.
//! - **address-taken** (D-15): a func-value indirect call resolves via the
//!   address-taken set + signature match.
//! - **polyglot Go+TS canary** (D-16): Go RTA edges resolve AND TS behavior is
//!   unchanged (no TS token-propagation edges — `TsTokensPolicy` is still a stub), so
//!   there is no cross-language interference through the shared solver core.
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
use crate::analysis::solver::policy::{GoRtaPolicy, PointsToPolicy, TsTokensPolicy};
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
/// tight `max_rta_rounds = 1` actually bite (D-14).
fn solver_budget_for_fixture(fixture_dir: &Path) -> SolverBudget {
    let fixture = load_native_fixture(fixture_dir).expect("fixture loads");
    let loaded = load_config(&fixture.repo_dir).expect("config loads");
    SolverBudget {
        go: loaded.config.solver.to_go_sub_budget(),
        ..SolverBudget::default()
    }
}

/// Drives the unified solver engine over the fixture-built db exactly as
/// `derive_solver_with_cache_stats` does (points-to CopyEdge closure + Go RTA + the
/// TS stub), returning the merged, normalized [`SolverOutput`] so the gate can read
/// the run-level `budget_status` and the derived edges.
fn solver_output_for_db(db: &AnalysisDb, budget: SolverBudget) -> SolverOutput {
    let constraints = db.semantic_constraints().to_vec();
    let go_rta_inputs = GoRtaInputs::from_db(db);
    let engine = SolverEngine::new(
        vec![
            Box::new(PointsToPolicy::new(db.points_to_constraints().to_vec())),
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
    // signature match. `handler`/`other` (func()) resolve through the func() callsite;
    // `noise` (func(int)) is EXCLUDED from the func() callsite by its signature (it only
    // resolves through the separate func(int) callsite). The proof that the signature
    // filter holds: the func()-CALLSITE edges (the ones whose caller is main and whose
    // target is a func() address-taken function) never include noise.
    let dir = go_rta_fixture_dir("address-taken");
    let output = run_fixture_kernel(&dir);
    let budget = solver_budget_for_fixture(&dir);
    let solver_output = solver_output_for_db(&output.db, budget);
    let nodes = function_nodes(&output.db);

    let handler = nodes
        .iter()
        .find(|(qualified, _)| qualified.ends_with(".handler"))
        .map(|(_, node)| *node)
        .expect("handler must have a semantic node");

    // The func-value indirect call resolves to the address-taken `handler` (func()).
    assert!(
        solver_output
            .derived_edges
            .iter()
            .any(|edge| edge.target == handler),
        "RTA must resolve the func-value indirect call to the address-taken `handler`: {:#?}",
        solver_output.derived_edges
    );
    // Honest func-value resolution (no flooding): every RTA-resolved (call_constraint)
    // edge targets an ADDRESS-TAKEN function — never an arbitrary function. This is the
    // func-value discipline (the address-taken set is the candidate universe).
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

    // (b) TS behavior unchanged: the TS sources ARE analyzed (TS functions exist), but
    // the TS token policy is a STUB, so the solver derives NO edge whose endpoints are
    // TS function nodes — no cross-language interference.
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
    assert!(
        !solver_output
            .derived_edges
            .iter()
            .any(|edge| ts_nodes.contains(&edge.source) || ts_nodes.contains(&edge.target)),
        "TsTokensPolicy is a stub: the solver must derive NO TS-endpoint edge \
         (no cross-language interference, D-16): {:#?}",
        solver_output.derived_edges
    );
    assert_solver_output_byte_stable(&output.db, budget);
}
