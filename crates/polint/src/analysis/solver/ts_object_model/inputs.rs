//! Closed JS/TS object-model input snapshot (JS-05).

use std::collections::BTreeMap;

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::semantic_graph::build::collect_ts_direct_bindings;
use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
use crate::analysis::semantic_graph::facts::NodeKind;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, FunctionId};
use crate::ts::binding::facts::{
    TsDirectBindingFact, TsDirectBindingReason, TsDirectBindingStatus,
};
use crate::ts::object_model::facts::{
    TsObjectModelStatus, TsPropertyReadFact, TsPropertyWriteFact, TsReceiverBindingKind,
};

use super::super::ts_tokens::TsTokenInputs;

/// One allocation-site object node that may own property buckets.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TsObjectAllocationInput {
    pub(crate) place_node: SemanticNodeId,
    pub(crate) object_node: SemanticNodeId,
    pub(crate) stable_key: String,
}

/// One property write into an allocation-site object's property bucket.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TsObjectPropertyWrite {
    pub(crate) base_object: SemanticNodeId,
    pub(crate) field: String,
    pub(crate) source_node: SemanticNodeId,
    pub(crate) stable_key: String,
}

/// One property read, optionally attached to a callsite obligation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TsObjectPropertyRead {
    pub(crate) base_object: SemanticNodeId,
    pub(crate) base_is_this: bool,
    pub(crate) field: String,
    pub(crate) destination_node: SemanticNodeId,
    pub(crate) callsite_node: Option<SemanticNodeId>,
    pub(crate) caller_node: Option<SemanticNodeId>,
    pub(crate) callsite_stable_key: Option<String>,
    pub(crate) constraint_stable_key: String,
    pub(crate) stable_key: String,
}

/// An unresolved direct-binding handoff eligible for the object-property driver.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TsObjectModelHandoff {
    pub(crate) callsite_node: SemanticNodeId,
    pub(crate) binding_stable_key: String,
    pub(crate) reason: TsDirectBindingReason,
}

/// One stable prototype edge `object -> prototype`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TsObjectPrototypeLink {
    pub(crate) object: SemanticNodeId,
    pub(crate) prototype: SemanticNodeId,
    pub(crate) stable_key: String,
}

/// One stable receiver/`this` binding fact attached to a callsite or callee.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TsObjectReceiverBinding {
    pub(crate) kind: TsReceiverBindingKind,
    pub(crate) callsite_node: Option<SemanticNodeId>,
    pub(crate) receiver_object: Option<SemanticNodeId>,
    pub(crate) stable_key: String,
}

/// Closed input snapshot for the TS object-model policy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TsObjectModelInputs {
    pub(crate) allocations: Vec<TsObjectAllocationInput>,
    pub(crate) property_writes: Vec<TsObjectPropertyWrite>,
    pub(crate) property_reads: Vec<TsObjectPropertyRead>,
    pub(crate) handoffs: Vec<TsObjectModelHandoff>,
    pub(crate) prototype_links: Vec<TsObjectPrototypeLink>,
    pub(crate) receiver_bindings: Vec<TsObjectReceiverBinding>,
    pub(crate) token_inputs: TsTokenInputs,
    pub(crate) node_kinds: BTreeMap<SemanticNodeId, NodeKind>,
}

impl TsObjectModelInputs {
    /// Build a closed object-model snapshot from already-stored object rows,
    /// semantic-graph constraints, callsite identities, and TS token seeds.
    pub(crate) fn from_db(db: &AnalysisDb) -> Self {
        let graph = GraphIndex::build(db);
        let constraints = ConstraintIndex::build(db.semantic_constraints());
        let direct_bindings = collect_ts_direct_bindings(db);

        let allocations = allocation_inputs(db, &constraints);
        let property_writes = property_write_inputs(db, &constraints);
        let property_reads = property_read_inputs(db, &graph, &constraints);
        let handoffs = property_handoffs_from_direct_bindings(&graph, &direct_bindings.bindings);
        let prototype_links = prototype_link_inputs(db, &constraints);
        let receiver_bindings = receiver_binding_inputs(db, &graph, &constraints);
        let node_kinds = db
            .semantic_nodes()
            .iter()
            .map(|node| (node.id, node.kind.clone()))
            .collect();

        Self {
            allocations,
            property_writes,
            property_reads,
            handoffs,
            prototype_links,
            receiver_bindings,
            token_inputs: TsTokenInputs::from_db(db),
            node_kinds,
        }
        .normalized()
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.allocations.sort();
        self.allocations.dedup();
        self.property_writes.sort();
        self.property_writes.dedup();
        self.property_reads.sort();
        self.property_reads.dedup();
        self.handoffs.sort();
        self.handoffs.dedup();
        self.prototype_links.sort();
        self.prototype_links.dedup();
        self.receiver_bindings.sort();
        self.receiver_bindings.dedup();
        self.token_inputs = self.token_inputs.normalized();
        self.node_kinds = self.node_kinds.into_iter().collect();
        self
    }
}

