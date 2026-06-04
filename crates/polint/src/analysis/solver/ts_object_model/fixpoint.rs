//! Deterministic JS/TS object/property bucket fixpoint.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::semantic_graph::facts::NodeKind;
use crate::analysis::solver::budget::{BudgetStatus, SolverBudget};
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis::solver::ts_tokens::fixpoint::{TsTokenVarState, solve_ts_tokens};

use super::dispatch::resolve_object_callsite;
use super::inputs::{TsObjectModelInputs, TsObjectPropertyWrite};
use super::prototype::lookup_property_with_prototypes;
use super::receiver::resolve_receiver_bindings;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsObjectValueToken {
    Function(SemanticNodeId),
    Object(SemanticNodeId),
}

/// One object property bucket keyed by `(object, field)`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsObjectPropertyBucketKey {
    pub(crate) object: SemanticNodeId,
    pub(crate) field: String,
}

/// Tokens stored in one object property bucket, with stable contributing evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TsObjectPropertyBucketState {
    pub(crate) tokens: BTreeMap<TsObjectValueToken, BTreeSet<String>>,
}

/// Result of the TS object-model solve.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TsObjectModelSolveResult {
    pub(crate) property_buckets: BTreeMap<TsObjectPropertyBucketKey, TsObjectPropertyBucketState>,
    pub(crate) derived_edges: Vec<DerivedEdgeFact>,
    pub(crate) budget_status: BudgetStatus,
    pub(crate) budget_reasons: BTreeSet<String>,
    pub(crate) steps: u64,
}

/// Run the bounded object/property fixpoint and derive property-backed call edges.
pub(crate) fn solve_ts_object_model(
    inputs: &TsObjectModelInputs,
    budget: &SolverBudget,
) -> TsObjectModelSolveResult {
    let token_result = solve_ts_tokens(&inputs.token_inputs, budget);
    let allocation_key_by_object = allocation_key_by_object(inputs);
    let receiver_bindings = resolve_receiver_bindings(&inputs.receiver_bindings);
    let evidence_by_callsite =
        evidence_by_callsite(inputs, &receiver_bindings.evidence_by_callsite);
    let token_states = token_result.tokens_by_var;

    let mut budget_reasons = BTreeSet::new();
    let mut steps = token_result.steps;

    enforce_objects_per_place(inputs, budget, &mut budget_reasons);
    let mut accumulator = PropertyBucketAccumulator::default();

    for write in &inputs.property_writes {
        steps += 1;
        if steps > budget.object.max_object_worklist_steps as u64 {
            budget_reasons.insert("max_object_worklist_steps".to_string());
            break;
        }

        let source_tokens = source_tokens_for_write(write, inputs, &token_states);
        for (token, mut evidence) in source_tokens {
            if let Some(allocation_key) = allocation_key_by_object.get(&write.base_object) {
                evidence.insert(allocation_key.clone());
            }
            evidence.insert(write.stable_key.clone());
            accumulator.insert(write, token, evidence, budget, &mut budget_reasons);
        }
    }

    let mut edges_by_key = BTreeMap::new();
    for read in &inputs.property_reads {
        steps += 1;
        if steps > budget.object.max_object_worklist_steps as u64 {
            budget_reasons.insert("max_object_worklist_steps".to_string());
            break;
        }
        let lookup = lookup_property_with_prototypes(
            &accumulator.buckets,
            &inputs.prototype_links,
            read.base_object,
            &read.field,
            budget,
        );
        budget_reasons.extend(lookup.budget_reasons);
        if lookup.tokens.is_empty() {
            continue;
        }
        let handoff_keys = read
            .callsite_node
            .and_then(|callsite| evidence_by_callsite.get(&callsite))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let resolution = resolve_object_callsite(
            read,
            &lookup.tokens,
            handoff_keys,
            budget.object.max_receiver_candidates_per_callsite,
            steps,
        );
        if resolution.candidate_cap_exceeded {
            budget_reasons.insert("max_receiver_candidates_per_callsite".to_string());
        }
        for edge in resolution.edges {
            edges_by_key.entry(edge.stable_key.clone()).or_insert(edge);
        }
    }

    if token_result.budget_status == BudgetStatus::BudgetExceeded {
        budget_reasons.insert("ts_token_seed_budget_exceeded".to_string());
    }

    TsObjectModelSolveResult {
        property_buckets: accumulator.buckets,
        derived_edges: edges_by_key.into_values().collect(),
        budget_status: if budget_reasons.is_empty() {
            BudgetStatus::WithinBudget
        } else {
            BudgetStatus::BudgetExceeded
        },
        budget_reasons,
        steps,
    }
}

