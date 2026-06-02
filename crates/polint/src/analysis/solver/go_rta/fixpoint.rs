//! The Go RTA fixpoint (D-06, D-07, D-13).
//!
//! [`solve_go_rta`] runs Rapid Type Analysis over a closed [`GoRtaInputs`] snapshot,
//! structurally mirroring `points_to::solver::solve` (a bounded worklist drain with
//! an honest `if !budget_ok { break; }`) and reusing `engine::derive_edges`'
//! disciplines: `BTreeMap`/`BTreeSet`-ordered accumulation, a GLOBAL monotonic
//! `solver_step` counter, and dense IDs assigned only after a stable-key sort via
//! [`SolverOutput::normalized`].
//!
//! The fixpoint iterates **reachability ⊗ dispatch**:
//!
//! 1. Seed the reachable function set from the Phase 43 reachability roots (D-07).
//! 2. Seed the instantiated runtime-type set and the address-taken set from the
//!    Go-frontend facts. These are the WHOLE reachable program's rapid-type /
//!    address-taken sets (the sidecar built SSA over `ssautil.AllPackages` and
//!    harvested `MakeInterface`/`MakeClosure` over the reachable program), so per
//!    D-06 they are "whole-program-instantiated-but-reachable" — the facts carry no
//!    per-function attribution to filter them further, and over-filtering would drop
//!    real RTA targets. The RTA discriminant (interface invoke resolves ONLY to
//!    callees whose receiver type is instantiated) is preserved end-to-end.
//! 3. Each round, for every `UnresolvedDynamic` callsite whose caller is currently
//!    reachable, resolve candidate callees via [`super::dispatch::resolve_callsite`]
//!    (filtered by the instantiated set), record the resolved edges, and add each
//!    newly-resolved callee to the reachable set. Iterate until the reachable set
//!    stops growing (a fixed point).
//!
//! Budget (D-13): the loop is bounded by `budget.go.max_rta_rounds` (round cap), the
//! cross-domain `budget.max_outer_iterations` (worklist-step cap), and
//! `budget.go.max_candidates_per_callsite` (per-callsite fan-out). The address-taken
//! set exceeding `budget.go.address_taken_threshold` also latches exhaustion. Any
//! cap latches the run-level [`BudgetStatus::BudgetExceeded`] — edges resolved before
//! the cap keep their honest status (review finding #R1); the loop never runs
//! unbounded.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::solver::budget::{BudgetStatus, SolverBudget};
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis::solver::store::SolverOutput;

use super::dispatch::resolve_callsite;
use super::inputs::GoRtaInputs;