fn prototype_link_inputs(
    db: &AnalysisDb,
    constraints: &ConstraintIndex<'_>,
) -> Vec<TsObjectPrototypeLink> {
    let mut inputs = Vec::new();
    for link in db.ts_prototype_links() {
        if !is_resolved(&link.status) {
            continue;
        }
        let Some((object, _, prototype)) = constraints.field_store_by_stable_key(&link.stable_key)
        else {
            continue;
        };
        inputs.push(TsObjectPrototypeLink {
            object,
            prototype,
            stable_key: link.stable_key.clone(),
        });
    }
    inputs.sort();
    inputs.dedup();
    inputs
}

fn receiver_binding_inputs(
    db: &AnalysisDb,
    graph: &GraphIndex,
    constraints: &ConstraintIndex<'_>,
) -> Vec<TsObjectReceiverBinding> {
    let mut inputs = Vec::new();
    for binding in db.ts_receiver_bindings() {
        if !is_resolved(&binding.status) {
            continue;
        }
        let receiver_object = constraints
            .copy_edge(&binding.stable_key)
            .map(|(_, src)| src);
        let callsite_node = constraints
            .call_constraint(&binding.stable_key)
            .or_else(|| {
                binding
                    .callsite_stable_key
                    .as_deref()
                    .and_then(|key| graph.callsite_node(key))
            });
        inputs.push(TsObjectReceiverBinding {
            kind: binding.kind,
            callsite_node,
            receiver_object,
            stable_key: binding.stable_key.clone(),
        });
    }
    inputs.sort();
    inputs.dedup();
    inputs
}

fn allocation_inputs(
    db: &AnalysisDb,
    constraints: &ConstraintIndex<'_>,
) -> Vec<TsObjectAllocationInput> {
    let mut inputs = Vec::new();
    for allocation in db.ts_object_allocations() {
        if !is_resolved(&allocation.status) {
            continue;
        }
        let Some((place_node, object_node)) = constraints.alloc(&allocation.stable_key) else {
            continue;
        };
        inputs.push(TsObjectAllocationInput {
            place_node,
            object_node,
            stable_key: allocation.stable_key.clone(),
        });
    }
    inputs.sort();
    inputs.dedup();
    inputs
}

fn property_write_inputs(
    db: &AnalysisDb,
    constraints: &ConstraintIndex<'_>,
) -> Vec<TsObjectPropertyWrite> {
    let mut inputs = Vec::new();
    for write in db.ts_property_writes() {
        if !is_resolved(&write.status) {
            continue;
        }
        let Some((base, field, source_node)) = constraints.field_store(write) else {
            continue;
        };
        inputs.push(TsObjectPropertyWrite {
            base_object: base,
            field,
            source_node,
            stable_key: write.stable_key.clone(),
        });
    }
    inputs.sort();
    inputs.dedup();
    inputs
}

fn property_read_inputs(
    db: &AnalysisDb,
    graph: &GraphIndex,
    constraints: &ConstraintIndex<'_>,
) -> Vec<TsObjectPropertyRead> {
    let mut inputs = Vec::new();
    for read in db.ts_property_reads() {
        if !is_resolved(&read.status) {
            continue;
        }
        let Some((destination_node, base_object, field)) = constraints.field_load(read) else {
            continue;
        };
        let callsite_node = constraints.call_constraint(&read.stable_key).or_else(|| {
            read.callsite_stable_key
                .as_deref()
                .and_then(|key| graph.callsite_node(key))
        });
        let caller_node = callsite_node
            .and_then(|callsite| graph.caller_node_for_callsite_node(callsite))
            .or_else(|| {
                read.callsite_stable_key
                    .as_deref()
                    .and_then(|key| graph.caller_node(key))
            });
        inputs.push(TsObjectPropertyRead {
            base_object,
            base_is_this: is_this_expression_object_key(&read.base_object_stable_key),
            field,
            destination_node,
            callsite_node,
            caller_node,
            callsite_stable_key: read.callsite_stable_key.clone(),
            constraint_stable_key: read.stable_key.clone(),
            stable_key: read.stable_key.clone(),
        });
    }
    inputs.sort();
    inputs.dedup();
    inputs
}

