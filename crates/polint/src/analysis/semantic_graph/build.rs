//! Population pass for `polint.semantic_graph` (GRAPH-02).
//!
//! [`build_semantic_graph`] projects a real-but-minimal semantic graph from the
//! already-available v1.2 fact families — functions, packages, scopes, call sites,
//! and values — emitting nodes, `Call`/`MemberOf` edges, and
//! `CopyEdge`/`CallConstraint` constraints. It is a **read-only projection**: it
//! reads `db` and returns a fresh [`SemanticGraphOutput`], mutating no upstream fact
//! family (D-13). All node/edge/constraint stable keys are composed from the
//! referenced existing stable identity via the length-prefixed recipe, never
//! run-local dense IDs (D-06); the dense IDs are assigned later by `normalized()`.
//!
//! Honesty boundaries (D-07/D-11) — these kinds emit **ZERO** rows in this minimal
//! pass, documented rather than fabricated:
//! - `ModelEdge`: there is no producer until Phase 49 (ADAPT-01). Reserved emptiness.
//! - `Alloc`: the abstract-object endpoint is a `NodeKind::AbstractObject(ObjectTokenId)`,
//!   but `ValueFact` allocations carry an `AllocationTokenId` (a distinct identity
//!   family). There is no honest 1:1 bridge at this layer, so `Alloc` emission is
//!   deferred to a later plan that wires the object-token bridge instead of
//!   fabricating an endpoint to inflate recall.
//! - `FieldLoad`/`FieldStore`: an `AccessPathFact` expresses a `base.field`
//!   projection but carries no distinct *destination* place identity, so emitting a
//!   field constraint would require fabricating a destination node. Deferred.
//! - `TypeConstraint`: not emitted in this minimal pass.

use std::collections::BTreeMap;

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
use crate::analysis::semantic_graph::facts::{
    EdgeKind, NodeKind, SemanticEdgeFact, SemanticNodeFact, SemanticPrecision,
};
use crate::analysis::semantic_graph::store::SemanticGraphOutput;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis::values::facts::{ValueKind, ValueSubject};
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, Span};

/// Projects a real-but-minimal semantic graph from already-stored v1.2 facts.
///
/// Reads `db` only; the returned [`SemanticGraphOutput`] is the sole new artifact
/// (D-13). Dense IDs are NOT assigned here — every node/edge/constraint is emitted
/// with the default dense id and a composed stable key; the caller runs
/// `normalized()` (and `SemanticGraphStore::from_output`) to sort by stable key and
/// assign dense IDs (D-05).
pub(crate) fn build_semantic_graph(db: &AnalysisDb) -> SemanticGraphOutput {
    let mut builder = GraphBuilder::default();

    builder.project_nodes(db);
    builder.project_call_edges_and_constraints(db);
    builder.project_member_of_edges(db);
    builder.project_value_constraints(db);
    // FieldLoad/FieldStore, TypeConstraint, and ModelEdge intentionally emit zero
    // rows in this minimal pass (see module docs / D-07 / D-11). The base places for
    // access-path projections are already projected as nodes above.

    builder.into_output()
}

/// Accumulates pre-normalization nodes/edges/constraints plus the lookup maps that
/// translate a referenced domain identity to its pre-sort [`SemanticNodeId`]. The
/// pre-sort IDs are just insertion handles; `normalized()` reassigns the persisted
/// dense IDs after the stable-key sort.
#[derive(Default)]
struct GraphBuilder {
    nodes: Vec<SemanticNodeFact>,
    edges: Vec<SemanticEdgeFact>,
    constraints: Vec<ConstraintFact>,
    next_node: u64,
    /// node stable key -> pre-sort SemanticNodeId, so a node identity is projected
    /// at most once and edges/constraints can resolve their endpoints.
    node_by_key: BTreeMap<String, SemanticNodeId>,
}

impl GraphBuilder {
    /// Intern a node by its composed stable key, returning the (stable) pre-sort id.
    /// Idempotent: a repeated identity reuses the existing node.
    fn intern_node(&mut self, kind: NodeKind, stable_key: String) -> SemanticNodeId {
        if let Some(&id) = self.node_by_key.get(&stable_key) {
            return id;
        }
        let id = SemanticNodeId(self.next_node);
        self.next_node += 1;
        self.node_by_key.insert(stable_key.clone(), id);
        self.nodes.push(SemanticNodeFact {
            id,
            kind,
            // Conservative: graph rows are derived/aggregated, never `ResolvedStatic`
            // (the Exact-equivalent ceiling). Plan 03 validates the ceiling.
            precision: SemanticPrecision::Conservative,
            stable_key,
        });
        id
    }

