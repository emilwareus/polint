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

use std::collections::{BTreeMap, btree_map::Entry};

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{Argument, BindingPattern, Expression, Function, Statement, VariableDeclarator};
use oxc_parser::Parser;
use oxc_semantic::{AstNodes, NodeId, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};

use crate::analysis::ids::{PlaceId, SemanticNodeId};
use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
use crate::analysis::semantic_graph::facts::{
    EdgeKind, NodeKind, SemanticEdgeFact, SemanticNodeFact, SemanticPrecision,
};
use crate::analysis::semantic_graph::store::SemanticGraphOutput;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis::values::facts::{ValueKind, ValueSubject};
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, FileId, SourceFile, Span};
use crate::go::semantic::facts::{GoSemanticCallStatus, GoSemanticCallsiteFact};
use crate::ts::binding::direct::{
    TsDirectBindingModuleFile, TsDirectBindingModuleInput, resolve_direct_bindings_with_modules,
};
use crate::ts::binding::facts::{TsDirectBindingFact, TsDirectBindingKind, TsDirectBindingStatus};
use crate::ts::binding::store::TsDirectBindingOutput;
use crate::ts::inventory::extract::extract_ts_inventory;
use crate::ts::inventory::store::TsInventoryOutput;
use crate::ts::scope::extract::extract_ts_scope;
use crate::ts::scope::store::TsScopeOutput;

/// Projects a real-but-minimal semantic graph from already-stored v1.2 facts.
///
/// Reads `db` only; the returned [`SemanticGraphOutput`] is the sole new artifact
/// (D-13). Dense IDs are NOT assigned here — every node/edge/constraint is emitted
/// with the default dense id and a composed stable key; the caller runs
/// `normalized()` (and `SemanticGraphStore::from_output`) to sort by stable key and
/// assign dense IDs (D-05).
pub(crate) fn build_semantic_graph(db: &AnalysisDb) -> SemanticGraphOutput {
    let ts_direct_bindings = collect_ts_direct_bindings(db);
    build_semantic_graph_with_ts_direct_bindings(db, &ts_direct_bindings.bindings)
}

pub(crate) fn build_semantic_graph_with_ts_direct_bindings(
    db: &AnalysisDb,
    ts_direct_bindings: &[TsDirectBindingFact],
) -> SemanticGraphOutput {
    let mut builder = GraphBuilder::default();

    builder.project_nodes(db);
    builder.project_call_edges_and_constraints(db);
    builder.project_go_semantic(db);
    builder.project_ts_direct_bindings(db, ts_direct_bindings);
    builder.project_ts_token_source_flows(db);
    builder.project_member_of_edges(db);
    builder.project_value_constraints(db);
    // FieldLoad/FieldStore, TypeConstraint, and ModelEdge intentionally emit zero
    // rows in this minimal pass (see module docs / D-07 / D-11). The base places for
    // access-path projections are already projected as nodes above.

    builder.into_output()
}

struct TsFileAnalysis {
    file: FileId,
    inventory: TsInventoryOutput,
    scope: TsScopeOutput,
}