fn property_handoffs_from_direct_bindings(
    graph: &GraphIndex,
    bindings: &[TsDirectBindingFact],
) -> Vec<TsObjectModelHandoff> {
    let mut handoffs = Vec::new();
    for binding in bindings {
        if binding.status != TsDirectBindingStatus::Unresolved
            || binding.reason != Some(TsDirectBindingReason::PropertyFlowRequired)
        {
            continue;
        }
        let Some(callsite_node) = graph.callsite_node(&binding.callsite_stable_key) else {
            continue;
        };
        handoffs.push(TsObjectModelHandoff {
            callsite_node,
            binding_stable_key: binding.stable_key.clone(),
            reason: TsDirectBindingReason::PropertyFlowRequired,
        });
    }
    handoffs.sort();
    handoffs.dedup();
    handoffs
}

fn is_resolved(status: &TsObjectModelStatus) -> bool {
    matches!(status, TsObjectModelStatus::Resolved)
}

#[derive(Debug)]
struct GraphIndex {
    callsite_node_by_key: BTreeMap<String, SemanticNodeId>,
    caller_node_by_callsite_key: BTreeMap<String, SemanticNodeId>,
    caller_node_by_callsite_node: BTreeMap<SemanticNodeId, SemanticNodeId>,
}

impl GraphIndex {
    fn build(db: &AnalysisDb) -> Self {
        let node_by_stable_key = db
            .semantic_nodes()
            .iter()
            .map(|node| (node.stable_key.clone(), node.id))
            .collect::<BTreeMap<_, _>>();
        let function_node_by_id = db
            .semantic_nodes()
            .iter()
            .filter_map(|node| {
                let NodeKind::Function(function) = node.kind else {
                    return None;
                };
                Some((function, node.id))
            })
            .collect::<BTreeMap<FunctionId, SemanticNodeId>>();

        let mut callsite_node_by_key = BTreeMap::new();
        let mut caller_node_by_callsite_key = BTreeMap::new();
        let mut caller_node_by_callsite_node = BTreeMap::new();
        for site in db.call_sites() {
            if !site.language.is_ts_family() {
                continue;
            }
            let key = node_key_from_identity("callsite", &site.stable_key);
            let Some(&node) = node_by_stable_key.get(&key) else {
                continue;
            };
            callsite_node_by_key.insert(site.stable_key.clone(), node);
            if let Some(&caller_node) = function_node_by_id.get(&site.caller) {
                caller_node_by_callsite_key.insert(site.stable_key.clone(), caller_node);
                caller_node_by_callsite_node.insert(node, caller_node);
            }
        }

        Self {
            callsite_node_by_key,
            caller_node_by_callsite_key,
            caller_node_by_callsite_node,
        }
    }

    fn callsite_node(&self, stable_key: &str) -> Option<SemanticNodeId> {
        self.callsite_node_by_key.get(stable_key).copied()
    }

    fn caller_node(&self, callsite_stable_key: &str) -> Option<SemanticNodeId> {
        self.caller_node_by_callsite_key
            .get(callsite_stable_key)
            .copied()
    }

    fn caller_node_for_callsite_node(
        &self,
        callsite_node: SemanticNodeId,
    ) -> Option<SemanticNodeId> {
        self.caller_node_by_callsite_node
            .get(&callsite_node)
            .copied()
    }
}

#[derive(Debug)]
struct ConstraintIndex<'a> {
    by_stable_key: BTreeMap<String, Vec<&'a ConstraintFact>>,
}

impl<'a> ConstraintIndex<'a> {
    fn build(constraints: &'a [ConstraintFact]) -> Self {
        let mut by_stable_key: BTreeMap<String, Vec<&ConstraintFact>> = BTreeMap::new();
        for constraint in constraints {
            by_stable_key
                .entry(constraint.stable_key.clone())
                .or_default()
                .push(constraint);
            if let Some(identity) = constraint_identity(&constraint.stable_key) {
                by_stable_key.entry(identity).or_default().push(constraint);
            }
        }
        Self { by_stable_key }
    }

