use crate::analysis::semantic_graph::build::{
    build_semantic_graph_with_ts_direct_bindings, collect_ts_direct_bindings,
};
use crate::analysis::semantic_graph::cache_key::semantic_graph_provider_parameter_digest;
use crate::analysis::semantic_graph::store::{SEMANTIC_GRAPH_PROVIDER_ID, SemanticGraphOutput};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};
use crate::ts::binding::store::{
    ts_direct_binding_output_digest, ts_direct_binding_provider_parameter_digest,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct SemanticGraphProviderRunOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

/// `polint.semantic_graph` provider entry point (D-16/D-17).
///
/// 7-phase pipeline mirroring `polint.reachability` (S5):
/// 1. project a real-but-minimal graph from existing facts via `build_semantic_graph`
///    (read-only, mutates no upstream family),
/// 2. `normalized()` — stable-key sort,
/// 3. compute the output digest over the stored stable KEYS (an edge key encodes its
///    endpoints; a constraint contributes its referenced nodes' stable keys), never
///    the run-local dense IDs; an empty-output sentinel distinguishes the empty graph,
/// 4. dense IDs are a post-digest read concern assigned inside `from_output`,
/// 5. `db.replace_semantic_graph_facts(...)` stores + referentially validates,
/// 6. on store error return `output_digest: None` so a cache layer never records a
///    hit for un-persisted state,
/// 7. surface the store error as an evidence-bearing diagnostic.
///
/// The digest folds in every consumed upstream provider output digest plus the
/// provider/schema/parameter digests (D-17), so any upstream change or algorithm bump
/// deterministically invalidates the semantic-graph cache.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_semantic_graph_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    calls_output_digest: Digest,
    identity_output_digest: Digest,
    abstract_domains_output_digest: Digest,
    entrypoints_output_digest: Digest,
    reachability_output_digest: Digest,
    type_value_alias_output_digest: Digest,
    symbol_output_digest: Digest,
    module_topology_output_digest: Digest,
    go_syntax_output_digest: Digest,
    ts_syntax_output_digest: Digest,
    semantic_mir_output_digest: Digest,
    go_semantic_output_digest: Digest,
) -> SemanticGraphProviderRunOutput {
    debug_assert_eq!(manifest.id, SEMANTIC_GRAPH_PROVIDER_ID);

    // Phase 1-2: project + normalize. The build is read-only; normalized() fixes the
    // stable-key order the digest is computed over.
    let ts_direct_bindings = collect_ts_direct_bindings(db);
    let ts_direct_binding_output_digest = ts_direct_binding_output_digest(&ts_direct_bindings);
    let output =
        build_semantic_graph_with_ts_direct_bindings(db, &ts_direct_bindings.bindings).normalized();

    // Phase 3: digest over the stored stable KEYS (never dense IDs — see
    // `semantic_graph_output_digest`), with the empty-output sentinel.
    let output_digest = semantic_graph_output_digest(
        db,
        manifest,
        input_snapshot,
        &calls_output_digest,
        &identity_output_digest,
        &abstract_domains_output_digest,
        &entrypoints_output_digest,
        &reachability_output_digest,
        &type_value_alias_output_digest,
        &symbol_output_digest,
        &module_topology_output_digest,
        &go_syntax_output_digest,
        &ts_syntax_output_digest,
        &semantic_mir_output_digest,
        &ts_direct_binding_output_digest,
        &go_semantic_output_digest,
        &output,
    );

    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    // Phase 4-7: store (assigns dense IDs + referentially validates inside
    // from_output). On store error the db keeps its prior state and the facts the
    // digest certifies were not persisted, so return output_digest: None.
    match db.replace_semantic_graph_facts(output) {
        Ok(()) => SemanticGraphProviderRunOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => SemanticGraphProviderRunOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: None,
        },
    }
}