/// Run the RTA fixpoint over the closed snapshot, returning a normalized
/// [`SolverOutput`] (D-06/D-07/D-13). See the module docs for the model + budget.
pub(crate) fn solve_go_rta(inputs: &GoRtaInputs, budget: &SolverBudget) -> SolverOutput {
    // Index callsites by caller for deterministic, efficient per-round scanning.
    let mut callsites_by_caller: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, callsite) in inputs.callsites.iter().enumerate() {
        callsites_by_caller
            .entry(callsite.caller.as_str())
            .or_default()
            .push(index);
    }

    // Seed the reachable set from roots (D-07). The instantiated + address-taken sets
    // are the whole reachable program's rapid-type / address-taken sets (see module
    // docs); they do not grow during the fixpoint, so the loop is bounded by reachable
    // growth alone (plus the explicit caps).
    let mut reachable: BTreeSet<String> = inputs.roots.clone();
    let instantiated = &inputs.instantiated;
    let address_taken = &inputs.address_taken;

    // Deduplicated edge accumulator keyed by stable key (the store rejects duplicate
    // stable keys; the same callee resolved in different rounds yields an identical
    // stable key, so we keep the first occurrence — its `solver_step` is the earliest,
    // a stable monotonic witness).
    let mut edges_by_key: BTreeMap<String, DerivedEdgeFact> = BTreeMap::new();

    let mut budget_exceeded = false;
    // GLOBAL monotonic step counter (R3): increments once per callsite resolution
    // across the WHOLE run and is never reset, so every derived edge's `solver_step`
    // is globally monotonic.
    let mut solver_step: u64 = 0;
    let mut round: usize = 0;

    loop {
        // Round cap (D-13): bound the reachability ⊗ dispatch iteration.
        if round >= budget.go.max_rta_rounds {
            budget_exceeded = true;
            break;
        }
        round += 1;

        // Address-taken threshold (D-10/D-13): a pathologically large func-value
        // surface latches exhaustion rather than exploding the candidate search.
        if address_taken.len() > budget.go.address_taken_threshold {
            budget_exceeded = true;
            break;
        }

        // Resolve every callsite whose caller is currently reachable; collect newly
        // reachable callees. Iterate `reachable` deterministically (BTreeSet order).
        let mut newly_reachable: BTreeSet<String> = BTreeSet::new();
        let reachable_snapshot: Vec<String> = reachable.iter().cloned().collect();
        for caller in &reachable_snapshot {
            let Some(callsite_indices) = callsites_by_caller.get(caller.as_str()) else {
                continue;
            };
            for &index in callsite_indices {
                // Worklist-step cap (D-13): one callsite resolution is one step.
                solver_step += 1;
                if solver_step > budget.max_outer_iterations as u64 {
                    // Edges resolved before the cap keep their honest status (R1).
                    return finish(edges_by_key, true);
                }

                let callsite = &inputs.callsites[index];
                let resolution = resolve_callsite(
                    callsite,
                    inputs,
                    instantiated,
                    address_taken,
                    budget.go.max_candidates_per_callsite,
                    solver_step,
                );
                if resolution.candidate_cap_exceeded {
                    budget_exceeded = true;
                }
                for edge in resolution.edges {
                    // A newly-resolved callee becomes reachable, so its own callsites
                    // are resolved next round (the fixpoint). Map the target node back
                    // to a qualified identity via the function index (reverse lookup).
                    if let Some(callee) = qualified_for_node(inputs, edge.target)
                        && !reachable.contains(&callee)
                    {
                        newly_reachable.insert(callee);
                    }
                    edges_by_key.entry(edge.stable_key.clone()).or_insert(edge);
                }
            }
        }

        if newly_reachable.is_empty() {
            // Fixed point: no new function became reachable this round.
            break;
        }
        reachable.extend(newly_reachable);
    }

    finish(edges_by_key, budget_exceeded)
}

/// Assemble the normalized output from the deduplicated edge accumulator + the
/// run-level budget status.
fn finish(edges_by_key: BTreeMap<String, DerivedEdgeFact>, budget_exceeded: bool) -> SolverOutput {
    let budget_status = if budget_exceeded {
        BudgetStatus::BudgetExceeded
    } else {
        BudgetStatus::WithinBudget
    };
    SolverOutput {
        derived_edges: edges_by_key.into_values().collect(),
        budget_status,
    }
    .normalized()
}

