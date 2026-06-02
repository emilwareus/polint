//! The closed Go RTA input snapshot (D-01, D-06, D-07).
//!
//! [`GoRtaInputs`] is the closed snapshot the [`super::super::policy::GoRtaPolicy`]
//! owns, mirroring how `PointsToPolicy` owns its `Vec<PointsToConstraintFact>`. It
//! is built once via [`GoRtaInputs::from_db`] from the stored Go-frontend facts plus
//! the already-built `polint.semantic_graph` function nodes, and is then iterated by
//! [`super::fixpoint::solve_go_rta`] without re-reading the db.
//!
//! Every accumulator is `BTree`-keyed on official Go string identity (`qualified`
//! function names, `type_name`s), never run-local discovery order — this is what
//! keeps the 10-shuffle determinism gate green (D-17).

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::semantic_graph::facts::NodeKind;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, FunctionFact, Language};
use crate::go::semantic::facts::{GoSemanticCallStatus, GoSemanticFunctionFact};

/// One concrete Go method, indexed by its receiver type, for interface-invoke
/// resolution. `qualified` is the method's official identity; `node` is the unified
/// semantic-graph node the resolved edge targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoRtaMethod {
    pub(crate) method_name: String,
    pub(crate) qualified: String,
    pub(crate) node: SemanticNodeId,
}

/// The closed dispatch obligation for one `UnresolvedDynamic` Go callsite that maps
/// to a `CallConstraint` node in the semantic graph, joined with its dispatch detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoRtaCallsite {
    /// The caller function's official `qualified` identity (a key into
    /// [`GoRtaInputs::function_node`] / the reachable set).
    pub(crate) caller: String,
    /// The unified `CallConstraint` callsite node (the edge SOURCE for resolved
    /// edges is the caller function node, not this — see the fixpoint).
    pub(crate) callsite_node: SemanticNodeId,
    /// The contributing callsite fact's stable key (recorded in edge provenance so
    /// the deletion-invalidation property holds — D-09).
    pub(crate) callsite_stable_key: String,
    /// Interface-invoke discriminant: the invoked interface method name, if any.
    pub(crate) interface_method: Option<String>,
    /// Func-value-call discriminant: the call signature, if any.
    pub(crate) signature: Option<String>,
    /// The dynamic-dispatch detail fact's stable key (a second contributing fact for
    /// the resolved edge's provenance).
    pub(crate) dispatch_stable_key: String,
}

/// The closed RTA input snapshot. See the module docs for the determinism contract.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GoRtaInputs {
    /// Reachability-root function identities (`qualified`) that map to a Go function
    /// node — the RTA seed (D-07).
    pub(crate) roots: BTreeSet<String>,
    /// Every `UnresolvedDynamic` callsite that maps to a `CallConstraint` node, with
    /// its dispatch detail joined by `callsite_stable_key`.
    pub(crate) callsites: Vec<GoRtaCallsite>,
    /// `type_name -> method names` (the Phase 46 method-set input).
    pub(crate) method_sets: BTreeMap<String, BTreeSet<String>>,
    /// `type_name -> the method-set fact's stable key` (a contributing fact).
    pub(crate) method_set_keys: BTreeMap<String, String>,
    /// The instantiated runtime-type set (the RTA rapid-type set).
    pub(crate) instantiated: BTreeSet<String>,
    /// `type_name -> the instantiated-type fact's stable key` (a contributing fact).
    pub(crate) instantiated_keys: BTreeMap<String, String>,
    /// Address-taken function identities (`qualified`) — func-value candidates.
    pub(crate) address_taken: BTreeSet<String>,
    /// `qualified -> the address-taken fact's stable key` (a contributing fact).
    pub(crate) address_taken_keys: BTreeMap<String, String>,
    /// `qualified -> the function's unified semantic node` (edge endpoints).
    pub(crate) function_node: BTreeMap<String, SemanticNodeId>,
    /// `qualified -> the function's signature` (func-value matching).
    pub(crate) function_signature: BTreeMap<String, String>,
    /// `receiver type_name -> its concrete methods` (interface-invoke resolution).
    pub(crate) methods_by_receiver: BTreeMap<String, Vec<GoRtaMethod>>,
}