    fn alloc(&self, stable_key: &str) -> Option<(SemanticNodeId, SemanticNodeId)> {
        self.by_stable_key
            .get(stable_key)?
            .iter()
            .find_map(|constraint| {
                let ConstraintKind::Alloc { dst, object } = constraint.kind else {
                    return None;
                };
                Some((dst, object))
            })
    }

    fn field_store(
        &self,
        write: &TsPropertyWriteFact,
    ) -> Option<(SemanticNodeId, String, SemanticNodeId)> {
        self.field_store_by_stable_key(&write.stable_key)
    }

    fn field_store_by_stable_key(
        &self,
        stable_key: &str,
    ) -> Option<(SemanticNodeId, String, SemanticNodeId)> {
        self.by_stable_key
            .get(stable_key)?
            .iter()
            .find_map(|constraint| {
                let ConstraintKind::FieldStore { base, field, src } = &constraint.kind else {
                    return None;
                };
                Some((*base, field.clone(), *src))
            })
    }

    fn copy_edge(&self, stable_key: &str) -> Option<(SemanticNodeId, SemanticNodeId)> {
        self.by_stable_key
            .get(stable_key)?
            .iter()
            .find_map(|constraint| {
                let ConstraintKind::CopyEdge { dst, src } = &constraint.kind else {
                    return None;
                };
                Some((*dst, *src))
            })
    }

    fn field_load(
        &self,
        read: &TsPropertyReadFact,
    ) -> Option<(SemanticNodeId, SemanticNodeId, String)> {
        self.by_stable_key
            .get(read.stable_key.as_str())?
            .iter()
            .find_map(|constraint| {
                let ConstraintKind::FieldLoad { dst, base, field } = &constraint.kind else {
                    return None;
                };
                Some((*dst, *base, field.clone()))
            })
    }

    fn call_constraint(&self, stable_key: &str) -> Option<SemanticNodeId> {
        self.by_stable_key
            .get(stable_key)?
            .iter()
            .find_map(|constraint| {
                let ConstraintKind::CallConstraint { callsite } = constraint.kind else {
                    return None;
                };
                Some(callsite)
            })
    }
}

fn constraint_identity(stable_key: &str) -> Option<String> {
    parse_length_prefixed_parts(stable_key).and_then(|parts| parts.get("identity").cloned())
}

fn is_this_expression_object_key(stable_key: &str) -> bool {
    let Some((prefix, parts)) = parse_length_prefixed_key(stable_key) else {
        return false;
    };
    prefix == "ts_object_expression"
        && parts
            .get("display")
            .is_some_and(|display| display == "this")
}

fn parse_length_prefixed_parts(stable_key: &str) -> Option<BTreeMap<String, String>> {
    parse_length_prefixed_key(stable_key).map(|(_, parts)| parts)
}

fn parse_length_prefixed_key(stable_key: &str) -> Option<(String, BTreeMap<String, String>)> {
    let mut cursor = 0;
    let (prefix, next) = parse_length_prefixed_token(stable_key, cursor)?;
    cursor = next;
    let mut parts = BTreeMap::new();

    while cursor < stable_key.len() {
        if stable_key.as_bytes().get(cursor) != Some(&b'|') {
            return None;
        }
        cursor += 1;
        let (label, next) = parse_length_prefixed_token(stable_key, cursor)?;
        cursor = next;
        if stable_key.as_bytes().get(cursor) != Some(&b'=') {
            return None;
        }
        cursor += 1;
        let (value, next) = parse_length_prefixed_token(stable_key, cursor)?;
        cursor = next;
        parts.insert(label, value);
    }

    Some((prefix, parts))
}

fn parse_length_prefixed_token(input: &str, start: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    let mut colon = start;
    while colon < bytes.len() && bytes[colon].is_ascii_digit() {
        colon += 1;
    }
    if colon == start || bytes.get(colon) != Some(&b':') {
        return None;
    }
    let len: usize = input[start..colon].parse().ok()?;
    let value_start = colon + 1;
    let value_end = value_start.checked_add(len)?;
    let value = input.get(value_start..value_end)?;
    Some((value.to_string(), value_end))
}

