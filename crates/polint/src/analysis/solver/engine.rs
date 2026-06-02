//! Unified solver engine: deterministic worklist core (D-02, D-11).
//!
//! The engine owns the single deterministic `VecDeque` worklist that drives the
//! registered [`super::policy::SolverPolicy`] impls to a **single fixpoint per
//! run** (D-11), enforces the [`SolverBudget`] ceilings, projects the outcome to
//! [`BudgetStatus`], and maintains a monotonic `u64` step counter (Plan 02 reads
//! this for the provenance solver-step field, D-08). The worklist drain mirrors
//! the proven `points_to::solver` shape (`while let Some(..) = queue.pop_front()`
//! with `if !budget_ok { break; }`) and `BTreeMap`/`BTreeSet`-ordered
//! accumulation, which is what makes the output byte-stable (D-02).
//!
//! The points-to fixpoint is folded in by composition (D-03): the engine drives
//! the [`super::policy::PointsToPolicy`], which invokes the existing
//! `solve_points_to` engine in place. Driving the points-to policy through this
//! engine produces a result byte-identical to calling `solve_points_to`
//! directly.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::analysis::ids::DerivedEdgeId;
use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};

use super::budget::{BudgetStatus, SolverBudget};
use super::facts::DerivedEdgeFact;
use super::policy::{PolicyOutcome, SolverPolicy};
use super::provenance::{ContributingFact, DerivedEdgeProvenance};
use super::store::SolverOutput;

/// Result of a single unified solver run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SolverRunResult {
    /// Per-policy outcomes, in the deterministic order the engine drained them.
    pub(crate) policy_outcomes: Vec<PolicyRunRecord>,
    /// The combined budget outcome across all driven policies (worst-case wins:
    /// any policy exceeding its budget surfaces [`BudgetStatus::BudgetExceeded`]).
    pub(crate) budget_status: BudgetStatus,
    /// Monotonic worklist step counter for this run (provenance solver-step).
    pub(crate) steps: u64,
}

/// One policy's contribution recorded by the engine, tagged by policy id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyRunRecord {
    pub(crate) policy_id: &'static str,
    pub(crate) outcome: PolicyOutcome,
}

/// The unified solver engine. Holds a closed, ordered set of registered policies
/// (the closed input set, D-11) and drives them to convergence with a single
/// deterministic worklist drain.
pub(crate) struct SolverEngine {
    policies: Vec<Box<dyn SolverPolicy>>,
    budget: SolverBudget,
}

impl SolverEngine {
    /// Build an engine over a closed, ordered policy set. Order is the
    /// registration order; the worklist drains it deterministically.
    pub(crate) fn new(policies: Vec<Box<dyn SolverPolicy>>, budget: SolverBudget) -> Self {
        Self { policies, budget }
    }

    /// Drive every registered policy to its single fixpoint, bounded by the
    /// budget, accumulating outcomes in deterministic order (D-02). The monotonic
    /// step counter increments once per worklist step and latches budget
    /// exhaustion honestly (D-06/D-11) rather than looping unbounded.
    pub(crate) fn run(&self) -> SolverRunResult {
        // Deterministic worklist of policy indices in stable registration order.
        // The bounded outer-iteration cap (D-11) guards against ever draining
        // more steps than the budget allows.
        let mut queue: VecDeque<usize> = (0..self.policies.len()).collect();
        let mut steps: u64 = 0;
        let mut budget_exceeded = false;
        let mut records: Vec<PolicyRunRecord> = Vec::with_capacity(self.policies.len());

        while let Some(index) = queue.pop_front() {
            // Bounded outer-iteration cap: each policy drained is one worklist
            // step; exceeding the cap latches exhaustion instead of looping.
            steps += 1;
            if steps > self.budget.max_outer_iterations as u64 {
                budget_exceeded = true;
                break;
            }

            let policy = &self.policies[index];
            let outcome = policy.solve(&self.budget);
            if outcome.budget_status == BudgetStatus::BudgetExceeded {
                budget_exceeded = true;
            }
            // Fold the policy's own internal step count into the run counter so
            // provenance (Plan 02) sees the full worklist progress.
            steps = steps.saturating_add(outcome.steps);
            records.push(PolicyRunRecord {
                policy_id: policy.id(),
                outcome,
            });
        }

        let budget_status = if budget_exceeded {
            BudgetStatus::BudgetExceeded
        } else if records.is_empty() {
            BudgetStatus::NotRun
        } else {
            BudgetStatus::WithinBudget
        };

        SolverRunResult {
            policy_outcomes: records,
            budget_status,
            steps,
        }
    }
}

