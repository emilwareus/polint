use serde::Serialize;
use std::fmt::Debug;

use super::cache_key::{
    data_flow_provider_parameter_digest, data_flow_provider_parameter_digest_for_snapshot,
};
use super::facts::{
    DataFlowAlgorithm, DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind, DataFlowModelFact,
    DataFlowModelKind, DataFlowNodeFact, DataFlowNodeKind, DataFlowPrecision, DataFlowProvenance,
    DataFlowStatus, DataFlowValidation,
};
use super::store::{
    DataFlowOutput, next_data_flow_edge_id, next_data_flow_model_id, next_data_flow_node_id,
};
use crate::analysis::entrypoints::facts::TrustBoundaryFact;
use crate::analysis::ids::{DataFlowModelId, DataFlowNodeId};
use crate::analysis::places::{PlaceFact, PlaceRoot};
use crate::analysis_kernel::incremental::{CacheStats, Digest, DigestKind, InputSnapshot};
use crate::analysis_kernel::{FactFamily, ProviderManifest, stable_key_from_parts};
use crate::core::AnalysisDb;
use crate::diagnostics::Diagnostic;

pub(crate) const DATA_FLOW_PROVIDER_ID: &str = "polint.data_flow";

#[derive(Debug, Clone, Default)]
pub(crate) struct DataFlowProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_data_flow_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    refined_calls_output_digest: Digest,
    direct_summaries_output_digest: Digest,
    type_value_alias_output_digest: Digest,
    entrypoints_output_digest: Digest,
    extensions_output_digest: Digest,
) -> DataFlowProviderOutput {
    debug_assert_eq!(manifest.id, DATA_FLOW_PROVIDER_ID);
    let mut output = DataFlowOutput::empty();
    derive_local_place_nodes(db, &mut output);
    super::local::derive_local_value_flow(db, &mut output);
    super::direct_calls::derive_direct_call_edges(db, &mut output);
    super::summary_edges::derive_summary_projected_edges(db, &mut output);
    derive_source_models(db, &mut output);
    derive_extension_models(db, &mut output);
    output = output.normalized();

    let output_digest = data_flow_output_digest(
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &cfg_output_digest,
        &calls_output_digest,
        &refined_calls_output_digest,
        &direct_summaries_output_digest,
        &type_value_alias_output_digest,
        &entrypoints_output_digest,
        &extensions_output_digest,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_data_flow_facts(output) {
        Ok(()) => DataFlowProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => DataFlowProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: None,
        },
    }
}

fn derive_local_place_nodes(db: &AnalysisDb, output: &mut DataFlowOutput) {
    for place in db.mir_places() {
        output
            .nodes
            .push(super::local::node_from_place(output, place, db));
    }
}

fn derive_source_models(db: &AnalysisDb, output: &mut DataFlowOutput) {
    for boundary in db.trust_boundary_facts() {
        let model_id = next_data_flow_model_id(&output.models);
        let stable_key = stable_key_from_parts(
            FactFamily::DataFlowModel,
            &[
                ("kind", "source".to_string()),
                ("trust_boundary", boundary.stable_key.clone()),
            ],
        );
        output.models.push(DataFlowModelFact {
            id: model_id,
            kind: DataFlowModelKind::Source,
            language: boundary.language,
            provider_id: boundary.provider_id.clone(),
            model_id: Some(format!("{:?}", boundary.source_kind)),
            source_stable_key: Some(boundary.stable_key.clone()),
            status: DataFlowStatus::Present,
            precision: DataFlowPrecision::SetupAware,
            validation: DataFlowValidation::ReferentiallyValidated,
            confidence: DataFlowConfidence::High,
            provenance: DataFlowProvenance::Native,
            evidence: vec!["trust_boundary".to_string()],
            payload_labels: vec![
                format!("source_kind={:?}", boundary.source_kind),
                boundary.access_path.clone().unwrap_or_default(),
            ],
            stable_key: stable_key.clone(),
        });
        let source_node = next_data_flow_node_id(&output.nodes);
        output.nodes.push(DataFlowNodeFact {
            id: source_node,
            kind: DataFlowNodeKind::Source,
            language: boundary.language,
            file: Some(boundary.file),
            function: boundary.target_parameter,
            body: None,
            operation: None,
            cfg_node: None,
            place: None,
            symbol: None,
            reference: None,
            call_site: None,
            model: Some(model_id),
            span: Some(boundary.span.clone()),
            stable_key: stable_key_from_parts(
                FactFamily::DataFlowNode,
                &[("source_model", stable_key)],
            ),
        });
        derive_source_introduction_edges(db, output, boundary, source_node, model_id);
    }
}

