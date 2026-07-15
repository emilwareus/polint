use crate::analysis::adaptation::budget::AdaptationModelBudget;
use crate::analysis::adaptation::cache_key::adaptation_model_digest;
use crate::analysis::adaptation::discovery::AdaptationModelInventory;
use crate::analysis::adaptation::loader::load_model_file;
use crate::analysis::adaptation::store::AdaptationModelStore;
use crate::analysis::adaptation::validate::ValidationUniverse;
use crate::analysis::semantic_graph::build::{
    build_semantic_graph_with_ts_direct_binding_collection,
    build_semantic_graph_with_ts_direct_binding_collection_and_adaptation_models,
    collect_ts_direct_binding_collection,
};
use crate::analysis::semantic_graph::cache_key::semantic_graph_provider_parameter_digest;
use crate::analysis::semantic_graph::store::{SEMANTIC_GRAPH_PROVIDER_ID, SemanticGraphOutput};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot, input_component_identity_rows,
};
use crate::config::LoadedConfig;
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};
use crate::ts::binding::store::{
    ts_direct_binding_output_digest, ts_direct_binding_provider_parameter_digest,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct SemanticGraphProviderRunOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome,
    pub(crate) output_digest: Option<Digest>,
}

/// `polint.semantic_graph` provider entry point (D-16/D-17).
///
/// 8-stage pipeline mirroring `polint.reachability` (S5):
/// 1. refresh the private TS object-model store from TS/JS source files,
/// 2. project a real-but-minimal graph from existing facts via `build_semantic_graph`
///    (the build itself is read-only, mutating no upstream family),
/// 3. `normalized()` — stable-key sort,
/// 4. compute the output digest over the stored stable KEYS (an edge key encodes its
///    endpoints; a constraint contributes its referenced nodes' stable keys), never
///    the run-local dense IDs; an empty-output sentinel distinguishes the empty graph,
/// 5. dense IDs are a post-digest read concern assigned inside `from_output`,
/// 6. `db.replace_semantic_graph_facts(...)` stores + referentially validates,
/// 7. on store error return `output_digest: None` so a cache layer never records a
///    hit for un-persisted state,
/// 8. surface the store error as an evidence-bearing diagnostic.
///
/// The digest folds in every consumed upstream provider output digest plus the
/// provider/schema/parameter digests (D-17), so any upstream change or algorithm bump
/// deterministically invalidates the semantic-graph cache.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_semantic_graph_with_cache_stats(
    db: &mut AnalysisDb,
    loaded: &LoadedConfig,
    adaptation_budget: AdaptationModelBudget,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    calls_output_digest: Digest,
    identity_output_digest: Digest,
    abstract_domains_output_digest: Digest,
    entrypoints_output_digest: Digest,
    type_value_alias_output_digest: Digest,
    symbol_output_digest: Digest,
    module_topology_output_digest: Digest,
    go_syntax_output_digest: Digest,
    ts_syntax_output_digest: Digest,
    semantic_mir_output_digest: Digest,
    go_semantic_output_digest: Digest,
) -> SemanticGraphProviderRunOutput {
    let adaptation_models =
        AdaptationModelInventory::discover(&loaded.root, adaptation_budget.max_model_files);
    derive_semantic_graph_with_cache_stats_and_models(
        db,
        adaptation_budget,
        &adaptation_models,
        input_snapshot,
        manifest,
        calls_output_digest,
        identity_output_digest,
        abstract_domains_output_digest,
        entrypoints_output_digest,
        type_value_alias_output_digest,
        symbol_output_digest,
        module_topology_output_digest,
        go_syntax_output_digest,
        ts_syntax_output_digest,
        semantic_mir_output_digest,
        go_semantic_output_digest,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_semantic_graph_with_cache_stats_and_models(
    db: &mut AnalysisDb,
    adaptation_budget: AdaptationModelBudget,
    adaptation_model_inventory: &AdaptationModelInventory,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    calls_output_digest: Digest,
    identity_output_digest: Digest,
    abstract_domains_output_digest: Digest,
    entrypoints_output_digest: Digest,
    type_value_alias_output_digest: Digest,
    symbol_output_digest: Digest,
    module_topology_output_digest: Digest,
    go_syntax_output_digest: Digest,
    ts_syntax_output_digest: Digest,
    semantic_mir_output_digest: Digest,
    go_semantic_output_digest: Digest,
) -> SemanticGraphProviderRunOutput {
    debug_assert_eq!(manifest.id, SEMANTIC_GRAPH_PROVIDER_ID);

    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    // Step: collect the private TS analysis side inputs once. This keeps
    // direct bindings, object-model rows, and token-source flow projection on
    // the same parse/semantic pass per TS file.
    let mut ts_direct_bindings = collect_ts_direct_binding_collection(db);

    // Step: refresh private TS object-model rows. This keeps the projection's
    // consumed object/property facts deterministic and digest-visible without
    // promoting a public object-model provider surface.
    let object_model = ts_direct_bindings.take_object_model_output();
    if let Err(error) = db.replace_ts_object_model_facts(object_model) {
        return SemanticGraphProviderRunOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed,
            output_digest: None,
        };
    }

    // Step: project + normalize. The build is read-only; normalized() fixes the
    // stable-key order the digest is computed over.
    let ts_direct_binding_output_digest =
        ts_direct_binding_output_digest(ts_direct_bindings.output());
    let base_output =
        build_semantic_graph_with_ts_direct_binding_collection(db, &ts_direct_bindings)
            .normalized();
    let adaptation_models =
        collect_adaptation_model_input(adaptation_model_inventory, &base_output, adaptation_budget);
    let output = if adaptation_models.store.accepted().is_empty() {
        base_output
    } else {
        build_semantic_graph_with_ts_direct_binding_collection_and_adaptation_models(
            db,
            &ts_direct_bindings,
            &adaptation_models.store,
        )
        .normalized()
    };

    // Step: digest over the stored stable KEYS (never dense IDs — see
    // `semantic_graph_output_digest`), with the empty-output sentinel.
    let output_digest = semantic_graph_output_digest(
        db,
        manifest,
        input_snapshot,
        &calls_output_digest,
        &identity_output_digest,
        &abstract_domains_output_digest,
        &entrypoints_output_digest,
        &type_value_alias_output_digest,
        &symbol_output_digest,
        &module_topology_output_digest,
        &go_syntax_output_digest,
        &ts_syntax_output_digest,
        &semantic_mir_output_digest,
        &ts_direct_binding_output_digest,
        &adaptation_models.digest,
        &go_semantic_output_digest,
        &output,
    );

    // Step: store (assigns dense IDs + referentially validates inside
    // from_output). On store error the db keeps its prior state and the facts the
    // digest certifies were not persisted, so return output_digest: None.
    match db.replace_normalized_semantic_graph_facts(output) {
        Ok(()) => {
            db.replace_adaptation_model_facts(
                adaptation_models.store.accepted().to_vec(),
                adaptation_models.store.rejected().to_vec(),
            );
            SemanticGraphProviderRunOutput {
                diagnostics: adaptation_models.diagnostics,
                cache_stats,
                execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Succeeded,
                output_digest: Some(output_digest),
            }
        }
        Err(error) => SemanticGraphProviderRunOutput {
            diagnostics: {
                let mut diagnostics = adaptation_models.diagnostics;
                diagnostics.push(provider_error_diagnostic(error.to_string()));
                diagnostics
            },
            cache_stats,
            execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed,
            output_digest: None,
        },
    }
}

#[derive(Debug)]
struct AdaptationModelProviderInput {
    store: AdaptationModelStore,
    diagnostics: Vec<Diagnostic>,
    digest: Digest,
}

fn collect_adaptation_model_input(
    inventory: &AdaptationModelInventory,
    base_output: &SemanticGraphOutput,
    budget: AdaptationModelBudget,
) -> AdaptationModelProviderInput {
    let mut diagnostics = inventory
        .issues
        .iter()
        .map(|issue| {
            adaptation_model_diagnostic(
                &issue.relative_path,
                &issue.message,
                issue.evidence_key,
                &issue.evidence_value,
            )
        })
        .collect::<Vec<_>>();
    let mut digest_parts = budget.digest_parts();
    digest_parts.push(format!("model_files={}", inventory.digest));
    digest_parts.push(format!("model_file_count={}", inventory.files.len()));

    let mut facts = Vec::new();
    for file in &inventory.files {
        match load_model_file(&file.relative_path, &file.contents) {
            Ok(mut loaded_facts) => facts.append(&mut loaded_facts),
            Err(error) => {
                diagnostics.push(adaptation_model_diagnostic(
                    &file.relative_path,
                    format!("Adaptation model file was ignored: {error}"),
                    "load_error",
                    error.to_string(),
                ));
                digest_parts.push(format!(
                    "model_file={}:load_error={error}",
                    file.relative_path
                ));
            }
        }
    }

    let node_keys = base_output
        .nodes
        .iter()
        .map(|node| node.stable_key.clone())
        .collect::<Vec<_>>();
    let universe = ValidationUniverse::new(node_keys.clone(), node_keys);
    let store = AdaptationModelStore::build(facts, &universe, budget);
    let model_digest = adaptation_model_digest(&store, budget);
    digest_parts.push(format!("validated_models={model_digest}"));

    digest_parts.sort();
    let refs = digest_parts.iter().map(String::as_str).collect::<Vec<_>>();
    let digest = Digest::from_parts(
        DigestKind::ProviderParameters,
        "adaptation_model_input",
        &refs,
    );

    AdaptationModelProviderInput {
        store,
        diagnostics,
        digest,
    }
}

fn adaptation_model_diagnostic(
    path: impl AsRef<str>,
    message: impl Into<String>,
    evidence_key: &'static str,
    evidence_value: impl Into<String>,
) -> Diagnostic {
    Diagnostic::warning(
        "polint/adaptation-model",
        path.as_ref(),
        TextRange::point(1, 1),
        message.into(),
    )
    .with_evidence("provider", SEMANTIC_GRAPH_PROVIDER_ID)
    .with_evidence(evidence_key, evidence_value.into())
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
/// MIR (places). `identity`/`abstract_domains`/`entrypoints`/`module_topology` are
/// folded as well so every declared input has an explicit consumer.
#[allow(clippy::too_many_arguments)]
fn semantic_graph_output_digest(
    db: &AnalysisDb,
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    calls_output_digest: &Digest,
    identity_output_digest: &Digest,
    abstract_domains_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    type_value_alias_output_digest: &Digest,
    symbol_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    go_syntax_output_digest: &Digest,
    ts_syntax_output_digest: &Digest,
    semantic_mir_output_digest: &Digest,
    ts_direct_binding_output_digest: &Digest,
    adaptation_model_input_digest: &Digest,
    go_semantic_output_digest: &Digest,
    output: &SemanticGraphOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", semantic_graph_provider_parameter_digest()),
        format!("calls_output={calls_output_digest}"),
        format!("identity_output={identity_output_digest}"),
        format!("abstract_domains_output={abstract_domains_output_digest}"),
        format!("entrypoints_output={entrypoints_output_digest}"),
        format!("type_value_alias_output={type_value_alias_output_digest}"),
        format!("symbol_graph={symbol_output_digest}"),
        format!("module_topology={module_topology_output_digest}"),
        format!("go_syntax={go_syntax_output_digest}"),
        format!("ts_syntax={ts_syntax_output_digest}"),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("ts_direct_binding_output={ts_direct_binding_output_digest}"),
        format!("adaptation_model_input={adaptation_model_input_digest}"),
        format!(
            "ts_direct_binding_parameters={}",
            ts_direct_binding_provider_parameter_digest()
        ),
        format!(
            "go_semantic_output={}",
            go_semantic_output_digest_from_db(db)
        ),
        format!("go_semantic_provider_output={go_semantic_output_digest}"),
        format!(
            "ts_object_model_output={}",
            ts_object_model_output_digest_from_db(db)
        ),
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

fn ts_object_model_output_digest_from_db(db: &AnalysisDb) -> String {
    let mut parts = Vec::new();
    parts.extend(db.ts_object_allocations().iter().map(|fact| {
        format!(
            "allocation={}|kind={}|status={}|reason={}",
            fact.stable_key,
            fact.kind.as_str(),
            fact.status.as_str(),
            fact.status.reason().unwrap_or("")
        )
    }));
    parts.extend(db.ts_property_writes().iter().map(|fact| {
        format!(
            "write={}|base={}|field={}|status={}|reason={}",
            fact.stable_key,
            fact.base_object_stable_key,
            fact.property_key.stable_label(),
            fact.status.as_str(),
            fact.status.reason().unwrap_or("")
        )
    }));
    parts.extend(db.ts_property_reads().iter().map(|fact| {
        format!(
            "read={}|base={}|field={}|status={}|reason={}",
            fact.stable_key,
            fact.base_object_stable_key,
            fact.property_key.stable_label(),
            fact.status.as_str(),
            fact.status.reason().unwrap_or("")
        )
    }));
    parts.extend(db.ts_receiver_bindings().iter().map(|fact| {
        format!(
            "receiver={}|kind={}|status={}|reason={}",
            fact.stable_key,
            fact.kind.as_str(),
            fact.status.as_str(),
            fact.status.reason().unwrap_or("")
        )
    }));
    parts.extend(db.ts_prototype_links().iter().map(|fact| {
        format!(
            "prototype={}|kind={}|object={}|prototype={}|status={}|reason={}",
            fact.stable_key,
            fact.kind.as_str(),
            fact.object_stable_key,
            fact.prototype_stable_key,
            fact.status.as_str(),
            fact.status.reason().unwrap_or("")
        )
    }));
    parts.sort();
    if parts.is_empty() {
        return crate::cache::stable_hash(&["ts_object_model_output=empty"]);
    }
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::cache::stable_hash(&refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(input_component_identity_rows(prefix, components));
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
    use crate::analysis::semantic_graph::build::build_semantic_graph_with_ts_direct_bindings;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, FunctionFact, FunctionId, Language, PackageFact, PackageId, Span,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn manifest() -> &'static ProviderManifest {
        AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == SEMANTIC_GRAPH_PROVIDER_ID)
            .expect("semantic_graph manifest")
    }

    fn loaded_config(root: &Path) -> LoadedConfig {
        fs::write(root.join(".polint.toml"), "").expect("config");
        load_config(root).expect("config loads")
    }

    fn snapshot_from_loaded(db: &AnalysisDb, loaded: &LoadedConfig) -> InputSnapshot {
        let empty_plan = AnalysisPlan::empty();
        assert!(empty_plan.requested_capability_snapshots().is_empty());
        let identity_sources = InputSnapshot::identity_sources_from_plan(loaded, &empty_plan);
        assert!(identity_sources.requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities")
        );

        InputSnapshot::from_run_inputs_with_plan(
            loaded,
            db,
            "config-a",
            "rules-a",
            &empty_plan,
            AnalysisKernel::provider_manifests(),
        )
    }

    fn snapshot(db: &AnalysisDb) -> InputSnapshot {
        let temp = tempdir().expect("tempdir");
        let loaded = loaded_config(temp.path());
        snapshot_from_loaded(db, &loaded)
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
        let temp = tempdir().expect("tempdir");
        let loaded = loaded_config(temp.path());
        run_with_loaded(db, &loaded)
    }

    fn run_with_loaded(
        db: &mut AnalysisDb,
        loaded: &LoadedConfig,
    ) -> SemanticGraphProviderRunOutput {
        let snapshot = snapshot_from_loaded(db, loaded);
        derive_semantic_graph_with_cache_stats(
            db,
            loaded,
            AdaptationModelBudget::default(),
            &snapshot,
            manifest(),
            absent("polint.calls"),
            absent("polint.identity"),
            absent("polint.abstract_domains"),
            absent("polint.entrypoints"),
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
    fn provider_refreshes_ts_object_model_rows_before_projection() {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "const holder = { target() {} }; holder.target();\n".to_string(),
        );

        let output = run(&mut db);

        assert!(output.output_digest.is_some());
        assert!(!db.ts_object_allocations().is_empty());
        assert!(
            db.semantic_constraints().iter().any(|constraint| {
                matches!(
                    constraint.kind,
                    crate::analysis::semantic_graph::constraints::ConstraintKind::Alloc { .. }
                        | crate::analysis::semantic_graph::constraints::ConstraintKind::FieldStore { .. }
                        | crate::analysis::semantic_graph::constraints::ConstraintKind::FieldLoad { .. }
                )
            }),
            "expected object-model constraints"
        );
    }

    #[test]
    fn provider_loads_repo_local_adaptation_models_into_model_constraints() {
        let temp = tempdir().expect("tempdir");
        let loaded = loaded_config(temp.path());
        let mut db = db_with_go_main();
        let base = build_semantic_graph_with_ts_direct_bindings(&db, &[]).normalized();
        let source = &base.nodes[0].stable_key;
        let target = &base.nodes[1].stable_key;
        let model_dir = temp.path().join(".polint/models");
        fs::create_dir_all(&model_dir).expect("model dir");
        fs::write(
            model_dir.join("framework.toml"),
            format!(
                r#"
[[facts]]
source_pattern = "{source}"
target_pattern = "{target}"
confidence = "heuristic"
language = "go"
scope = "cmd/app/main.go"
evidence = ["cmd/app/main.go:1"]
"#
            ),
        )
        .expect("model file");

        let output = run_with_loaded(&mut db, &loaded);

        assert!(output.output_digest.is_some());
        assert!(output.diagnostics.is_empty());
        assert!(
            db.semantic_constraints().iter().any(|constraint| matches!(
                constraint.kind,
                crate::analysis::semantic_graph::constraints::ConstraintKind::ModelEdge { .. }
            )),
            "expected loaded adaptation model to lower into a ModelEdge constraint"
        );
    }

    #[test]
    fn adaptation_model_discovery_stops_at_model_file_budget() {
        let temp = tempdir().expect("tempdir");
        let model_dir = temp.path().join(".polint/models");
        fs::create_dir_all(&model_dir).expect("model dir");
        fs::write(model_dir.join("a.toml"), "").expect("first model");
        fs::write(model_dir.join("b.toml"), "").expect("second model");

        let discovery = AdaptationModelInventory::discover(temp.path(), 1);

        assert_eq!(
            discovery
                .files
                .iter()
                .map(|file| file.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec![".polint/models/a.toml"]
        );
        assert_eq!(
            discovery.budget_exceeded_at.as_deref(),
            Some(".polint/models/b.toml")
        );
        assert_eq!(discovery.issues.len(), 1);
        assert_eq!(discovery.issues[0].evidence_key, "budget");
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
        let temp = tempdir().expect("tempdir");
        let loaded = loaded_config(temp.path());
        let snapshot = snapshot_from_loaded(&AnalysisDb::new(), &loaded);
        let mut db_a = db_with_go_main();
        let with_absent = derive_semantic_graph_with_cache_stats(
            &mut db_a,
            &loaded,
            AdaptationModelBudget::default(),
            &snapshot,
            manifest(),
            absent("polint.calls"),
            absent("polint.identity"),
            absent("polint.abstract_domains"),
            absent("polint.entrypoints"),
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
            &loaded,
            AdaptationModelBudget::default(),
            &snapshot,
            manifest(),
            Digest::from_parts(DigestKind::ProviderOutput, "polint.calls", &["changed"]),
            absent("polint.identity"),
            absent("polint.abstract_domains"),
            absent("polint.entrypoints"),
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
            &absent("polint.type_value_alias"),
            &absent("polint.symbol_graph"),
            &base_module_topology,
            &absent("polint.go.syntax"),
            &absent("polint.ts.syntax"),
            &absent("polint.semantic_mir"),
            &base_ts_direct,
            &absent("adaptation_model_input"),
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
            &absent("polint.type_value_alias"),
            &absent("polint.symbol_graph"),
            &base_module_topology,
            &absent("polint.go.syntax"),
            &absent("polint.ts.syntax"),
            &absent("polint.semantic_mir"),
            &changed_ts_direct,
            &absent("adaptation_model_input"),
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
            &absent("polint.type_value_alias"),
            &absent("polint.symbol_graph"),
            &changed_module_topology,
            &absent("polint.go.syntax"),
            &absent("polint.ts.syntax"),
            &absent("polint.semantic_mir"),
            &base_ts_direct,
            &absent("adaptation_model_input"),
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
