//! Closed JS/TS function-token input snapshot (JS-04).
//!
//! [`TsTokenInputs`] is built once from the already-populated [`AnalysisDb`] and then
//! owned by `TsTokensPolicy`. It reads stable TS inventory/direct-binding identities and
//! the stored `polint.semantic_graph` constraints, mapping everything onto existing
//! [`SemanticNodeId`] endpoints. No source text or run-local dense IDs are used as token
//! identity.

use std::collections::BTreeMap;

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::semantic_graph::build::collect_ts_direct_bindings;
use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
use crate::analysis::semantic_graph::facts::NodeKind;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, FileId, FunctionFact, Span};
use crate::ts::binding::facts::{
    TsDirectBindingFact, TsDirectBindingReason, TsDirectBindingStatus,
};

/// One function-token atom. The token's callable identity is the existing semantic
/// function node; the stable keys are contributing evidence for derived edges.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TsFunctionToken {
    pub(crate) function_node: SemanticNodeId,
    pub(crate) function_stable_key: String,
    pub(crate) source_stable_key: String,
}

/// One token-flow copy obligation `src -> dst`, sourced from a semantic graph
/// `CopyEdge` constraint.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TsTokenCopyEdge {
    pub(crate) dst: SemanticNodeId,
    pub(crate) src: SemanticNodeId,
    pub(crate) stable_key: String,
}

/// One JS/TS callsite whose callable token set can produce derived call edges.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TsTokenCallsite {
    pub(crate) caller_node: SemanticNodeId,
    pub(crate) callsite_node: SemanticNodeId,
    pub(crate) callsite_stable_key: String,
    pub(crate) constraint_stable_key: String,
}

/// An unresolved direct-binding row that is eligible for the function-token driver.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TsTokenHandoff {
    pub(crate) callsite_node: SemanticNodeId,
    pub(crate) binding_stable_key: String,
    pub(crate) reason: TsDirectBindingReason,
}

/// Closed input snapshot for the TS function-token policy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TsTokenInputs {
    pub(crate) function_tokens: BTreeMap<SemanticNodeId, TsFunctionToken>,
    pub(crate) copy_edges: Vec<TsTokenCopyEdge>,
    pub(crate) callsites: Vec<TsTokenCallsite>,
    pub(crate) handoffs: Vec<TsTokenHandoff>,
}

impl TsTokenInputs {
    /// Build the closed token snapshot from already-stored graph facts plus the same
    /// TS direct-binding projection consumed by `polint.semantic_graph`.
    pub(crate) fn from_db(db: &AnalysisDb) -> Self {
        let graph_index = GraphIndex::build(db);
        let direct_bindings = collect_ts_direct_bindings(db);

        let mut function_tokens = ts_function_tokens(db, &graph_index, &direct_bindings.bindings);
        seed_direct_binding_target_tokens(
            db,
            &graph_index,
            &direct_bindings.bindings,
            &mut function_tokens,
        );

        let copy_edges = token_copy_edges(db.semantic_constraints());
        let callsites = token_callsites(db, &graph_index);
        let handoffs =
            token_handoffs_from_direct_bindings(db, &graph_index, &direct_bindings.bindings);

        Self {
            function_tokens,
            copy_edges,
            callsites,
            handoffs,
        }
        .normalized()
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.function_tokens = self.function_tokens.into_iter().collect();
        self.copy_edges.sort();
        self.copy_edges.dedup();
        self.callsites.sort();
        self.callsites.dedup();
        self.handoffs.sort();
        self.handoffs.dedup();
        self
    }
}

fn ts_function_tokens(
    db: &AnalysisDb,
    graph_index: &GraphIndex,
    bindings: &[TsDirectBindingFact],
) -> BTreeMap<SemanticNodeId, TsFunctionToken> {
    let inventory_key_by_node = inventory_target_key_by_node(db, graph_index, bindings);
    db.functions()
        .iter()
        .filter(|function| function.language.is_ts_family())
        .filter_map(|function| {
            let node = graph_index.function_node(function)?;
            let node_stable_key = graph_index.node_stable_key(node)?;
            let function_stable_key = inventory_key_by_node
                .get(&node)
                .cloned()
                .unwrap_or_else(|| node_stable_key.clone());
            Some((
                node,
                TsFunctionToken {
                    function_node: node,
                    function_stable_key,
                    source_stable_key: node_stable_key,
                },
            ))
        })
        .collect()
}