    fn push_edge(&mut self, kind: EdgeKind, source: SemanticNodeId, target: SemanticNodeId) {
        // Edge stable key is composed from (edge-kind label, source node stable key,
        // target node stable key) — never dense IDs (D-06). Resolve the endpoint
        // stable keys from the interned node table.
        let source_key = self.node_key_for(source);
        let target_key = self.node_key_for(target);
        let stable_key = semantic_stable_key(
            FactFamily::Reference,
            &[
                ("edge_kind", kind.as_str().to_string()),
                ("source", source_key),
                ("target", target_key),
            ],
        )
        .into_string();
        self.edges.push(SemanticEdgeFact {
            id: Default::default(),
            source,
            target,
            kind,
            precision: SemanticPrecision::Conservative,
            stable_key,
        });
    }

    fn push_constraint(&mut self, kind: ConstraintKind, identity: &str) {
        let stable_key = semantic_stable_key(
            FactFamily::PointsToConstraint,
            &[
                ("constraint_kind", kind.as_str().to_string()),
                ("identity", identity.to_string()),
            ],
        )
        .into_string();
        self.constraints.push(ConstraintFact {
            id: Default::default(),
            kind,
            status: PointsToStatus::Present,
            // FlowInsensitive: these are projected, not solved (the solver lands in
            // Phase 47); never an exact precision.
            precision: PointsToPrecision::FlowInsensitive,
            stable_key,
        });
    }

    fn node_key_for(&self, id: SemanticNodeId) -> String {
        self.node_by_key
            .iter()
            .find_map(|(key, &node_id)| (node_id == id).then(|| key.clone()))
            .unwrap_or_default()
    }

    // -- node projection ----------------------------------------------------

    fn project_nodes(&mut self, db: &AnalysisDb) {
        for function in db.functions() {
            let key = function_node_key(db, function);
            self.intern_node(NodeKind::Function(function.id), key);
        }
        for package in db.packages() {
            let key = package_node_key(db, package);
            self.intern_node(NodeKind::Package(package.id), key);
        }
        for scope in db.scopes() {
            let key = node_key_from_identity("scope", &scope.stable_key);
            self.intern_node(NodeKind::Scope(scope.id), key);
        }
        for site in db.call_sites() {
            let key = node_key_from_identity("callsite", &site.stable_key);
            self.intern_node(NodeKind::Callsite(site.id), key);
        }
    }

    fn function_node(
        &self,
        db: &AnalysisDb,
        function: &crate::core::FunctionFact,
    ) -> SemanticNodeId {
        let key = function_node_key(db, function);
        self.node_by_key
            .get(&key)
            .copied()
            .unwrap_or(SemanticNodeId(u64::MAX))
    }

    // -- call edges + call constraints --------------------------------------

    fn project_call_edges_and_constraints(&mut self, db: &AnalysisDb) {
        // Map FunctionId -> its node id by walking functions once.
        let func_node: BTreeMap<_, _> = db
            .functions()
            .iter()
            .map(|function| (function.id, self.function_node(db, function)))
            .collect();

        for site in db.call_sites() {
            let site_key = node_key_from_identity("callsite", &site.stable_key);
            let Some(&callsite_node) = self.node_by_key.get(&site_key) else {
                continue;
            };
            // Call edge: caller function node -> callsite node, only where the caller
            // resolves to a projected function node (honest: no fabricated endpoints).
            if let Some(&caller_node) = func_node.get(&site.caller)
                && caller_node != SemanticNodeId(u64::MAX)
            {
                self.push_edge(EdgeKind::Call, caller_node, callsite_node);
            }
            // CallConstraint anchored at the callsite node — one per call site.
            self.push_constraint(
                ConstraintKind::CallConstraint {
                    callsite: callsite_node,
                },
                &site.stable_key,
            );
        }
    }

    // -- member-of edges ----------------------------------------------------