pub(crate) fn collect_ts_direct_bindings(db: &AnalysisDb) -> TsDirectBindingOutput {
    let analyses = db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
        .map(|file| TsFileAnalysis {
            file: file.id,
            inventory: extract_ts_inventory(file),
            scope: extract_ts_scope(file),
        })
        .collect::<Vec<_>>();
    if analyses.is_empty() {
        return TsDirectBindingOutput::default();
    }

    let analysis_by_file = analyses
        .iter()
        .enumerate()
        .map(|(index, analysis)| (analysis.file, index))
        .collect::<BTreeMap<_, _>>();
    let module_files = db
        .module_nodes()
        .iter()
        .filter_map(|node| {
            let file = node.file?;
            let analysis = analyses.get(*analysis_by_file.get(&file)?)?;
            Some(TsDirectBindingModuleFile {
                module_node: node.id,
                inventory: &analysis.inventory,
                scope: &analysis.scope,
            })
        })
        .collect::<Vec<_>>();
    let module_input = TsDirectBindingModuleInput {
        imports: db.imports(),
        resolved_imports: db.resolved_imports(),
        module_nodes: db.module_nodes(),
        module_files: &module_files,
    };

    let mut bindings = Vec::new();
    for analysis in &analyses {
        let output = resolve_direct_bindings_with_modules(
            &analysis.inventory,
            &analysis.scope,
            &module_input,
        );
        bindings.extend(output.bindings);
    }

    TsDirectBindingOutput { bindings }.normalized()
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
    /// pre-sort SemanticNodeId (as index) -> node stable key, the O(1) reverse of
    /// `node_by_key` so `node_key_for` (called twice per edge) never linear-scans.
    key_by_node: Vec<String>,
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
        // key_by_node is index-aligned with the sequential pre-sort id, so id.0 ==
        // key_by_node.len() at the moment of the push.
        debug_assert_eq!(id.0 as usize, self.key_by_node.len());
        self.key_by_node.push(stable_key.clone());
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

    /// Resolves a pre-sort node id back to its stable key in O(1) via the
    /// index-aligned `key_by_node` sidecar. `push_edge` only ever receives ids
    /// returned by `intern_node`, so the index is always in range; an out-of-range
    /// id is a builder invariant violation, not an expected empty-key fallback.
    fn node_key_for(&self, id: SemanticNodeId) -> String {
        self.key_by_node
            .get(id.0 as usize)
            .cloned()
            .expect("edge endpoint must reference an interned node")
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
    ) -> Option<SemanticNodeId> {
        let key = function_node_key(db, function);
        self.node_by_key.get(&key).copied()
    }

    // -- call edges + call constraints --------------------------------------

    fn project_call_edges_and_constraints(&mut self, db: &AnalysisDb) {
        // Map FunctionId -> its node id by walking functions once. Functions that
        // were not interned as nodes are simply absent (no sentinel).
        let func_node: BTreeMap<_, _> = db
            .functions()
            .iter()
            .filter_map(|function| {
                self.function_node(db, function)
                    .map(|node| (function.id, node))
            })
            .collect();

        for site in db.call_sites() {
            let site_key = node_key_from_identity("callsite", &site.stable_key);
            let Some(&callsite_node) = self.node_by_key.get(&site_key) else {
                continue;
            };
            // Call edge: caller function node -> callsite node, only where the caller
            // resolves to a projected function node (honest: no fabricated endpoints).
            if let Some(&caller_node) = func_node.get(&site.caller) {
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

    // -- TS direct binding constraints -------------------------------------

    fn project_ts_direct_bindings(&mut self, db: &AnalysisDb, bindings: &[TsDirectBindingFact]) {
        // Phase 45 semantic-graph-only projection: TS direct binding rows emit
        // CopyEdge/CallConstraint rows here and do not create CallTargetFact rows.
        // The calls/refined-calls contract remains owned by analysis::calls until a
        // later plan deliberately promotes direct TS bindings into that fact family.
        let context = TsDirectBindingNodeContext::new(db);
        for binding in bindings {
            if binding.status != TsDirectBindingStatus::Resolved {
                continue;
            }
            let Some(callsite_node) =
                context.callsite_node_for_binding(self, &binding.callsite_stable_key)
            else {
                continue;
            };
            self.push_constraint(
                ConstraintKind::CallConstraint {
                    callsite: callsite_node,
                },
                &binding.stable_key,
            );

            let Some(target_key) = binding.target_function_stable_key.as_deref() else {
                continue;
            };
            let Some(target_node) = context.function_node_for_binding(self, target_key) else {
                continue;
            };
            if direct_binding_emits_copy_edge(binding.kind) {
                self.push_constraint(
                    ConstraintKind::CopyEdge {
                        dst: callsite_node,
                        src: target_node,
                    },
                    &binding.stable_key,
                );
            }
        }
    }

    fn project_ts_token_source_flows(&mut self, db: &AnalysisDb) {
        let context = TsDirectBindingNodeContext::new(db);
        for file in db
            .files()
            .iter()
            .filter(|file| file.language.is_ts_family())
        {
            for flow in collect_ts_token_source_flows(file) {
                let Some(source_node) =
                    context.function_node_for_binding(self, &flow.source_function_stable_key)
                else {
                    continue;
                };
                let Some(callsite_node) =
                    context.callsite_node_for_binding(self, &flow.callsite_stable_key)
                else {
                    continue;
                };
                self.push_constraint(
                    ConstraintKind::CopyEdge {
                        dst: callsite_node,
                        src: source_node,
                    },
                    &flow.stable_key,
                );
            }
        }
    }

    fn project_go_semantic(&mut self, db: &AnalysisDb) {
        let function_node_by_qualified = db
            .go_semantic_functions()
            .iter()
            .filter_map(|semantic_function| {
                let function = matching_core_function(db, semantic_function)?;
                let node = self.function_node(db, function)?;
                Some((semantic_function.qualified.as_str(), node))
            })
            .collect::<BTreeMap<_, _>>();

        for callsite in db.go_semantic_callsites() {
            let Some(core_callsite) = matching_core_callsite(db, callsite) else {
                continue;
            };
            let site_key = node_key_from_identity("callsite", &core_callsite.stable_key);
            let Some(&callsite_node) = self.node_by_key.get(&site_key) else {
                continue;
            };
            self.push_constraint(
                ConstraintKind::CallConstraint {
                    callsite: callsite_node,
                },
                &callsite.stable_key,
            );
            if callsite.status != GoSemanticCallStatus::ResolvedStatic {
                continue;
            }
            let Some(static_callee) = callsite.static_callee.as_deref() else {
                continue;
            };
            let Some(&target_node) = function_node_by_qualified.get(static_callee) else {
                continue;
            };
            self.push_constraint(
                ConstraintKind::CopyEdge {
                    dst: callsite_node,
                    src: target_node,
                },
                &callsite.stable_key,
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
        // `PlaceId` -> the place's own content-stable key. Keying a Place node by the
        // *place* identity (not the owning value fact) means the SAME place referenced
        // by different value facts maps to ONE node, so the Phase 47 solver can unify
        // copy chains through a shared place. A place absent from the MIR place table
        // has no honest stable identity at this layer, so its copy is skipped rather
        // than fabricated (D-07), mirroring the Alloc/Field deferrals.
        let place_key_by_id: BTreeMap<PlaceId, &str> = db
            .mir_places()
            .iter()
            .map(|place| (place.id, place.stable_key.as_str()))
            .collect();

        for value in db.value_facts() {
            // Only project where the value's subject is a concrete place — that is the
            // `dst` of the copy relationship.
            let ValueSubject::Place(dst_place) = value.subject else {
                continue;
            };

            // `dst <- src`: a value copy from another concrete place.
            let src_place = match &value.kind {
                ValueKind::PlaceRef(src) | ValueKind::CallReturn(src) => *src,
                _ => continue,
            };

            // Both endpoints must resolve to a place with an honest, shared stable
            // identity; otherwise skip rather than fabricate a per-fact place node.
            let (Some(&dst_stable), Some(&src_stable)) = (
                place_key_by_id.get(&dst_place),
                place_key_by_id.get(&src_place),
            ) else {
                continue;
            };

            let dst_node = self.intern_node(NodeKind::Place(dst_place), place_node_key(dst_stable));
            let src_node = self.intern_node(NodeKind::Place(src_place), place_node_key(src_stable));
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

fn direct_binding_emits_copy_edge(kind: TsDirectBindingKind) -> bool {
    matches!(
        kind,
        TsDirectBindingKind::LocalAlias
            | TsDirectBindingKind::ImportedNamed
            | TsDirectBindingKind::ImportedDefault
            | TsDirectBindingKind::ImportedNamespaceMember
            | TsDirectBindingKind::ReExportedAlias
            | TsDirectBindingKind::CommonJsRequireMember
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TsTokenSourceFlow {
    source_function_stable_key: String,
    callsite_stable_key: String,
    stable_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TsTokenSourceFlowKind {
    ParameterArgument,
    ReturnValue,
}

impl TsTokenSourceFlowKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ParameterArgument => "parameter_argument",
            Self::ReturnValue => "return_value",
        }
    }
}

#[derive(Debug, Clone)]
struct TsFunctionInventoryRef {
    stable_key: String,
    display_name: Option<String>,
}

#[derive(Debug, Default)]
struct TsTokenSourceFlowIndex {
    function_by_span: BTreeMap<(u32, u32), TsFunctionInventoryRef>,
    callsite_key_by_span: BTreeMap<(u32, u32), String>,
    unique_function_key_by_name: BTreeMap<String, Option<String>>,
    unique_function_node_by_name: BTreeMap<String, Option<NodeId>>,
    parameter_name_by_function: BTreeMap<NodeId, Vec<String>>,
    parameter_calls_by_function: BTreeMap<NodeId, BTreeMap<usize, Vec<String>>>,
    local_alias_target_by_function_var: BTreeMap<(NodeId, String), Option<String>>,
    returned_function_key_by_function: BTreeMap<NodeId, Option<String>>,
    local_return_token_by_function_var: BTreeMap<(NodeId, String), Option<String>>,
}

fn collect_ts_token_source_flows(file: &SourceFile) -> Vec<TsTokenSourceFlow> {
    let source = file.source.as_ref();
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, parse_source_type(&file.path)).parse();

    if parsed.panicked && parsed.program.body.is_empty() {
        return Vec::new();
    }

    let inventory = extract_ts_inventory(file);
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let nodes = semantic.nodes();
    let mut index = TsTokenSourceFlowIndex::from_inventory(&inventory, nodes);
    index.collect_parameter_calls(nodes);
    index.collect_local_alias_assignments(nodes);
    index.collect_returned_functions(nodes);
    index.collect_local_return_assignments(nodes);
    index.collect_flows(nodes)
}

impl TsTokenSourceFlowIndex {
    fn from_inventory(inventory: &TsInventoryOutput, nodes: &AstNodes<'_>) -> Self {
        let mut index = Self::default();
        for function in &inventory.functions {
            let key = (function.span.start_byte, function.span.end_byte);
            let reference = TsFunctionInventoryRef {
                stable_key: function.stable_key.clone(),
                display_name: function.display_name.clone(),
            };
            if let Some(display_name) = &reference.display_name {
                insert_unique(
                    &mut index.unique_function_key_by_name,
                    display_name.clone(),
                    reference.stable_key.clone(),
                );
            }
            index.function_by_span.insert(key, reference);
        }
        for callsite in &inventory.callsites {
            index.callsite_key_by_span.insert(
                (callsite.span.start_byte, callsite.span.end_byte),
                callsite.stable_key.clone(),
            );
        }

        for (node_id, node) in nodes.iter_enumerated() {
            let Some(reference) = index.function_reference_for_kind(node.kind()).cloned() else {
                continue;
            };
            if let Some(display_name) = &reference.display_name {
                insert_unique(
                    &mut index.unique_function_node_by_name,
                    display_name.clone(),
                    node_id,
                );
            }
            if let Some(params) = function_parameter_names(node.kind()) {
                index.parameter_name_by_function.insert(node_id, params);
            }
        }

        index
    }

    fn collect_parameter_calls(&mut self, nodes: &AstNodes<'_>) {
        for (node_id, node) in nodes.iter_enumerated() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            let Some(callee_name) = expression_identifier_name(&call.callee) else {
                continue;
            };
            let Some(enclosing_function) = enclosing_function_node(nodes, node_id) else {
                continue;
            };
            let Some(parameter_names) = self.parameter_name_by_function.get(&enclosing_function)
            else {
                continue;
            };
            let Some(position) = parameter_names.iter().position(|name| name == &callee_name)
            else {
                continue;
            };
            let Some(callsite_key) = self.callsite_key_for_kind(node.kind()) else {
                continue;
            };

            self.parameter_calls_by_function
                .entry(enclosing_function)
                .or_default()
                .entry(position)
                .or_default()
                .push(callsite_key);
        }
    }

    fn collect_local_alias_assignments(&mut self, nodes: &AstNodes<'_>) {
        for (node_id, node) in nodes.iter_enumerated() {
            let AstKind::VariableDeclarator(declarator) = node.kind() else {
                continue;
            };
            let Some(variable_name) = variable_declarator_name(declarator) else {
                continue;
            };
            let Some(init) = declarator.init.as_ref() else {
                continue;
            };
            let Some(target_name) = expression_identifier_name(init) else {
                continue;
            };
            let Some(enclosing_function) = enclosing_function_node(nodes, node_id) else {
                continue;
            };
            let Some(target_function_key) =
                unique_lookup(&self.unique_function_key_by_name, &target_name).cloned()
            else {
                continue;
            };
            insert_unique(
                &mut self.local_alias_target_by_function_var,
                (enclosing_function, variable_name),
                target_function_key,
            );
        }
    }

    fn collect_returned_functions(&mut self, nodes: &AstNodes<'_>) {
        for (node_id, node) in nodes.iter_enumerated() {
            let AstKind::ReturnStatement(statement) = node.kind() else {
                continue;
            };
            let Some(argument) = statement.argument.as_ref() else {
                continue;
            };
            let Some(enclosing_function) = enclosing_function_node(nodes, node_id) else {
                continue;
            };
            let Some(returned_function_key) =
                self.function_key_from_return_argument(enclosing_function, argument)
            else {
                continue;
            };

            insert_unique(
                &mut self.returned_function_key_by_function,
                enclosing_function,
                returned_function_key,
            );
        }
    }

    fn collect_local_return_assignments(&mut self, nodes: &AstNodes<'_>) {
        for (node_id, node) in nodes.iter_enumerated() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            let Some(variable_name) = variable_declarator_name_for_call(nodes, node_id) else {
                continue;
            };
            let Some(callee_name) = expression_identifier_name(&call.callee) else {
                continue;
            };
            let Some(callee_function) =
                unique_lookup(&self.unique_function_node_by_name, &callee_name).copied()
            else {
                continue;
            };
            let Some(enclosing_function) = enclosing_function_node(nodes, node_id) else {
                continue;
            };
            let Some(returned_function_key) =
                unique_lookup(&self.returned_function_key_by_function, &callee_function).cloned()
            else {
                continue;
            };
            insert_unique(
                &mut self.local_return_token_by_function_var,
                (enclosing_function, variable_name),
                returned_function_key,
            );
        }
    }

    fn collect_flows(&self, nodes: &AstNodes<'_>) -> Vec<TsTokenSourceFlow> {
        let mut flows = Vec::new();
        for (node_id, node) in nodes.iter_enumerated() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            let Some(callsite_key) = self.callsite_key_for_kind(node.kind()) else {
                continue;
            };
            let Some(callee_name) = expression_identifier_name(&call.callee) else {
                continue;
            };

            self.collect_parameter_argument_flows(&mut flows, &callee_name, call, &callsite_key);
            self.collect_return_value_flow(&mut flows, nodes, node_id, &callee_name, &callsite_key);
        }
        flows.sort();
        flows.dedup();
        flows
    }

    fn collect_parameter_argument_flows(
        &self,
        flows: &mut Vec<TsTokenSourceFlow>,
        callee_name: &str,
        call: &oxc_ast::ast::CallExpression<'_>,
        source_callsite_key: &str,
    ) {
        let Some(callee_function) =
            unique_lookup(&self.unique_function_node_by_name, &callee_name.to_string()).copied()
        else {
            return;
        };
        let Some(parameter_calls) = self.parameter_calls_by_function.get(&callee_function) else {
            return;
        };
        for (position, argument) in call.arguments.iter().enumerate() {
            let Some(argument_name) = argument_identifier_name(argument) else {
                continue;
            };
            let Some(argument_function_key) =
                unique_lookup(&self.unique_function_key_by_name, &argument_name)
            else {
                continue;
            };
            let Some(destination_callsites) = parameter_calls.get(&position) else {
                continue;
            };
            for destination_callsite_key in destination_callsites {
                flows.push(ts_token_source_flow(
                    TsTokenSourceFlowKind::ParameterArgument,
                    argument_function_key,
                    destination_callsite_key,
                    source_callsite_key,
                ));
            }
        }
    }

    fn collect_return_value_flow(
        &self,
        flows: &mut Vec<TsTokenSourceFlow>,
        nodes: &AstNodes<'_>,
        node_id: NodeId,
        callee_name: &str,
        callsite_key: &str,
    ) {
        let Some(enclosing_function) = enclosing_function_node(nodes, node_id) else {
            return;
        };
        let Some(returned_function_key) = unique_lookup(
            &self.local_return_token_by_function_var,
            &(enclosing_function, callee_name.to_string()),
        ) else {
            return;
        };
        flows.push(ts_token_source_flow(
            TsTokenSourceFlowKind::ReturnValue,
            returned_function_key,
            callsite_key,
            callee_name,
        ));
    }

    fn function_reference_for_kind(&self, kind: AstKind<'_>) -> Option<&TsFunctionInventoryRef> {
        if !matches!(
            kind,
            AstKind::Function(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::MethodDefinition(_)
        ) {
            return None;
        }
        let span = kind.span();
        self.function_by_span.get(&(span.start, span.end))
    }

    fn callsite_key_for_kind(&self, kind: AstKind<'_>) -> Option<String> {
        let span = kind.span();
        self.callsite_key_by_span
            .get(&(span.start, span.end))
            .cloned()
    }

    fn function_key_from_return_argument(
        &self,
        enclosing_function: NodeId,
        argument: &Expression<'_>,
    ) -> Option<String> {
        match argument {
            Expression::Identifier(identifier) => {
                let name = identifier.name.to_string();
                self.lookup_function_or_local_alias(enclosing_function, &name)
            }
            Expression::FunctionExpression(function) => self
                .returned_function_expression_target(enclosing_function, function)
                .or_else(|| {
                    self.function_by_span
                        .get(&(function.span.start, function.span.end))
                        .map(|reference| reference.stable_key.clone())
                }),
            Expression::ParenthesizedExpression(expression) => {
                self.function_key_from_return_argument(enclosing_function, &expression.expression)
            }
            Expression::TSAsExpression(expression) => {
                self.function_key_from_return_argument(enclosing_function, &expression.expression)
            }
            Expression::TSSatisfiesExpression(expression) => {
                self.function_key_from_return_argument(enclosing_function, &expression.expression)
            }
            Expression::TSNonNullExpression(expression) => {
                self.function_key_from_return_argument(enclosing_function, &expression.expression)
            }
            Expression::TSTypeAssertion(expression) => {
                self.function_key_from_return_argument(enclosing_function, &expression.expression)
            }
            Expression::TSInstantiationExpression(expression) => {
                self.function_key_from_return_argument(enclosing_function, &expression.expression)
            }
            _ => None,
        }
    }

    fn returned_function_expression_target(
        &self,
        enclosing_function: NodeId,
        function: &Function<'_>,
    ) -> Option<String> {
        let body = function.body.as_ref()?;
        for statement in &body.statements {
            let Statement::ReturnStatement(statement) = statement else {
                continue;
            };
            let Some(Expression::CallExpression(call)) = statement.argument.as_ref() else {
                continue;
            };
            let Some(callee_name) = expression_identifier_name(&call.callee) else {
                continue;
            };
            if let Some(target) =
                self.lookup_function_or_local_alias(enclosing_function, &callee_name)
            {
                return Some(target);
            }
        }
        None
    }

    fn lookup_function_or_local_alias(
        &self,
        enclosing_function: NodeId,
        name: &str,
    ) -> Option<String> {
        unique_lookup(
            &self.local_alias_target_by_function_var,
            &(enclosing_function, name.to_string()),
        )
        .cloned()
        .or_else(|| unique_lookup(&self.unique_function_key_by_name, &name.to_string()).cloned())
    }
}

fn ts_token_source_flow(
    kind: TsTokenSourceFlowKind,
    source_function_stable_key: &str,
    callsite_stable_key: &str,
    evidence: &str,
) -> TsTokenSourceFlow {
    TsTokenSourceFlow {
        source_function_stable_key: source_function_stable_key.to_string(),
        callsite_stable_key: callsite_stable_key.to_string(),
        stable_key: semantic_stable_key(
            FactFamily::PointsToConstraint,
            &[
                ("source", source_function_stable_key.to_string()),
                ("callsite", callsite_stable_key.to_string()),
                ("kind", kind.as_str().to_string()),
                ("evidence", evidence.to_string()),
            ],
        )
        .into_string(),
    }
}

fn insert_unique<K: Ord, V: PartialEq>(map: &mut BTreeMap<K, Option<V>>, key: K, value: V) {
    match map.entry(key) {
        Entry::Vacant(entry) => {
            entry.insert(Some(value));
        }
        Entry::Occupied(mut entry) => {
            if entry
                .get()
                .as_ref()
                .is_some_and(|existing| existing != &value)
            {
                entry.insert(None);
            }
        }
    }
}

fn unique_lookup<'a, K: Ord, V>(map: &'a BTreeMap<K, Option<V>>, key: &K) -> Option<&'a V> {
    map.get(key).and_then(Option::as_ref)
}

fn function_parameter_names(kind: AstKind<'_>) -> Option<Vec<String>> {
    let params = match kind {
        AstKind::Function(function) => function.params.as_ref(),
        AstKind::ArrowFunctionExpression(function) => &function.params,
        _ => return None,
    };
    Some(
        params
            .items
            .iter()
            .filter_map(|parameter| binding_pattern_identifier_name(&parameter.pattern))
            .collect(),
    )
}

fn enclosing_function_node(nodes: &AstNodes<'_>, node_id: NodeId) -> Option<NodeId> {
    nodes
        .ancestors_enumerated(node_id)
        .find_map(|(ancestor_id, node)| {
            if matches!(
                node.kind(),
                AstKind::Function(_)
                    | AstKind::ArrowFunctionExpression(_)
                    | AstKind::MethodDefinition(_)
            ) {
                Some(ancestor_id)
            } else {
                None
            }
        })
}

fn variable_declarator_name_for_call(nodes: &AstNodes<'_>, node_id: NodeId) -> Option<String> {
    let AstKind::VariableDeclarator(declarator) = nodes.parent_kind(node_id) else {
        return None;
    };
    if !declarator.init.as_ref().is_some_and(|init| {
        let AstKind::CallExpression(call) = nodes.kind(node_id) else {
            return false;
        };
        init.span() == call.span
    }) {
        return None;
    }
    variable_declarator_name(declarator)
}

fn variable_declarator_name(declarator: &VariableDeclarator<'_>) -> Option<String> {
    binding_pattern_identifier_name(&declarator.id)
}

fn binding_pattern_identifier_name(pattern: &BindingPattern<'_>) -> Option<String> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.to_string()),
        BindingPattern::AssignmentPattern(pattern) => {
            binding_pattern_identifier_name(&pattern.left)
        }
        _ => None,
    }
}

fn argument_identifier_name(argument: &Argument<'_>) -> Option<String> {
    match argument {
        Argument::Identifier(identifier) => Some(identifier.name.to_string()),
        Argument::ParenthesizedExpression(expression) => {
            expression_identifier_name(&expression.expression)
        }
        Argument::TSAsExpression(expression) => expression_identifier_name(&expression.expression),
        Argument::TSSatisfiesExpression(expression) => {
            expression_identifier_name(&expression.expression)
        }
        Argument::TSNonNullExpression(expression) => {
            expression_identifier_name(&expression.expression)
        }
        Argument::TSTypeAssertion(expression) => expression_identifier_name(&expression.expression),
        Argument::TSInstantiationExpression(expression) => {
            expression_identifier_name(&expression.expression)
        }
        _ => None,
    }
}

fn expression_identifier_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::ParenthesizedExpression(expression) => {
            expression_identifier_name(&expression.expression)
        }
        Expression::TSAsExpression(expression) => {
            expression_identifier_name(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            expression_identifier_name(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            expression_identifier_name(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            expression_identifier_name(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            expression_identifier_name(&expression.expression)
        }
        _ => None,
    }
}

fn parse_source_type(path: &std::path::Path) -> SourceType {
    SourceType::from_path(path).unwrap_or_default()
}

fn matching_core_function<'a>(
    db: &'a AnalysisDb,
    semantic_function: &crate::go::semantic::facts::GoSemanticFunctionFact,
) -> Option<&'a crate::core::FunctionFact> {
    let file = semantic_function.file?;
    let span = semantic_function.span.as_ref()?;
    db.functions().iter().find(|function| {
        function.file == file
            && function.language == crate::core::Language::Go
            && function.name == semantic_function.name
            && function.span.start_byte == span.start_byte
            && function.span.end_byte == span.end_byte
    })
}

fn matching_core_callsite<'a>(
    db: &'a AnalysisDb,
    semantic_callsite: &GoSemanticCallsiteFact,
) -> Option<&'a crate::analysis::calls::facts::CallSiteFact> {
    let file = semantic_callsite.file?;
    let span = semantic_callsite.span.as_ref()?;
    db.call_sites().iter().find(|callsite| {
        callsite.file == file
            && callsite.language == crate::core::Language::Go
            && callsite.span.start_byte == span.start_byte
            && callsite.span.end_byte == span.end_byte
    })
}

