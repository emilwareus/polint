use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId, UnsupportedId};
use crate::analysis::mir::body::MirOutput;
use crate::analysis::mir::lower_go::lower_go_mir;
use crate::analysis::mir::lower_ts::lower_ts_mir;
use crate::analysis::mir::op::{MirOperationKind, MirValue};
use crate::analysis::places::{PlaceProjection, PlaceRoot};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

#[derive(Debug, Clone, Default)]
pub(crate) struct SemanticMirProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

pub(crate) fn derive_semantic_mir_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    module_topology_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> SemanticMirProviderOutput {
    let output = merge_language_outputs([lower_go_mir(db), lower_ts_mir(db)]);
    let output_digest = semantic_mir_output_digest(
        manifest,
        input_snapshot,
        &module_topology_output_digest,
        &symbol_graph_output_digest,
        &upstream_syntax_output_digests,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_semantic_mir(output) {
        Ok(()) => SemanticMirProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => SemanticMirProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

fn merge_language_outputs(outputs: impl IntoIterator<Item = MirOutput>) -> MirOutput {
    let mut merged = MirOutput {
        bodies: Vec::new(),
        places: Vec::new(),
        operations: Vec::new(),
        unsupported: Vec::new(),
    };
    for output in outputs {
        append_language_output(&mut merged, output);
    }
    merged.normalized()
}

fn append_language_output(merged: &mut MirOutput, mut output: MirOutput) {
    let body_offset = merged.bodies.len() as u64;
    let place_offset = merged.places.len() as u64;
    let operation_offset = merged.operations.len() as u64;
    let unsupported_offset = merged.unsupported.len() as u64;

    for body in &mut output.bodies {
        body.id = offset_body_id(body.id, body_offset);
    }
    for place in &mut output.places {
        place.id = offset_place_id(place.id, place_offset);
        if let PlaceRoot::Temporary { body, .. } = &mut place.root {
            *body = offset_body_id(*body, body_offset);
        }
    }
    for operation in &mut output.operations {
        operation.id = offset_operation_id(operation.id, operation_offset);
        operation.body = offset_body_id(operation.body, body_offset);
        offset_operation_kind_refs(&mut operation.kind, place_offset, unsupported_offset);
    }
    for row in &mut output.unsupported {
        row.id = offset_unsupported_id(row.id, unsupported_offset);
        row.body = row.body.map(|body| offset_body_id(body, body_offset));
        row.operation = row
            .operation
            .map(|operation| offset_operation_id(operation, operation_offset));
        for place in &mut row.affected_places {
            *place = offset_place_id(*place, place_offset);
        }
    }

    merged.bodies.extend(output.bodies);
    merged.places.extend(output.places);
    merged.operations.extend(output.operations);
    merged.unsupported.extend(output.unsupported);
}

fn offset_operation_kind_refs(
    kind: &mut MirOperationKind,
    place_offset: u64,
    unsupported_offset: u64,
) {
    match kind {
        MirOperationKind::StorageLive { place } | MirOperationKind::Read { place } => {
            *place = offset_place_id(*place, place_offset);
        }
        MirOperationKind::Bind { place, value }
        | MirOperationKind::Assign { place, value, .. }
        | MirOperationKind::Write { place, value } => {
            *place = offset_place_id(*place, place_offset);
            offset_value_ref(value, place_offset);
        }
        MirOperationKind::Branch {
            predicate_place, ..
        } => {
            if let Some(place) = predicate_place {
                *place = offset_place_id(*place, place_offset);
            }
        }
        MirOperationKind::Call {
            callee,
            arguments,
            return_place,
            ..
        } => {
            offset_value_ref(callee, place_offset);
            for argument in arguments {
                *argument = offset_place_id(*argument, place_offset);
            }
            *return_place = offset_place_id(*return_place, place_offset);
        }
        MirOperationKind::Return { value } => {
            if let Some(value) = value {
                offset_value_ref(value, place_offset);
            }
        }
        MirOperationKind::Unsupported { unsupported } => {
            *unsupported = offset_unsupported_id(*unsupported, unsupported_offset);
        }
    }
}

fn offset_value_ref(value: &mut MirValue, place_offset: u64) {
    if let MirValue::Place(place) = value {
        *place = offset_place_id(*place, place_offset);
    }
}

fn offset_body_id(id: MirBodyId, offset: u64) -> MirBodyId {
    MirBodyId(id.0 + offset)
}

fn offset_place_id(id: PlaceId, offset: u64) -> PlaceId {
    PlaceId(id.0 + offset)
}

fn offset_operation_id(id: MirOpId, offset: u64) -> MirOpId {
    MirOpId(id.0 + offset)
}

fn offset_unsupported_id(id: UnsupportedId, offset: u64) -> UnsupportedId {
    UnsupportedId(id.0 + offset)
}

fn semantic_mir_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    module_topology_output_digest: &Digest,
    symbol_graph_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    output: &MirOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("config={}", input_snapshot.config.digest),
        format!("module_topology={module_topology_output_digest}"),
        format!("symbol_graph={symbol_graph_output_digest}"),
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
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);

    let mut syntax = upstream_syntax_output_digests
        .iter()
        .map(|digest| format!("upstream_syntax={digest}"))
        .collect::<Vec<_>>();
    syntax.sort();
    parts.extend(syntax);

    for body in &output.bodies {
        parts.push(format!(
            "body={} status={:?} owner={} span={}:{} file={:?} function={:?}",
            body.stable_key,
            body.status,
            body.owner_stable_key,
            body.span.start_byte,
            body.span.end_byte,
            body.file,
            body.function,
        ));
    }
    for place in &output.places {
        parts.push(format!(
            "place={} status={:?} root={} projections={}",
            place.stable_key,
            place.status,
            place_root_fragment(&place.root),
            place
                .projections
                .iter()
                .map(place_projection_fragment)
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    for operation in &output.operations {
        parts.push(format!(
            "operation={} body={:?} ordinal={} kind={} status={:?}",
            operation.stable_key,
            operation.body,
            operation.ordinal,
            operation_kind_fragment(&operation.kind),
            operation.status,
        ));
    }
    for unsupported in &output.unsupported {
        parts.push(format!(
            "unsupported={} construct={} action={:?} status={:?}",
            unsupported.stable_key,
            unsupported.construct,
            unsupported.conservative_action,
            unsupported.status,
        ));
    }

    parts.sort();
    let digest_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderOutput,
        "semantic_mir_output",
        &digest_refs,
    )
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    let mut rows = components
        .iter()
        .map(|component| {
            format!(
                "{prefix}:{}={:?}:{}",
                component.name, component.status, component.digest
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    parts.extend(rows);
}

fn place_root_fragment(root: &PlaceRoot) -> String {
    match root {
        PlaceRoot::Local { function, name } => {
            format!("local:{function:?}:{name}")
        }
        PlaceRoot::Parameter {
            function,
            index,
            name,
        } => format!(
            "parameter:{function:?}:{index}:{}",
            name.as_deref().unwrap_or("")
        ),
        PlaceRoot::Global { symbol, name } => format!("global:{symbol:?}:{name}"),
        PlaceRoot::Temporary { body, ordinal } => format!("temporary:{body:?}:{ordinal}"),
        PlaceRoot::CallReturn { call } => format!("call_return:{call:?}"),
        PlaceRoot::Unknown { evidence } => format!("unknown:{evidence}"),
    }
}

fn place_projection_fragment(projection: &PlaceProjection) -> String {
    match projection {
        PlaceProjection::Field(name) => format!("field:{name}"),
        PlaceProjection::Property(name) => format!("property:{name}"),
        PlaceProjection::IndexKnown(index) => format!("index_known:{index}"),
        PlaceProjection::IndexUnknown { evidence } => format!("index_unknown:{evidence}"),
        PlaceProjection::Deref => "deref".to_string(),
        PlaceProjection::AwaitResult => "await_result".to_string(),
        PlaceProjection::CallReturn(call) => format!("call_return:{call:?}"),
        PlaceProjection::Unknown { evidence } => format!("unknown:{evidence}"),
    }
}

fn operation_kind_fragment(kind: &MirOperationKind) -> String {
    match kind {
        MirOperationKind::StorageLive { place } => format!("storage_live:{place:?}"),
        MirOperationKind::Bind { place, value } => {
            format!("bind:{place:?}:{}", value_fragment(value))
        }
        MirOperationKind::Assign { place, value, mode } => {
            format!("assign:{place:?}:{mode:?}:{}", value_fragment(value))
        }
        MirOperationKind::Write { place, value } => {
            format!("write:{place:?}:{}", value_fragment(value))
        }
        MirOperationKind::Read { place } => format!("read:{place:?}"),
        MirOperationKind::Branch {
            predicate,
            predicate_place,
        } => format!("branch:{predicate:?}:{predicate_place:?}"),
        MirOperationKind::Call {
            site,
            callee,
            arguments,
            return_place,
        } => format!(
            "call:{site:?}:{}:{}:{return_place:?}",
            value_fragment(callee),
            arguments
                .iter()
                .map(|argument| format!("{argument:?}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        MirOperationKind::Return { value } => {
            format!(
                "return:{}",
                value.as_ref().map(value_fragment).unwrap_or_default()
            )
        }
        MirOperationKind::Unsupported { unsupported } => {
            format!("unsupported:{unsupported:?}")
        }
    }
}

fn value_fragment(value: &MirValue) -> String {
    match value {
        MirValue::Place(place) => format!("place:{place:?}"),
        MirValue::Literal { value } => format!("literal:{value}"),
        MirValue::Temporary(value) => format!("temporary:{value:?}"),
        MirValue::CallReturn(call) => format!("call_return:{call:?}"),
        MirValue::Unknown { evidence } => format!("unknown:{evidence}"),
    }
}

fn provider_error_diagnostic(reason: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "semantic MIR provider",
        TextRange::point(1, 1),
        "Semantic MIR provider rejected lowered rows.",
    )
    .with_evidence("family", "SemanticMir")
    .with_evidence("stable_key", "")
    .with_evidence("field", "provider")
    .with_evidence("reason", reason)
}

#[cfg(test)]
mod semantic_mir_provider {
    use crate::analysis::ids::{MirBodyId, MirOpId, MirPredicateId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{MirOperation, MirOperationKind};
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::analysis_kernel::incremental::{Digest, DigestKind};
    use crate::analysis_kernel::{AnalysisKernel, KernelInput};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use crate::core::{FileId, FunctionId, Language, Span};

    fn span() -> Span {
        Span {
            file: FileId(1),
            start_byte: 1,
            end_byte: 2,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 2,
        }
    }

    fn body(id: u64, function: u64, stable_key: &str) -> MirBody {
        MirBody {
            id: MirBodyId(id),
            language: Language::Go,
            file: FileId(1),
            function: FunctionId(function),
            package: None,
            module: None,
            owner_stable_key: stable_key.to_string(),
            span: span(),
            stable_key: stable_key.to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn local_place(id: u64, function: u64, stable_key: &str) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::Go,
            file: Some(FileId(1)),
            function: Some(FunctionId(function)),
            root: PlaceRoot::Local {
                function: FunctionId(function),
                name: stable_key.to_string(),
            },
            projections: Vec::new(),
            stable_key: stable_key.to_string(),
            status: PlaceStatus::Resolved,
        }
    }

    fn branch_operation(
        id: u64,
        body: u64,
        predicate_place: PlaceId,
        stable_key: &str,
    ) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(body),
            ordinal: 1,
            span: span(),
            kind: MirOperationKind::Branch {
                predicate: MirPredicateId(1),
                predicate_place: Some(predicate_place),
            },
            stable_key: stable_key.to_string(),
            status: MirStatus::Resolved,
        }
    }

    #[test]
    fn derivation_stores_non_empty_mixed_language_mir() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("main.go"),
            "package main\nfunc add(left int, right int) int { total := left + right; return total }\n",
        )
        .expect("write go");
        std::fs::write(
            temp.path().join("app.ts"),
            "export function render(value: number) { const next = value + 1; return next; }\n",
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["control_flow"]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");

        assert!(
            output.db.mir_bodies().len() >= 2,
            "expected Go and TS MIR bodies"
        );
        assert!(output.db.mir_places().len() >= 2, "expected lowered places");
        assert!(
            output.db.mir_operations().len() >= 2,
            "expected lowered operations"
        );
    }

    #[test]
    fn derivation_output_digest_changes_with_upstream_inputs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let loaded = load_config(temp.path()).expect("default config loads");
        let mut db = crate::core::AnalysisDb::new();
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.semantic_mir")
            .expect("semantic MIR manifest");
        let empty_plan = AnalysisPlan::empty();
        assert!(empty_plan.requested_capability_snapshots().is_empty());
        let identity_sources =
            crate::analysis_kernel::incremental::InputSnapshot::identity_sources_from_plan(
                &loaded,
                &empty_plan,
            );
        assert!(identity_sources.requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities")
        );
        let input_snapshot =
            crate::analysis_kernel::incremental::InputSnapshot::from_run_inputs_with_plan(
                &loaded,
                &db,
                "config",
                "rules",
                &empty_plan,
                AnalysisKernel::provider_manifests(),
            );
        let module_topology = Digest::from_parts(
            DigestKind::ProviderOutput,
            "module_topology",
            &["topology-a"],
        );
        let symbol_graph =
            Digest::from_parts(DigestKind::ProviderOutput, "symbol_graph", &["symbols-a"]);
        let syntax_a = vec![Digest::from_parts(
            DigestKind::ProviderOutput,
            "syntax",
            &["go-a"],
        )];
        let syntax_b = vec![Digest::from_parts(
            DigestKind::ProviderOutput,
            "syntax",
            &["go-b"],
        )];

        let first = super::derive_semantic_mir_with_cache_stats(
            &mut db,
            &input_snapshot,
            manifest,
            module_topology.clone(),
            symbol_graph.clone(),
            syntax_a,
        );
        let second = super::derive_semantic_mir_with_cache_stats(
            &mut db,
            &input_snapshot,
            manifest,
            module_topology,
            symbol_graph,
            syntax_b,
        );

        assert_ne!(first.output_digest, second.output_digest);
    }

    #[test]
    fn merge_language_outputs_offsets_branch_predicate_place_references() {
        let first = MirOutput {
            bodies: vec![body(0, 1, "body:first")],
            places: vec![local_place(0, 1, "place:first")],
            operations: Vec::new(),
            unsupported: Vec::new(),
        };
        let second = MirOutput {
            bodies: vec![body(0, 2, "body:second")],
            places: vec![local_place(0, 2, "place:second")],
            operations: vec![branch_operation(0, 0, PlaceId(0), "op:second:branch")],
            unsupported: Vec::new(),
        };

        let output = super::merge_language_outputs([first, second]);
        let branch = output
            .operations
            .iter()
            .find(|operation| operation.stable_key == "op:second:branch")
            .expect("merged branch operation");

        assert!(matches!(
            branch.kind,
            MirOperationKind::Branch {
                predicate_place: Some(PlaceId(1)),
                ..
            }
        ));
    }
}