fn source_tokens_for_write(
    write: &TsObjectPropertyWrite,
    inputs: &TsObjectModelInputs,
    token_states: &BTreeMap<SemanticNodeId, TsTokenVarState>,
) -> Vec<(TsObjectValueToken, BTreeSet<String>)> {
    match inputs.node_kinds.get(&write.source_node) {
        Some(NodeKind::Function(_)) => vec![(
            TsObjectValueToken::Function(write.source_node),
            BTreeSet::from([node_evidence(inputs, write.source_node)]),
        )],
        Some(NodeKind::AbstractObject(_)) => vec![(
            TsObjectValueToken::Object(write.source_node),
            BTreeSet::from([node_evidence(inputs, write.source_node)]),
        )],
        _ => token_states
            .get(&write.source_node)
            .and_then(TsTokenVarState::tokens)
            .into_iter()
            .flat_map(|tokens| {
                tokens.iter().map(|(&function_node, evidence)| {
                    (
                        TsObjectValueToken::Function(function_node),
                        evidence.clone(),
                    )
                })
            })
            .collect(),
    }
}

fn node_evidence(inputs: &TsObjectModelInputs, node: SemanticNodeId) -> String {
    inputs
        .token_inputs
        .function_tokens
        .get(&node)
        .map(|token| token.function_stable_key.clone())
        .or_else(|| {
            inputs
                .allocations
                .iter()
                .find(|allocation| allocation.object_node == node)
                .map(|allocation| allocation.stable_key.clone())
        })
        .unwrap_or_else(|| format!("semantic_node:{}", node.0))
}

#[derive(Default)]
struct PropertyBucketAccumulator {
    buckets: BTreeMap<TsObjectPropertyBucketKey, TsObjectPropertyBucketState>,
    properties_by_object: BTreeMap<SemanticNodeId, BTreeSet<String>>,
    computed_buckets_by_object: BTreeMap<SemanticNodeId, BTreeSet<String>>,
}

impl PropertyBucketAccumulator {
    fn insert(
        &mut self,
        write: &TsObjectPropertyWrite,
        token: TsObjectValueToken,
        evidence: BTreeSet<String>,
        budget: &SolverBudget,
        budget_reasons: &mut BTreeSet<String>,
    ) {
        let object_properties = self
            .properties_by_object
            .entry(write.base_object)
            .or_default();
        if !object_properties.contains(&write.field)
            && object_properties.len() >= budget.object.max_properties_per_object
        {
            budget_reasons.insert("max_properties_per_object".to_string());
            return;
        }
        if is_computed_bucket(&write.field) {
            let computed = self
                .computed_buckets_by_object
                .entry(write.base_object)
                .or_default();
            if !computed.contains(&write.field)
                && computed.len() >= budget.object.max_computed_buckets_per_object
            {
                budget_reasons.insert("max_computed_buckets_per_object".to_string());
                return;
            }
            computed.insert(write.field.clone());
        }
        object_properties.insert(write.field.clone());

        let key = TsObjectPropertyBucketKey {
            object: write.base_object,
            field: write.field.clone(),
        };
        let bucket = self.buckets.entry(key).or_default();
        if !bucket.tokens.contains_key(&token)
            && bucket.tokens.len() >= budget.object.max_tokens_per_property
        {
            budget_reasons.insert("max_tokens_per_property".to_string());
            return;
        }
        bucket.tokens.entry(token).or_default().extend(evidence);
    }
}

fn is_computed_bucket(field: &str) -> bool {
    matches!(field, "computed_bucket" | "unknown_bucket")
}

fn allocation_key_by_object(inputs: &TsObjectModelInputs) -> BTreeMap<SemanticNodeId, String> {
    inputs
        .allocations
        .iter()
        .map(|allocation| (allocation.object_node, allocation.stable_key.clone()))
        .collect()
}

fn enforce_objects_per_place(
    inputs: &TsObjectModelInputs,
    budget: &SolverBudget,
    budget_reasons: &mut BTreeSet<String>,
) {
    let mut objects_by_place: BTreeMap<SemanticNodeId, BTreeSet<SemanticNodeId>> = BTreeMap::new();
    for allocation in &inputs.allocations {
        let objects = objects_by_place.entry(allocation.place_node).or_default();
        if !objects.contains(&allocation.object_node)
            && objects.len() >= budget.object.max_objects_per_place
        {
            budget_reasons.insert("max_objects_per_place".to_string());
            return;
        }
        objects.insert(allocation.object_node);
    }
}