struct TsDirectBindingNodeContext<'a> {
    db: &'a AnalysisDb,
    file_by_relative_path: BTreeMap<&'a str, FileId>,
}

impl<'a> TsDirectBindingNodeContext<'a> {
    fn new(db: &'a AnalysisDb) -> Self {
        Self {
            db,
            file_by_relative_path: db
                .files()
                .iter()
                .map(|file| (file.relative_path.as_str(), file.id))
                .collect(),
        }
    }

    fn callsite_node_for_binding(
        &self,
        builder: &GraphBuilder,
        callsite_stable_key: &str,
    ) -> Option<SemanticNodeId> {
        let identity = TsInventoryStableIdentity::parse(callsite_stable_key)?;
        let file = self.file_by_relative_path.get(identity.file.as_str())?;
        self.db
            .call_sites()
            .iter()
            .find(|site| {
                site.file == *file
                    && site.span.start_byte == identity.start
                    && site.span.end_byte == identity.end
            })
            .and_then(|site| {
                let key = node_key_from_identity("callsite", &site.stable_key);
                builder.node_by_key.get(&key).copied()
            })
    }

    fn function_node_for_binding(
        &self,
        builder: &GraphBuilder,
        function_stable_key: &str,
    ) -> Option<SemanticNodeId> {
        let identity = TsInventoryStableIdentity::parse(function_stable_key)?;
        let file = self.file_by_relative_path.get(identity.file.as_str())?;
        self.db
            .functions()
            .iter()
            .find(|function| {
                function.file == *file
                    && function.span.start_byte == identity.start
                    && function.span.end_byte == identity.end
                    && identity
                        .display
                        .as_deref()
                        .is_none_or(|display| display == function.name)
            })
            .and_then(|function| {
                let key = function_node_key(self.db, function);
                builder.node_by_key.get(&key).copied()
            })
    }
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

/// Place node key derived from the place's OWN content-stable identity (the
/// `PlaceFact.stable_key`, which the MIR layer composes from stable context, never
/// run-local dense IDs). Keying on the place identity — not the owning value fact —
/// is what makes the same place a single shared node across copies.
fn place_node_key(place_stable_key: &str) -> String {
    semantic_stable_key(
        FactFamily::Place,
        &[
            ("node_kind", "place".to_string()),
            ("identity", place_stable_key.to_string()),
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
        AnalysisDb, FileId, FunctionFact, FunctionId, ImportFact, ImportId, Language, ModuleNode,
        ModuleNodeId, ModuleNodeKind, PackageFact, PackageId, ResolutionPrecision,
        ResolutionStatus, ResolvedImportFact, Span,
    };
    use crate::ts::binding::direct::{
        TsDirectBindingModuleFile, TsDirectBindingModuleInput, resolve_direct_bindings,
        resolve_direct_bindings_with_modules,
    };
    use crate::ts::binding::facts::TsDirectBindingReason;
    use crate::ts::inventory::facts::{TsInventoryCallsiteFact, TsInventoryFunctionFact};

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

    fn ts_db_with_file(path: &str, source: &str) -> (AnalysisDb, FileId) {
        let mut db = AnalysisDb::new();
        let file = db.add_file(PathBuf::from(path), path.to_string(), source.to_string());
        (db, file)
    }

    fn push_ts_function(db: &mut AnalysisDb, function: &TsInventoryFunctionFact) -> FunctionId {
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file: function.file,
            name: function
                .display_name
                .clone()
                .unwrap_or_else(|| "<anonymous>".to_string()),
            span: function.span.clone(),
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        })
    }