impl GoRtaInputs {
    /// Build the closed snapshot from the stored Go-frontend facts + the already-built
    /// `polint.semantic_graph` function nodes (D-01). Reads only the AnalysisDb
    /// accessors; builds `BTree`-keyed structures for determinism. Honest by
    /// construction: a function with no matching semantic node is simply absent from
    /// [`Self::function_node`] (its edges cannot be emitted), never fabricated.
    pub(crate) fn from_db(db: &AnalysisDb) -> Self {
        // qualified -> SemanticNodeId, reconstructed from the function-node stable key
        // recipe `polint.semantic_graph` used (composition over a private coupling):
        // a Go semantic function maps to a node iff a core FunctionFact matches it and
        // that function was interned as a graph node.
        let function_node_by_stable_key: BTreeMap<&str, SemanticNodeId> = db
            .semantic_nodes()
            .iter()
            .filter(|node| matches!(node.kind, NodeKind::Function(_)))
            .map(|node| (node.stable_key.as_str(), node.id))
            .collect();

        let mut function_node: BTreeMap<String, SemanticNodeId> = BTreeMap::new();
        let mut function_signature: BTreeMap<String, String> = BTreeMap::new();
        let mut methods_by_receiver: BTreeMap<String, Vec<GoRtaMethod>> = BTreeMap::new();

        for semantic_function in db.go_semantic_functions() {
            function_signature.insert(
                semantic_function.qualified.clone(),
                semantic_function.signature.clone(),
            );

            let Some(node) = matching_core_function(db, semantic_function).and_then(|function| {
                let key = function_node_key(db, function);
                function_node_by_stable_key.get(key.as_str()).copied()
            }) else {
                continue;
            };
            function_node.insert(semantic_function.qualified.clone(), node);

            // A method (receiver-bearing function) is an interface-invoke candidate
            // callee, indexed by its normalized receiver type.
            if let Some(receiver) = semantic_function.receiver.as_deref() {
                methods_by_receiver
                    .entry(normalize_type(receiver))
                    .or_default()
                    .push(GoRtaMethod {
                        method_name: semantic_function.name.clone(),
                        qualified: semantic_function.qualified.clone(),
                        node,
                    });
            }
        }
        // Deterministic order within each receiver bucket (by qualified identity).
        for methods in methods_by_receiver.values_mut() {
            methods.sort_by(|left, right| left.qualified.cmp(&right.qualified));
            methods.dedup();
        }

        // Reachability roots that resolve to a Go function node, mapped to `qualified`.
        let mut roots: BTreeSet<String> = BTreeSet::new();
        for root in db.reachability_roots() {
            if root.language != Language::Go {
                continue;
            }
            if let Some(qualified) = qualified_for_function_id(db, root.target_function) {
                roots.insert(qualified);
            }
        }

        // Method-sets: type_name -> methods (+ contributing fact key).
        let mut method_sets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut method_set_keys: BTreeMap<String, String> = BTreeMap::new();
        for method_set in db.go_semantic_method_sets() {
            let type_name = normalize_type(&method_set.type_name);
            method_sets
                .entry(type_name.clone())
                .or_default()
                .extend(method_set.methods.iter().cloned());
            method_set_keys.insert(type_name, method_set.stable_key.clone());
        }

        // Instantiated runtime types (the rapid-type set) + contributing fact key.
        let mut instantiated: BTreeSet<String> = BTreeSet::new();
        let mut instantiated_keys: BTreeMap<String, String> = BTreeMap::new();
        for fact in db.go_semantic_instantiated_types() {
            let type_name = normalize_type(&fact.type_name);
            instantiated.insert(type_name.clone());
            instantiated_keys.insert(type_name, fact.stable_key.clone());
        }

        // Address-taken functions (func-value candidates) + contributing fact key.
        let mut address_taken: BTreeSet<String> = BTreeSet::new();
        let mut address_taken_keys: BTreeMap<String, String> = BTreeMap::new();
        for fact in db.go_semantic_address_taken() {
            address_taken.insert(fact.function.clone());
            address_taken_keys.insert(fact.function.clone(), fact.stable_key.clone());
        }

        // Join dynamic-dispatch detail to its callsite by `callsite_stable_key`. Only
        // `UnresolvedDynamic` callsites that map to a `CallConstraint` node are RTA
        // obligations; the rest are statically resolved or unsupported (honest).
        let callsite_by_stable_key: BTreeMap<
            &str,
            &crate::go::semantic::facts::GoSemanticCallsiteFact,
        > = db
            .go_semantic_callsites()
            .iter()
            .map(|callsite| (callsite.stable_key.as_str(), callsite))
            .collect();

        let mut callsites: Vec<GoRtaCallsite> = Vec::new();
        for dispatch in db.go_semantic_dynamic_dispatch() {
            let Some(callsite) = callsite_by_stable_key.get(dispatch.callsite_stable_key.as_str())
            else {
                continue;
            };
            if callsite.status != GoSemanticCallStatus::UnresolvedDynamic {
                continue;
            }
            // The callsite must map to a `CallConstraint` node the semantic graph
            // emitted; if it did not (e.g. no matching core call site), there is no
            // edge SOURCE/anchor — skip rather than fabricate.
            let Some(callsite_node) = callsite_constraint_node(db, callsite) else {
                continue;
            };
            callsites.push(GoRtaCallsite {
                caller: dispatch.caller.clone(),
                callsite_node,
                callsite_stable_key: callsite.stable_key.clone(),
                interface_method: dispatch.method.clone(),
                signature: dispatch.signature.clone(),
                dispatch_stable_key: dispatch.stable_key.clone(),
            });
        }
        // Deterministic callsite order (by callsite stable key, then dispatch key).
        callsites.sort_by(|left, right| {
            (
                left.callsite_stable_key.as_str(),
                left.dispatch_stable_key.as_str(),
            )
                .cmp(&(
                    right.callsite_stable_key.as_str(),
                    right.dispatch_stable_key.as_str(),
                ))
        });

        Self {
            roots,
            callsites,
            method_sets,
            method_set_keys,
            instantiated,
            instantiated_keys,
            address_taken,
            address_taken_keys,
            function_node,
            function_signature,
            methods_by_receiver,
        }
    }
}

