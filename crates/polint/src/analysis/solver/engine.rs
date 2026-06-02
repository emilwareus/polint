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
/// Provenance is LOAD-BEARING (D-09): a derived transitive edge exists only because
/// of the specific contributing constraints on its derivation path, so deleting any
/// one of them removes the edge from the recomputed closure (proven by the deletion
/// property test). The traversal is a deterministic `BTreeMap`/`BTreeSet` worklist —
/// dense IDs are assigned only after the stable-key sort in
/// [`SolverOutput::normalized`], so the output is byte-stable (D-02).
///
/// Budget: the per-step worklist cap (`budget.max_steps`) bounds the closure; on
/// exhaustion the emitted edges latch [`PointsToStatus::BudgetExceeded`] honestly
/// (D-06) rather than dropping silently. Derived edges never claim exact precision
/// (D-06): they are `FlowInsensitive` at most.
pub(crate) fn derive_edges(constraints: &[ConstraintFact], budget: &SolverBudget) -> SolverOutput {
    // Primitive copy adjacency `src -> {dst}`, plus the contributing constraint
    // stable key for each primitive `src -> dst` hop. BTree-ordered for determinism.
    let mut adjacency: BTreeMap<SemanticNodeRef, BTreeSet<SemanticNodeRef>> = BTreeMap::new();
    let mut hop_keys: BTreeMap<(SemanticNodeRef, SemanticNodeRef), String> = BTreeMap::new();
    for constraint in constraints {
        if let ConstraintKind::CopyEdge { dst, src } = &constraint.kind {
            let (src, dst) = (src.0, dst.0);
            adjacency.entry(src).or_default().insert(dst);
            // A constraint's EXISTING stable key is the contributing identity.
            hop_keys
                .entry((src, dst))
                .or_insert_with(|| constraint.stable_key.clone());
        }
    }

    // For each source, BFS the closure accumulating the contributing primitive hops
    // along the path. `reached[node]` is the totally-ordered set of contributing
    // stable keys for the best (and, in an acyclic chain, only) path to `node`.
    let mut steps: u64 = 0;
    let mut budget_exceeded = false;
    let mut edges: Vec<DerivedEdgeFact> = Vec::new();

    let sources: Vec<SemanticNodeRef> = adjacency.keys().copied().collect();
    for start in sources {
        // reached: node -> contributing stable keys on the path start..node.
        let mut reached: BTreeMap<SemanticNodeRef, BTreeSet<String>> = BTreeMap::new();
        let mut queue: VecDeque<SemanticNodeRef> = VecDeque::new();
        reached.insert(start, BTreeSet::new());
        queue.push_back(start);

        while let Some(node) = queue.pop_front() {
            steps += 1;
            if steps > budget.max_steps as u64 {
                budget_exceeded = true;
                break;
            }
            let path_keys = reached.get(&node).cloned().unwrap_or_default();
            let Some(targets) = adjacency.get(&node) else {
                continue;
            };
            for &next in targets {
                if next == start {
                    // Skip self-loops back to the start (cycle guard).
                    continue;
                }
                let hop_key = hop_keys.get(&(node, next)).cloned().unwrap_or_default();
                let mut next_keys = path_keys.clone();
                next_keys.insert(hop_key);
                // Visit a node once per source (acyclic-chain assumption keeps the
                // contributing set well-defined and the closure bounded).
                if let std::collections::btree_map::Entry::Vacant(slot) = reached.entry(next) {
                    slot.insert(next_keys);
                    queue.push_back(next);
                }
            }
        }

        // Emit a derived edge for every node reachable from `start` in >= 1 hop.
        for (&node, contributing) in &reached {
            if node == start || contributing.is_empty() {
                continue;
            }
            let provenance = DerivedEdgeProvenance::new(
                contributing.iter().map(|key| ContributingFact {
                    stable_key: key.clone(),
                }),
                &ConstraintKind::CopyEdge {
                    dst: crate::analysis::ids::SemanticNodeId(node),
                    src: crate::analysis::ids::SemanticNodeId(start),
                },
                steps,
            );
            let status = if budget_exceeded {
                PointsToStatus::BudgetExceeded
            } else {
                PointsToStatus::Present
            };
            // Honest precision ceiling (D-06): transitive copy edges are at most
            // flow-insensitive, never exact.
            let precision = if budget_exceeded {
                PointsToPrecision::Unknown
            } else {
                PointsToPrecision::FlowInsensitive
            };
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
                status,
                precision,
                stable_key,
                provenance,
            });
        }

        if budget_exceeded {
            break;
        }
    }

    SolverOutput {
        derived_edges: edges,
    }
    .normalized()
}

/// Run-local node handle (the `SemanticNodeId.0` value) used as a deterministic
/// `BTreeMap`/`BTreeSet` key during closure derivation.
type SemanticNodeRef = u64;

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