fn derive_source_introduction_edges(
    db: &AnalysisDb,
    output: &mut DataFlowOutput,
    boundary: &TrustBoundaryFact,
    source_node: DataFlowNodeId,
    model_id: DataFlowModelId,
) {
    let Some(target_function) = boundary.target_parameter else {
        return;
    };
    let targets = db
        .mir_places()
        .iter()
        .filter(|place| parameter_matches_boundary(place, target_function, boundary))
        .filter_map(|place| {
            let node = output
                .nodes
                .iter()
                .find(|node| node.place == Some(place.id))
                .map(|node| node.id)?;
            Some((place, node))
        })
        .collect::<Vec<_>>();

    for (place, target_node) in targets {
        push_source_introduction_edge(output, boundary, place, source_node, target_node, model_id);
    }
}

fn parameter_matches_boundary(
    place: &PlaceFact,
    target_function: crate::core::FunctionId,
    boundary: &TrustBoundaryFact,
) -> bool {
    let PlaceRoot::Parameter {
        function, index, ..
    } = &place.root
    else {
        return false;
    };
    if *function != target_function {
        return false;
    }
    match boundary.target_parameter_index {
        Some(target_index) => *index as usize == target_index,
        None => true,
    }
}

fn push_source_introduction_edge(
    output: &mut DataFlowOutput,
    boundary: &TrustBoundaryFact,
    place: &PlaceFact,
    source_node: DataFlowNodeId,
    target_node: DataFlowNodeId,
    model_id: DataFlowModelId,
) {
    let stable_key = stable_key_from_parts(
        FactFamily::DataFlowEdge,
        &[
            ("kind", "SourceIntroduction".to_string()),
            ("trust_boundary", boundary.stable_key.clone()),
            ("place", place.stable_key.clone()),
        ],
    );
    if output
        .edges
        .iter()
        .any(|edge| edge.stable_key == stable_key)
    {
        return;
    }
    output.edges.push(DataFlowEdgeFact {
        id: next_data_flow_edge_id(&output.edges),
        from: source_node,
        to: target_node,
        kind: DataFlowEdgeKind::SourceIntroduction,
        algorithm: DataFlowAlgorithm::ExtensionModel,
        status: if boundary.target_parameter_index.is_some() {
            DataFlowStatus::Present
        } else {
            DataFlowStatus::Unknown
        },
        precision: if boundary.target_parameter_index.is_some() {
            DataFlowPrecision::SetupAware
        } else {
            DataFlowPrecision::Unknown
        },
        validation: DataFlowValidation::ReferentiallyValidated,
        confidence: if boundary.target_parameter_index.is_some() {
            DataFlowConfidence::High
        } else {
            DataFlowConfidence::Low
        },
        provenance: DataFlowProvenance::Native,
        call_site: None,
        call_target: None,
        refined_call: None,
        model: Some(model_id),
        budget: None,
        evidence: vec![
            "trust_boundary_source_introduction".to_string(),
            format!("source_kind={:?}", boundary.source_kind),
            boundary
                .target_parameter_index
                .map(|index| format!("target_parameter_index={index}"))
                .unwrap_or_else(|| "target_parameter_index=unknown".to_string()),
        ],
        input_stable_keys: vec![boundary.stable_key.clone(), place.stable_key.clone()],
        stable_key,
    });
}

fn derive_extension_models(db: &AnalysisDb, output: &mut DataFlowOutput) {
    for fact in db.extension_facts() {
        let kind = match fact.fact_family.as_str() {
            "data_flow.source" | "source" => DataFlowModelKind::Source,
            "data_flow.sink" | "sink" => DataFlowModelKind::Sink,
            "data_flow.sanitizer" | "sanitizer" => DataFlowModelKind::Sanitizer,
            "data_flow.barrier" | "barrier" => DataFlowModelKind::Barrier,
            "data_flow.tito" | "tito" => DataFlowModelKind::Tito,
            _ => continue,
        };
        output.models.push(DataFlowModelFact {
            id: next_data_flow_model_id(&output.models),
            kind,
            language: crate::core::Language::Unknown,
            provider_id: fact.provider_id.clone(),
            model_id: Some(fact.extension_id.clone()),
            source_stable_key: Some(fact.stable_key.clone()),
            status: DataFlowStatus::Present,
            precision: extension_precision(fact.precision),
            validation: DataFlowValidation::ExtensionValidated,
            confidence: extension_confidence(fact.confidence),
            provenance: DataFlowProvenance::Extension,
            evidence: fact.evidence.clone(),
            payload_labels: fact.payload_labels.clone(),
            stable_key: stable_key_from_parts(
                FactFamily::DataFlowModel,
                &[
                    ("kind", format!("{kind:?}")),
                    ("extension_fact", fact.stable_key.clone()),
                ],
            ),
        });
    }
}