/// Normalize a Go type identity for matching: strip a single leading `*` so a
/// pointer receiver (`*pkg.T`) matches the value type-name (`pkg.T`) used by the
/// instantiated-type / method-set facts. Idempotent and honest (no other rewriting).
pub(crate) fn normalize_type(type_name: &str) -> String {
    type_name.strip_prefix('*').unwrap_or(type_name).to_string()
}

/// The `qualified` identity of the Go semantic function matching a core `FunctionId`,
/// if any. Mirrors the file+span+name join `matching_core_function` performs, in
/// reverse: a core function -> its Go semantic `qualified`.
fn qualified_for_function_id(
    db: &AnalysisDb,
    function_id: crate::core::FunctionId,
) -> Option<String> {
    let function = db
        .functions()
        .iter()
        .find(|function| function.id == function_id)?;
    db.go_semantic_functions()
        .iter()
        .find(|semantic_function| {
            semantic_function.file == Some(function.file)
                && semantic_function.name == function.name
                && semantic_function.span.as_ref().is_some_and(|span| {
                    span.start_byte == function.span.start_byte
                        && span.end_byte == function.span.end_byte
                })
        })
        .map(|semantic_function| semantic_function.qualified.clone())
}

/// The core `FunctionFact` matching a Go semantic function (file + language + name +
/// span), mirroring `semantic_graph::build::matching_core_function`.
fn matching_core_function<'a>(
    db: &'a AnalysisDb,
    semantic_function: &GoSemanticFunctionFact,
) -> Option<&'a FunctionFact> {
    let file = semantic_function.file?;
    let span = semantic_function.span.as_ref()?;
    db.functions().iter().find(|function| {
        function.file == file
            && function.language == Language::Go
            && function.name == semantic_function.name
            && function.span.start_byte == span.start_byte
            && function.span.end_byte == span.end_byte
    })
}

/// Reproduces `semantic_graph::build::function_node_key` so a Go function can be
/// looked up in the already-built `polint.semantic_graph` function nodes by stable
/// key. Keep in lockstep with the builder recipe (node-kind label + path + name +
/// span identity, `FactFamily::Function`).
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

/// The unified `CallConstraint` callsite node for a Go callsite, reconstructed from
/// the `callsite` node-key recipe `semantic_graph::build` used. Returns `None` when
/// the callsite was not interned as a node (honest — no edge anchor).
fn callsite_constraint_node(
    db: &AnalysisDb,
    callsite: &crate::go::semantic::facts::GoSemanticCallsiteFact,
) -> Option<SemanticNodeId> {
    let file = callsite.file?;
    let span = callsite.span.as_ref()?;
    let core_callsite = db.call_sites().iter().find(|site| {
        site.file == file
            && site.language == Language::Go
            && site.span.start_byte == span.start_byte
            && site.span.end_byte == span.end_byte
    })?;
    let node_key = node_key_from_identity("callsite", &core_callsite.stable_key);
    db.semantic_nodes()
        .iter()
        .find(|node| node.stable_key == node_key && matches!(node.kind, NodeKind::Callsite(_)))
        .map(|node| node.id)
}

/// Reproduces `semantic_graph::build::node_key_from_identity` (node-kind label +
/// identity, `FactFamily::Scope`). Keep in lockstep with the builder recipe.
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

