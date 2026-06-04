use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::analysis::adaptation::budget::AdaptationModelBudget;
use crate::analysis::adaptation::cache_key::adaptation_model_digest;
use crate::analysis::adaptation::loader::load_model_file;
use crate::analysis::adaptation::store::AdaptationModelStore;
use crate::analysis::adaptation::validate::ValidationUniverse;
use crate::analysis::semantic_graph::build::{
    build_semantic_graph_with_ts_direct_bindings,
    build_semantic_graph_with_ts_direct_bindings_and_adaptation_models, collect_ts_direct_bindings,
};
use crate::analysis::semantic_graph::cache_key::semantic_graph_provider_parameter_digest;
use crate::analysis::semantic_graph::store::{SEMANTIC_GRAPH_PROVIDER_ID, SemanticGraphOutput};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::config::LoadedConfig;
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};
use crate::repo_fs::read_repo_file_to_string_with_limit;
use crate::ts::binding::store::{
    ts_direct_binding_output_digest, ts_direct_binding_provider_parameter_digest,
};
use crate::ts::object_model::extract::extract_ts_object_model;
use crate::ts::object_model::store::TsObjectModelOutput;

const ADAPTATION_MODEL_DIR: &str = ".polint/models";
const ADAPTATION_MODEL_MAX_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, Default)]
pub(crate) struct SemanticGraphProviderRunOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