/// Reverse-lookup a function's `qualified` identity from its semantic node, so a
/// resolved callee can be added to the reachable set. Linear over the function index
/// (small per run); deterministic.
fn qualified_for_node(
    inputs: &GoRtaInputs,
    node: crate::analysis::ids::SemanticNodeId,
) -> Option<String> {
    inputs
        .function_node
        .iter()
        .find(|&(_, &candidate)| candidate == node)
        .map(|(qualified, _)| qualified.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use crate::analysis::ids::SemanticNodeId;
    use crate::analysis::points_to::facts::PointsToStatus;
    use crate::analysis::solver::go_rta::inputs::{GoRtaCallsite, GoRtaInputs, GoRtaMethod};

    /// Build a small interface-dispatch scenario: a `main` caller, an interface invoke
    /// of method `Read` at a callsite, and a concrete type `pkg.File` with a `Read`
    /// method. Whether `pkg.File` is instantiated is the RTA discriminant.
    ///
    /// Nodes: main=1, callsite=2, File.Read=3, Buf.Read=4.
    fn interface_scenario(instantiated_types: &[&str]) -> GoRtaInputs {
        let mut function_node = BTreeMap::new();
        function_node.insert("main.main".to_string(), SemanticNodeId(1));
        function_node.insert("(pkg.File).Read".to_string(), SemanticNodeId(3));
        function_node.insert("(pkg.Buf).Read".to_string(), SemanticNodeId(4));

        let mut method_sets = BTreeMap::new();
        method_sets.insert(
            "pkg.File".to_string(),
            BTreeSet::from(["Read".to_string(), "Close".to_string()]),
        );
        method_sets.insert("pkg.Buf".to_string(), BTreeSet::from(["Read".to_string()]));

        let mut method_set_keys = BTreeMap::new();
        method_set_keys.insert("pkg.File".to_string(), "ms|pkg.File".to_string());
        method_set_keys.insert("pkg.Buf".to_string(), "ms|pkg.Buf".to_string());

        let mut methods_by_receiver = BTreeMap::new();
        methods_by_receiver.insert(
            "pkg.File".to_string(),
            vec![GoRtaMethod {
                method_name: "Read".to_string(),
                qualified: "(pkg.File).Read".to_string(),
                node: SemanticNodeId(3),
            }],
        );
        methods_by_receiver.insert(
            "pkg.Buf".to_string(),
            vec![GoRtaMethod {
                method_name: "Read".to_string(),
                qualified: "(pkg.Buf).Read".to_string(),
                node: SemanticNodeId(4),
            }],
        );

        let mut instantiated = BTreeSet::new();
        let mut instantiated_keys = BTreeMap::new();
        for type_name in instantiated_types {
            instantiated.insert((*type_name).to_string());
            instantiated_keys.insert((*type_name).to_string(), format!("inst|{type_name}"));
        }

        GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            callsites: vec![GoRtaCallsite {
                caller: "main.main".to_string(),
                callsite_node: SemanticNodeId(2),
                callsite_stable_key: "cs|main:read".to_string(),
                interface_method: Some("Read".to_string()),
                signature: None,
                dispatch_stable_key: "dd|main:read".to_string(),
            }],
            method_sets,
            method_set_keys,
            instantiated,
            instantiated_keys,
            function_node,
            methods_by_receiver,
            ..GoRtaInputs::default()
        }
    }

    #[test]
    fn interface_invoke_resolves_only_instantiated_receiver() {
        // pkg.File is instantiated → exactly the pkg.File.Read edge is derived; pkg.Buf
        // is NOT instantiated → no Buf.Read edge (the RTA filter, D-06).
        let inputs = interface_scenario(&["pkg.File"]);
        let output = solve_go_rta(&inputs, &SolverBudget::default());

        assert_eq!(output.budget_status, BudgetStatus::WithinBudget);
        // main(1) -> File.Read(3) resolved; main -> Buf.Read(4) NOT resolved.
        assert!(
            output
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3)),
            "instantiated receiver's method must be resolved"
        );
        assert!(
            !output
                .derived_edges
                .iter()
                .any(|e| e.target == SemanticNodeId(4)),
            "a NON-instantiated receiver's method must NOT be resolved (RTA filter)"
        );
        // The resolved edge is honest: Present status, never-exact precision, and its
        // provenance lists the producing CallConstraint + the contributing facts.
        let edge = output
            .derived_edges
            .iter()
            .find(|e| e.target == SemanticNodeId(3))
            .expect("File.Read edge");
        assert_eq!(edge.status, PointsToStatus::Present);
        assert!(edge.honors_precision_ceiling());
        assert_eq!(edge.provenance.constraint_kind, "call_constraint");
        // callsite + dispatch + method-set + instantiated-type facts.
        assert_eq!(edge.provenance.contributing_len(), 4);
    }

    #[test]
    fn type_not_in_instantiated_set_derives_no_edge() {
        // No type instantiated at all → the interface invoke resolves to nothing
        // (honest unresolved, not a fabricated edge — D-08).
        let inputs = interface_scenario(&[]);
        let output = solve_go_rta(&inputs, &SolverBudget::default());
        assert!(output.derived_edges.is_empty());
        assert_eq!(output.budget_status, BudgetStatus::WithinBudget);
    }

    #[test]
    fn func_value_resolves_via_address_taken_by_signature() {
        // A func-value callsite resolves to address-taken functions whose signature
        // matches. handler (addr-taken, sig "func()") matches; other (sig "func(int)")
        // does not.
        let mut function_node = BTreeMap::new();
        function_node.insert("main.main".to_string(), SemanticNodeId(1));
        function_node.insert("pkg.handler".to_string(), SemanticNodeId(3));
        function_node.insert("pkg.other".to_string(), SemanticNodeId(4));

        let mut function_signature = BTreeMap::new();
        function_signature.insert("pkg.handler".to_string(), "func()".to_string());
        function_signature.insert("pkg.other".to_string(), "func(int)".to_string());

        let mut address_taken = BTreeSet::new();
        address_taken.insert("pkg.handler".to_string());
        address_taken.insert("pkg.other".to_string());
        let mut address_taken_keys = BTreeMap::new();
        address_taken_keys.insert("pkg.handler".to_string(), "at|pkg.handler".to_string());
        address_taken_keys.insert("pkg.other".to_string(), "at|pkg.other".to_string());

        let inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            callsites: vec![GoRtaCallsite {
                caller: "main.main".to_string(),
                callsite_node: SemanticNodeId(2),
                callsite_stable_key: "cs|main:fv".to_string(),
                interface_method: None,
                signature: Some("func()".to_string()),
                dispatch_stable_key: "dd|main:fv".to_string(),
            }],
            address_taken,
            address_taken_keys,
            function_node,
            function_signature,
            ..GoRtaInputs::default()
        };

        let output = solve_go_rta(&inputs, &SolverBudget::default());
        assert!(
            output
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3)),
            "the signature-matching address-taken function must resolve"
        );
        assert!(
            !output
                .derived_edges
                .iter()
                .any(|e| e.target == SemanticNodeId(4)),
            "a signature-mismatching function must NOT resolve"
        );
    }

    #[test]
    fn honest_unresolved_when_method_set_has_no_match() {
        // The instantiated type's method-set does NOT contain the invoked method →
        // no edge (honest unresolved, D-08).
        let mut inputs = interface_scenario(&["pkg.File"]);
        // The callsite invokes a method nobody declares.
        inputs.callsites[0].interface_method = Some("Frobnicate".to_string());
        let output = solve_go_rta(&inputs, &SolverBudget::default());
        assert!(output.derived_edges.is_empty());
    }

    #[test]
    fn deleting_a_contributing_fact_invalidates_the_rta_edge() {
        // D-09 for RTA: the resolved edge depends on its contributing instantiated-type
        // / method-set / callsite / dispatch facts. Removing the instantiated-type fact
        // (i.e. the type is no longer instantiated) does NOT reproduce the SAME edge.
        let baseline = solve_go_rta(&interface_scenario(&["pkg.File"]), &SolverBudget::default());
        let edge = baseline
            .derived_edges
            .iter()
            .find(|e| e.target == SemanticNodeId(3))
            .expect("baseline File.Read edge");
        let baseline_key = edge.stable_key.clone();

        // Remove pkg.File from the instantiated set (delete the contributing fact).
        let without_instantiated = solve_go_rta(&interface_scenario(&[]), &SolverBudget::default());
        assert!(
            !without_instantiated
                .derived_edges
                .iter()
                .any(|e| e.stable_key == baseline_key),
            "deleting the instantiated-type contributing fact must invalidate THAT edge"
        );

        // Removing the method-set match (the type no longer declares the method) also
        // invalidates the edge.
        let mut no_method_set = interface_scenario(&["pkg.File"]);
        no_method_set
            .method_sets
            .get_mut("pkg.File")
            .unwrap()
            .remove("Read");
        let rerun = solve_go_rta(&no_method_set, &SolverBudget::default());
        assert!(
            !rerun
                .derived_edges
                .iter()
                .any(|e| e.stable_key == baseline_key),
            "deleting the method-set contributing fact must invalidate THAT edge"
        );
    }

    #[test]
    fn solve_go_rta_is_shuffle_stable() {
        // BTree-keyed inputs make the output insensitive to insertion order. Build the
        // same scenario via two different instantiated-type insertion orders and assert
        // byte-identical normalized output.
        let forward = solve_go_rta(
            &interface_scenario(&["pkg.File", "pkg.Buf"]),
            &SolverBudget::default(),
        );
        let reversed = solve_go_rta(
            &interface_scenario(&["pkg.Buf", "pkg.File"]),
            &SolverBudget::default(),
        );

        let forward_json =
            serde_json::to_string(&forward.derived_edges).expect("serialize forward");
        let reversed_json =
            serde_json::to_string(&reversed.derived_edges).expect("serialize reversed");
        assert_eq!(forward_json, reversed_json);
    }

    #[test]
    fn runaway_dispatch_latches_budget_exceeded_not_unbounded() {
        // A deliberately tight round cap forces the fixpoint to latch BudgetExceeded
        // rather than loop unbounded (D-13). Build a reachable chain main -> a -> b so
        // resolution needs more than one round, then cap rounds at 1.
        let mut function_node = BTreeMap::new();
        function_node.insert("main.main".to_string(), SemanticNodeId(1));
        function_node.insert("(pkg.A).Step".to_string(), SemanticNodeId(3));
        function_node.insert("(pkg.B).Step".to_string(), SemanticNodeId(5));

        let mut method_sets = BTreeMap::new();
        method_sets.insert("pkg.A".to_string(), BTreeSet::from(["Step".to_string()]));
        method_sets.insert("pkg.B".to_string(), BTreeSet::from(["Step".to_string()]));
        let mut method_set_keys = BTreeMap::new();
        method_set_keys.insert("pkg.A".to_string(), "ms|A".to_string());
        method_set_keys.insert("pkg.B".to_string(), "ms|B".to_string());

        let mut methods_by_receiver = BTreeMap::new();
        methods_by_receiver.insert(
            "pkg.A".to_string(),
            vec![GoRtaMethod {
                method_name: "Step".to_string(),
                qualified: "(pkg.A).Step".to_string(),
                node: SemanticNodeId(3),
            }],
        );
        methods_by_receiver.insert(
            "pkg.B".to_string(),
            vec![GoRtaMethod {
                method_name: "Step".to_string(),
                qualified: "(pkg.B).Step".to_string(),
                node: SemanticNodeId(5),
            }],
        );

        let instantiated = BTreeSet::from(["pkg.A".to_string(), "pkg.B".to_string()]);
        let mut instantiated_keys = BTreeMap::new();
        instantiated_keys.insert("pkg.A".to_string(), "inst|A".to_string());
        instantiated_keys.insert("pkg.B".to_string(), "inst|B".to_string());

        let inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            callsites: vec![
                // main -> A.Step (resolvable round 1)
                GoRtaCallsite {
                    caller: "main.main".to_string(),
                    callsite_node: SemanticNodeId(2),
                    callsite_stable_key: "cs|main".to_string(),
                    interface_method: Some("Step".to_string()),
                    signature: None,
                    dispatch_stable_key: "dd|main".to_string(),
                },
                // A.Step -> B.Step (only reachable in round 2)
                GoRtaCallsite {
                    caller: "(pkg.A).Step".to_string(),
                    callsite_node: SemanticNodeId(4),
                    callsite_stable_key: "cs|a".to_string(),
                    interface_method: Some("Step".to_string()),
                    signature: None,
                    dispatch_stable_key: "dd|a".to_string(),
                },
            ],
            method_sets,
            method_set_keys,
            instantiated,
            instantiated_keys,
            methods_by_receiver,
            function_node,
            ..GoRtaInputs::default()
        };

        let mut budget = SolverBudget::default();
        budget.go.max_rta_rounds = 1;
        let output = solve_go_rta(&inputs, &budget);
        // The cap is hit before the second round resolves A.Step's callsite, so the
        // run latches BudgetExceeded honestly rather than looping.
        assert_eq!(output.budget_status, BudgetStatus::BudgetExceeded);
    }
}