/// Reproduces `semantic_graph::build::span_identity`.
fn span_identity(span: &crate::core::Span) -> String {
    format!("{}:{}..{}", span.file.0, span.start_byte, span.end_byte)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::analysis::semantic_graph::facts::{SemanticNodeFact, SemanticPrecision};
    use crate::analysis::semantic_graph::store::SemanticGraphOutput;
    use crate::core::{FunctionId, Span};
    use crate::go::semantic::facts::{
        GoSemanticFunctionFact, GoSemanticFunctionKind, GoSemanticInstantiatedTypeFact,
        GoSemanticMethodSetFact,
    };
    use crate::go::semantic::store::GoSemanticFactsOutput;

    fn span(file: crate::core::FileId, start: u32, end: u32) -> Span {
        Span {
            file,
            start_byte: start,
            end_byte: end,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 1,
        }
    }

    /// `from_db` joins the stored Go-frontend facts to the already-built semantic-graph
    /// function nodes: a Go function maps to its node, its receiver indexes it as a
    /// method, and the instantiated-type / method-set facts populate the RTA sets.
    #[test]
    fn from_db_maps_go_function_to_its_semantic_node_and_indexes_methods() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("pkg/file.go"),
            "pkg/file.go".to_string(),
            "package pkg\n".to_string(),
        );
        let method_span = span(file, 10, 40);

        // Core function for the concrete method (pkg.File).Read.
        let function_id = db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "Read".to_string(),
            span: method_span.clone(),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });

        // The semantic-graph function node, keyed exactly like the builder would key it.
        let node_stable_key = function_node_key(
            &db,
            db.functions().iter().find(|f| f.id == function_id).unwrap(),
        );
        db.replace_semantic_graph_facts(SemanticGraphOutput {
            nodes: vec![SemanticNodeFact {
                id: SemanticNodeId(0),
                kind: NodeKind::Function(function_id),
                precision: SemanticPrecision::Conservative,
                stable_key: node_stable_key.clone(),
            }],
            edges: Vec::new(),
            constraints: Vec::new(),
        })
        .expect("semantic graph stores");
        let expected_node = db
            .semantic_nodes()
            .iter()
            .find(|node| node.stable_key == node_stable_key)
            .expect("function node stored")
            .id;

        // Go-frontend facts: the function (matching the core function), an instantiated
        // type, and its method-set.
        db.replace_go_semantic_facts(GoSemanticFactsOutput {
            functions: vec![GoSemanticFunctionFact {
                id: crate::go::semantic::facts::GoSemanticFunctionId(0),
                stable_key: "gofn|(pkg.File).Read".to_string(),
                package_id: "pkg".to_string(),
                package_path: "pkg".to_string(),
                name: "Read".to_string(),
                qualified: "(pkg.File).Read".to_string(),
                signature: "func() (int, error)".to_string(),
                kind: GoSemanticFunctionKind::Method,
                receiver: Some("*pkg.File".to_string()),
                relative_file: Some("pkg/file.go".to_string()),
                file: Some(file),
                span: Some(method_span),
            }],
            instantiated_types: vec![GoSemanticInstantiatedTypeFact {
                id: crate::go::semantic::facts::GoSemanticInstantiatedTypeId(0),
                stable_key: "inst|pkg.File".to_string(),
                package_id: "pkg".to_string(),
                package_path: "pkg".to_string(),
                type_name: "pkg.File".to_string(),
            }],
            method_sets: vec![GoSemanticMethodSetFact {
                id: crate::go::semantic::facts::GoSemanticMethodSetId(0),
                stable_key: "ms|pkg.File".to_string(),
                package_id: "pkg".to_string(),
                package_path: "pkg".to_string(),
                type_name: "pkg.File".to_string(),
                methods: vec!["Read".to_string()],
            }],
            ..GoSemanticFactsOutput::default()
        })
        .expect("go semantic facts store");

        let inputs = GoRtaInputs::from_db(&db);

        // The Go function maps to its semantic node.
        assert_eq!(
            inputs.function_node.get("(pkg.File).Read"),
            Some(&expected_node)
        );
        // The pointer receiver is normalized to the value type-name and indexes the
        // concrete method for interface-invoke resolution.
        let methods = inputs
            .methods_by_receiver
            .get("pkg.File")
            .expect("methods indexed by normalized receiver");
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].method_name, "Read");
        assert_eq!(methods[0].node, expected_node);
        // The instantiated-type + method-set sets are populated (normalized type-name).
        assert!(inputs.instantiated.contains("pkg.File"));
        assert!(
            inputs
                .method_sets
                .get("pkg.File")
                .is_some_and(|methods| methods.contains("Read"))
        );
        // The function signature is recorded for func-value matching.
        assert_eq!(
            inputs
                .function_signature
                .get("(pkg.File).Read")
                .map(String::as_str),
            Some("func() (int, error)")
        );
    }

    #[test]
    fn normalize_type_strips_single_leading_pointer() {
        assert_eq!(normalize_type("*pkg.T"), "pkg.T");
        assert_eq!(normalize_type("pkg.T"), "pkg.T");
        // Only a single leading `*` is stripped (honest, idempotent on the result).
        assert_eq!(normalize_type("**pkg.T"), "*pkg.T");
    }
}