fn seed_direct_binding_target_tokens(
    db: &AnalysisDb,
    graph_index: &GraphIndex,
    bindings: &[TsDirectBindingFact],
    tokens: &mut BTreeMap<SemanticNodeId, TsFunctionToken>,
) {
    for binding in bindings {
        if binding.status != TsDirectBindingStatus::Resolved {
            continue;
        }
        let Some(target_key) = binding.target_function_stable_key.as_deref() else {
            continue;
        };
        let Some(node) = function_node_for_inventory_key(db, graph_index, target_key) else {
            continue;
        };
        let source_stable_key = graph_index
            .node_stable_key(node)
            .unwrap_or_else(|| target_key.to_string());
        tokens.entry(node).or_insert_with(|| TsFunctionToken {
            function_node: node,
            function_stable_key: target_key.to_string(),
            source_stable_key,
        });
    }
}

fn inventory_target_key_by_node(
    db: &AnalysisDb,
    graph_index: &GraphIndex,
    bindings: &[TsDirectBindingFact],
) -> BTreeMap<SemanticNodeId, String> {
    let mut by_node = BTreeMap::new();
    for binding in bindings {
        let Some(target_key) = binding.target_function_stable_key.as_deref() else {
            continue;
        };
        let Some(node) = function_node_for_inventory_key(db, graph_index, target_key) else {
            continue;
        };
        by_node
            .entry(node)
            .or_insert_with(|| target_key.to_string());
    }
    by_node
}

fn token_copy_edges(constraints: &[ConstraintFact]) -> Vec<TsTokenCopyEdge> {
    let mut copy_edges = constraints
        .iter()
        .filter_map(|constraint| {
            let ConstraintKind::CopyEdge { dst, src } = &constraint.kind else {
                return None;
            };
            Some(TsTokenCopyEdge {
                dst: *dst,
                src: *src,
                stable_key: constraint.stable_key.clone(),
            })
        })
        .collect::<Vec<_>>();
    copy_edges.sort();
    copy_edges.dedup();
    copy_edges
}

fn token_callsites(db: &AnalysisDb, graph_index: &GraphIndex) -> Vec<TsTokenCallsite> {
    let callsite_constraints = db
        .semantic_constraints()
        .iter()
        .filter_map(|constraint| {
            let ConstraintKind::CallConstraint { callsite } = &constraint.kind else {
                return None;
            };
            Some((*callsite, constraint.stable_key.as_str()))
        })
        .collect::<BTreeMap<_, _>>();

    let mut callsites = Vec::new();
    for site in db.call_sites() {
        if !site.language.is_ts_family() {
            continue;
        }
        let Some(callsite_node) = graph_index.callsite_node_by_stable_key(&site.stable_key) else {
            continue;
        };
        let Some(&constraint_stable_key) = callsite_constraints.get(&callsite_node) else {
            continue;
        };
        let Some(caller) = graph_index.function_node_by_id(site.caller) else {
            continue;
        };
        callsites.push(TsTokenCallsite {
            caller_node: caller,
            callsite_node,
            callsite_stable_key: site.stable_key.clone(),
            constraint_stable_key: constraint_stable_key.to_string(),
        });
    }
    callsites.sort();
    callsites.dedup();
    callsites
}

fn token_handoffs_from_direct_bindings(
    db: &AnalysisDb,
    graph_index: &GraphIndex,
    bindings: &[TsDirectBindingFact],
) -> Vec<TsTokenHandoff> {
    let mut handoffs = Vec::new();
    for binding in bindings {
        let Some(reason) = binding.reason else {
            continue;
        };
        if binding.status != TsDirectBindingStatus::Unresolved
            || !reason.is_function_token_handoff()
        {
            continue;
        }
        let Some(callsite_node) =
            callsite_node_for_inventory_key(db, graph_index, &binding.callsite_stable_key)
        else {
            continue;
        };
        handoffs.push(TsTokenHandoff {
            callsite_node,
            binding_stable_key: binding.stable_key.clone(),
            reason,
        });
    }
    handoffs.sort();
    handoffs.dedup();
    handoffs
}

#[derive(Debug)]
struct GraphIndex {
    stable_key_by_node: BTreeMap<SemanticNodeId, String>,
    function_node_by_id: BTreeMap<crate::core::FunctionId, SemanticNodeId>,
    callsite_node_by_key: BTreeMap<String, SemanticNodeId>,
    file_by_relative_path: BTreeMap<String, FileId>,
}