/// Output digest over stable KEYS, never dense IDs (D-17).
///
/// Every node/edge/constraint contribution is composed from content-derived stable
/// keys, NOT the run-local dense `SemanticNodeId`/`SemanticEdgeId` handles: a node's
/// stable key encodes its kind + referenced identity; an edge's stable key already
/// encodes both endpoints by their stable keys; a constraint contributes its stable
/// key plus the stable keys of the nodes it references (resolved through the
/// post-normalize node table). This honors the D-17 "never dense IDs" contract — the
/// digest is invariant under any future change to dense-ID numbering that preserves
/// stable keys.
///
/// The folded upstream digests cover the producers of every fact family
/// `build_semantic_graph` reads: `go`/`ts` syntax (functions, packages), symbol
/// graph (scopes), calls (call sites), type/value/alias (value facts) and semantic
/// MIR (places). `identity`/`abstract_domains`/`entrypoints`/`reachability`/
/// `module_topology` are folded as well so the keystone over-invalidates rather than
/// risks a stale graph as later phases begin consuming them.
#[allow(clippy::too_many_arguments)]
fn semantic_graph_output_digest(
    db: &AnalysisDb,
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    calls_output_digest: &Digest,
    identity_output_digest: &Digest,
    abstract_domains_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    reachability_output_digest: &Digest,
    type_value_alias_output_digest: &Digest,
    symbol_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    go_syntax_output_digest: &Digest,
    ts_syntax_output_digest: &Digest,
    semantic_mir_output_digest: &Digest,
    ts_direct_binding_output_digest: &Digest,
    go_semantic_output_digest: &Digest,
    output: &SemanticGraphOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", semantic_graph_provider_parameter_digest()),
        format!("config={}", input_snapshot.config.digest),
        format!("calls_output={calls_output_digest}"),
        format!("identity_output={identity_output_digest}"),
        format!("abstract_domains_output={abstract_domains_output_digest}"),
        format!("entrypoints_output={entrypoints_output_digest}"),
        format!("reachability_output={reachability_output_digest}"),
        format!("type_value_alias_output={type_value_alias_output_digest}"),
        format!("symbol_graph={symbol_output_digest}"),
        format!("module_topology={module_topology_output_digest}"),
        format!("go_syntax={go_syntax_output_digest}"),
        format!("ts_syntax={ts_syntax_output_digest}"),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("ts_direct_binding_output={ts_direct_binding_output_digest}"),
        format!(
            "ts_direct_binding_parameters={}",
            ts_direct_binding_provider_parameter_digest()
        ),
        format!(
            "go_semantic_output={}",
            go_semantic_output_digest_from_db(db)
        ),
        format!("go_semantic_provider_output={go_semantic_output_digest}"),
    ];
    extend_component_parts(
        &mut parts,
        "go_lifecycle",
        &input_snapshot.go_lifecycle.components,
    );
    extend_component_parts(
        &mut parts,
        "ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );

    // Post-normalize node dense id (== position) -> stable key, so a constraint can
    // contribute its endpoints by their stable keys rather than dense handles.
    let node_key_by_id: Vec<&str> = output
        .nodes
        .iter()
        .map(|node| node.stable_key.as_str())
        .collect();

    parts.extend(
        output
            .nodes
            .iter()
            .map(|node| format!("node={}|prec={:?}", node.stable_key, node.precision)),
    );
    parts.extend(
        output
            .edges
            .iter()
            .map(|edge| format!("edge={}|prec={:?}", edge.stable_key, edge.precision)),
    );
    parts.extend(output.constraints.iter().map(|constraint| {
        let mut refs: Vec<&str> = constraint
            .kind
            .referenced_nodes()
            .into_iter()
            .map(|node| {
                node_key_by_id
                    .get(node.0 as usize)
                    .copied()
                    .unwrap_or("<unresolved>")
            })
            .collect();
        refs.sort_unstable();
        format!(
            "constraint={}|status={:?}|prec={:?}|refs=[{}]",
            constraint.stable_key,
            constraint.status,
            constraint.precision,
            refs.join(","),
        )
    }));
    if output.nodes.is_empty() && output.edges.is_empty() && output.constraints.is_empty() {
        parts.push("semantic_graph_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "semantic_graph_output", &refs)
}

fn go_semantic_output_digest_from_db(db: &AnalysisDb) -> String {
    let mut parts = Vec::new();
    parts.extend(
        db.go_semantic_packages()
            .iter()
            .map(|fact| format!("package={}", fact.stable_key)),
    );
    parts.extend(
        db.go_semantic_functions()
            .iter()
            .map(|fact| format!("function={}", fact.stable_key)),
    );
    parts.extend(
        db.go_semantic_callsites()
            .iter()
            .map(|fact| format!("callsite={}", fact.stable_key)),
    );
    parts.sort();
    if parts.is_empty() {
        return crate::cache::stable_hash(&["go_semantic_output=empty"]);
    }
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::cache::stable_hash(&refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Semantic-graph analysis failed; semantic-graph facts were not stored.",
    )
    .with_evidence("provider", SEMANTIC_GRAPH_PROVIDER_ID)
    .with_evidence("reason", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, FunctionFact, FunctionId, Language, PackageFact, PackageId, Span,
    };
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn manifest() -> &'static ProviderManifest {
        AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == SEMANTIC_GRAPH_PROVIDER_ID)
            .expect("semantic_graph manifest")
    }

    fn snapshot(db: &AnalysisDb) -> InputSnapshot {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        InputSnapshot::from_run_inputs(
            &loaded,
            db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        )
    }

    fn absent(kind: &str) -> Digest {
        Digest::absent(DigestKind::ProviderOutput, kind)
    }

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

    fn db_with_go_main() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("cmd/app/main.go"),
            "cmd/app/main.go".to_string(),
            "package main\nfunc main() {}\n".to_string(),
        );
        db.push_package(PackageFact {
            id: PackageId(0),
            file,
            name: "main".to_string(),
            span: span(file, 0, 1),
            language: Language::Go,
        });
        db.push_function(FunctionFact {
            id: FunctionId(1),
            file,
            name: "main".to_string(),
            span: span(file, 1, 2),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db
    }

    fn run(db: &mut AnalysisDb) -> SemanticGraphProviderRunOutput {
        let snapshot = snapshot(db);
        derive_semantic_graph_with_cache_stats(
            db,
            &snapshot,
            manifest(),
            absent("polint.calls"),
            absent("polint.identity"),
            absent("polint.abstract_domains"),
            absent("polint.entrypoints"),
            absent("polint.reachability"),
            absent("polint.type_value_alias"),
            absent("polint.symbol_graph"),
            absent("polint.module_topology"),
            absent("polint.go.syntax"),
            absent("polint.ts.syntax"),
            absent("polint.semantic_mir"),
            absent("polint.go.semantic"),
        )
    }

    #[test]
    fn empty_db_produces_empty_output_sentinel_digest() {
        let mut db = AnalysisDb::new();
        let output = run(&mut db);
        // An empty graph still produces a present digest (the sentinel part keeps it
        // distinct from a populated graph) and stores cleanly.
        assert!(output.output_digest.is_some());
        assert!(output.diagnostics.is_empty());
        assert!(db.semantic_nodes().is_empty());
    }

    #[test]
    fn populated_db_stores_nodes_and_yields_digest() {
        let mut db = db_with_go_main();
        let output = run(&mut db);
        assert!(output.output_digest.is_some());
        assert!(
            !db.semantic_nodes().is_empty(),
            "expected projected nodes stored"
        );
    }

    #[test]
    fn permuted_fact_insertion_order_produces_identical_output_digest() {
        let mut first = db_with_go_main();
        let mut second = {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("cmd/app/main.go"),
                "cmd/app/main.go".to_string(),
                "package main\nfunc main() {}\n".to_string(),
            );
            db.push_function(FunctionFact {
                id: FunctionId(7),
                file,
                name: "main".to_string(),
                span: span(file, 1, 2),
                language: Language::Go,
                is_test: false,
                is_exported: false,
                cyclomatic_complexity: 1,
                calls: Vec::new(),
            });
            db.push_package(PackageFact {
                id: PackageId(0),
                file,
                name: "main".to_string(),
                span: span(file, 0, 1),
                language: Language::Go,
            });
            db
        };
        let first_digest = run(&mut first).output_digest;
        let second_digest = run(&mut second).output_digest;
        assert_eq!(first_digest, second_digest);
    }

    #[test]
    fn upstream_digest_change_invalidates_output_digest() {
        // Two runs over the SAME db facts but with a different upstream calls digest
        // must produce different output digests (D-17: every consumed provider output
        // digest is folded in).
        let snapshot = snapshot(&AnalysisDb::new());
        let mut db_a = db_with_go_main();
        let with_absent = derive_semantic_graph_with_cache_stats(
            &mut db_a,
            &snapshot,
            manifest(),
            absent("polint.calls"),
            absent("polint.identity"),
            absent("polint.abstract_domains"),
            absent("polint.entrypoints"),
            absent("polint.reachability"),
            absent("polint.type_value_alias"),
            absent("polint.symbol_graph"),
            absent("polint.module_topology"),
            absent("polint.go.syntax"),
            absent("polint.ts.syntax"),
            absent("polint.semantic_mir"),
            absent("polint.go.semantic"),
        )
        .output_digest;
        let mut db_b = db_with_go_main();
        let with_present = derive_semantic_graph_with_cache_stats(
            &mut db_b,
            &snapshot,
            manifest(),
            Digest::from_parts(DigestKind::ProviderOutput, "polint.calls", &["changed"]),
            absent("polint.identity"),
            absent("polint.abstract_domains"),
            absent("polint.entrypoints"),
            absent("polint.reachability"),
            absent("polint.type_value_alias"),
            absent("polint.symbol_graph"),
            absent("polint.module_topology"),
            absent("polint.go.syntax"),
            absent("polint.ts.syntax"),
            absent("polint.semantic_mir"),
            absent("polint.go.semantic"),
        )
        .output_digest;
        assert_ne!(with_absent, with_present);
    }

    #[test]
    fn output_digest_folds_ts_direct_binding_and_module_topology_digests() {
        let snapshot = snapshot(&AnalysisDb::new());
        let output = SemanticGraphOutput::empty();
        let base_ts_direct =
            Digest::from_parts(DigestKind::ProviderOutput, "ts_direct_binding", &["base"]);
        let changed_ts_direct = Digest::from_parts(
            DigestKind::ProviderOutput,
            "ts_direct_binding",
            &["changed"],
        );
        let base_module_topology =
            Digest::from_parts(DigestKind::ProviderOutput, "module_topology", &["base"]);
        let changed_module_topology =
            Digest::from_parts(DigestKind::ProviderOutput, "module_topology", &["changed"]);
        let db = AnalysisDb::new();

        let base = semantic_graph_output_digest(
            &db,
            manifest(),
            &snapshot,
            &absent("polint.calls"),
            &absent("polint.identity"),
            &absent("polint.abstract_domains"),
            &absent("polint.entrypoints"),
            &absent("polint.reachability"),
            &absent("polint.type_value_alias"),
            &absent("polint.symbol_graph"),
            &base_module_topology,
            &absent("polint.go.syntax"),
            &absent("polint.ts.syntax"),
            &absent("polint.semantic_mir"),
            &base_ts_direct,
            &absent("polint.go.semantic"),
            &output,
        );
        let changed_direct = semantic_graph_output_digest(
            &db,
            manifest(),
            &snapshot,
            &absent("polint.calls"),
            &absent("polint.identity"),
            &absent("polint.abstract_domains"),
            &absent("polint.entrypoints"),
            &absent("polint.reachability"),
            &absent("polint.type_value_alias"),
            &absent("polint.symbol_graph"),
            &base_module_topology,
            &absent("polint.go.syntax"),
            &absent("polint.ts.syntax"),
            &absent("polint.semantic_mir"),
            &changed_ts_direct,
            &absent("polint.go.semantic"),
            &output,
        );
        let changed_topology = semantic_graph_output_digest(
            &db,
            manifest(),
            &snapshot,
            &absent("polint.calls"),
            &absent("polint.identity"),
            &absent("polint.abstract_domains"),
            &absent("polint.entrypoints"),
            &absent("polint.reachability"),
            &absent("polint.type_value_alias"),
            &absent("polint.symbol_graph"),
            &changed_module_topology,
            &absent("polint.go.syntax"),
            &absent("polint.ts.syntax"),
            &absent("polint.semantic_mir"),
            &base_ts_direct,
            &absent("polint.go.semantic"),
            &output,
        );

        assert_ne!(base, changed_direct);
        assert_ne!(base, changed_topology);
    }

    #[test]
    fn provider_manifests_list_semantic_graph_between_type_value_alias_and_refined_calls() {
        let manifests = AnalysisKernel::provider_manifests();
        let tva = manifests
            .iter()
            .position(|manifest| manifest.id == "polint.type_value_alias")
            .expect("type_value_alias present");
        assert_eq!(manifests[tva + 1].id, "polint.semantic_graph");
        assert_eq!(manifests[tva + 2].id, "polint.solver");
        assert_eq!(manifests[tva + 3].id, "polint.refined_calls");
    }

    #[test]
    fn semantic_graph_manifest_precision_ceiling_is_setup_aware() {
        use crate::analysis_kernel::PrecisionCeiling;
        assert_eq!(manifest().precision_ceiling, PrecisionCeiling::SetupAware);
    }
}