    fn ts_function<'a>(
        inventory: &'a TsInventoryOutput,
        display_name: &str,
    ) -> &'a TsInventoryFunctionFact {
        inventory
            .functions
            .iter()
            .find(|function| function.display_name.as_deref() == Some(display_name))
            .unwrap_or_else(|| panic!("missing TS function {display_name}"))
    }

    fn ts_callsite<'a>(
        inventory: &'a TsInventoryOutput,
        display_name: &str,
    ) -> &'a TsInventoryCallsiteFact {
        inventory
            .callsites
            .iter()
            .find(|callsite| callsite.display_name.as_deref() == Some(display_name))
            .unwrap_or_else(|| panic!("missing TS callsite {display_name}"))
    }

    fn replace_single_ts_callsite(
        db: &mut AnalysisDb,
        caller: FunctionId,
        callsite: &TsInventoryCallsiteFact,
    ) {
        db.replace_call_facts(CallOutput {
            sites: vec![CallSiteFact {
                id: CallSiteId(0),
                language: Language::TypeScript,
                file: callsite.file,
                caller,
                owner_symbol: None,
                body: MirBodyId(0),
                operation: MirOpId(0),
                span: callsite.span.clone(),
                kind: CallSyntaxKind::Function,
                callee: CallCallee::Identifier {
                    reference: None,
                    name: callsite
                        .display_name
                        .clone()
                        .unwrap_or_else(|| "<dynamic>".to_string()),
                },
                receiver: None,
                arguments: Vec::new(),
                result: None,
                status: CallTargetStatus::Resolved,
                precision: CallPrecision::SetupAware,
                stable_key: format!("ts-callsite:{}", callsite.stable_key),
            }],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call replace");
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

    mod ts_direct_bindings {
        use super::*;

        #[test]
        fn projects_copy_edge_and_call_constraint_for_local_alias() {
            let (mut db, file) = ts_db_with_file(
                "src/app.ts",
                r#"
function f() {}
const alias = f;
function run() {
  alias();
}
"#,
            );
            let inventory = extract_ts_inventory(db.file(file).expect("file"));
            let scope = extract_ts_scope(db.file(file).expect("file"));
            let target = ts_function(&inventory, "f").clone();
            let caller = ts_function(&inventory, "run").clone();
            let callsite = ts_callsite(&inventory, "alias").clone();
            push_ts_function(&mut db, &target);
            let caller = push_ts_function(&mut db, &caller);
            replace_single_ts_callsite(&mut db, caller, &callsite);

            let bindings = resolve_direct_bindings(&inventory, &scope);
            let output = build_semantic_graph_with_ts_direct_bindings(&db, &bindings.bindings);
            assert_ts_direct_projection(output);
        }

        #[test]
        fn projects_copy_edge_and_call_constraint_for_imported_alias() {
            let mut db = AnalysisDb::new();
            let app_file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                r#"
import { f as g } from "./m";
function run() {
  g();
}
"#
                .to_string(),
            );
            let module_file = db.add_file(
                PathBuf::from("src/m.ts"),
                "src/m.ts".to_string(),
                "export function f() {}".to_string(),
            );
            let app_inventory = extract_ts_inventory(db.file(app_file).expect("app"));
            let app_scope = extract_ts_scope(db.file(app_file).expect("app"));
            let module_inventory = extract_ts_inventory(db.file(module_file).expect("module"));
            let module_scope = extract_ts_scope(db.file(module_file).expect("module"));

            let target = ts_function(&module_inventory, "f").clone();
            let caller = ts_function(&app_inventory, "run").clone();
            let callsite = ts_callsite(&app_inventory, "g").clone();
            push_ts_function(&mut db, &target);
            let caller = push_ts_function(&mut db, &caller);
            replace_single_ts_callsite(&mut db, caller, &callsite);
            let import = db.push_import(ImportFact {
                id: ImportId(0),
                file: app_file,
                package: None,
                path: "./m".to_string(),
                span: Span::point(app_file, 1, 0),
                language: Language::TypeScript,
            });
            db.replace_module_graph_facts(
                vec![ResolvedImportFact {
                    id: crate::core::ResolvedImportId(0),
                    import,
                    from_file: app_file,
                    target_node: Some(ModuleNodeId(1)),
                    status: ResolutionStatus::Resolved,
                    precision: ResolutionPrecision::ExactFile,
                    reason: None,
                }],
                vec![
                    ModuleNode {
                        id: ModuleNodeId(0),
                        kind: ModuleNodeKind::File,
                        label: "src/app.ts".to_string(),
                        file: Some(app_file),
                        package: None,
                        language: Some(Language::TypeScript),
                    },
                    ModuleNode {
                        id: ModuleNodeId(1),
                        kind: ModuleNodeKind::File,
                        label: "src/m.ts".to_string(),
                        file: Some(module_file),
                        package: None,
                        language: Some(Language::TypeScript),
                    },
                ],
                Vec::new(),
            );
            let module_files = vec![TsDirectBindingModuleFile {
                module_node: ModuleNodeId(1),
                inventory: &module_inventory,
                scope: &module_scope,
            }];
            let module_input = TsDirectBindingModuleInput {
                imports: db.imports(),
                resolved_imports: db.resolved_imports(),
                module_nodes: db.module_nodes(),
                module_files: &module_files,
            };

            let bindings =
                resolve_direct_bindings_with_modules(&app_inventory, &app_scope, &module_input);
            let output = build_semantic_graph_with_ts_direct_bindings(&db, &bindings.bindings);
            assert_ts_direct_projection(output);
        }

        #[test]
        fn semantic_graph_only_projection_does_not_add_call_target_facts() {
            let (mut db, file) = ts_db_with_file(
                "src/app.ts",
                r#"
function f() {}
const alias = f;
function run() {
  alias();
}
"#,
            );
            let inventory = extract_ts_inventory(db.file(file).expect("file"));
            let scope = extract_ts_scope(db.file(file).expect("file"));
            let target = ts_function(&inventory, "f").clone();
            let caller = ts_function(&inventory, "run").clone();
            let callsite = ts_callsite(&inventory, "alias").clone();
            push_ts_function(&mut db, &target);
            let caller = push_ts_function(&mut db, &caller);
            replace_single_ts_callsite(&mut db, caller, &callsite);

            let bindings = resolve_direct_bindings(&inventory, &scope);
            let before_targets = db.call_targets().len();
            let output = build_semantic_graph_with_ts_direct_bindings(&db, &bindings.bindings);

            assert_ts_direct_projection(output);
            assert_eq!(db.call_targets().len(), before_targets);
        }

        #[test]
        fn source_documents_semantic_graph_only_projection_for_phase_45() {
            let source = include_str!("build.rs");

            assert!(source.contains("Phase 45 semantic-graph-only projection"));
            assert!(source.contains("do not create CallTargetFact rows"));
        }

        #[test]
        fn unresolved_no_solver_boundary_rows_emit_zero_target_constraints() {
            let db = AnalysisDb::new();
            let rows = [
                TsDirectBindingReason::TokenFlowRequired,
                TsDirectBindingReason::PropertyFlowRequired,
                TsDirectBindingReason::PrototypeModelRequired,
                TsDirectBindingReason::ThisModelRequired,
            ]
            .into_iter()
            .map(unresolved_binding)
            .collect::<Vec<_>>();

            let output = build_semantic_graph_with_ts_direct_bindings(&db, &rows);

            assert!(output.constraints.iter().all(|constraint| {
                !matches!(constraint.kind, ConstraintKind::CopyEdge { .. })
                    && !matches!(constraint.kind, ConstraintKind::CallConstraint { .. })
            }));
        }

        #[test]
        fn source_has_no_phase_45_solver_reference_or_token_worklist() {
            let source = include_str!("build.rs");

            assert!(!source.contains(concat!("analysis", "::solver")));
            assert!(!source.contains(concat!("fixed", "-point")));
            assert!(!source.contains(concat!("token", "-set")));
            assert!(!source.contains(concat!("token", " propagation")));
        }

        fn assert_ts_direct_projection(output: SemanticGraphOutput) {
            let copy_edges = output
                .constraints
                .iter()
                .filter(|constraint| matches!(constraint.kind, ConstraintKind::CopyEdge { .. }))
                .count();
            let call_constraints = output
                .constraints
                .iter()
                .filter(|constraint| {
                    matches!(constraint.kind, ConstraintKind::CallConstraint { .. })
                })
                .count();

            assert!(copy_edges >= 1, "expected TS direct CopyEdge");
            assert!(call_constraints >= 1, "expected TS direct CallConstraint");
            SemanticGraphStore::from_output(output).expect("TS direct graph validates");
        }

        fn unresolved_binding(reason: TsDirectBindingReason) -> TsDirectBindingFact {
            TsDirectBindingFact {
                id: crate::analysis::ids::TsDirectBindingId(0),
                callsite: crate::analysis::ids::TsInventoryCallsiteId(1),
                callsite_stable_key: format!("callsite:{}", reason.as_str()),
                target_function: None,
                target_function_stable_key: None,
                scope_binding: None,
                scope_binding_stable_key: None,
                resolved_import: None,
                module_node: None,
                kind: TsDirectBindingKind::LocalFunction,
                status: TsDirectBindingStatus::Unresolved,
                reason: Some(reason),
                stable_key: format!("ts_direct_binding:{}", reason.as_str()),
            }
        }
    }

    mod go_semantic {
        use super::*;
        use crate::go::semantic::facts::{
            GoSemanticCallStatus, GoSemanticCallsiteFact, GoSemanticCallsiteId,
            GoSemanticFunctionFact, GoSemanticFunctionId, GoSemanticFunctionKind,
        };
        use crate::go::semantic::store::GoSemanticFactsOutput;

        #[test]
        fn direct_go_call_emits_call_constraint_and_static_evidence() {
            let mut db = go_semantic_db();
            db.replace_go_semantic_facts(go_semantic_output(
                GoSemanticCallStatus::ResolvedStatic,
                Some("example.com/p.run"),
            ))
            .expect("go semantic facts replace");

            let output = build_semantic_graph(&db);

            assert!(output.constraints.iter().any(|constraint| {
                matches!(
                    constraint.kind,
                    ConstraintKind::CallConstraint { .. } | ConstraintKind::CopyEdge { .. }
                ) && constraint.stable_key.contains("go-call")
            }));
            SemanticGraphStore::from_output(output).expect("Go semantic graph validates");
        }

        #[test]
        fn dynamic_go_call_emits_no_static_target_evidence() {
            let mut db = go_semantic_db();
            db.replace_go_semantic_facts(go_semantic_output(
                GoSemanticCallStatus::UnresolvedDynamic,
                None,
            ))
            .expect("go semantic facts replace");

            let output = build_semantic_graph(&db);

            let go_copy_edges = output
                .constraints
                .iter()
                .filter(|constraint| {
                    matches!(constraint.kind, ConstraintKind::CopyEdge { .. })
                        && constraint.stable_key.contains("go-call")
                })
                .count();
            assert_eq!(go_copy_edges, 0);
        }

        fn go_semantic_db() -> AnalysisDb {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("main.go"),
                "main.go".to_string(),
                "package main\nfunc main(){ run() }\nfunc run(){}\n".to_string(),
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
                span: span_at(file, 20, 24),
                language: Language::Go,
                is_test: false,
                is_exported: false,
                cyclomatic_complexity: 1,
                calls: vec!["run".to_string()],
            });
            db.push_function(FunctionFact {
                id: FunctionId(0),
                file,
                name: "run".to_string(),
                span: span_at(file, 34, 37),
                language: Language::Go,
                is_test: false,
                is_exported: false,
                cyclomatic_complexity: 1,
                calls: Vec::new(),
            });
            db.replace_call_facts(CallOutput {
                sites: vec![CallSiteFact {
                    id: CallSiteId(0),
                    language: Language::Go,
                    file,
                    caller,
                    owner_symbol: None,
                    body: MirBodyId(0),
                    operation: MirOpId(0),
                    span: span_at(file, 28, 31),
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
                    stable_key: "go-core-callsite:run".to_string(),
                }],
                targets: Vec::new(),
                unresolved: Vec::new(),
            })
            .expect("call replace");
            db
        }

        fn go_semantic_output(
            status: GoSemanticCallStatus,
            static_callee: Option<&str>,
        ) -> GoSemanticFactsOutput {
            GoSemanticFactsOutput {
                functions: vec![GoSemanticFunctionFact {
                    id: GoSemanticFunctionId(0),
                    stable_key: "go-fn-run".to_string(),
                    package_id: "example.com/p".to_string(),
                    package_path: "example.com/p".to_string(),
                    name: "run".to_string(),
                    qualified: "example.com/p.run".to_string(),
                    signature: "()".to_string(),
                    kind: GoSemanticFunctionKind::Function,
                    receiver: None,
                    relative_file: Some("main.go".to_string()),
                    file: Some(FileId(0)),
                    span: Some(span_at(FileId(0), 34, 37)),
                }],
                callsites: vec![GoSemanticCallsiteFact {
                    id: GoSemanticCallsiteId(0),
                    stable_key: "go-call".to_string(),
                    package_id: "example.com/p".to_string(),
                    package_path: "example.com/p".to_string(),
                    caller: "example.com/p.main".to_string(),
                    static_callee: static_callee.map(str::to_string),
                    status,
                    reason: None,
                    relative_file: Some("main.go".to_string()),
                    file: Some(FileId(0)),
                    span: Some(span_at(FileId(0), 28, 31)),
                }],
                ..GoSemanticFactsOutput::default()
            }
        }

        fn span_at(file: FileId, start: u32, end: u32) -> Span {
            Span {
                file,
                start_byte: start,
                end_byte: end,
                start_line: 1,
                start_col: start,
                end_line: 1,
                end_col: end,
            }
        }
    }
}