/// Derive transitive copy edges over the unified `ConstraintKind::CopyEdge`
/// vocabulary, attaching full [`DerivedEdgeProvenance`] (GRAPH-04, D-08) to each
/// derived edge.
///
/// A `CopyEdge { dst, src }` constraint is a primitive value-flow obligation
/// `src -> dst`. The unified solver derives the TRANSITIVE closure: if `a -> b` and
/// `b -> c` are primitive copy edges, the solver derives `a -> c`. Each derived edge
/// records, as its provenance, the totally-ordered set of contributing primitive
/// `CopyEdge` constraints (referenced by their EXISTING stable keys) whose
/// composition produced it, the producing `ConstraintKind` (`copy_edge`), and the
/// `solver_step` at which it was emitted.
///
/// Provenance records a DETERMINISTIC WITNESSING derivation (D-09): each derived
/// `source -> target` edge carries the contributing primitive `CopyEdge` constraints
/// on a single deterministic witness path (the BFS-shortest path, ties broken by the
/// `BTreeMap`/`BTreeSet` stable-key order), plus every constraint that justifies a hop
/// on that path (duplicate justifications for one hop are all recorded — review
/// finding #10). The edge fact's `stable_key` embeds that witness, so deleting any
/// witness fact means THIS edge fact (by stable key) is not reproduced. On a
/// multi-path graph the underlying `source -> target` value-flow may persist via an
/// alternate path under a DIFFERENT edge identity — provenance describes a witness,
/// not a global dependency of the (source, target) pair. Dense IDs are assigned only
/// after the stable-key sort in [`SolverOutput::normalized`], so the output is
/// byte-stable (D-02).
///
/// Honesty (D-06): the derived edge inherits the WEAKEST status/precision across
/// EVERY discovered path to the target — not just its witness path — so an untrusted
/// upstream hop on any derivation conservatively downgrades the edge and is never
/// laundered into a confident one (review finding #4; `keys`/`step` stay the
/// deterministic witness, `status`/`precision` are the conservative join). The
/// worklist cap (`budget.max_steps`) is applied PER SOURCE; exhausting it on one
/// source sets the run-level
/// [`crate::analysis::solver::budget::BudgetStatus::BudgetExceeded`] (surfaced as a
/// provider diagnostic) WITHOUT silently dropping the other, independent sources
/// (review finding #3). Edges fully derived before a source's cap was hit keep their
/// honest status — exhaustion costs the edges never reached, not the ones already
/// derived, so a complete edge is never spuriously downgraded (review finding #R1).
/// `solver_step` is a GLOBAL monotonic counter, independent of the per-source budget
/// counter (review finding #R3). Derived edges never claim exact precision (D-06).
pub(crate) fn derive_edges(constraints: &[ConstraintFact], budget: &SolverBudget) -> SolverOutput {
    // Primitive copy adjacency `src -> {dst}`. For each hop `src -> dst` accumulate
    // (a) the UNION of contributing constraint stable keys (#10 — never drop a
    // justifying identity) and (b) the WEAKEST status/precision across the
    // constraints asserting it (#4 — never launder an untrusted hop). BTree-ordered
    // for determinism.
    let mut adjacency: BTreeMap<SemanticNodeRef, BTreeSet<SemanticNodeRef>> = BTreeMap::new();
    let mut hop_keys: BTreeMap<(SemanticNodeRef, SemanticNodeRef), BTreeSet<String>> =
        BTreeMap::new();
    let mut hop_meta: BTreeMap<
        (SemanticNodeRef, SemanticNodeRef),
        (PointsToStatus, PointsToPrecision),
    > = BTreeMap::new();
    for constraint in constraints {
        if let ConstraintKind::CopyEdge { dst, src } = &constraint.kind {
            let (src, dst) = (src.0, dst.0);
            adjacency.entry(src).or_default().insert(dst);
            hop_keys
                .entry((src, dst))
                .or_default()
                .insert(constraint.stable_key.clone());
            let meta = hop_meta
                .entry((src, dst))
                .or_insert((PointsToStatus::Present, PointsToPrecision::FlowInsensitive));
            meta.0 = weakest_status(meta.0, constraint.status);
            meta.1 = weakest_precision(meta.1, constraint.precision);
        }
    }

    let mut edges: Vec<DerivedEdgeFact> = Vec::new();
    let mut run_budget_exceeded = false;
    // GLOBAL monotonic derivation step (R3): increments once per worklist pop across
    // the WHOLE run and is never reset, so every derived edge's `solver_step` is
    // globally monotonic and the run's progress is totally ordered. The PER-SOURCE
    // budget counter below is separate, so the per-source budget (#3) does not
    // perturb this contract.
    let mut solver_step: u64 = 0;

    let sources: Vec<SemanticNodeRef> = adjacency.keys().copied().collect();
    for start in sources {
        // PER-SOURCE budget (#3): reset the budget counter for each source so one
        // large source cannot starve the others. Exhaustion is recorded run-level,
        // never as a silent cross-source drop.
        let mut budget_steps: u64 = 0;
        let mut source_budget_exceeded = false;
        // reached: node -> path metadata. `keys` + `step` are fixed by the first
        // (BFS-shortest) witness path; `status`/`precision` are CONSERVATIVELY weakened
        // over EVERY discovered path to the node (R2), so an untrusted alternate path
        // can never be laundered into a confident edge.
        let mut reached: BTreeMap<SemanticNodeRef, PathMeta> = BTreeMap::new();
        let mut queue: VecDeque<SemanticNodeRef> = VecDeque::new();
        reached.insert(start, PathMeta::seed());
        queue.push_back(start);

        while let Some(node) = queue.pop_front() {
            budget_steps += 1;
            solver_step += 1;
            if budget_steps > budget.max_steps as u64 {
                source_budget_exceeded = true;
                break;
            }
            let path = reached.get(&node).cloned().unwrap_or_else(PathMeta::seed);
            let Some(targets) = adjacency.get(&node) else {
                continue;
            };
            for &next in targets {
                if next == start {
                    // Skip self-loops back to the start (cycle guard).
                    continue;
                }
                let mut next_meta = path.clone();
                if let Some(keys) = hop_keys.get(&(node, next)) {
                    next_meta.keys.extend(keys.iter().cloned());
                }
                if let Some((status, precision)) = hop_meta.get(&(node, next)) {
                    next_meta.status = weakest_status(next_meta.status, *status);
                    next_meta.precision = weakest_precision(next_meta.precision, *precision);
                }
                next_meta.step = solver_step;
                match reached.entry(next) {
                    // First visit: the deterministic BFS-shortest witness path fixes
                    // the contributing keys and the solver step.
                    std::collections::btree_map::Entry::Vacant(slot) => {
                        slot.insert(next_meta);
                        queue.push_back(next);
                    }
                    // Revisit via an alternate path (R2): KEEP the witness keys/step but
                    // CONSERVATIVELY weaken status/precision over this path too. Do not
                    // re-enqueue (witness is fixed); `weakest_*` is commutative, so the
                    // converged value is order-independent (deterministic).
                    std::collections::btree_map::Entry::Occupied(mut slot) => {
                        let current = slot.get_mut();
                        current.status = weakest_status(current.status, next_meta.status);
                        current.precision =
                            weakest_precision(current.precision, next_meta.precision);
                    }
                }
            }
        }

        if source_budget_exceeded {
            run_budget_exceeded = true;
        }

        // Emit a derived edge for every node reachable from `start` in >= 1 hop. Every
        // node in `reached` was traversed via a COMPLETE witness path, so its edge is
        // sound and keeps its honest (conservatively weakened) status/precision even
        // when this source later exhausted the budget — budget exhaustion costs the
        // edges we never reached, which is conveyed by the run-level signal, NOT a
        // downgrade of the edges we did derive (R1). Derived edges are at most
        // flow-insensitive, never exact (D-06).
        for (&node, meta) in &reached {
            if node == start || meta.keys.is_empty() {
                continue;
            }
            let provenance = DerivedEdgeProvenance::new(
                meta.keys.iter().map(|key| ContributingFact {
                    stable_key: key.clone(),
                }),
                &ConstraintKind::CopyEdge {
                    dst: crate::analysis::ids::SemanticNodeId(node),
                    src: crate::analysis::ids::SemanticNodeId(start),
                },
                meta.step,
            );
            let stable_key = stable_key_from_parts(
                FactFamily::SolverDerivedEdge,
                &[
                    ("source", start.to_string()),
                    ("target", node.to_string()),
                    ("provenance", provenance.stable_key_fragment()),
                ],
            );
            edges.push(DerivedEdgeFact {
                id: DerivedEdgeId(0),
                source: crate::analysis::ids::SemanticNodeId(start),
                target: crate::analysis::ids::SemanticNodeId(node),
                status: meta.status,
                precision: meta.precision,
                stable_key,
                provenance,
            });
        }
        // No `break`: every source is processed so budget exhaustion on one source
        // never silently drops the independent edges of another (#3).
    }

    let budget_status = if run_budget_exceeded {
        BudgetStatus::BudgetExceeded
    } else {
        BudgetStatus::WithinBudget
    };

    SolverOutput {
        derived_edges: edges,
        budget_status,
    }
    .normalized()
}

