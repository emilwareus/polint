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
//! 3. Each round, for every caller that became reachable in the PRIOR round (a
//!    FRONTIER, seeded from roots), the fixpoint (a) grows reachability over that
//!    caller's STATIC-call edges (`inputs.static_call_targets`) — standard RTA closes
//!    reachability over BOTH static-call AND resolved-dynamic-dispatch edges, so a
//!    function reached only via a direct call that is not itself a root still enters
//!    the worklist and its dispatch is resolved; static edges GROW reachability only
//!    and do NOT emit edges — and (b) resolves that caller's `UnresolvedDynamic`
//!    callsites via [`super::dispatch::resolve_callsite`] (filtered by the instantiated
//!    set), records the resolved edges, and adds each newly-resolved callee to the
//!    reachable set. The union of statically- and dynamically-newly-reached functions
//!    is the next round's frontier. Iterate until no new function becomes reachable (a
//!    fixed point). A caller is processed exactly ONCE (when it first enters the
//!    frontier), never re-scanned every round.
//!
//! Budget (D-13): the loop is bounded by `budget.go.max_rta_rounds` (the DISPATCH-round
//! cap), the Go-scaled `budget.go.max_worklist_steps` (worklist-step cap — one step per
//! callsite resolution AND one per static-call-edge expansion, so a deep static chain
//! consumes the same budget; sized like points-to `max_steps` = 10_000, NOT the
//! policy-count `max_outer_iterations` = 64 — review CR-01), and
//! `budget.go.max_candidates_per_callsite` (per-callsite fan-out). The address-taken
//! set exceeding `budget.go.address_taken_threshold` also latches exhaustion (but only
//! when func-value resolution is actually NEEDED — see FIX 2). The round cap bounds only
//! GENUINE dynamic-dispatch re-iteration: it counts a round ONLY when that round
//! (re)resolved a dynamic callsite and could thus grow the resolved-edge set. Static
//! reachability growth does NOT consume the round budget — it runs to completion bounded
//! solely by `max_worklist_steps` — so a deep first-party STATIC call chain whose depth
//! exceeds `max_rta_rounds` still converges and resolves its dispatch (FIX 1). Counting
//! static-BFS levels instead made `round` equal the static call-graph depth, silently
//! truncating reachability (and latching a FALSE BudgetExceeded) for any function deeper
//! than the cap. Termination does not depend on the round cap: `reachable` is monotonic
//! and bounded by the finite function set; `max_worklist_steps` is the real runaway guard.
//! A cap latches the run-level [`BudgetStatus::BudgetExceeded`] only when it actually
//! prevents pending work — the round cap latches BudgetExceeded only if the cap-time
//! frontier still has a static-call target or a dynamic callsite to resolve; a non-empty
//! frontier whose members have neither is already converged, so hitting the cap there is
//! honest convergence, not truncation (FINDING 6). Edges resolved before a cap keep their
//! honest status (review finding #R1); the loop never runs unbounded.
//!
//! The frontier-only scan is what keeps the worklist-step count proportional to real
//! work (≈ Σ reachable_callers × their_callsites) instead of growing super-linearly
//! with the round count: a full per-round re-scan of the whole reachable set inflated
//! the step counter so a modest multi-round graph could exceed a 64-step cap and drop
//! real edges (review CR-01). Because the instantiated / address-taken sets are seeded
//! once and do NOT grow during the fixpoint, resolving each caller exactly once yields
//! the same edge set a full per-round re-scan would.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::solver::budget::{BudgetReason, BudgetStatus, SolverBudget};
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

    // Reverse index `SemanticNodeId -> qualified` so a resolved callee node maps back
    // to its identity in O(1), built once (no per-edge linear scan of the function
    // index). Deterministic: the source map is BTree-keyed.
    let node_to_qualified: BTreeMap<crate::analysis::ids::SemanticNodeId, &str> = inputs
        .function_node
        .iter()
        .map(|(qualified, &node)| (node, qualified.as_str()))
        .collect();

    // Seed the reachable set from roots (D-07). The instantiated + address-taken sets
    // are the whole reachable program's rapid-type / address-taken sets (see module
    // docs); they do not grow during the fixpoint, so the loop is bounded by reachable
    // growth alone (plus the explicit caps). The instantiated-type FILTER is baked into
    // `inputs.interface_candidate_index` at build time (FIX 2), so the fixpoint no longer
    // threads the instantiated set into `resolve_callsite` — it consults the index.
    let mut reachable: BTreeSet<String> = inputs.roots.clone();
    let address_taken = &inputs.address_taken;

    // The FRONTIER: callers whose callsites still need resolving THIS round. Round 1's
    // frontier is the roots; each subsequent round's frontier is the callees newly
    // reached in the prior round. A caller is resolved exactly once, so the step count
    // tracks real work rather than re-scanning the whole reachable set every round
    // (review CR-01). Deterministic (BTreeSet order).
    let mut frontier: BTreeSet<String> = inputs.roots.clone();

    // Deduplicated edge accumulator keyed by stable key (the store rejects duplicate
    // stable keys; the same callee resolved in different rounds yields an identical
    // stable key, so we keep the first occurrence — its `solver_step` is the earliest,
    // a stable monotonic witness).
    let mut edges_by_key: BTreeMap<String, DerivedEdgeFact> = BTreeMap::new();

    let mut budget_exceeded = false;
    let mut budget_reasons = BTreeSet::new();
    // GLOBAL monotonic step counter (R3): increments once per callsite resolution
    // across the WHOLE run and is never reset, so every derived edge's `solver_step`
    // is globally monotonic.
    let mut solver_step: u64 = 0;
    // DISPATCH-round counter (FIX 1): the round cap bounds GENUINE dynamic-dispatch
    // re-iteration, NOT static-call-graph DEPTH. Only a round that actually (re)resolved
    // a dynamic callsite — and could therefore grow the resolved-edge set — counts toward
    // `max_rta_rounds`. Pure static-reachability growth does NOT consume the round budget;
    // it runs to completion bounded solely by `max_worklist_steps`. Counting static-BFS
    // levels here (the prior behavior) made `round` equal the static call-graph depth, so
    // any first-party function whose shortest static-call path from a root exceeded the cap
    // had its dispatch silently dropped AND the run falsely latched `BudgetExceeded`.
    // Termination does not depend on this cap: `reachable` grows monotonically and is
    // bounded by the finite function set, and every static-edge expansion + callsite
    // resolution consumes one `max_worklist_steps` step (the real runaway guard).
    let mut dispatch_rounds: usize = 0;

    // Address-taken threshold (D-10/D-13, FINDING 7): a pathologically large func-value
    // surface disables FUNC-VALUE (signature) resolution rather than exploding the
    // candidate search — but it is SCOPED to func-value resolution only. Pure interface
    // dispatch never consults the address-taken set, so it must STILL proceed; aborting
    // the whole fixpoint here would drop real interface-invoke edges that have nothing to
    // do with the func-value surface. The set is seeded once and never grows, so this is
    // loop-invariant (review IN-03): evaluate once.
    let resolve_func_values = address_taken.len() <= budget.go.address_taken_threshold;
    // Latch BudgetExceeded for disabled func-value resolution ONLY when the fixpoint reaches
    // a signature-bearing callsite and therefore actually skips work. An unreachable
    // func-value callsite must not taint the run-level budget status.

    // A frontier member carries PENDING WORK only if it has a static-call target or a
    // dynamic callsite to resolve. A non-empty frontier whose members have neither is
    // already CONVERGED — processing it would resolve nothing (FINDING 6).
    let frontier_has_pending_work = |frontier: &BTreeSet<String>| -> bool {
        frontier.iter().any(|caller| {
            inputs.static_call_targets.contains_key(caller.as_str())
                || callsites_by_caller.contains_key(caller.as_str())
        })
    };

    while !frontier.is_empty() {
        // Round cap (D-13, FIX 1): bound only GENUINE dynamic-dispatch re-iteration —
        // `dispatch_rounds` counts only rounds that actually resolved a dynamic callsite,
        // never static-BFS levels. Latch BudgetExceeded ONLY when the cap actually prevents
        // genuinely pending work (FINDING 6): a non-empty frontier whose members have no
        // static target and no dynamic callsite is converged, so hitting the cap there is
        // honest convergence, not truncation — break WithinBudget rather than a false
        // BudgetExceeded. A genuine dispatch fixpoint that re-iterates past the cap (e.g.
        // the runaway test) still latches honestly here; a genuine work explosion latches
        // via `max_worklist_steps`.
        if dispatch_rounds >= budget.go.max_rta_rounds {
            if frontier_has_pending_work(&frontier) {
                budget_exceeded = true;
                budget_reasons.insert(BudgetReason::GoMaxRtaRounds.as_str().to_string());
            }
            break;
        }

        // Resolve only the callsites of THIS round's frontier (the callers newly
        // reached in the prior round, or the roots in round 1). Collect callees that
        // become newly reachable — they are the next round's frontier. Track whether this
        // round resolved any dynamic callsite, so only genuine dispatch waves count toward
        // the round cap (FIX 1).
        let mut newly_reachable: BTreeSet<String> = BTreeSet::new();
        let mut resolved_dispatch_this_round = false;
        for caller in &frontier {
            // Static-call edges GROW reachability (FINDING 1): a function reached only via
            // a direct (static) call that is not itself a root must still enter the
            // worklist so dispatch inside it is resolved (standard RTA = closure over
            // static-call ⊗ dynamic-dispatch edges from roots). Static edges do NOT emit
            // a DerivedEdgeFact — only dynamic-dispatch resolution does. Each static-edge
            // expansion consumes one worklist step too (CR-01: caps must account for deep
            // static chains), bounded by `max_worklist_steps`.
            if let Some(static_callees) = inputs.static_call_targets.get(caller.as_str()) {
                for callee in static_callees {
                    solver_step += 1;
                    if solver_step > budget.go.max_worklist_steps as u64 {
                        budget_reasons
                            .insert(BudgetReason::GoMaxWorklistSteps.as_str().to_string());
                        return finish(edges_by_key, true, budget_reasons);
                    }
                    if !reachable.contains(callee) {
                        newly_reachable.insert(callee.clone());
                    }
                }
            }

            let Some(callsite_indices) = callsites_by_caller.get(caller.as_str()) else {
                continue;
            };
            // This caller has at least one dynamic callsite: the round performs genuine
            // dispatch resolution, so it counts toward the round cap (FIX 1).
            resolved_dispatch_this_round = true;
            for &index in callsite_indices {
                // Worklist-step cap (D-13, CR-01): one callsite resolution is one step,
                // bounded by the Go-scaled `max_worklist_steps` (10_000), NOT the
                // policy-count `max_outer_iterations` (64).
                solver_step += 1;
                if solver_step > budget.go.max_worklist_steps as u64 {
                    // Edges resolved before the cap keep their honest status (R1).
                    budget_reasons.insert(BudgetReason::GoMaxWorklistSteps.as_str().to_string());
                    return finish(edges_by_key, true, budget_reasons);
                }

                let callsite = &inputs.callsites[index];
                if !resolve_func_values
                    && callsite.interface_method.is_none()
                    && callsite.signature.is_some()
                {
                    budget_exceeded = true;
                    budget_reasons
                        .insert(BudgetReason::GoAddressTakenThreshold.as_str().to_string());
                }
                let resolution = resolve_callsite(
                    callsite,
                    inputs,
                    address_taken,
                    budget.go.max_candidates_per_callsite,
                    resolve_func_values,
                    solver_step,
                );
                if resolution.candidate_cap_exceeded {
                    budget_exceeded = true;
                    budget_reasons.insert(
                        BudgetReason::GoMaxCandidatesPerCallsite
                            .as_str()
                            .to_string(),
                    );
                }
                for edge in resolution.edges {
                    // A newly-resolved callee becomes reachable, so its own callsites
                    // are resolved next round (the fixpoint). Map the target node back
                    // to a qualified identity via the prebuilt reverse index.
                    if let Some(&callee) = node_to_qualified.get(&edge.target)
                        && !reachable.contains(callee)
                    {
                        newly_reachable.insert(callee.to_string());
                    }
                    edges_by_key.entry(edge.stable_key.clone()).or_insert(edge);
                }
            }
        }

        // Only a round that genuinely resolved dynamic dispatch counts toward the round
        // cap (FIX 1); a pure static-BFS round leaves `dispatch_rounds` untouched so deep
        // static chains do not consume the dispatch budget.
        if resolved_dispatch_this_round {
            dispatch_rounds += 1;
        }

        // The callees newly reached this round are the next round's frontier; fold them
        // into the reachable set so they are never re-resolved.
        reachable.extend(newly_reachable.iter().cloned());
        frontier = newly_reachable;
    }

    finish(edges_by_key, budget_exceeded, budget_reasons)
}