fn extension_precision(
    precision: crate::analysis::extensions::sinks::ExtensionFactPrecision,
) -> DataFlowPrecision {
    match precision {
        crate::analysis::extensions::sinks::ExtensionFactPrecision::Exact => {
            DataFlowPrecision::Exact
        }
        crate::analysis::extensions::sinks::ExtensionFactPrecision::SetupAware => {
            DataFlowPrecision::SetupAware
        }
        crate::analysis::extensions::sinks::ExtensionFactPrecision::Heuristic => {
            DataFlowPrecision::Heuristic
        }
        crate::analysis::extensions::sinks::ExtensionFactPrecision::GeneratedUnvalidated => {
            DataFlowPrecision::Heuristic
        }
    }
}

fn extension_confidence(
    confidence: crate::analysis::extensions::sinks::ExtensionFactConfidence,
) -> DataFlowConfidence {
    match confidence {
        crate::analysis::extensions::sinks::ExtensionFactConfidence::High => {
            DataFlowConfidence::High
        }
        crate::analysis::extensions::sinks::ExtensionFactConfidence::Medium => {
            DataFlowConfidence::Medium
        }
        crate::analysis::extensions::sinks::ExtensionFactConfidence::Low => DataFlowConfidence::Low,
    }
}

#[allow(clippy::too_many_arguments)]
fn data_flow_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    calls_output_digest: &Digest,
    refined_calls_output_digest: &Digest,
    direct_summaries_output_digest: &Digest,
    type_value_alias_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    extensions_output_digest: &Digest,
    output: &DataFlowOutput,
) -> Digest {
    let upstream = vec![
        semantic_mir_output_digest.clone(),
        cfg_output_digest.clone(),
        calls_output_digest.clone(),
        refined_calls_output_digest.clone(),
        direct_summaries_output_digest.clone(),
        type_value_alias_output_digest.clone(),
        entrypoints_output_digest.clone(),
        extensions_output_digest.clone(),
    ];
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", data_flow_provider_parameter_digest()),
        format!(
            "input_parameters={}",
            data_flow_provider_parameter_digest_for_snapshot(input_snapshot, &upstream)
        ),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("calls={calls_output_digest}"),
        format!("refined_calls={refined_calls_output_digest}"),
        format!("direct_summaries={direct_summaries_output_digest}"),
        format!("type_value_alias={type_value_alias_output_digest}"),
        format!("entrypoints={entrypoints_output_digest}"),
        format!("extensions={extensions_output_digest}"),
    ];
    parts.extend(
        output
            .nodes
            .iter()
            .map(|node| format!("data_flow_node={}", stable_fact_payload(node))),
    );
    parts.extend(
        output
            .edges
            .iter()
            .map(|edge| format!("data_flow_edge={}", stable_fact_payload(edge))),
    );
    parts.extend(
        output
            .models
            .iter()
            .map(|model| format!("data_flow_model={}", stable_fact_payload(model))),
    );
    parts.extend(
        output
            .budgets
            .iter()
            .map(|budget| format!("data_flow_budget={}", stable_fact_payload(budget))),
    );
    if output.nodes.is_empty() && output.edges.is_empty() && output.models.is_empty() {
        parts.push("data_flow_output=empty".to_string());
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "data_flow_output", &refs)
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
        crate::diagnostics::TextRange::point(1, 1),
        format!("Data-flow provider failed: {message}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::entrypoints::facts::{
        EntrypointConfidence, EntrypointFact, EntrypointKind, EntrypointPrecision,
        EntrypointProvenance, EntrypointStatus, TriggerMetadata, TrustBoundarySourceKind,
    };
    use crate::analysis::entrypoints::store::EntrypointOutput;
    use crate::analysis::ids::{EntrypointId, MirBodyId, PlaceId, TrustBoundaryId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::places::{PlaceProjection, PlaceStatus};
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::keys::config_hash;
    use crate::config::{LoadedConfig, PolintConfig, RuleConfig};
    use crate::core::{FileId, FunctionFact, FunctionId, Language, Span};
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    #[test]
    fn output_identity_tracks_dataflow_inputs_and_ignores_rule_settings() {
        let temp = tempdir().expect("tempdir");
        let baseline_loaded = loaded_with_rule(temp.path());
        let plan = AnalysisPlan::from_capability_names_for_test(&["dataflow", "events"]);
        let baseline_snapshot = identity_snapshot(&baseline_loaded, &plan);
        let upstream = Digest::from_parts(DigestKind::ProviderOutput, "upstream", &["base"]);
        let output = DataFlowOutput::empty();
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == DATA_FLOW_PROVIDER_ID)
            .expect("data-flow manifest");
        let digest = |snapshot: &InputSnapshot, upstream: &Digest| {
            data_flow_output_digest(
                manifest, snapshot, upstream, upstream, upstream, upstream, upstream, upstream,
                upstream, upstream, &output,
            )
        };
        let baseline_digest = digest(&baseline_snapshot, &upstream);

        let mut rule_settings = baseline_loaded;
        rule_settings.config.rules.config[0]
            .settings
            .insert("threshold".to_string(), toml::Value::Integer(7));
        let rule_snapshot = identity_snapshot(&rule_settings, &plan);
        assert_ne!(
            baseline_snapshot.config_identity,
            rule_snapshot.config_identity
        );
        assert_eq!(baseline_digest, digest(&rule_snapshot, &upstream));

        let mut relevant = baseline_snapshot.clone();
        relevant
            .requested_capabilities
            .iter_mut()
            .find(|row| row.capability == "dataflow")
            .expect("dataflow capability")
            .analysis_dependency_digest =
            Digest::from_parts(DigestKind::AnalysisRequirements, "dataflow", &["changed"]);
        assert_ne!(baseline_digest, digest(&relevant, &upstream));

        let mut unrelated = baseline_snapshot.clone();
        unrelated
            .requested_capabilities
            .iter_mut()
            .find(|row| row.capability == "events")
            .expect("events capability")
            .analysis_dependency_digest =
            Digest::from_parts(DigestKind::AnalysisRequirements, "events", &["changed"]);
        assert_eq!(baseline_digest, digest(&unrelated, &upstream));

        let changed_upstream =
            Digest::from_parts(DigestKind::ProviderOutput, "upstream", &["changed"]);
        assert_ne!(
            baseline_digest,
            digest(&baseline_snapshot, &changed_upstream)
        );
    }

    fn loaded_with_rule(root: &Path) -> LoadedConfig {
        let mut config = PolintConfig::default();
        config.rules.config.push(RuleConfig {
            id: "local/provider-identity".to_string(),
            severity: None,
            files: Vec::new(),
            allow_files: Vec::new(),
            allow: Vec::new(),
            max: None,
            deny: Vec::new(),
            forbidden_imports: Default::default(),
            settings: Default::default(),
        });
        LoadedConfig {
            root: root.to_path_buf(),
            config,
            missing: false,
            respect_gitignore: true,
        }
    }

    fn identity_snapshot(loaded: &LoadedConfig, plan: &AnalysisPlan) -> InputSnapshot {
        let config_digest = config_hash(loaded);
        InputSnapshot::from_run_inputs_with_plan(
            loaded,
            &AnalysisDb::new(),
            &config_digest,
            "rule-digest",
            plan,
            AnalysisKernel::provider_manifests(),
        )
    }

    #[test]
    fn source_models_create_source_introduction_edges_to_matching_parameters() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.ts"),
            "src/main.ts".to_string(),
            "export function handler(req: Request) {}\n".to_string(),
        );
        let function = db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "handler".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.replace_semantic_mir(MirOutput {
            bodies: vec![mir_body(file, function)],
            places: vec![parameter_place(file, function)],
            operations: Vec::new(),
            unsupported: Vec::new(),
        })
        .expect("valid MIR");
        db.replace_entrypoint_facts(EntrypointOutput {
            entrypoints: vec![entrypoint(file, function)],
            trust_boundaries: vec![trust_boundary(file, function)],
            dispatch_edges: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid entrypoint facts");
        let mut output = DataFlowOutput::empty();
        derive_local_place_nodes(&db, &mut output);

        derive_source_models(&db, &mut output);

        assert!(output.nodes.iter().any(|node| {
            node.kind == DataFlowNodeKind::Source && node.model == Some(DataFlowModelId(0))
        }));
        let edge = output
            .edges
            .iter()
            .find(|edge| edge.kind == DataFlowEdgeKind::SourceIntroduction)
            .unwrap_or_else(|| panic!("missing source introduction edge: {output:#?}"));
        assert_eq!(edge.status, DataFlowStatus::Present);
        assert_eq!(edge.model, Some(DataFlowModelId(0)));
        assert!(
            edge.evidence
                .iter()
                .any(|value| value == "source_kind=QueryString")
        );
    }

    #[test]
    fn source_models_downgrade_unknown_parameter_index_edges() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.ts"),
            "src/main.ts".to_string(),
            "export function handler(req: Request, res: Response) {}\n".to_string(),
        );
        let function = db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "handler".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.replace_semantic_mir(MirOutput {
            bodies: vec![mir_body(file, function)],
            places: vec![
                parameter_place_with_index(file, function, 0, "req"),
                parameter_place_with_index(file, function, 1, "res"),
            ],
            operations: Vec::new(),
            unsupported: Vec::new(),
        })
        .expect("valid MIR");
        let mut boundary = trust_boundary(file, function);
        boundary.target_parameter_index = None;
        db.replace_entrypoint_facts(EntrypointOutput {
            entrypoints: vec![entrypoint(file, function)],
            trust_boundaries: vec![boundary],
            dispatch_edges: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid entrypoint facts");
        let mut output = DataFlowOutput::empty();
        derive_local_place_nodes(&db, &mut output);

        derive_source_models(&db, &mut output);

        let source_edges = output
            .edges
            .iter()
            .filter(|edge| edge.kind == DataFlowEdgeKind::SourceIntroduction)
            .collect::<Vec<_>>();
        assert_eq!(source_edges.len(), 2);
        assert!(source_edges.iter().all(|edge| {
            edge.status == DataFlowStatus::Unknown
                && edge.precision == DataFlowPrecision::Unknown
                && edge.confidence == DataFlowConfidence::Low
                && edge
                    .evidence
                    .iter()
                    .any(|value| value == "target_parameter_index=unknown")
        }));
    }

    fn mir_body(file: FileId, function: FunctionId) -> MirBody {
        MirBody {
            id: MirBodyId(0),
            language: Language::TypeScript,
            file,
            function,
            package: None,
            module: None,
            owner_stable_key: "function:handler".to_string(),
            span: Span::point(file, 1, 1),
            stable_key: "body:handler".to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn parameter_place(file: FileId, function: FunctionId) -> PlaceFact {
        parameter_place_with_index(file, function, 0, "req")
    }

    fn parameter_place_with_index(
        file: FileId,
        function: FunctionId,
        index: u32,
        name: &str,
    ) -> PlaceFact {
        PlaceFact {
            id: PlaceId(index as u64),
            language: Language::TypeScript,
            file: Some(file),
            function: Some(function),
            root: PlaceRoot::Parameter {
                function,
                index,
                name: Some(name.to_string()),
            },
            projections: Vec::<PlaceProjection>::new(),
            stable_key: format!("place:{name}"),
            status: PlaceStatus::Resolved,
        }
    }

    fn entrypoint(file: FileId, function: FunctionId) -> EntrypointFact {
        EntrypointFact {
            id: EntrypointId(0),
            language: Language::TypeScript,
            framework_id: "express".to_string(),
            kind: EntrypointKind::HttpRoute,
            target_function: function,
            target_symbol: None,
            registration_span: Span::point(file, 1, 1),
            registration_file: file,
            trigger_metadata: TriggerMetadata::empty(),
            trust_boundary_link: None,
            precision: EntrypointPrecision::ResolvedStatic,
            provenance: EntrypointProvenance::NativeRecognizer,
            confidence: EntrypointConfidence::High,
            status: EntrypointStatus::Resolved,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: "entrypoint:handler".to_string(),
        }
    }

    fn trust_boundary(file: FileId, function: FunctionId) -> TrustBoundaryFact {
        TrustBoundaryFact {
            id: TrustBoundaryId(0),
            entrypoint_stable_key: "entrypoint:handler".to_string(),
            source_kind: TrustBoundarySourceKind::QueryString,
            target_parameter: Some(function),
            target_parameter_index: Some(0),
            access_path: None,
            protocol: None,
            language: Language::TypeScript,
            file,
            span: Span::point(file, 1, 1),
            precision: EntrypointPrecision::ResolvedStatic,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: "trust-boundary:query".to_string(),
        }
    }
}