/// Run-local node handle (the `SemanticNodeId.0` value) used as a deterministic
/// `BTreeMap`/`BTreeSet` key during closure derivation.
type SemanticNodeRef = u64;

/// Path metadata accumulated during the per-source BFS. `keys` (the union of
/// contributing constraint stable keys) and `step` (the global monotonic derivation
/// step) are fixed by the FIRST (BFS-shortest) witness path; `status`/`precision` are
/// CONSERVATIVELY weakened over EVERY discovered path to the node (R2). `BTreeSet`
/// keeps the contributing keys totally ordered (byte-stable).
#[derive(Debug, Clone)]
struct PathMeta {
    keys: BTreeSet<String>,
    status: PointsToStatus,
    precision: PointsToPrecision,
    /// The global monotonic `solver_step` at which this node was first reached.
    step: u64,
}

impl PathMeta {
    /// The start node's seed: zero contributing facts, fully trusted, step 0.
    fn seed() -> Self {
        Self {
            keys: BTreeSet::new(),
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            step: 0,
        }
    }
}

/// Worst-of-two derivation status (#4): `Present` is the best/most-trusted; any
/// non-`Present` status downgrades, and the more severe one wins. Deterministic and
/// independent of input order.
fn weakest_status(a: PointsToStatus, b: PointsToStatus) -> PointsToStatus {
    fn rank(status: PointsToStatus) -> u8 {
        match status {
            PointsToStatus::Present => 0,
            PointsToStatus::Unknown => 1,
            PointsToStatus::Unsupported => 2,
            PointsToStatus::SetupMissing => 3,
            PointsToStatus::BudgetExceeded => 4,
        }
    }
    if rank(a) >= rank(b) { a } else { b }
}