fn node_key_from_identity(node_kind: &str, identity: &str) -> String {
    semantic_stable_key(
        FactFamily::Scope,
        &[
            ("node_kind", node_kind.to_string()),
            ("identity", identity.to_string()),
        ],
    )
    .into_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_sorts_and_deduplicates_object_inputs() {
        let inputs = TsObjectModelInputs {
            allocations: vec![
                allocation("alloc:b", 2),
                allocation("alloc:a", 1),
                allocation("alloc:a", 1),
            ],
            property_writes: vec![
                write("write:b", 2),
                write("write:a", 1),
                write("write:a", 1),
            ],
            property_reads: vec![read("read:b", 2), read("read:a", 1), read("read:a", 1)],
            prototype_links: vec![
                prototype_link("proto:b", 2, 3),
                prototype_link("proto:a", 1, 2),
                prototype_link("proto:a", 1, 2),
            ],
            ..TsObjectModelInputs::default()
        }
        .normalized();

        assert_eq!(
            inputs
                .allocations
                .iter()
                .map(|allocation| allocation.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["alloc:a", "alloc:b"]
        );
        assert_eq!(
            inputs
                .property_writes
                .iter()
                .map(|write| write.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["write:a", "write:b"]
        );
        assert_eq!(
            inputs
                .property_reads
                .iter()
                .map(|read| read.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["read:a", "read:b"]
        );
        assert_eq!(
            inputs
                .prototype_links
                .iter()
                .map(|link| link.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["proto:a", "proto:b"]
        );
    }

    #[test]
    fn property_handoff_filter_keeps_only_property_flow_required() {
        let bindings = [
            unresolved_binding("property", TsDirectBindingReason::PropertyFlowRequired),
            unresolved_binding("prototype", TsDirectBindingReason::PrototypeModelRequired),
            unresolved_binding("this", TsDirectBindingReason::ThisModelRequired),
            unresolved_binding("token", TsDirectBindingReason::TokenFlowRequired),
        ];
        let graph = GraphIndex {
            callsite_node_by_key: BTreeMap::from([(
                "callsite:property".to_string(),
                SemanticNodeId(1),
            )]),
            caller_node_by_callsite_key: BTreeMap::new(),
            caller_node_by_callsite_node: BTreeMap::new(),
        };

        let handoffs = property_handoffs_from_direct_bindings(&graph, &bindings);

        assert_eq!(handoffs.len(), 1);
        assert_eq!(
            handoffs[0].reason,
            TsDirectBindingReason::PropertyFlowRequired
        );
        assert_eq!(handoffs[0].binding_stable_key, "binding:property");
    }

    fn allocation(stable_key: &str, node: u64) -> TsObjectAllocationInput {
        TsObjectAllocationInput {
            place_node: SemanticNodeId(100 + node),
            object_node: SemanticNodeId(node),
            stable_key: stable_key.to_string(),
        }
    }

    fn write(stable_key: &str, base: u64) -> TsObjectPropertyWrite {
        TsObjectPropertyWrite {
            base_object: SemanticNodeId(base),
            field: "static:target".to_string(),
            source_node: SemanticNodeId(9),
            stable_key: stable_key.to_string(),
        }
    }

    fn read(stable_key: &str, base: u64) -> TsObjectPropertyRead {
        TsObjectPropertyRead {
            base_object: SemanticNodeId(base),
            base_is_this: false,
            field: "static:target".to_string(),
            destination_node: SemanticNodeId(99),
            callsite_node: Some(SemanticNodeId(7)),
            caller_node: Some(SemanticNodeId(1)),
            callsite_stable_key: Some("callsite:target".to_string()),
            constraint_stable_key: stable_key.to_string(),
            stable_key: stable_key.to_string(),
        }
    }

    fn prototype_link(stable_key: &str, object: u64, prototype: u64) -> TsObjectPrototypeLink {
        TsObjectPrototypeLink {
            object: SemanticNodeId(object),
            prototype: SemanticNodeId(prototype),
            stable_key: stable_key.to_string(),
        }
    }

    fn unresolved_binding(suffix: &str, reason: TsDirectBindingReason) -> TsDirectBindingFact {
        TsDirectBindingFact {
            id: crate::analysis::ids::TsDirectBindingId(0),
            callsite: crate::analysis::ids::TsInventoryCallsiteId(1),
            callsite_stable_key: format!("callsite:{suffix}"),
            target_function: None,
            target_function_stable_key: None,
            scope_binding: None,
            scope_binding_stable_key: None,
            resolved_import: None,
            module_node: None,
            kind: crate::ts::binding::facts::TsDirectBindingKind::LocalFunction,
            status: TsDirectBindingStatus::Unresolved,
            reason: Some(reason),
            stable_key: format!("binding:{suffix}"),
        }
    }
}