fn evidence_by_callsite(
    inputs: &TsObjectModelInputs,
    receiver_evidence: &BTreeMap<SemanticNodeId, Vec<String>>,
) -> BTreeMap<SemanticNodeId, Vec<String>> {
    let mut by_callsite: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for handoff in &inputs.handoffs {
        by_callsite
            .entry(handoff.callsite_node)
            .or_default()
            .push(handoff.binding_stable_key.clone());
    }
    for (callsite, evidence) in receiver_evidence {
        by_callsite
            .entry(*callsite)
            .or_default()
            .extend(evidence.iter().cloned());
    }
    by_callsite
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::solver::ts_object_model::inputs::TsObjectPropertyRead;
    use crate::analysis::solver::ts_tokens::inputs::TsFunctionToken;

    #[test]
    fn exact_property_bucket_emits_property_backed_edge() {
        let result =
            solve_ts_object_model(&basic_inputs("static:target"), &SolverBudget::default());

        assert_eq!(result.budget_status, BudgetStatus::WithinBudget);
        assert_eq!(result.derived_edges.len(), 1);
        assert_eq!(result.derived_edges[0].source, SemanticNodeId(1));
        assert_eq!(result.derived_edges[0].target, SemanticNodeId(2));
        let contributing = result.derived_edges[0]
            .provenance
            .contributing_facts
            .iter()
            .map(|fact| fact.stable_key.as_str())
            .collect::<BTreeSet<_>>();
        assert!(contributing.contains("allocation:holder"));
        assert!(contributing.contains("write:target"));
        assert!(contributing.contains("read:target"));
    }

    #[test]
    fn computed_bucket_does_not_read_every_exact_property() {
        let mut inputs = basic_inputs("static:target");
        inputs.property_reads[0].field = "computed_bucket".to_string();

        let result = solve_ts_object_model(&inputs, &SolverBudget::default());

        assert!(result.derived_edges.is_empty());
    }

    #[test]
    fn matching_computed_bucket_can_emit_edge() {
        let result =
            solve_ts_object_model(&basic_inputs("computed_bucket"), &SolverBudget::default());

        assert_eq!(result.derived_edges.len(), 1);
    }

    #[test]
    fn prototype_bucket_lookup_emits_edge_with_prototype_evidence() {
        let mut inputs = basic_inputs("static:m");
        inputs.property_writes[0].base_object = SemanticNodeId(11);
        inputs
            .prototype_links
            .push(super::super::inputs::TsObjectPrototypeLink {
                object: SemanticNodeId(10),
                prototype: SemanticNodeId(11),
                stable_key: "prototype:C".to_string(),
            });

        let result = solve_ts_object_model(&inputs.normalized(), &SolverBudget::default());

        assert_eq!(result.derived_edges.len(), 1);
        let contributing = result.derived_edges[0]
            .provenance
            .contributing_facts
            .iter()
            .map(|fact| fact.stable_key.as_str())
            .collect::<BTreeSet<_>>();
        assert!(contributing.contains("prototype:C"));
    }

    #[test]
    fn receiver_binding_evidence_is_included_in_edge_provenance() {
        let mut inputs = basic_inputs("static:target");
        inputs
            .receiver_bindings
            .push(super::super::inputs::TsObjectReceiverBinding {
                kind: crate::ts::object_model::facts::TsReceiverBindingKind::MethodCall,
                callsite_node: Some(SemanticNodeId(9)),
                receiver_object: Some(SemanticNodeId(10)),
                stable_key: "receiver:method".to_string(),
            });

        let result = solve_ts_object_model(&inputs.normalized(), &SolverBudget::default());

        let contributing = result.derived_edges[0]
            .provenance
            .contributing_facts
            .iter()
            .map(|fact| fact.stable_key.as_str())
            .collect::<BTreeSet<_>>();
        assert!(contributing.contains("receiver:method"));
    }

    #[test]
    fn max_properties_per_object_latches_budget_exceeded() {
        let mut inputs = basic_inputs("static:a");
        inputs.property_writes.push(TsObjectPropertyWrite {
            field: "static:b".to_string(),
            stable_key: "write:b".to_string(),
            ..inputs.property_writes[0].clone()
        });
        let mut budget = SolverBudget::default();
        budget.object.max_properties_per_object = 1;

        let result = solve_ts_object_model(&inputs.normalized(), &budget);

        assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
        assert!(result.budget_reasons.contains("max_properties_per_object"));
    }

    #[test]
    fn max_objects_per_place_latches_budget_exceeded() {
        let mut inputs = basic_inputs("static:target");
        inputs
            .allocations
            .push(super::super::inputs::TsObjectAllocationInput {
                place_node: SemanticNodeId(30),
                object_node: SemanticNodeId(11),
                stable_key: "allocation:other".to_string(),
            });
        let mut budget = SolverBudget::default();
        budget.object.max_objects_per_place = 1;

        let result = solve_ts_object_model(&inputs.normalized(), &budget);

        assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
        assert!(result.budget_reasons.contains("max_objects_per_place"));
    }

    #[test]
    fn max_tokens_per_property_latches_budget_exceeded_and_keeps_precap_edge() {
        let mut inputs = basic_inputs("static:target");
        inputs.node_kinds.insert(
            SemanticNodeId(3),
            NodeKind::Function(crate::core::FunctionId(3)),
        );
        inputs.token_inputs.function_tokens.insert(
            SemanticNodeId(3),
            TsFunctionToken {
                function_node: SemanticNodeId(3),
                function_stable_key: "function:other".to_string(),
                source_stable_key: "node:function:other".to_string(),
            },
        );
        inputs.property_writes.push(TsObjectPropertyWrite {
            source_node: SemanticNodeId(3),
            stable_key: "write:other".to_string(),
            ..inputs.property_writes[0].clone()
        });
        let mut budget = SolverBudget::default();
        budget.object.max_tokens_per_property = 1;

        let result = solve_ts_object_model(&inputs.normalized(), &budget);

        assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
        assert!(result.budget_reasons.contains("max_tokens_per_property"));
        assert_eq!(result.derived_edges.len(), 1);
    }

    #[test]
    fn max_computed_buckets_per_object_latches_budget_exceeded() {
        let mut inputs = basic_inputs("computed_bucket");
        inputs.property_writes.push(TsObjectPropertyWrite {
            field: "unknown_bucket".to_string(),
            stable_key: "write:unknown".to_string(),
            ..inputs.property_writes[0].clone()
        });
        let mut budget = SolverBudget::default();
        budget.object.max_computed_buckets_per_object = 1;

        let result = solve_ts_object_model(&inputs.normalized(), &budget);

        assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
        assert!(
            result
                .budget_reasons
                .contains("max_computed_buckets_per_object")
        );
    }

    #[test]
    fn permuted_input_rows_produce_byte_identical_output() {
        let mut forward = basic_inputs("static:target");
        forward.property_writes.push(TsObjectPropertyWrite {
            field: "static:other".to_string(),
            stable_key: "write:other".to_string(),
            ..forward.property_writes[0].clone()
        });
        let mut reversed = forward.clone();
        reversed.property_writes.reverse();
        reversed.property_reads.reverse();
        reversed.allocations.reverse();

        let a = solve_ts_object_model(&forward.normalized(), &SolverBudget::default());
        let b = solve_ts_object_model(&reversed.normalized(), &SolverBudget::default());

        assert_eq!(
            serde_json::to_string(&(a.derived_edges, a.budget_status)).unwrap(),
            serde_json::to_string(&(b.derived_edges, b.budget_status)).unwrap()
        );
    }

    fn basic_inputs(field: &str) -> TsObjectModelInputs {
        TsObjectModelInputs {
            allocations: vec![super::super::inputs::TsObjectAllocationInput {
                place_node: SemanticNodeId(30),
                object_node: SemanticNodeId(10),
                stable_key: "allocation:holder".to_string(),
            }],
            property_writes: vec![TsObjectPropertyWrite {
                base_object: SemanticNodeId(10),
                field: field.to_string(),
                source_node: SemanticNodeId(2),
                stable_key: "write:target".to_string(),
            }],
            property_reads: vec![TsObjectPropertyRead {
                base_object: SemanticNodeId(10),
                field: field.to_string(),
                destination_node: SemanticNodeId(20),
                callsite_node: Some(SemanticNodeId(9)),
                caller_node: Some(SemanticNodeId(1)),
                callsite_stable_key: Some("callsite:target".to_string()),
                constraint_stable_key: "read:target".to_string(),
                stable_key: "read:target".to_string(),
            }],
            token_inputs: crate::analysis::solver::ts_tokens::inputs::TsTokenInputs {
                function_tokens: BTreeMap::from([(
                    SemanticNodeId(2),
                    TsFunctionToken {
                        function_node: SemanticNodeId(2),
                        function_stable_key: "function:target".to_string(),
                        source_stable_key: "node:function:target".to_string(),
                    },
                )]),
                ..Default::default()
            },
            node_kinds: BTreeMap::from([
                (
                    SemanticNodeId(1),
                    NodeKind::Function(crate::core::FunctionId(1)),
                ),
                (
                    SemanticNodeId(2),
                    NodeKind::Function(crate::core::FunctionId(2)),
                ),
                (
                    SemanticNodeId(9),
                    NodeKind::Callsite(crate::analysis::ids::CallSiteId(9)),
                ),
                (
                    SemanticNodeId(10),
                    NodeKind::AbstractObject(crate::analysis::ids::ObjectTokenId(10)),
                ),
                (
                    SemanticNodeId(20),
                    NodeKind::Place(crate::analysis::ids::PlaceId(20)),
                ),
            ]),
            ..Default::default()
        }
        .normalized()
    }
}