/// Worst-of-two precision (#4): `FlowInsensitive` is the most precise a derived edge
/// may claim; weaker tiers win. Deterministic and independent of input order.
fn weakest_precision(a: PointsToPrecision, b: PointsToPrecision) -> PointsToPrecision {
    fn rank(precision: PointsToPrecision) -> u8 {
        match precision {
            PointsToPrecision::FlowInsensitive => 0,
            PointsToPrecision::LocalFlowSensitive => 1,
            PointsToPrecision::SummaryProjected => 2,
            PointsToPrecision::Heuristic => 3,
            PointsToPrecision::Unknown => 4,
            PointsToPrecision::Unsupported => 5,
        }
    }
    if rank(a) >= rank(b) { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{ObjectTokenId, PointsToConstraintId, PtVarId};
    use crate::analysis::points_to::facts::{
        PointsToConstraintFact, PointsToConstraintKind, PointsToPrecision, PointsToStatus,
    };
    use crate::analysis::points_to::solver::{PointsToBudget, solve_points_to};
    use crate::analysis::solver::policy::{GoRtaPolicy, PointsToPolicy, TsTokensPolicy};

    fn constraint(stable_key: &str, kind: PointsToConstraintKind) -> PointsToConstraintFact {
        PointsToConstraintFact {
            id: PointsToConstraintId(0),
            kind,
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: stable_key.to_string(),
        }
    }

    fn sample_constraints() -> Vec<PointsToConstraintFact> {
        vec![
            constraint(
                "addr-a",
                PointsToConstraintKind::AddressOf {
                    dst: PtVarId(1),
                    object: ObjectTokenId(10),
                },
            ),
            constraint(
                "copy",
                PointsToConstraintKind::Copy {
                    dst: PtVarId(2),
                    src: PtVarId(1),
                },
            ),
            constraint(
                "field-store",
                PointsToConstraintKind::FieldStore {
                    base: PtVarId(1),
                    field: "name".to_string(),
                    src: PtVarId(2),
                },
            ),
            constraint(
                "field-load",
                PointsToConstraintKind::FieldLoad {
                    dst: PtVarId(3),
                    base: PtVarId(1),
                    field: "name".to_string(),
                },
            ),
        ]
    }

    #[test]
    fn points_to_via_engine_equals_solve_points_to() {
        // The fold (D-03) preserves behavior: driving the points-to policy through
        // the unified engine yields exactly the standalone `solve_points_to`
        // result over the same constraints.
        let constraints = sample_constraints();
        let budget = SolverBudget::default();

        let direct = solve_points_to(&constraints, PointsToBudget::default());

        let policy = PointsToPolicy::new(constraints);
        let engine = SolverEngine::new(vec![Box::new(policy)], budget);
        let run = engine.run();

        let via_engine = run.policy_outcomes[0]
            .outcome
            .points_to
            .clone()
            .expect("points-to policy yields a folded result");

        assert_eq!(via_engine, direct);
    }

    #[test]
    fn engine_is_deterministic_across_two_runs() {
        let constraints = sample_constraints();
        let budget = SolverBudget::default();

        let first = SolverEngine::new(
            vec![Box::new(PointsToPolicy::new(constraints.clone()))],
            budget,
        )
        .run();
        let second =
            SolverEngine::new(vec![Box::new(PointsToPolicy::new(constraints))], budget).run();

        assert_eq!(first, second);
        assert_eq!(first.budget_status, BudgetStatus::WithinBudget);
    }

    #[test]
    fn engine_surfaces_budget_exhaustion_honestly() {
        // A tight per-sub-domain object cap forces the points-to fixpoint to latch
        // exhaustion; the engine projects it to BudgetStatus::BudgetExceeded
        // rather than dropping silently (D-06).
        let constraints = vec![
            constraint(
                "addr-a",
                PointsToConstraintKind::AddressOf {
                    dst: PtVarId(1),
                    object: ObjectTokenId(10),
                },
            ),
            constraint(
                "addr-b",
                PointsToConstraintKind::AddressOf {
                    dst: PtVarId(1),
                    object: ObjectTokenId(11),
                },
            ),
        ];
        let mut budget = SolverBudget::default();
        budget.points_to.max_objects_per_var = 1;

        let engine = SolverEngine::new(vec![Box::new(PointsToPolicy::new(constraints))], budget);
        let run = engine.run();

        assert_eq!(run.budget_status, BudgetStatus::BudgetExceeded);
    }

    #[test]
    fn engine_drives_stubs_to_zero_results() {
        let budget = SolverBudget::default();
        let engine = SolverEngine::new(
            vec![Box::new(GoRtaPolicy), Box::new(TsTokensPolicy)],
            budget,
        );
        let run = engine.run();

        assert_eq!(run.policy_outcomes.len(), 2);
        assert_eq!(run.policy_outcomes[0].policy_id, "go_rta");
        assert_eq!(run.policy_outcomes[1].policy_id, "ts_tokens");
        assert!(
            run.policy_outcomes
                .iter()
                .all(|record| record.outcome.points_to.is_none())
        );
        assert_eq!(run.budget_status, BudgetStatus::WithinBudget);
    }

    #[test]
    fn empty_engine_reports_not_run() {
        let engine = SolverEngine::new(Vec::new(), SolverBudget::default());
        let run = engine.run();
        assert_eq!(run.budget_status, BudgetStatus::NotRun);
        assert!(run.policy_outcomes.is_empty());
    }

    fn copy_constraint(stable_key: &str, src: u64, dst: u64) -> ConstraintFact {
        use crate::analysis::ids::{SemanticConstraintId, SemanticNodeId};
        ConstraintFact {
            id: SemanticConstraintId(0),
            kind: ConstraintKind::CopyEdge {
                dst: SemanticNodeId(dst),
                src: SemanticNodeId(src),
            },
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn derive_edges_emits_transitive_copy_edge_with_provenance() {
        // Chain a -> b -> c: the solver derives the transitive edge a -> c whose
        // provenance carries BOTH contributing copy constraints.
        let constraints = vec![
            copy_constraint("copy|a-b", 1, 2),
            copy_constraint("copy|b-c", 2, 3),
        ];
        let output = derive_edges(&constraints, &SolverBudget::default());

        use crate::analysis::ids::SemanticNodeId;
        let transitive = output
            .derived_edges
            .iter()
            .find(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3))
            .expect("transitive a -> c derived");

        assert_eq!(transitive.provenance.constraint_kind, "copy_edge");
        assert_eq!(transitive.provenance.contributing_len(), 2);
        assert!(transitive.provenance.solver_step > 0);
    }

    #[test]
    fn derive_edges_propagates_weakest_contributing_status_and_precision() {
        // a -> b is a clean Present/FlowInsensitive hop; b -> c is an UNTRUSTED hop
        // (BudgetExceeded status, Unknown precision). The derived transitive a -> c must
        // inherit the WEAKEST status/precision, never launder an untrusted input hop into
        // a confidently-precise derived edge (review finding #4 / D-06).
        let clean = copy_constraint("copy|a-b", 1, 2);
        let mut untrusted = copy_constraint("copy|b-c", 2, 3);
        untrusted.status = PointsToStatus::BudgetExceeded;
        untrusted.precision = PointsToPrecision::Unknown;
        let constraints = vec![clean, untrusted];

        let output = derive_edges(&constraints, &SolverBudget::default());

        use crate::analysis::ids::SemanticNodeId;
        let edge = output
            .derived_edges
            .iter()
            .find(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3))
            .expect("transitive a -> c derived");
        assert_eq!(
            edge.status,
            PointsToStatus::BudgetExceeded,
            "weakest contributing status must win"
        );
        assert_eq!(
            edge.precision,
            PointsToPrecision::Unknown,
            "weakest contributing precision must win"
        );
    }

    #[test]
    fn derive_edges_records_all_contributing_keys_for_a_duplicated_hop() {
        // Two distinct constraints both assert a -> b; the derived a -> c provenance must
        // record BOTH contributing identities, order-independently (review finding #10).
        let constraints = vec![
            copy_constraint("copy|a-b#1", 1, 2),
            copy_constraint("copy|a-b#2", 1, 2),
            copy_constraint("copy|b-c", 2, 3),
        ];

        let output = derive_edges(&constraints, &SolverBudget::default());

        use crate::analysis::ids::SemanticNodeId;
        let edge = output
            .derived_edges
            .iter()
            .find(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3))
            .expect("transitive a -> c derived");
        let keys: Vec<&str> = edge
            .provenance
            .contributing_facts
            .iter()
            .map(|f| f.stable_key.as_str())
            .collect();
        assert!(keys.contains(&"copy|a-b#1"), "{keys:?}");
        assert!(keys.contains(&"copy|a-b#2"), "{keys:?}");
        assert!(keys.contains(&"copy|b-c"), "{keys:?}");
    }

    #[test]
    fn budget_exhaustion_does_not_drop_independent_sources_and_is_signalled() {
        // Source 1 has a chain long enough to exhaust a tiny per-source step budget;
        // source 100 has a single short hop. The independent source 100 MUST still derive
        // its edge (no silent cross-source drop), and the run-level budget_status MUST
        // surface BudgetExceeded honestly (review finding #3 / D-06).
        let constraints = vec![
            copy_constraint("copy|1-2", 1, 2),
            copy_constraint("copy|2-3", 2, 3),
            copy_constraint("copy|3-4", 3, 4),
            copy_constraint("copy|100-101", 100, 101),
        ];
        let budget = SolverBudget {
            max_steps: 2,
            ..SolverBudget::default()
        };

        let output = derive_edges(&constraints, &budget);

        use crate::analysis::ids::SemanticNodeId;
        assert!(
            output
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId(100) && e.target == SemanticNodeId(101)),
            "the independent source 100 must not be silently dropped when source 1 exhausts the budget"
        );
        assert_eq!(
            output.budget_status,
            BudgetStatus::BudgetExceeded,
            "budget exhaustion must surface as a run-level signal, never a silent drop"
        );
    }

    #[test]
    fn diamond_provenance_is_a_deterministic_witness_not_a_global_dependency() {
        // Diamond: a(1) -> b(2) -> d(4) AND a(1) -> c(3) -> d(4). There is ONE a -> d edge
        // whose provenance is a DETERMINISTIC witnessing path ({a-b, b-d} by BFS order).
        // Deleting a fact NOT on the witness path does NOT remove the a -> d flow (it
        // persists via the witness); deleting a witness fact does NOT reproduce the SAME
        // edge fact (by stable key) — the flow re-derives via the other path under a new
        // identity. This is the honest multi-path semantic (review finding #2 / D-09).
        let constraints = vec![
            copy_constraint("copy|a-b", 1, 2),
            copy_constraint("copy|b-d", 2, 4),
            copy_constraint("copy|a-c", 1, 3),
            copy_constraint("copy|c-d", 3, 4),
        ];
        let budget = SolverBudget::default();
        use crate::analysis::ids::SemanticNodeId;

        let base = derive_edges(&constraints, &budget);
        let ad: Vec<&DerivedEdgeFact> = base
            .derived_edges
            .iter()
            .filter(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(4))
            .collect();
        assert_eq!(ad.len(), 1, "exactly one a -> d edge");
        let witness_key = ad[0].stable_key.clone();
        let witness: Vec<&str> = ad[0]
            .provenance
            .contributing_facts
            .iter()
            .map(|f| f.stable_key.as_str())
            .collect();
        assert!(
            witness.contains(&"copy|a-b") && witness.contains(&"copy|b-d"),
            "deterministic witness path is a-b -> b-d: {witness:?}"
        );

        // Delete a fact NOT on the witness path: the a -> d flow persists via the witness.
        let without_off_path: Vec<ConstraintFact> = constraints
            .iter()
            .filter(|c| c.stable_key != "copy|a-c")
            .cloned()
            .collect();
        let rerun_off = derive_edges(&without_off_path, &budget);
        assert!(
            rerun_off
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(4)),
            "deleting an off-witness fact must NOT remove the a -> d flow"
        );

        // Delete a witness fact: the SAME edge fact (by stable key) is NOT reproduced;
        // the flow re-derives via the other path under a different identity.
        let without_witness: Vec<ConstraintFact> = constraints
            .iter()
            .filter(|c| c.stable_key != "copy|a-b")
            .cloned()
            .collect();
        let rerun_witness = derive_edges(&without_witness, &budget);
        assert!(
            !rerun_witness
                .derived_edges
                .iter()
                .any(|e| e.stable_key == witness_key),
            "deleting a witness fact must invalidate THAT derived edge fact (by stable key)"
        );
    }

    #[test]
    fn budget_exhaustion_keeps_complete_pretruncation_edges_present() {
        // R1: a source's edges that were FULLY derived before the per-source step cap
        // was hit are sound and must keep their honest status — budget exhaustion costs
        // the edges never reached (signalled run-level), not the ones already derived.
        let constraints = vec![
            copy_constraint("copy|1-2", 1, 2),
            copy_constraint("copy|2-3", 2, 3),
            copy_constraint("copy|3-4", 3, 4),
        ];
        let budget = SolverBudget {
            max_steps: 2,
            ..SolverBudget::default()
        };

        let output = derive_edges(&constraints, &budget);

        use crate::analysis::ids::SemanticNodeId;
        let edge_1_2 = output
            .derived_edges
            .iter()
            .find(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(2))
            .expect("complete edge 1 -> 2 derived before truncation");
        assert_eq!(
            edge_1_2.status,
            PointsToStatus::Present,
            "a complete pre-truncation edge must NOT be downgraded"
        );
        assert_eq!(edge_1_2.precision, PointsToPrecision::FlowInsensitive);
        // The unreached edge is simply absent; the run-level signal conveys the loss.
        assert!(
            !output
                .derived_edges
                .iter()
                .any(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(4)),
            "the edge never reached under budget is absent"
        );
        assert_eq!(output.budget_status, BudgetStatus::BudgetExceeded);
    }

    #[test]
    fn untrusted_hop_on_alternate_path_conservatively_downgrades_derived_edge() {
        // R2: diamond a(1) -> b(2) -> d(4) (clean) AND a(1) -> c(3) -> d(4) where a -> c
        // is UNTRUSTED. The BFS witness reaches d via b (the shorter, clean path), but
        // because an untrusted alternate path also derives a -> d, the edge must be
        // conservatively downgraded — never laundered to a confident edge (#4, fully).
        let mut untrusted = copy_constraint("copy|a-c", 1, 3);
        untrusted.status = PointsToStatus::BudgetExceeded;
        untrusted.precision = PointsToPrecision::Unknown;
        let constraints = vec![
            copy_constraint("copy|a-b", 1, 2),
            copy_constraint("copy|b-d", 2, 4),
            untrusted,
            copy_constraint("copy|c-d", 3, 4),
        ];

        let output = derive_edges(&constraints, &SolverBudget::default());

        use crate::analysis::ids::SemanticNodeId;
        let edge = output
            .derived_edges
            .iter()
            .find(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(4))
            .expect("transitive a -> d derived");
        assert_eq!(
            edge.status,
            PointsToStatus::BudgetExceeded,
            "an untrusted alternate path must conservatively downgrade the edge"
        );
        assert_eq!(edge.precision, PointsToPrecision::Unknown);
    }

    #[test]
    fn solver_step_is_globally_monotonic_across_sources() {
        // R3: the solver step is a GLOBAL monotonic counter, not reset per source, so an
        // edge from a later source carries a strictly larger solver_step than an edge
        // from an earlier source (the documented monotonic contract).
        let constraints = vec![
            copy_constraint("copy|1-2", 1, 2),
            copy_constraint("copy|10-11", 10, 11),
        ];

        let output = derive_edges(&constraints, &SolverBudget::default());

        use crate::analysis::ids::SemanticNodeId;
        let first = output
            .derived_edges
            .iter()
            .find(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(2))
            .expect("edge 1 -> 2");
        let later = output
            .derived_edges
            .iter()
            .find(|e| e.source == SemanticNodeId(10) && e.target == SemanticNodeId(11))
            .expect("edge 10 -> 11");
        assert!(
            later.provenance.solver_step > first.provenance.solver_step,
            "later source's solver_step ({}) must exceed the earlier source's ({})",
            later.provenance.solver_step,
            first.provenance.solver_step
        );
    }

    #[test]
    fn derive_edges_is_shuffle_stable() {
        let constraints = vec![
            copy_constraint("copy|a-b", 1, 2),
            copy_constraint("copy|b-c", 2, 3),
            copy_constraint("copy|c-d", 3, 4),
        ];
        let mut shuffled = constraints.clone();
        shuffled.reverse();

        let a = derive_edges(&constraints, &SolverBudget::default());
        let b = derive_edges(&shuffled, &SolverBudget::default());

        let a_json = serde_json::to_string(&a.derived_edges).expect("serialize a");
        let b_json = serde_json::to_string(&b.derived_edges).expect("serialize b");
        assert_eq!(a_json, b_json);
    }
}