/// `polint.semantic_graph` provider entry point (D-16/D-17).
///
/// 8-phase pipeline mirroring `polint.reachability` (S5):
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

    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    // Phase 1: refresh private TS object-model rows. This keeps the projection's
    // consumed object/property facts deterministic and digest-visible without
    // promoting a public object-model provider surface.
    let object_model = collect_ts_object_model(db);
    if let Err(error) = db.replace_ts_object_model_facts(object_model) {
        return SemanticGraphProviderRunOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: None,
        };
    }

    // Phase 2-3: project + normalize. The build is read-only; normalized() fixes the
    // stable-key order the digest is computed over.
    let ts_direct_bindings = collect_ts_direct_bindings(db);
    let ts_direct_binding_output_digest = ts_direct_binding_output_digest(&ts_direct_bindings);
    let base_output =
        build_semantic_graph_with_ts_direct_bindings(db, &ts_direct_bindings.bindings).normalized();
    let adaptation_models = collect_adaptation_model_input(loaded, &base_output, adaptation_budget);
    let output = build_semantic_graph_with_ts_direct_bindings_and_adaptation_models(
        db,
        &ts_direct_bindings.bindings,
        &adaptation_models.store,
    )
    .normalized();

    // Phase 4: digest over the stored stable KEYS (never dense IDs — see
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
        &adaptation_models.digest,
        &go_semantic_output_digest,
        &output,
    );

    // Phase 5-8: store (assigns dense IDs + referentially validates inside
    // from_output). On store error the db keeps its prior state and the facts the
    // digest certifies were not persisted, so return output_digest: None.
    match db.replace_semantic_graph_facts(output) {
        Ok(()) => SemanticGraphProviderRunOutput {
            diagnostics: adaptation_models.diagnostics,
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => SemanticGraphProviderRunOutput {
            diagnostics: {
                let mut diagnostics = adaptation_models.diagnostics;
                diagnostics.push(provider_error_diagnostic(error.to_string()));
                diagnostics
            },
            cache_stats,
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

#[derive(Debug, Default)]
struct AdaptationModelDiscovery {
    paths: Vec<String>,
    budget_exceeded_at: Option<String>,
}

#[derive(Debug)]
enum AdaptationModelDiscoveryEntry {
    Directory(PathBuf),
    File,
}

fn collect_adaptation_model_input(
    loaded: &LoadedConfig,
    base_output: &SemanticGraphOutput,
    budget: AdaptationModelBudget,
) -> AdaptationModelProviderInput {
    let mut diagnostics = Vec::new();
    let mut digest_parts = budget.digest_parts();
    let discovery = discover_adaptation_model_paths(
        &loaded.root,
        budget.max_model_files,
        &mut diagnostics,
        &mut digest_parts,
    );
    let model_paths = discovery.paths;
    digest_parts.push(format!("model_file_count={}", model_paths.len()));
    if let Some(relative_path) = discovery.budget_exceeded_at {
        diagnostics.push(adaptation_model_diagnostic(
            &relative_path,
            "Adaptation model discovery stopped because the model-file budget was exceeded.",
            "budget",
            format!("max_model_files={}", budget.max_model_files),
        ));
        digest_parts.push(format!(
            "model_discovery_budget_exceeded_at={relative_path}"
        ));
    }

    let mut facts = Vec::new();
    for relative_path in &model_paths {
        match read_repo_file_to_string_with_limit(
            &loaded.root,
            relative_path,
            ADAPTATION_MODEL_MAX_BYTES,
        ) {
            Ok(contents) => {
                digest_parts.push(format!(
                    "model_file={relative_path}:content={}",
                    crate::cache::stable_hash(&[contents.as_str()])
                ));
                match load_model_file(relative_path, &contents) {
                    Ok(mut loaded_facts) => facts.append(&mut loaded_facts),
                    Err(error) => {
                        diagnostics.push(adaptation_model_diagnostic(
                            relative_path,
                            format!("Adaptation model file was ignored: {error}"),
                            "load_error",
                            error.to_string(),
                        ));
                        digest_parts.push(format!("model_file={relative_path}:load_error={error}"));
                    }
                }
            }
            Err(error) => {
                diagnostics.push(adaptation_model_diagnostic(
                    relative_path,
                    format!("Adaptation model file was ignored: {error}"),
                    "read_error",
                    error.stable_reason().to_string(),
                ));
                digest_parts.push(format!(
                    "model_file={relative_path}:read_error={}",
                    error.stable_reason()
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

fn discover_adaptation_model_paths(
    root: &Path,
    max_model_files: usize,
    diagnostics: &mut Vec<Diagnostic>,
    digest_parts: &mut Vec<String>,
) -> AdaptationModelDiscovery {
    let model_root = root.join(ADAPTATION_MODEL_DIR);
    let metadata = match fs::symlink_metadata(&model_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return AdaptationModelDiscovery::default();
        }
        Err(_) => {
            diagnostics.push(adaptation_model_diagnostic(
                ADAPTATION_MODEL_DIR,
                "Adaptation model directory could not be read.",
                "read_error",
                "metadata unavailable",
            ));
            digest_parts.push(format!(
                "model_path={ADAPTATION_MODEL_DIR}:read_error=metadata unavailable"
            ));
            return AdaptationModelDiscovery::default();
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        diagnostics.push(adaptation_model_diagnostic(
            ADAPTATION_MODEL_DIR,
            "Adaptation model directory was ignored because it is not a regular directory.",
            "read_error",
            "not a directory",
        ));
        digest_parts.push(format!(
            "model_path={ADAPTATION_MODEL_DIR}:read_error=not a directory"
        ));
        return AdaptationModelDiscovery::default();
    }

    let mut budget_exceeded_at = None;
    let mut paths = Vec::new();
    let mut pending = BTreeMap::from([(
        ADAPTATION_MODEL_DIR.to_string(),
        AdaptationModelDiscoveryEntry::Directory(model_root),
    )]);
    while let Some((relative_dir, entry)) = pending.pop_first() {
        let AdaptationModelDiscoveryEntry::Directory(absolute_dir) = entry else {
            if paths.len() >= max_model_files {
                budget_exceeded_at = Some(relative_dir);
                break;
            }
            paths.push(relative_dir);
            continue;
        };

        let raw_entries = match fs::read_dir(&absolute_dir) {
            Ok(entries) => entries,
            Err(_) => {
                diagnostics.push(adaptation_model_diagnostic(
                    &relative_dir,
                    "Adaptation model directory entry could not be read.",
                    "read_error",
                    "read_dir failed",
                ));
                digest_parts.push(format!(
                    "model_path={relative_dir}:read_error=read_dir failed"
                ));
                continue;
            }
        };
        let mut entries = Vec::new();
        for entry in raw_entries {
            match entry {
                Ok(entry) => entries.push(entry),
                Err(_) => {
                    diagnostics.push(adaptation_model_diagnostic(
                        &relative_dir,
                        "Adaptation model directory entry could not be read.",
                        "read_error",
                        "directory entry unavailable",
                    ));
                    digest_parts.push(format!(
                        "model_path={relative_dir}:read_error=directory entry unavailable"
                    ));
                }
            }
        }
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let relative_path = format!("{relative_dir}/{file_name}");
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => {
                    diagnostics.push(adaptation_model_diagnostic(
                        &relative_path,
                        "Adaptation model path could not be read.",
                        "read_error",
                        "metadata unavailable",
                    ));
                    digest_parts.push(format!(
                        "model_path={relative_path}:read_error=metadata unavailable"
                    ));
                    continue;
                }
            };
            if metadata.file_type().is_symlink() {
                diagnostics.push(adaptation_model_diagnostic(
                    &relative_path,
                    "Adaptation model path was ignored because symlinks are not allowed.",
                    "read_error",
                    "symlink",
                ));
                digest_parts.push(format!("model_path={relative_path}:read_error=symlink"));
                continue;
            }
            if metadata.is_dir() {
                pending.insert(
                    relative_path,
                    AdaptationModelDiscoveryEntry::Directory(path),
                );
            } else if metadata.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "toml")
            {
                pending.insert(relative_path, AdaptationModelDiscoveryEntry::File);
            }
        }
    }
    AdaptationModelDiscovery {
        paths,
        budget_exceeded_at,
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

fn collect_ts_object_model(db: &AnalysisDb) -> TsObjectModelOutput {
    let mut output = TsObjectModelOutput::default();
    for file in db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
    {
        let file_output = extract_ts_object_model(file);
        output.allocations.extend(file_output.allocations);
        output.property_writes.extend(file_output.property_writes);
        output.property_reads.extend(file_output.property_reads);
        output
            .receiver_bindings
            .extend(file_output.receiver_bindings);
        output.prototype_links.extend(file_output.prototype_links);
    }
    output
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
    adaptation_model_input_digest: &Digest,
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
        InputSnapshot::from_run_inputs(
            loaded,
            db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
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

        let mut diagnostics = Vec::new();
        let mut digest_parts = Vec::new();
        let discovery =
            discover_adaptation_model_paths(temp.path(), 1, &mut diagnostics, &mut digest_parts);

        assert_eq!(discovery.paths, vec![".polint/models/a.toml"]);
        assert_eq!(
            discovery.budget_exceeded_at.as_deref(),
            Some(".polint/models/b.toml")
        );
        assert!(diagnostics.is_empty());
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
            &loaded,
            AdaptationModelBudget::default(),
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
            &absent("polint.reachability"),
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
            &absent("polint.reachability"),
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