impl GraphIndex {
    fn build(db: &AnalysisDb) -> Self {
        let node_by_stable_key = db
            .semantic_nodes()
            .iter()
            .map(|node| (node.stable_key.clone(), node.id))
            .collect::<BTreeMap<_, _>>();
        let stable_key_by_node = db
            .semantic_nodes()
            .iter()
            .map(|node| (node.id, node.stable_key.clone()))
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
            .collect();
        let callsite_node_by_key = db
            .call_sites()
            .iter()
            .filter_map(|site| {
                let key = node_key_from_identity("callsite", &site.stable_key);
                let node = node_by_stable_key.get(&key).copied()?;
                Some((site.stable_key.clone(), node))
            })
            .collect();
        let file_by_relative_path = db
            .files()
            .iter()
            .map(|file| (file.relative_path.clone(), file.id))
            .collect();

        Self {
            stable_key_by_node,
            function_node_by_id,
            callsite_node_by_key,
            file_by_relative_path,
        }
    }

    fn function_node(&self, function: &FunctionFact) -> Option<SemanticNodeId> {
        self.function_node_by_id.get(&function.id).copied()
    }

    fn function_node_by_id(&self, function: crate::core::FunctionId) -> Option<SemanticNodeId> {
        self.function_node_by_id.get(&function).copied()
    }

    fn callsite_node_by_stable_key(&self, stable_key: &str) -> Option<SemanticNodeId> {
        self.callsite_node_by_key.get(stable_key).copied()
    }

    fn node_stable_key(&self, node: SemanticNodeId) -> Option<String> {
        self.stable_key_by_node.get(&node).cloned()
    }
}

fn function_node_for_inventory_key(
    db: &AnalysisDb,
    graph_index: &GraphIndex,
    inventory_key: &str,
) -> Option<SemanticNodeId> {
    let identity = TsInventoryStableIdentity::parse(inventory_key)?;
    let file = *graph_index
        .file_by_relative_path
        .get(identity.file.as_str())?;
    db.functions()
        .iter()
        .find(|function| {
            function.language.is_ts_family()
                && function.file == file
                && function.span.start_byte == identity.start
                && function.span.end_byte == identity.end
                && identity
                    .display
                    .as_deref()
                    .is_none_or(|display| display == function.name)
        })
        .and_then(|function| graph_index.function_node(function))
}

fn callsite_node_for_inventory_key(
    db: &AnalysisDb,
    graph_index: &GraphIndex,
    inventory_key: &str,
) -> Option<SemanticNodeId> {
    let identity = TsInventoryStableIdentity::parse(inventory_key)?;
    let file = *graph_index
        .file_by_relative_path
        .get(identity.file.as_str())?;
    db.call_sites()
        .iter()
        .find(|site| {
            site.language.is_ts_family()
                && site.file == file
                && site.span.start_byte == identity.start
                && site.span.end_byte == identity.end
        })
        .and_then(|site| graph_index.callsite_node_by_stable_key(&site.stable_key))
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

#[allow(
    dead_code,
    reason = "kept in lockstep with semantic_graph::build for tests"
)]
fn function_node_key(db: &AnalysisDb, function: &FunctionFact) -> String {
    let path = db.path_for(function.file);
    semantic_stable_key(
        FactFamily::Function,
        &[
            ("node_kind", "function".to_string()),
            ("path", path),
            ("name", function.name.clone()),
            ("span", span_identity(&function.span)),
        ],
    )
    .into_string()
}

