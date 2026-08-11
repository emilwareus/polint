//! TypeScript/JavaScript field-sensitive points-to projection for the unified solver.
//!
//! This module owns the frontend-specific projection from semantic-graph constraints
//! to Andersen constraints and the derived indirect-call edges.

use std::collections::{BTreeMap, BTreeSet};

use polint_analysis::AnalysisHost;

use polint_analysis::ids::{
    DerivedEdgeId, ObjectTokenId, PointsToConstraintId, PtVarId, SemanticNodeId,
};
use polint_analysis::points_to::facts::{
    PointsToConstraintFact, PointsToConstraintKind, PointsToPrecision, PointsToStatus,
};
use polint_analysis::points_to::solver::{PointsToSolveResult, solve_points_to};
use polint_analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
use polint_analysis::semantic_graph::facts::NodeKind;
use polint_analysis::solver::budget::{BudgetStatus, SolverBudget};
use polint_analysis::solver::facts::DerivedEdgeFact;
use polint_analysis::solver::provenance::{ContributingFact, DerivedEdgeProvenance};
use polint_analysis_api::{FactFamily, stable_key_from_parts};
use polint_core::StableKeyId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsPointsToCallsite {
    caller_node: SemanticNodeId,
    callsite_node: SemanticNodeId,
    callsite_stable_key: StableKeyId,
    constraint_stable_key: StableKeyId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TsPointsToInputs {
    constraints: Vec<ConstraintFact>,
    callable_objects: BTreeMap<ObjectTokenId, (SemanticNodeId, StableKeyId)>,
    callsites: Vec<TsPointsToCallsite>,
}

impl TsPointsToInputs {
    pub fn from_db(db: &impl AnalysisHost) -> Self {
        let function_node_by_id = db
            .semantic_nodes()
            .iter()
            .filter_map(|node| match node.kind {
                NodeKind::Function(function) => Some((function, node.id)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let callsite_node_by_id = db
            .semantic_nodes()
            .iter()
            .filter_map(|node| match node.kind {
                NodeKind::Callsite(callsite) => Some((callsite, node.id)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let node_key_by_id = db
            .semantic_nodes()
            .iter()
            .map(|node| (node.id, node.stable_key))
            .collect::<BTreeMap<_, _>>();
        let constraint_key_by_callsite = db
            .semantic_constraints()
            .iter()
            .filter_map(|constraint| match constraint.kind {
                ConstraintKind::CallConstraint { callsite } => {
                    Some((callsite, constraint.stable_key))
                }
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();

        let callable_objects = db
            .semantic_nodes()
            .iter()
            .filter_map(|node| match node.kind {
                NodeKind::Function(_) => {
                    Some((object_for_node(node.id), (node.id, node.stable_key)))
                }
                _ => None,
            })
            .collect();
        let callsites = db
            .call_sites()
            .iter()
            .filter(|site| site.language.is_ts_family())
            .filter_map(|site| {
                let callsite_node = *callsite_node_by_id.get(&site.id)?;
                let caller_node = *function_node_by_id.get(&site.caller)?;
                Some(TsPointsToCallsite {
                    caller_node,
                    callsite_node,
                    callsite_stable_key: *node_key_by_id.get(&callsite_node)?,
                    constraint_stable_key: constraint_key_by_callsite
                        .get(&callsite_node)
                        .copied()
                        .unwrap_or(site.stable_key),
                })
            })
            .collect();

        Self {
            constraints: db.semantic_constraints().to_vec(),
            callable_objects,
            callsites,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsPointsToSolveResult {
    pub points_to: PointsToSolveResult,
    pub derived_edges: Vec<DerivedEdgeFact>,
}

pub fn solve_ts_points_to(
    interner: &polint_core::StableKeyInterner,
    inputs: &TsPointsToInputs,
    budget: &SolverBudget,
) -> TsPointsToSolveResult {
    let constraints = project_constraints(interner, inputs);
    let points_to = solve_points_to(interner, &constraints, budget.points_to_budget());
    let sets = points_to
        .sets
        .iter()
        .map(|set| (set.variable, set))
        .collect::<BTreeMap<_, _>>();
    // Membership-only dedup by StableKeyId; emission order is assigned below via
    // `SolverOutput::normalized` (resolved-text sort), never allocation order.
    let mut edges = std::collections::HashMap::new();

    for callsite in &inputs.callsites {
        let Some(set) = sets.get(&var_for_node(callsite.callsite_node)) else {
            continue;
        };
        if set.status != PointsToStatus::Present {
            continue;
        }
        for object in &set.objects {
            let Some((target, target_stable_key)) = inputs.callable_objects.get(object) else {
                continue;
            };
            let provenance = DerivedEdgeProvenance::new(
                interner,
                vec![
                    ContributingFact {
                        stable_key: callsite.constraint_stable_key,
                    },
                    ContributingFact {
                        stable_key: callsite.callsite_stable_key,
                    },
                    ContributingFact {
                        stable_key: *target_stable_key,
                    },
                    ContributingFact {
                        stable_key: set.stable_key,
                    },
                ],
                &ConstraintKind::CallConstraint {
                    callsite: callsite.callsite_node,
                },
                0,
            );
            let stable_key = stable_key_from_parts(
                interner,
                FactFamily::SolverDerivedEdge,
                &[
                    ("source", callsite.caller_node.0.to_string()),
                    ("target", target.0.to_string()),
                    ("provenance", provenance.stable_key_fragment(interner)),
                ],
            );
            edges.entry(stable_key).or_insert(DerivedEdgeFact {
                id: DerivedEdgeId(0),
                source: callsite.caller_node,
                target: *target,
                status: set.status,
                precision: set.precision,
                stable_key,
                provenance,
            });
        }
    }

    // Direct/policy consumers must never observe allocation-order emission.
    let derived_edges = polint_analysis::solver::store::SolverOutput {
        derived_edges: edges.into_values().collect(),
        ..polint_analysis::solver::store::SolverOutput::default()
    }
    .normalized(interner)
    .derived_edges;

    TsPointsToSolveResult {
        points_to,
        derived_edges,
    }
}

fn project_constraints(
    interner: &polint_core::StableKeyInterner,
    inputs: &TsPointsToInputs,
) -> Vec<PointsToConstraintFact> {
    let mut kinds = BTreeSet::new();
    let callsite_by_source_key = inputs
        .constraints
        .iter()
        .filter_map(|constraint| match constraint.kind {
            ConstraintKind::CallConstraint { callsite } => Some((constraint.stable_key, callsite)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();

    for (&object, &(node, _)) in &inputs.callable_objects {
        kinds.insert(PointsToConstraintKind::AddressOf {
            dst: var_for_node(node),
            object,
        });
    }
    for constraint in &inputs.constraints {
        if constraint.status != PointsToStatus::Present {
            continue;
        }
        match &constraint.kind {
            ConstraintKind::CopyEdge { dst, src } => {
                kinds.insert(PointsToConstraintKind::Copy {
                    dst: var_for_node(*dst),
                    src: var_for_node(*src),
                });
            }
            ConstraintKind::Alloc { dst, object } => {
                let object_token = object_for_node(*object);
                kinds.insert(PointsToConstraintKind::AddressOf {
                    dst: var_for_node(*object),
                    object: object_token,
                });
                kinds.insert(PointsToConstraintKind::AddressOf {
                    dst: var_for_node(*dst),
                    object: object_token,
                });
            }
            ConstraintKind::FieldLoad { dst, base, field } => {
                kinds.insert(PointsToConstraintKind::FieldLoad {
                    dst: var_for_node(*dst),
                    base: var_for_node(*base),
                    field: field.clone(),
                });
                if let Some(callsite) = callsite_by_source_key.get(&constraint.stable_key) {
                    kinds.insert(PointsToConstraintKind::Copy {
                        dst: var_for_node(*callsite),
                        src: var_for_node(*dst),
                    });
                }
            }
            ConstraintKind::FieldStore { base, field, src } => {
                kinds.insert(PointsToConstraintKind::FieldStore {
                    base: var_for_node(*base),
                    field: field.clone(),
                    src: var_for_node(*src),
                });
            }
            ConstraintKind::CallConstraint { .. }
            | ConstraintKind::ModelEdge { .. }
            | ConstraintKind::TypeConstraint { .. } => {}
        }
    }

    kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| PointsToConstraintFact {
            id: PointsToConstraintId(index as u64),
            stable_key: stable_key_from_parts(
                interner,
                FactFamily::PointsToConstraint,
                &[("kind", format!("{kind:?}"))],
            ),
            kind,
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
        })
        .collect()
}

fn var_for_node(node: SemanticNodeId) -> PtVarId {
    PtVarId(node.0)
}

fn object_for_node(node: SemanticNodeId) -> ObjectTokenId {
    ObjectTokenId(node.0)
}

pub fn budget_status(result: &PointsToSolveResult) -> BudgetStatus {
    BudgetStatus::from_points_to(result.budget_status)
}