    fn project_member_of_edges(&mut self, db: &AnalysisDb) {
        // Scope-in-package containment: scope node -> package node, where both the
        // scope and its package are projected nodes.
        let pkg_node: BTreeMap<_, _> = db
            .packages()
            .iter()
            .filter_map(|package| {
                let key = package_node_key(db, package);
                self.node_by_key.get(&key).map(|&id| (package.id, id))
            })
            .collect();

        for scope in db.scopes() {
            let Some(package_id) = scope.package else {
                continue;
            };
            let scope_key = node_key_from_identity("scope", &scope.stable_key);
            let Some(&scope_node) = self.node_by_key.get(&scope_key) else {
                continue;
            };
            if let Some(&package_node) = pkg_node.get(&package_id) {
                self.push_edge(EdgeKind::MemberOf, scope_node, package_node);
            }
        }
    }

    // -- value-derived constraints (CopyEdge) -------------------------------
    //
    // `Alloc` constraints are NOT emitted here: the abstract-object endpoint is a
    // `NodeKind::AbstractObject(ObjectTokenId)`, but `ValueFact` allocations carry an
    // `AllocationTokenId` (a distinct identity family with no honest 1:1 mapping at
    // this layer). Synthesizing an object node from an allocation token would
    // fabricate an endpoint, so real `Alloc` emission is deferred to a later plan
    // that wires the object-token bridge (D-07 honesty), rather than inflating recall.

    fn project_value_constraints(&mut self, db: &AnalysisDb) {
        for value in db.value_facts() {
            // Only project where the value's subject is a concrete place — that is the
            // `dst` of the copy relationship.
            let ValueSubject::Place(dst_place) = value.subject else {
                continue;
            };

            // `dst <- src`: a value copy from another concrete place. Both endpoints
            // are real `Place` nodes; nothing is fabricated.
            let src_place = match &value.kind {
                ValueKind::PlaceRef(src) | ValueKind::CallReturn(src) => *src,
                _ => continue,
            };
            let dst_key = place_node_key(&value.stable_key, "subject");
            let dst_node = self.intern_node(NodeKind::Place(dst_place), dst_key);
            let src_key = place_node_key(&value.stable_key, "source");
            let src_node = self.intern_node(NodeKind::Place(src_place), src_key);
            self.push_constraint(
                ConstraintKind::CopyEdge {
                    dst: dst_node,
                    src: src_node,
                },
                &value.stable_key,
            );
        }
    }

    fn into_output(self) -> SemanticGraphOutput {
        SemanticGraphOutput {
            nodes: self.nodes,
            edges: self.edges,
            constraints: self.constraints,
        }
    }
}

// ---------------------------------------------------------------------------
// Stable-key composition helpers (D-06: composed from referenced identity)
// ---------------------------------------------------------------------------