/// Assemble the normalized output from the deduplicated edge accumulator + the
/// run-level budget status.
fn finish(
    edges_by_key: BTreeMap<String, DerivedEdgeFact>,
    budget_exceeded: bool,
    budget_reasons: BTreeSet<String>,
) -> SolverOutput {
    let budget_status = if budget_exceeded {
        BudgetStatus::BudgetExceeded
    } else {
        BudgetStatus::WithinBudget
    };
    SolverOutput {
        derived_edges: edges_by_key.into_values().collect(),
        budget_status,
        budget_reasons,
    }
    .normalized()
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
        // FIX 2: derive the interface-dispatch index from the hand-built primary fields so
        // dispatch resolution (which now reads the index) behaves as the scan would have.
        .finalize_indexes()
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
    fn static_call_edge_brings_non_root_caller_into_worklist_resolving_its_dispatch() {
        // FINDING 1: `main` (the only root) makes a DIRECT (static) call to `run`, a
        // non-root helper. The interface invoke `s.Speak()` lives in `run`, not in main.
        // RTA reachability must close over the static main -> run edge so run enters the
        // worklist and its dispatch resolves to (pkg.File).Read. Without static-edge
        // growth, run is never scanned and the edge is silently dropped.
        let mut function_node = BTreeMap::new();
        function_node.insert("main.main".to_string(), SemanticNodeId(1));
        function_node.insert("main.run".to_string(), SemanticNodeId(7));
        function_node.insert("(pkg.File).Read".to_string(), SemanticNodeId(3));

        let mut method_sets = BTreeMap::new();
        method_sets.insert("pkg.File".to_string(), BTreeSet::from(["Read".to_string()]));
        let mut method_set_keys = BTreeMap::new();
        method_set_keys.insert("pkg.File".to_string(), "ms|pkg.File".to_string());

        let mut methods_by_receiver = BTreeMap::new();
        methods_by_receiver.insert(
            "pkg.File".to_string(),
            vec![GoRtaMethod {
                method_name: "Read".to_string(),
                qualified: "(pkg.File).Read".to_string(),
                node: SemanticNodeId(3),
            }],
        );

        let instantiated = BTreeSet::from(["pkg.File".to_string()]);
        let mut instantiated_keys = BTreeMap::new();
        instantiated_keys.insert("pkg.File".to_string(), "inst|pkg.File".to_string());

        // The dispatch obligation belongs to `run` (NOT main); main only statically calls run.
        let mut static_call_targets = BTreeMap::new();
        static_call_targets.insert(
            "main.main".to_string(),
            BTreeSet::from(["main.run".to_string()]),
        );

        let inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            callsites: vec![GoRtaCallsite {
                caller: "main.run".to_string(),
                callsite_node: SemanticNodeId(2),
                callsite_stable_key: "cs|run:read".to_string(),
                interface_method: Some("Read".to_string()),
                signature: None,
                dispatch_stable_key: "dd|run:read".to_string(),
            }],
            method_sets,
            method_set_keys,
            instantiated,
            instantiated_keys,
            function_node,
            methods_by_receiver,
            static_call_targets,
            ..GoRtaInputs::default()
        }
        .finalize_indexes();

        let output = solve_go_rta(&inputs, &SolverBudget::default());
        assert_eq!(output.budget_status, BudgetStatus::WithinBudget);
        // run(7) -> File.Read(3): the dispatch in the statically-reached helper resolves.
        assert!(
            output
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId(7) && e.target == SemanticNodeId(3)),
            "dispatch inside a statically-reached non-root function must resolve: {:#?}",
            output.derived_edges
        );
        // Static edges emit NO derived edge themselves: there is no main(1) -> run(7) edge.
        assert!(
            !output
                .derived_edges
                .iter()
                .any(|e| e.target == SemanticNodeId(7)),
            "static-call edges grow reachability only; they must not emit a derived edge"
        );
    }

    #[test]
    fn deep_static_chain_with_bottom_dispatch_converges_at_default_budget() {
        // FIX 1 (HIGH): the round cap must bound only GENUINE dispatch re-iteration, NOT
        // static-call-graph DEPTH. A deep first-party STATIC chain
        // main -> s1 -> s2 -> ... -> s200 whose BOTTOM function does a dynamic dispatch
        // MUST converge WithinBudget AT THE DEFAULT budget (no inflated round cap) and
        // resolve the deep dispatch edge. Static-reachability growth runs to completion
        // bounded solely by `max_worklist_steps`; the round cap counts only the dispatch
        // waves (here exactly one), so the 200-deep static chain never trips the default
        // `max_rta_rounds = 32`. Before the fix, `round` incremented once per static hop,
        // so depth 200 hit the cap at hop 32, latched a FALSE BudgetExceeded, AND silently
        // dropped the deep dispatch — the original FINDING-1 silent-drop bug above depth 32.
        const CHAIN: usize = 200;
        let qualified = |i: usize| -> String {
            if i == 0 {
                "main.main".to_string()
            } else {
                format!("pkg.s{i}")
            }
        };

        // Node ids: chain function i -> SemanticNodeId(i+1); the dispatch target gets a
        // distinct high id so it cannot collide with a chain node.
        const TARGET_NODE: u64 = 100_000;
        let mut function_node = BTreeMap::new();
        let mut static_call_targets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for i in 0..=CHAIN {
            function_node.insert(qualified(i), SemanticNodeId((i + 1) as u64));
        }
        for i in 0..CHAIN {
            static_call_targets
                .entry(qualified(i))
                .or_default()
                .insert(qualified(i + 1));
        }
        // The bottom of the chain (s200) is a method-bearing function whose body holds a
        // dynamic interface invoke of `Read`, resolving to the instantiated (pkg.File).Read.
        function_node.insert("(pkg.File).Read".to_string(), SemanticNodeId(TARGET_NODE));

        let mut method_sets = BTreeMap::new();
        method_sets.insert("pkg.File".to_string(), BTreeSet::from(["Read".to_string()]));
        let mut method_set_keys = BTreeMap::new();
        method_set_keys.insert("pkg.File".to_string(), "ms|pkg.File".to_string());

        let mut methods_by_receiver = BTreeMap::new();
        methods_by_receiver.insert(
            "pkg.File".to_string(),
            vec![GoRtaMethod {
                method_name: "Read".to_string(),
                qualified: "(pkg.File).Read".to_string(),
                node: SemanticNodeId(TARGET_NODE),
            }],
        );

        let instantiated = BTreeSet::from(["pkg.File".to_string()]);
        let mut instantiated_keys = BTreeMap::new();
        instantiated_keys.insert("pkg.File".to_string(), "inst|pkg.File".to_string());

        let inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            // The dynamic dispatch lives in the DEEPEST statically-reached function.
            callsites: vec![GoRtaCallsite {
                caller: qualified(CHAIN),
                callsite_node: SemanticNodeId(900_000),
                callsite_stable_key: "cs|deep:read".to_string(),
                interface_method: Some("Read".to_string()),
                signature: None,
                dispatch_stable_key: "dd|deep:read".to_string(),
            }],
            method_sets,
            method_set_keys,
            instantiated,
            instantiated_keys,
            methods_by_receiver,
            function_node,
            static_call_targets,
            ..GoRtaInputs::default()
        }
        .finalize_indexes();

        // THE DEFAULT BUDGET — no inflated `max_rta_rounds`. The default `max_rta_rounds`
        // (32) is far below the chain depth (200); the fix is what lets this converge.
        let output = solve_go_rta(&inputs, &SolverBudget::default());

        // (a) The deep dispatch edge resolves: s200 -> (pkg.File).Read.
        assert!(
            output
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId((CHAIN + 1) as u64)
                    && e.target == SemanticNodeId(TARGET_NODE)),
            "the dispatch at the bottom of a deep static chain must resolve at the DEFAULT \
             budget: {:#?}",
            output.derived_edges
        );
        // (b) The run converged honestly — a deep first-party static chain is NOT a runaway.
        assert_eq!(
            output.budget_status,
            BudgetStatus::WithinBudget,
            "a deep finite static chain (depth {CHAIN} >> default max_rta_rounds) must \
             converge WithinBudget at the DEFAULT budget, not latch exhaustion"
        );
    }

    #[test]
    fn pure_deep_static_chain_converges_at_default_budget() {
        // FIX 1 companion: a deep-but-finite STATIC chain with NO dispatch anywhere must
        // also converge WithinBudget AT THE DEFAULT budget. Static-edge expansion consumes
        // the worklist-step budget (10_000, comfortably admitting the chain) and grows no
        // dispatch waves, so the round cap is never approached. No dynamic callsites → no
        // derived edges.
        const CHAIN: usize = 200;
        let qualified = |i: usize| -> String {
            if i == 0 {
                "main.main".to_string()
            } else {
                format!("pkg.s{i}")
            }
        };

        let mut function_node = BTreeMap::new();
        let mut static_call_targets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for i in 0..=CHAIN {
            function_node.insert(qualified(i), SemanticNodeId((i + 1) as u64));
        }
        for i in 0..CHAIN {
            static_call_targets
                .entry(qualified(i))
                .or_default()
                .insert(qualified(i + 1));
        }

        let inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            function_node,
            static_call_targets,
            ..GoRtaInputs::default()
        }
        .finalize_indexes();

        let output = solve_go_rta(&inputs, &SolverBudget::default());
        assert_eq!(
            output.budget_status,
            BudgetStatus::WithinBudget,
            "a deep finite static chain must converge WithinBudget at the DEFAULT budget"
        );
        // No dynamic callsites anywhere, so no edges are derived (static edges emit none).
        assert!(output.derived_edges.is_empty());
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
        }
        .finalize_indexes();

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
        // invalidates the edge. Mutating a primary field requires rebuilding the derived
        // dispatch index (FIX 2) — exactly what re-running `from_db` would do.
        let mut no_method_set = interface_scenario(&["pkg.File"]);
        no_method_set
            .method_sets
            .get_mut("pkg.File")
            .unwrap()
            .remove("Read");
        let no_method_set = no_method_set.finalize_indexes();
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
        }
        .finalize_indexes();

        let mut budget = SolverBudget::default();
        budget.go.max_rta_rounds = 1;
        let output = solve_go_rta(&inputs, &budget);
        // The cap is hit before the second round resolves A.Step's callsite, so the
        // run latches BudgetExceeded honestly rather than looping.
        assert_eq!(output.budget_status, BudgetStatus::BudgetExceeded);
    }

    #[test]
    fn address_taken_threshold_suppresses_only_func_value_not_interface_dispatch() {
        // FINDING 7: a pathologically large address-taken (func-value) surface must
        // disable only FUNC-VALUE (signature) resolution; pure INTERFACE dispatch (which
        // does not consult the address-taken set) must still resolve. `main` has an
        // interface invoke of `Read` (resolves to the instantiated (pkg.File).Read) AND a
        // func-value call of `func()` (would resolve to the address-taken handlers). With
        // address_taken_threshold = 1 and TWO address-taken handlers, func-value resolution
        // is suppressed but the interface edge must still be derived; the run latches
        // BudgetExceeded honestly (the func-value surface was too large to resolve).
        let mut function_node = BTreeMap::new();
        function_node.insert("main.main".to_string(), SemanticNodeId(1));
        function_node.insert("(pkg.File).Read".to_string(), SemanticNodeId(3));
        function_node.insert("pkg.h1".to_string(), SemanticNodeId(5));
        function_node.insert("pkg.h2".to_string(), SemanticNodeId(6));

        let mut method_sets = BTreeMap::new();
        method_sets.insert("pkg.File".to_string(), BTreeSet::from(["Read".to_string()]));
        let mut method_set_keys = BTreeMap::new();
        method_set_keys.insert("pkg.File".to_string(), "ms|pkg.File".to_string());

        let mut methods_by_receiver = BTreeMap::new();
        methods_by_receiver.insert(
            "pkg.File".to_string(),
            vec![GoRtaMethod {
                method_name: "Read".to_string(),
                qualified: "(pkg.File).Read".to_string(),
                node: SemanticNodeId(3),
            }],
        );

        let instantiated = BTreeSet::from(["pkg.File".to_string()]);
        let mut instantiated_keys = BTreeMap::new();
        instantiated_keys.insert("pkg.File".to_string(), "inst|pkg.File".to_string());

        // Two address-taken func() handlers; threshold = 1, so the func-value surface is
        // "too large" and func-value resolution is suppressed.
        let address_taken = BTreeSet::from(["pkg.h1".to_string(), "pkg.h2".to_string()]);
        let mut address_taken_keys = BTreeMap::new();
        address_taken_keys.insert("pkg.h1".to_string(), "at|h1".to_string());
        address_taken_keys.insert("pkg.h2".to_string(), "at|h2".to_string());
        let mut function_signature = BTreeMap::new();
        function_signature.insert("pkg.h1".to_string(), "func()".to_string());
        function_signature.insert("pkg.h2".to_string(), "func()".to_string());

        let inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            callsites: vec![
                // Interface invoke of Read (must still resolve).
                GoRtaCallsite {
                    caller: "main.main".to_string(),
                    callsite_node: SemanticNodeId(2),
                    callsite_stable_key: "cs|main:read".to_string(),
                    interface_method: Some("Read".to_string()),
                    signature: None,
                    dispatch_stable_key: "dd|main:read".to_string(),
                },
                // Func-value call of func() (must be suppressed under the threshold).
                GoRtaCallsite {
                    caller: "main.main".to_string(),
                    callsite_node: SemanticNodeId(4),
                    callsite_stable_key: "cs|main:fv".to_string(),
                    interface_method: None,
                    signature: Some("func()".to_string()),
                    dispatch_stable_key: "dd|main:fv".to_string(),
                },
            ],
            method_sets,
            method_set_keys,
            instantiated,
            instantiated_keys,
            address_taken,
            address_taken_keys,
            function_node,
            function_signature,
            methods_by_receiver,
            ..GoRtaInputs::default()
        }
        .finalize_indexes();

        let mut budget = SolverBudget::default();
        budget.go.address_taken_threshold = 1;
        let output = solve_go_rta(&inputs, &budget);

        // Interface dispatch still resolves despite the large address-taken set.
        assert!(
            output
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3)),
            "interface dispatch must resolve even when the address-taken surface exceeds the threshold: {:#?}",
            output.derived_edges
        );
        // Func-value resolution is suppressed: no edge targets an address-taken handler.
        assert!(
            !output
                .derived_edges
                .iter()
                .any(|e| e.target == SemanticNodeId(5) || e.target == SemanticNodeId(6)),
            "func-value resolution must be suppressed under the address-taken threshold"
        );
        // The suppression is signalled honestly.
        assert_eq!(output.budget_status, BudgetStatus::BudgetExceeded);
    }

    #[test]
    fn large_address_taken_with_no_func_value_callsite_stays_within_budget() {
        // FIX 2 (MEDIUM): an oversized address-taken (func-value) surface must latch
        // BudgetExceeded ONLY when func-value resolution is actually NEEDED — i.e. at
        // least one reachable callsite is a func-value (signature-bearing) callsite.
        // Here the only callsite is an INTERFACE invoke (no signature), so disabling
        // func-value resolution costs nothing: interface dispatch still resolves and the
        // run must stay WithinBudget. Before the fix, the threshold latched BudgetExceeded
        // unconditionally, producing a FALSE exhaustion signal (folded into
        // solver_output_digest / WR-06) on a codebase that has many address-taken
        // functions but no indirect func-value calls.
        let mut inputs = interface_scenario(&["pkg.File"]);
        // A large address-taken surface (3 funcs) with threshold 1: func-value resolution
        // is "disabled", but there is NO func-value callsite to resolve. The scenario's
        // sole callsite (main -> Read) is a pure interface invoke (signature: None).
        inputs.address_taken = BTreeSet::from([
            "pkg.h1".to_string(),
            "pkg.h2".to_string(),
            "pkg.h3".to_string(),
        ]);
        inputs.address_taken_keys = BTreeMap::from([
            ("pkg.h1".to_string(), "at|h1".to_string()),
            ("pkg.h2".to_string(), "at|h2".to_string()),
            ("pkg.h3".to_string(), "at|h3".to_string()),
        ]);
        debug_assert!(inputs.callsites.iter().all(|c| c.signature.is_none()));

        let mut budget = SolverBudget::default();
        budget.go.address_taken_threshold = 1;
        let output = solve_go_rta(&inputs, &budget);

        // Interface dispatch still resolves: main(1) -> (pkg.File).Read(3).
        assert!(
            output
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3)),
            "interface dispatch must still resolve: {:#?}",
            output.derived_edges
        );
        // No func-value callsite exists, so disabling func-value resolution cost nothing —
        // the run is WithinBudget, NOT a false BudgetExceeded.
        assert_eq!(
            output.budget_status,
            BudgetStatus::WithinBudget,
            "a large address-taken surface with NO func-value callsite must NOT latch \
             BudgetExceeded (func-value resolution was never needed)"
        );
    }

    #[test]
    fn large_address_taken_with_only_unreachable_func_value_callsite_stays_within_budget() {
        // A signature-bearing func-value callsite that is NOT reachable from any RTA root
        // must not latch BudgetExceeded. The threshold signal is about skipped reachable
        // work, not dormant obligations in disconnected functions.
        let mut inputs = interface_scenario(&["pkg.File"]);
        inputs
            .function_node
            .insert("pkg.unreachable".to_string(), SemanticNodeId(50));
        inputs
            .function_node
            .insert("pkg.h1".to_string(), SemanticNodeId(51));
        inputs
            .function_node
            .insert("pkg.h2".to_string(), SemanticNodeId(52));
        inputs
            .function_signature
            .insert("pkg.h1".to_string(), "func()".to_string());
        inputs
            .function_signature
            .insert("pkg.h2".to_string(), "func()".to_string());
        inputs.address_taken = BTreeSet::from(["pkg.h1".to_string(), "pkg.h2".to_string()]);
        inputs.address_taken_keys = BTreeMap::from([
            ("pkg.h1".to_string(), "at|h1".to_string()),
            ("pkg.h2".to_string(), "at|h2".to_string()),
        ]);
        inputs.callsites.push(GoRtaCallsite {
            caller: "pkg.unreachable".to_string(),
            callsite_node: SemanticNodeId(53),
            callsite_stable_key: "cs|unreachable:fv".to_string(),
            interface_method: None,
            signature: Some("func()".to_string()),
            dispatch_stable_key: "dd|unreachable:fv".to_string(),
        });

        let mut budget = SolverBudget::default();
        budget.go.address_taken_threshold = 1;
        let output = solve_go_rta(&inputs.finalize_indexes(), &budget);

        assert!(
            output
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3)),
            "reachable interface dispatch must still resolve: {:#?}",
            output.derived_edges
        );
        assert!(
            !output
                .derived_edges
                .iter()
                .any(|e| e.target == SemanticNodeId(51) || e.target == SemanticNodeId(52)),
            "the unreachable func-value callsite must not resolve"
        );
        assert_eq!(
            output.budget_status,
            BudgetStatus::WithinBudget,
            "an unreachable func-value callsite must not produce a false BudgetExceeded"
        );
    }

    #[test]
    fn convergence_at_round_boundary_with_no_pending_work_stays_within_budget() {
        // FINDING 6: at the `max_rta_rounds` boundary the cap must latch BudgetExceeded
        // ONLY when there is genuinely pending dispatch work. Here `main` (root) invokes
        // `Step`, resolving to (pkg.A).Step in round 1; (pkg.A).Step has NO callsites, so
        // the round-2 frontier {A.Step} has nothing left to resolve — the analysis has
        // CONVERGED. With max_rta_rounds = 1 the old code latched BudgetExceeded purely
        // because the frontier was non-empty, a false positive. (Contrast the runaway test
        // where A.Step DOES invoke B.Step — genuine pending work that still latches.)
        let mut function_node = BTreeMap::new();
        function_node.insert("main.main".to_string(), SemanticNodeId(1));
        function_node.insert("(pkg.A).Step".to_string(), SemanticNodeId(3));

        let mut method_sets = BTreeMap::new();
        method_sets.insert("pkg.A".to_string(), BTreeSet::from(["Step".to_string()]));
        let mut method_set_keys = BTreeMap::new();
        method_set_keys.insert("pkg.A".to_string(), "ms|A".to_string());

        let mut methods_by_receiver = BTreeMap::new();
        methods_by_receiver.insert(
            "pkg.A".to_string(),
            vec![GoRtaMethod {
                method_name: "Step".to_string(),
                qualified: "(pkg.A).Step".to_string(),
                node: SemanticNodeId(3),
            }],
        );

        let instantiated = BTreeSet::from(["pkg.A".to_string()]);
        let mut instantiated_keys = BTreeMap::new();
        instantiated_keys.insert("pkg.A".to_string(), "inst|A".to_string());

        let inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            // Only main has a callsite; (pkg.A).Step has none → the round-2 frontier is
            // converged with no pending work.
            callsites: vec![GoRtaCallsite {
                caller: "main.main".to_string(),
                callsite_node: SemanticNodeId(2),
                callsite_stable_key: "cs|main".to_string(),
                interface_method: Some("Step".to_string()),
                signature: None,
                dispatch_stable_key: "dd|main".to_string(),
            }],
            method_sets,
            method_set_keys,
            instantiated,
            instantiated_keys,
            methods_by_receiver,
            function_node,
            ..GoRtaInputs::default()
        }
        .finalize_indexes();

        let mut budget = SolverBudget::default();
        budget.go.max_rta_rounds = 1;
        let output = solve_go_rta(&inputs, &budget);
        // The single resolvable edge is derived AND the run is WithinBudget (converged at
        // the boundary, no pending work the cap prevented).
        assert!(
            output
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3)),
            "the resolvable edge must be derived"
        );
        assert_eq!(
            output.budget_status,
            BudgetStatus::WithinBudget,
            "convergence at the round boundary with no pending work must NOT latch BudgetExceeded"
        );
    }

    /// Regression for CR-01: a multi-round RTA convergence whose total dynamic
    /// callsite-visits EXCEED the old policy-count cap (64) but which is NOT runaway
    /// dispatch. Before the fix, the fixpoint capped its per-callsite worklist-step
    /// counter against `max_outer_iterations` (= 64) AND re-scanned the whole reachable
    /// set every round, so this graph spuriously latched `BudgetExceeded` and dropped
    /// real edges. After the fix (Go-scaled `max_worklist_steps` = 10_000 + frontier-
    /// only per-round scan) it must converge `WithinBudget` and resolve EVERY chain
    /// edge.
    ///
    /// Shape: a deep reachable chain `step.0 -> step.1 -> ... -> step.N`. Each `step.i`
    /// is a method on a distinct instantiated type `pkg.S{i}`; it has one interface
    /// invoke of the uniquely-named method `M{i+1}`, which resolves ONLY to `step.{i+1}`
    /// (on `pkg.S{i+1}`). Resolving the whole chain therefore needs N rounds and N
    /// callsite-visits. With N = 80 (> 64) the old step cap would trip mid-chain.
    #[test]
    fn deep_multi_round_chain_exceeding_64_visits_stays_within_budget() {
        const CHAIN: usize = 80;

        let mut function_node = BTreeMap::new();
        let mut method_sets = BTreeMap::new();
        let mut method_set_keys = BTreeMap::new();
        let mut methods_by_receiver = BTreeMap::new();
        let mut instantiated = BTreeSet::new();
        let mut instantiated_keys = BTreeMap::new();
        let mut callsites = Vec::new();

        // step.0 is the root (a plain function, the entry); step.1..=CHAIN are methods
        // on instantiated types pkg.S1..pkg.S{CHAIN}. Node ids: step.i -> SemanticNodeId(i+1).
        let qualified = |i: usize| -> String {
            if i == 0 {
                "main.main".to_string()
            } else {
                format!("(pkg.S{i}).M{i}")
            }
        };
        for i in 0..=CHAIN {
            function_node.insert(qualified(i), SemanticNodeId((i + 1) as u64));
        }
        // Each instantiated type pkg.S{i} declares its method M{i} and is in the
        // instantiated set, so an invoke of M{i} resolves to (pkg.S{i}).M{i}.
        for i in 1..=CHAIN {
            let type_name = format!("pkg.S{i}");
            let method = format!("M{i}");
            method_sets.insert(type_name.clone(), BTreeSet::from([method.clone()]));
            method_set_keys.insert(type_name.clone(), format!("ms|{type_name}"));
            methods_by_receiver.insert(
                type_name.clone(),
                vec![GoRtaMethod {
                    method_name: method,
                    qualified: qualified(i),
                    node: SemanticNodeId((i + 1) as u64),
                }],
            );
            instantiated.insert(type_name.clone());
            instantiated_keys.insert(type_name.clone(), format!("inst|{type_name}"));
        }
        // Caller step.i (for i in 0..CHAIN) invokes M{i+1}, reaching step.{i+1}.
        for i in 0..CHAIN {
            let next = i + 1;
            callsites.push(GoRtaCallsite {
                caller: qualified(i),
                callsite_node: SemanticNodeId((1000 + i) as u64),
                callsite_stable_key: format!("cs|{i}"),
                interface_method: Some(format!("M{next}")),
                signature: None,
                dispatch_stable_key: format!("dd|{i}"),
            });
        }

        let inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            callsites,
            method_sets,
            method_set_keys,
            instantiated,
            instantiated_keys,
            methods_by_receiver,
            function_node,
            ..GoRtaInputs::default()
        }
        .finalize_indexes();

        // Default worklist-step cap (10_000) easily admits the 80 visits; the round cap
        // must be raised above the chain depth so the round cap itself is not what trips
        // (we are pinning the STEP cap's scale, the CR-01 regression). The default
        // worklist-step cap stays untouched, proving 80 visits no longer trip 64.
        let mut budget = SolverBudget::default();
        budget.go.max_rta_rounds = CHAIN + 1;
        assert!(
            CHAIN > budget.max_outer_iterations,
            "the chain must exceed the OLD policy-count cap ({}) to be a real regression",
            budget.max_outer_iterations
        );

        let output = solve_go_rta(&inputs, &budget);

        // The fixpoint converges honestly — NOT a spurious BudgetExceeded.
        assert_eq!(
            output.budget_status,
            BudgetStatus::WithinBudget,
            "an {CHAIN}-visit multi-round convergence must NOT trip the worklist-step cap (CR-01)"
        );
        // Every chain edge step.i -> step.{i+1} is resolved (no edge silently dropped).
        assert_eq!(
            output.derived_edges.len(),
            CHAIN,
            "every chain edge must resolve; a mid-chain cap would drop the tail"
        );
        for i in 0..CHAIN {
            let source = SemanticNodeId((i + 1) as u64);
            let target = SemanticNodeId((i + 2) as u64);
            assert!(
                output
                    .derived_edges
                    .iter()
                    .any(|edge| edge.source == source && edge.target == target),
                "chain edge step.{i} -> step.{} must be resolved",
                i + 1
            );
        }
    }

    /// CR-01 honesty floor: genuine runaway dispatch must STILL latch `BudgetExceeded`
    /// once it exceeds the (Go-scaled) worklist-step cap. A tight `max_worklist_steps`
    /// forces the same chain to truncate mid-resolution, proving the cap still bites
    /// when work genuinely exceeds it (the `iteration-cap` fixture stays green for the
    /// candidate-cap path; this pins the step-cap path).
    #[test]
    fn worklist_step_cap_still_latches_budget_exceeded_when_genuinely_exceeded() {
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
                GoRtaCallsite {
                    caller: "main.main".to_string(),
                    callsite_node: SemanticNodeId(2),
                    callsite_stable_key: "cs|main".to_string(),
                    interface_method: Some("Step".to_string()),
                    signature: None,
                    dispatch_stable_key: "dd|main".to_string(),
                },
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
        }
        .finalize_indexes();

        // A worklist-step cap of 1 admits only the first callsite resolution, so the
        // chain truncates and the run latches BudgetExceeded honestly.
        let mut budget = SolverBudget::default();
        budget.go.max_worklist_steps = 1;
        let output = solve_go_rta(&inputs, &budget);
        assert_eq!(output.budget_status, BudgetStatus::BudgetExceeded);
    }
}
