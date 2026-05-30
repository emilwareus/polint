use serde::Serialize;
use std::fmt::Debug;

use crate::analysis::semantic_graph::build::build_semantic_graph;
use crate::analysis::semantic_graph::cache_key::semantic_graph_provider_parameter_digest;
use crate::analysis::semantic_graph::store::{SEMANTIC_GRAPH_PROVIDER_ID, SemanticGraphOutput};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

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
/// 3. compute the output digest over EXACTLY the stored stable serde payloads
///    (`#[serde(skip)]` strips dense IDs; an empty-output sentinel distinguishes the
///    empty graph), never dense IDs,
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
) -> SemanticGraphProviderRunOutput {
    debug_assert_eq!(manifest.id, SEMANTIC_GRAPH_PROVIDER_ID);

    // Phase 1-2: project + normalize. The build is read-only; normalized() fixes the
    // stable-key order the digest is computed over.
    let output = build_semantic_graph(db).normalized();

    // Phase 3: digest over the stored stable payloads (never dense IDs; the dense
    // `id` fields carry `#[serde(skip)]`), with the empty-output sentinel.
    let output_digest = semantic_graph_output_digest(
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

/// Output digest over stable payloads, never dense IDs (D-17).
#[allow(clippy::too_many_arguments)]
fn semantic_graph_output_digest(
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

    parts.extend(
        output
            .nodes
            .iter()
            .map(|node| format!("node={}", stable_fact_payload(node))),
    );
    parts.extend(
        output
            .edges
            .iter()
            .map(|edge| format!("edge={}", stable_fact_payload(edge))),
    );
    parts.extend(
        output
            .constraints
            .iter()
            .map(|constraint| format!("constraint={}", stable_fact_payload(constraint))),
    );
    if output.nodes.is_empty() && output.edges.is_empty() && output.constraints.is_empty() {
        parts.push("semantic_graph_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "semantic_graph_output", &refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn stable_fact_payload<T>(fact: &T) -> String
where
    T: Serialize + Debug,
{
    serde_json::to_string(fact).unwrap_or_else(|_| format!("{fact:?}"))
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
        )
        .output_digest;
        assert_ne!(with_absent, with_present);
    }

    #[test]
    fn provider_manifests_list_semantic_graph_between_type_value_alias_and_refined_calls() {
        let manifests = AnalysisKernel::provider_manifests();
        let tva = manifests
            .iter()
            .position(|manifest| manifest.id == "polint.type_value_alias")
            .expect("type_value_alias present");
        assert_eq!(manifests[tva + 1].id, "polint.semantic_graph");
        assert_eq!(manifests[tva + 2].id, "polint.refined_calls");
    }

    #[test]
    fn semantic_graph_manifest_precision_ceiling_is_setup_aware() {
        use crate::analysis_kernel::PrecisionCeiling;
        assert_eq!(manifest().precision_ceiling, PrecisionCeiling::SetupAware);
    }
}