/// Function node key = (node-kind label, function stable identity: path + name +
/// span), composed via the length-prefixed recipe.
fn function_node_key(db: &AnalysisDb, function: &crate::core::FunctionFact) -> String {
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

fn package_node_key(db: &AnalysisDb, package: &crate::core::PackageFact) -> String {
    let path = db.path_for(package.file);
    semantic_stable_key(
        FactFamily::Package,
        &[
            ("node_kind", "package".to_string()),
            ("path", path),
            ("name", package.name.clone()),
        ],
    )
    .into_string()
}

/// Node key for an entity that already carries its own composed stable key (scopes,
/// call sites). The node-kind label disambiguates kinds that might otherwise share a
/// referenced identity string.
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

/// Place node key derived from the owning fact's stable key plus a role label, so the
/// `subject`/`source` place of the same value fact map to distinct nodes without
/// fabricating dense place IDs as identity.
fn place_node_key(owner_stable_key: &str, role: &str) -> String {
    semantic_stable_key(
        FactFamily::Place,
        &[
            ("node_kind", "place".to_string()),
            ("owner", owner_stable_key.to_string()),
            ("role", role.to_string()),
        ],
    )
    .into_string()
}

fn span_identity(span: &Span) -> String {
    format!("{}:{}..{}", span.file.0, span.start_byte, span.end_byte)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::analysis::calls::facts::{
        CallCallee, CallPrecision, CallSiteFact, CallSyntaxKind, CallTargetStatus,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId};
    use crate::analysis::semantic_graph::store::SemanticGraphStore;
    use crate::core::{
        AnalysisDb, FileId, FunctionFact, FunctionId, Language, PackageFact, PackageId, Span,
    };

    fn span(file: FileId) -> Span {
        Span {
            file,
            start_byte: 0,
            end_byte: 10,
            start_line: 1,
            start_col: 0,
            end_line: 1,
            end_col: 10,
        }
    }

    /// Builds a tiny in-memory db with one package, one function, and one call site
    /// (the function calling something) — enough to exercise >=1 node, >=1 Call edge,
    /// and >=1 CallConstraint without running the full language pipeline.
    fn fixture_db() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("main.go"),
            "main.go".to_string(),
            "package main\nfunc main() { run() }\n".to_string(),
        );
        db.push_package(PackageFact {
            id: PackageId(0),
            file,
            name: "main".to_string(),
            span: span(file),
            language: Language::Go,
        });
        let caller = db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "main".to_string(),
            span: span(file),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: vec!["run".to_string()],
        });

        let site = CallSiteFact {
            id: CallSiteId(0),
            language: Language::Go,
            file,
            caller,
            owner_symbol: None,
            body: MirBodyId(0),
            operation: MirOpId(0),
            span: span(file),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: "run".to_string(),
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::SetupAware,
            stable_key: "call-site:main:run".to_string(),
        };
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call replace");

        db
    }

    #[test]
    fn build_emits_nodes_call_edge_and_call_constraint_with_zero_model_edges() {
        let db = fixture_db();
        let output = build_semantic_graph(&db);

        // At least one node was projected from real facts.
        assert!(!output.nodes.is_empty(), "expected projected nodes");
        // A function node and a callsite node are present.
        assert!(
            output
                .nodes
                .iter()
                .any(|n| matches!(n.kind, NodeKind::Function(_))),
            "expected a function node"
        );
        assert!(
            output
                .nodes
                .iter()
                .any(|n| matches!(n.kind, NodeKind::Callsite(_))),
            "expected a callsite node"
        );

        // >=1 Call edge and >=1 CallConstraint from the call site.
        let call_edges = output
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Call)
            .count();
        assert!(call_edges >= 1, "expected >=1 Call edge, got {call_edges}");

        let call_constraints = output
            .constraints
            .iter()
            .filter(|c| matches!(c.kind, ConstraintKind::CallConstraint { .. }))
            .count();
        assert!(
            call_constraints >= 1,
            "expected >=1 CallConstraint, got {call_constraints}"
        );

        // EXACTLY zero ModelEdge constraints (honest emptiness, D-11).
        let model_edges = output
            .constraints
            .iter()
            .filter(|c| matches!(c.kind, ConstraintKind::ModelEdge))
            .count();
        assert_eq!(model_edges, 0, "ModelEdge must emit zero rows");

        // No precision label is the exact ceiling — graph rows are conservative.
        assert!(
            output
                .nodes
                .iter()
                .all(|n| n.precision == SemanticPrecision::Conservative)
        );

        // The projected graph passes referential validation end-to-end: every edge
        // endpoint and every constraint node reference resolves to a stored node.
        let store = SemanticGraphStore::from_output(output).expect("graph validates");
        assert!(!store.nodes().is_empty());
    }

    #[test]
    fn build_is_read_only_and_deterministic() {
        let db = fixture_db();
        // Building twice over the same db yields byte-identical normalized output —
        // the projection mutates nothing and is order-independent.
        let a = build_semantic_graph(&db).normalized();
        let b = build_semantic_graph(&db).normalized();
        let a_json = serde_json::to_string(&a.constraints).expect("serialize a");
        let b_json = serde_json::to_string(&b.constraints).expect("serialize b");
        assert_eq!(a_json, b_json);
        let a_nodes = serde_json::to_string(&a.nodes).expect("serialize a nodes");
        let b_nodes = serde_json::to_string(&b.nodes).expect("serialize b nodes");
        assert_eq!(a_nodes, b_nodes);

        // Building does not mutate any upstream fact family (D-13): the db's call
        // sites and functions are unchanged after a build.
        let before_sites = db.call_sites().len();
        let before_funcs = db.functions().len();
        let _ = build_semantic_graph(&db);
        assert_eq!(db.call_sites().len(), before_sites);
        assert_eq!(db.functions().len(), before_funcs);
    }
}