fn span_identity(span: &Span) -> String {
    format!("{}:{}..{}", span.file.0, span.start_byte, span.end_byte)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TsInventoryStableIdentity {
    file: String,
    start: u32,
    end: u32,
    display: Option<String>,
}

impl TsInventoryStableIdentity {
    fn parse(stable_key: &str) -> Option<Self> {
        let parts = parse_length_prefixed_parts(stable_key)?;
        Some(Self {
            file: parts.get("file")?.clone(),
            start: parts.get("start")?.parse().ok()?,
            end: parts.get("end")?.parse().ok()?,
            display: parts.get("display").cloned(),
        })
    }
}

fn parse_length_prefixed_parts(stable_key: &str) -> Option<BTreeMap<String, String>> {
    let mut cursor = 0;
    let (_prefix, next) = parse_length_prefixed_token(stable_key, cursor)?;
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

    Some(parts)
}

fn parse_length_prefixed_token(input: &str, start: usize) -> Option<(String, usize)> {
    let colon = input[start..].find(':')? + start;
    let len = input[start..colon].parse::<usize>().ok()?;
    let value_start = colon + 1;
    let value_end = value_start.checked_add(len)?;
    let value = input.get(value_start..value_end)?.to_string();
    Some((value, value_end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{SemanticConstraintId, TsDirectBindingId, TsInventoryCallsiteId};
    use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
    use crate::core::{ModuleNodeId, ResolvedImportId};
    use crate::ts::binding::facts::{TsDirectBindingKind, TsDirectBindingStatus};

    #[test]
    fn normalized_sorts_and_deduplicates_inputs_by_stable_key() {
        let inputs = TsTokenInputs {
            copy_edges: vec![
                copy_edge("copy:b", 2, 3),
                copy_edge("copy:a", 1, 2),
                copy_edge("copy:a", 1, 2),
            ],
            callsites: vec![
                callsite("call:b", "constraint:b", 2),
                callsite("call:a", "constraint:a", 1),
            ],
            ..TsTokenInputs::default()
        }
        .normalized();

        assert_eq!(
            inputs
                .copy_edges
                .iter()
                .map(|edge| edge.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["copy:a", "copy:b"]
        );
        assert_eq!(
            inputs
                .callsites
                .iter()
                .map(|site| site.callsite_stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["call:a", "call:b"]
        );
    }

    #[test]
    fn token_handoff_filter_keeps_only_token_flow_required_reasons() {
        let bindings = [
            unresolved_binding("token", TsDirectBindingReason::TokenFlowRequired),
            unresolved_binding("property", TsDirectBindingReason::PropertyFlowRequired),
            unresolved_binding("prototype", TsDirectBindingReason::PrototypeModelRequired),
            unresolved_binding("this", TsDirectBindingReason::ThisModelRequired),
        ];

        let kept = bindings
            .iter()
            .filter_map(|binding| binding.reason)
            .filter(|reason| reason.is_function_token_handoff())
            .map(TsDirectBindingReason::as_str)
            .collect::<Vec<_>>();

        assert_eq!(kept, vec!["token_flow_required"]);
    }

    #[test]
    fn copy_edge_inputs_are_extracted_from_semantic_constraints_in_stable_order() {
        let edges = token_copy_edges(&[
            constraint("copy:z", 3, 4),
            constraint("call:ignored", 0, 0),
            constraint("copy:a", 1, 2),
        ]);

        assert_eq!(
            edges
                .iter()
                .map(|edge| (edge.stable_key.as_str(), edge.src.0, edge.dst.0))
                .collect::<Vec<_>>(),
            vec![("copy:a", 1, 2), ("copy:z", 3, 4)]
        );
    }

    fn copy_edge(stable_key: &str, src: u64, dst: u64) -> TsTokenCopyEdge {
        TsTokenCopyEdge {
            dst: SemanticNodeId(dst),
            src: SemanticNodeId(src),
            stable_key: stable_key.to_string(),
        }
    }

    fn callsite(stable_key: &str, constraint_key: &str, node: u64) -> TsTokenCallsite {
        TsTokenCallsite {
            caller_node: SemanticNodeId(100 + node),
            callsite_node: SemanticNodeId(node),
            callsite_stable_key: stable_key.to_string(),
            constraint_stable_key: constraint_key.to_string(),
        }
    }

    fn constraint(stable_key: &str, src: u64, dst: u64) -> ConstraintFact {
        let kind = if stable_key.starts_with("copy:") {
            ConstraintKind::CopyEdge {
                dst: SemanticNodeId(dst),
                src: SemanticNodeId(src),
            }
        } else {
            ConstraintKind::CallConstraint {
                callsite: SemanticNodeId(9),
            }
        };
        ConstraintFact {
            id: SemanticConstraintId(0),
            kind,
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: stable_key.to_string(),
        }
    }

    fn unresolved_binding(suffix: &str, reason: TsDirectBindingReason) -> TsDirectBindingFact {
        TsDirectBindingFact {
            id: TsDirectBindingId(0),
            callsite: TsInventoryCallsiteId(1),
            callsite_stable_key: format!("callsite:{suffix}"),
            target_function: None,
            target_function_stable_key: None,
            scope_binding: None,
            scope_binding_stable_key: None,
            resolved_import: Some(ResolvedImportId(1)),
            module_node: Some(ModuleNodeId(1)),
            kind: TsDirectBindingKind::LocalFunction,
            status: TsDirectBindingStatus::Unresolved,
            reason: Some(reason),
            stable_key: format!("binding:{suffix}"),
        }
    }
}
