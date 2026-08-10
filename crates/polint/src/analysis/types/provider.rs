use super::cache_key::{
    type_value_alias_provider_parameter_digest,
    type_value_alias_provider_parameter_digest_for_snapshot,
};
use super::facts::{NarrowedTypeFact, TypeFact};
use super::store::TypeValueAliasOutput;
use crate::analysis::access_paths::facts::AccessPathFact;
use crate::analysis::aliases::facts::AliasAnswerFact;
use crate::analysis::points_to::facts::{PointsToConstraintFact, PointsToSetFact};
use crate::analysis::values::facts::{AllocationTokenFact, ValueFact};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::{AnalysisDb, StableKeyInterner};
use crate::diagnostics::Diagnostic;
use serde::Serialize;

pub(crate) const TYPE_VALUE_ALIAS_PROVIDER_ID: &str = "polint.type_value_alias";

#[derive(Debug, Clone, Default)]
pub(crate) struct TypeValueAliasProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_type_value_alias_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    abstract_domains_output_digest: Digest,
    direct_summaries_output_digest: Digest,
    entrypoints_output_digest: Digest,
    extensions_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> TypeValueAliasProviderOutput {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    debug_assert_eq!(manifest.id, TYPE_VALUE_ALIAS_PROVIDER_ID);
    let mut diagnostics = Vec::new();
    let mut output = super::go::derive_go_type_value_alias(db);
    let ts_js_output = super::ts_js::derive_ts_js_type_value_alias(db);
    output.types.types.extend(ts_js_output.types.types);
    output.types.narrowed.extend(ts_js_output.types.narrowed);
    output.values.values.extend(ts_js_output.values.values);
    output
        .values
        .allocations
        .extend(ts_js_output.values.allocations);
    output
        .access_paths
        .access_paths
        .extend(ts_js_output.access_paths.access_paths);
    output = output.normalized(interner);
    let base_merge = super::validate::merge_extension_type_value_alias_base_facts(
        interner,
        output,
        db.extension_facts(),
    );
    output = base_merge.output.normalized(interner);
    diagnostics.extend(base_merge.diagnostics);
    let relation_merge = super::validate::merge_extension_type_value_alias_relation_facts(
        interner,
        output,
        db.extension_facts(),
    );
    output = relation_merge.output;
    diagnostics.extend(relation_merge.diagnostics);
    let mut points_to_constraints =
        crate::analysis::points_to::constraints::derive_points_to_constraints(interner, &output);
    points_to_constraints.extend(std::mem::take(&mut output.points_to.constraints));
    let points_to_output = crate::analysis::points_to::solver::output_with_solved_sets(
        interner,
        points_to_constraints,
        crate::analysis::points_to::solver::PointsToBudget::default(),
    );
    let mut alias_output = crate::analysis::aliases::provider_stack::derive_alias_answers(
        interner,
        &output.access_paths.access_paths,
        &points_to_output.sets,
    );
    alias_output
        .answers
        .extend(std::mem::take(&mut output.aliases.answers));
    output.points_to = points_to_output;
    output.aliases = alias_output;
    output = output.normalized(interner);
    let output_digest = type_value_alias_output_digest(
        interner,
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &cfg_output_digest,
        &calls_output_digest,
        &abstract_domains_output_digest,
        &direct_summaries_output_digest,
        &entrypoints_output_digest,
        &extensions_output_digest,
        &symbol_graph_output_digest,
        &module_topology_output_digest,
        &upstream_syntax_output_digests,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    db.replace_normalized_type_value_alias_facts(output);
    TypeValueAliasProviderOutput {
        diagnostics,
        cache_stats,
        output_digest: Some(output_digest),
    }
}

#[allow(clippy::too_many_arguments)]
fn type_value_alias_output_digest(
    interner: &crate::core::StableKeyInterner,
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    calls_output_digest: &Digest,
    abstract_domains_output_digest: &Digest,
    direct_summaries_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    extensions_output_digest: &Digest,
    symbol_graph_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    output: &TypeValueAliasOutput,
) -> Digest {
    let mut upstream = vec![
        semantic_mir_output_digest.clone(),
        cfg_output_digest.clone(),
        calls_output_digest.clone(),
        abstract_domains_output_digest.clone(),
        direct_summaries_output_digest.clone(),
        entrypoints_output_digest.clone(),
        extensions_output_digest.clone(),
        symbol_graph_output_digest.clone(),
        module_topology_output_digest.clone(),
    ];
    upstream.extend(upstream_syntax_output_digests.iter().cloned());

    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!(
            "parameters={}",
            type_value_alias_provider_parameter_digest()
        ),
        format!(
            "input_parameters={}",
            type_value_alias_provider_parameter_digest_for_snapshot(input_snapshot, &upstream)
        ),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("calls={calls_output_digest}"),
        format!("abstract_domains={abstract_domains_output_digest}"),
        format!("direct_summaries={direct_summaries_output_digest}"),
        format!("entrypoints={entrypoints_output_digest}"),
        format!("extensions={extensions_output_digest}"),
        format!("symbol_graph={symbol_graph_output_digest}"),
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
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);
    parts.extend(
        upstream_syntax_output_digests
            .iter()
            .map(|digest| format!("upstream_syntax={digest}")),
    );
    parts.extend(
        output
            .types
            .types
            .iter()
            .map(|row| format!("type={}", type_fact_payload(interner, row))),
    );
    parts.extend(output.types.narrowed.iter().map(|row| {
        format!(
            "narrowed_type={}",
            narrowed_type_fact_payload(interner, row)
        )
    }));
    parts.extend(
        output
            .values
            .values
            .iter()
            .map(|row| format!("value={}", value_fact_payload(interner, row))),
    );
    parts.extend(
        output
            .values
            .allocations
            .iter()
            .map(|row| format!("allocation={}", allocation_fact_payload(interner, row))),
    );
    parts.extend(
        output
            .access_paths
            .access_paths
            .iter()
            .map(|row| format!("access_path={}", access_path_fact_payload(interner, row))),
    );
    parts.extend(output.points_to.constraints.iter().map(|row| {
        format!(
            "points_to_constraint={}",
            points_to_constraint_payload(interner, row)
        )
    }));
    parts.extend(
        output
            .points_to
            .sets
            .iter()
            .map(|row| format!("points_to_set={}", points_to_set_payload(interner, row))),
    );
    parts.extend(
        output
            .aliases
            .answers
            .iter()
            .map(|row| format!("alias_answer={}", alias_answer_payload(interner, row))),
    );
    if output.types.types.is_empty()
        && output.types.narrowed.is_empty()
        && output.values.values.is_empty()
        && output.values.allocations.is_empty()
        && output.access_paths.access_paths.is_empty()
        && output.points_to.constraints.is_empty()
        && output.points_to.sets.is_empty()
        && output.aliases.answers.is_empty()
    {
        parts.push("type_value_alias_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "type_value_alias_output", &refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    if components.is_empty() {
        parts.push(format!("{prefix}=absent"));
        return;
    }
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn type_fact_payload(interner: &StableKeyInterner, fact: &TypeFact) -> String {
    stable_json_payload(TypeFactDigest {
        id: fact.id,
        subject: &fact.subject,
        type_set: fact.type_set,
        shape: &fact.shape,
        phase: fact.phase,
        language: fact.language,
        file: fact.file,
        function: fact.function,
        body: fact.body,
        place: fact.place,
        cfg_block: fact.cfg_block,
        operation: fact.operation,
        precision: fact.precision,
        confidence: fact.confidence,
        status: fact.status,
        provenance: &fact.provenance,
        stable_key: interner.resolve(fact.stable_key).as_ref(),
    })
}

fn narrowed_type_fact_payload(interner: &StableKeyInterner, fact: &NarrowedTypeFact) -> String {
    stable_json_payload(NarrowedTypeFactDigest {
        id: fact.id,
        place: fact.place,
        type_set: fact.type_set,
        cfg_block: fact.cfg_block,
        operation: fact.operation,
        predicate: fact.predicate,
        evidence: fact.evidence.as_str(),
        language: fact.language,
        file: fact.file,
        function: fact.function,
        body: fact.body,
        precision: fact.precision,
        status: fact.status,
        stable_key: interner.resolve(fact.stable_key).as_ref(),
    })
}

fn value_fact_payload(interner: &StableKeyInterner, fact: &ValueFact) -> String {
    stable_json_payload(ValueFactDigest {
        id: fact.id,
        subject: &fact.subject,
        value: fact.value,
        kind: &fact.kind,
        language: fact.language,
        file: fact.file,
        function: fact.function,
        body: fact.body,
        precision: fact.precision,
        status: fact.status,
        provenance: &fact.provenance,
        stable_key: interner.resolve(fact.stable_key).as_ref(),
    })
}

fn allocation_fact_payload(interner: &StableKeyInterner, fact: &AllocationTokenFact) -> String {
    stable_json_payload(AllocationFactDigest {
        id: fact.id,
        kind: fact.kind,
        language: fact.language,
        file: fact.file,
        function: fact.function,
        body: fact.body,
        source_place: fact.source_place,
        source_operation: fact.source_operation,
        span: fact.span.as_ref(),
        provenance: &fact.provenance,
        stable_key: interner.resolve(fact.stable_key).as_ref(),
    })
}

fn access_path_fact_payload(interner: &StableKeyInterner, fact: &AccessPathFact) -> String {
    stable_json_payload(AccessPathFactDigest {
        id: fact.id,
        base: fact.base,
        projections: &fact.projections,
        depth: fact.depth,
        language: fact.language,
        file: fact.file,
        function: fact.function,
        body: fact.body,
        status: fact.status,
        stable_key: interner.resolve(fact.stable_key).as_ref(),
    })
}

fn points_to_constraint_payload(
    interner: &StableKeyInterner,
    fact: &PointsToConstraintFact,
) -> String {
    stable_json_payload(PointsToConstraintDigest {
        id: fact.id,
        kind: &fact.kind,
        status: fact.status,
        precision: fact.precision,
        stable_key: interner.resolve(fact.stable_key).as_ref(),
    })
}

fn points_to_set_payload(interner: &StableKeyInterner, fact: &PointsToSetFact) -> String {
    stable_json_payload(PointsToSetDigest {
        id: fact.id,
        variable: fact.variable,
        objects: &fact.objects,
        status: fact.status,
        precision: fact.precision,
        budget: fact.budget,
        stable_key: interner.resolve(fact.stable_key).as_ref(),
    })
}

fn alias_answer_payload(interner: &StableKeyInterner, fact: &AliasAnswerFact) -> String {
    stable_json_payload(AliasAnswerDigest {
        id: fact.id,
        left: fact.left,
        right: fact.right,
        status: fact.status,
        reason: &fact.reason,
        evidence: &fact.evidence,
        precision: fact.precision,
        stable_key: interner.resolve(fact.stable_key).as_ref(),
    })
}

fn stable_json_payload<T: Serialize>(payload: T) -> String {
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

#[derive(Serialize)]
struct TypeFactDigest<'a> {
    id: crate::analysis::ids::TypeFactId,
    subject: &'a super::facts::TypeSubject,
    type_set: crate::analysis::ids::TypeSetId,
    shape: &'a super::facts::TypeShape,
    phase: super::facts::TypePhase,
    language: crate::core::Language,
    file: Option<crate::core::FileId>,
    function: Option<crate::core::FunctionId>,
    body: Option<crate::analysis::ids::MirBodyId>,
    place: Option<crate::analysis::ids::PlaceId>,
    cfg_block: Option<crate::analysis::cfg::ids::BasicBlockId>,
    operation: Option<crate::analysis::ids::MirOpId>,
    precision: super::facts::TypePrecision,
    confidence: super::facts::TypeConfidence,
    status: super::facts::TypeStatus,
    provenance: &'a super::facts::TypeProvenance,
    stable_key: &'a str,
}

#[derive(Serialize)]
struct NarrowedTypeFactDigest<'a> {
    id: crate::analysis::ids::NarrowedTypeId,
    place: crate::analysis::ids::PlaceId,
    type_set: crate::analysis::ids::TypeSetId,
    cfg_block: Option<crate::analysis::cfg::ids::BasicBlockId>,
    operation: Option<crate::analysis::ids::MirOpId>,
    predicate: Option<crate::analysis::ids::PlaceId>,
    evidence: &'a str,
    language: crate::core::Language,
    file: Option<crate::core::FileId>,
    function: Option<crate::core::FunctionId>,
    body: Option<crate::analysis::ids::MirBodyId>,
    precision: super::facts::TypePrecision,
    status: super::facts::TypeStatus,
    stable_key: &'a str,
}

#[derive(Serialize)]
struct ValueFactDigest<'a> {
    id: crate::analysis::ids::ValueFactId,
    subject: &'a crate::analysis::values::facts::ValueSubject,
    value: crate::analysis::ids::AbstractValueId,
    kind: &'a crate::analysis::values::facts::ValueKind,
    language: crate::core::Language,
    file: Option<crate::core::FileId>,
    function: Option<crate::core::FunctionId>,
    body: Option<crate::analysis::ids::MirBodyId>,
    precision: crate::analysis::values::facts::ValuePrecision,
    status: crate::analysis::values::facts::ValueStatus,
    provenance: &'a crate::analysis::values::facts::ValueProvenance,
    stable_key: &'a str,
}

#[derive(Serialize)]
struct AllocationFactDigest<'a> {
    id: crate::analysis::ids::AllocationTokenId,
    kind: crate::analysis::values::facts::AllocationKind,
    language: crate::core::Language,
    file: Option<crate::core::FileId>,
    function: Option<crate::core::FunctionId>,
    body: Option<crate::analysis::ids::MirBodyId>,
    source_place: Option<crate::analysis::ids::PlaceId>,
    source_operation: Option<crate::analysis::ids::MirOpId>,
    span: Option<&'a crate::core::Span>,
    provenance: &'a crate::analysis::values::facts::ValueProvenance,
    stable_key: &'a str,
}

#[derive(Serialize)]
struct AccessPathFactDigest<'a> {
    id: crate::analysis::ids::AccessPathId,
    base: crate::analysis::ids::PlaceId,
    projections: &'a [crate::analysis::access_paths::facts::AccessPathProjection],
    depth: u32,
    language: crate::core::Language,
    file: Option<crate::core::FileId>,
    function: Option<crate::core::FunctionId>,
    body: Option<crate::analysis::ids::MirBodyId>,
    status: crate::analysis::access_paths::facts::AccessPathStatus,
    stable_key: &'a str,
}

#[derive(Serialize)]
struct PointsToConstraintDigest<'a> {
    id: crate::analysis::ids::PointsToConstraintId,
    kind: &'a crate::analysis::points_to::facts::PointsToConstraintKind,
    status: crate::analysis::points_to::facts::PointsToStatus,
    precision: crate::analysis::points_to::facts::PointsToPrecision,
    stable_key: &'a str,
}

#[derive(Serialize)]
struct PointsToSetDigest<'a> {
    id: crate::analysis::ids::PointsToSetId,
    variable: crate::analysis::ids::PtVarId,
    objects: &'a [crate::analysis::ids::ObjectTokenId],
    status: crate::analysis::points_to::facts::PointsToStatus,
    precision: crate::analysis::points_to::facts::PointsToPrecision,
    budget: crate::analysis::points_to::facts::PointsToBudgetStatus,
    stable_key: &'a str,
}

#[derive(Serialize)]
struct AliasAnswerDigest<'a> {
    id: crate::analysis::ids::AliasAnswerId,
    left: crate::analysis::aliases::facts::AliasOperand,
    right: crate::analysis::aliases::facts::AliasOperand,
    status: crate::analysis::aliases::facts::AliasStatus,
    reason: &'a crate::analysis::aliases::facts::AliasReason,
    evidence: &'a [String],
    precision: crate::analysis::aliases::facts::AliasPrecision,
    stable_key: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::extensions::sinks::{
        ExtensionFactConfidence, ExtensionFactPrecision, ExtensionFactStatus,
        TYPE_VALUE_ALIAS_ALLOCATION_FAMILY, TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY,
    };
    use crate::analysis::extensions::store::{AcceptedExtensionFact, ExtensionOutput};
    use crate::analysis::ids::AllocationTokenId;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use tempfile::tempdir;

    #[test]
    fn type_value_alias_provider_accepts_empty_output_with_deterministic_digest() {
        let mut first_db = AnalysisDb::new();
        let mut second_db = AnalysisDb::new();
        let first = derive_for_test(&mut first_db);
        let second = derive_for_test(&mut second_db);

        assert_eq!(first.output_digest, second.output_digest);
        assert!(first_db.type_facts().is_empty());
        assert!(first_db.alias_answers().is_empty());
        assert_eq!(first.cache_stats.recomputes, 1);
        assert!(first.diagnostics.is_empty());
    }

    #[test]
    fn type_value_alias_provider_manifest_declares_private_outputs() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == TYPE_VALUE_ALIAS_PROVIDER_ID)
            .expect("type/value/alias manifest should exist");

        assert_eq!(
            manifest.primary_schema_label(),
            "type-value-alias-facts-1:1"
        );
        for output in [
            "type_facts",
            "narrowed_type_facts",
            "value_facts",
            "allocation_tokens",
            "access_paths",
            "points_to_constraints",
            "points_to_sets",
            "alias_answers",
        ] {
            assert!(manifest.outputs.contains(&output));
        }
    }

    #[test]
    fn type_value_alias_provider_stores_points_to_and_alias_answers_for_ts_js_facts() {
        let mut db = AnalysisDb::new();
        db.add_file(
            "src/app.ts".into(),
            "src/app.ts".to_string(),
            r#"
export function flow(input, key) {
  const obj = { name: input };
  const same = obj;
  const field = obj[key];
  return same.name ?? field;
}
"#
            .to_string(),
        );
        let diagnostics = crate::ts::analyze_with_options(
            &mut db,
            &polint_analysis_api::DisabledAnalysisCache,
            "",
            "",
            false,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir)
            .expect("semantic MIR replacement");

        let output = derive_for_test(&mut db);

        assert!(output.diagnostics.is_empty());
        assert!(!db.points_to_constraints().is_empty());
        assert!(!db.points_to_sets().is_empty());
        assert!(!db.alias_answers().is_empty());
    }

    #[test]
    fn type_value_alias_provider_solves_extension_points_to_constraints() {
        let mut db = AnalysisDb::new();
        db.replace_extension_facts(ExtensionOutput {
            accepted: vec![
                accepted_extension_fact(
                    TYPE_VALUE_ALIAS_ALLOCATION_FAMILY,
                    "allocation:extension:object",
                    &["kind=object"],
                ),
                accepted_extension_fact(
                    TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY,
                    "pt:extension:address",
                    &["kind=address_of", "dst=allocation:0", "object=allocation:0"],
                ),
            ],
            rejected: Vec::new(),
            activations: Vec::new(),
        });

        let output = derive_for_test(&mut db);

        assert!(output.diagnostics.is_empty());
        assert!(db.points_to_sets().iter().any(|set| {
            set.variable == crate::analysis::points_to::vars::allocation_var(AllocationTokenId(0))
                && set
                    .objects
                    .contains(&crate::analysis::points_to::vars::allocation_object(
                        AllocationTokenId(0),
                    ))
        }));
    }

    #[test]
    fn type_value_alias_metadata_uses_provider_identity() {
        use crate::analysis::ids::{TypeFactId, TypeSetId};
        use crate::analysis::types::facts::{
            TypeConfidence, TypeFact, TypePhase, TypePrecision, TypeProvenance, TypeShape,
            TypeStatus, TypeSubject,
        };
        use crate::analysis::types::store::{TypeOutput, TypeValueAliasOutput};
        use crate::analysis_kernel::{FactFamily, FactRef};
        use crate::core::Language;

        let mut db = AnalysisDb::new();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            types: TypeOutput {
                types: vec![TypeFact {
                    id: TypeFactId(7),
                    subject: TypeSubject::Synthetic("type:seed".to_string()),
                    type_set: TypeSetId(0),
                    shape: TypeShape::Unknown {
                        reason: "seed".to_string(),
                    },
                    phase: TypePhase::Unknown,
                    language: Language::Unknown,
                    file: None,
                    function: None,
                    body: None,
                    place: None,
                    cfg_block: None,
                    operation: None,
                    precision: TypePrecision::Unknown,
                    confidence: TypeConfidence::Low,
                    status: TypeStatus::Unknown,
                    provenance: TypeProvenance::Native,
                    stable_key: crate::core::stable_key_for_test("type:seed"),
                }],
                narrowed: Vec::new(),
            },
            ..TypeValueAliasOutput::default()
        });

        let metadata = db
            .metadata_for(FactRef::new(FactFamily::Type, 0))
            .expect("type metadata");
        assert_eq!(metadata.producer_id, TYPE_VALUE_ALIAS_PROVIDER_ID);
        assert_eq!(metadata.layer_id, TYPE_VALUE_ALIAS_PROVIDER_ID);
    }

    fn derive_for_test(db: &mut AnalysisDb) -> TypeValueAliasProviderOutput {
        let temp = tempdir().expect("tempdir");
        std::fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let input_snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            db,
            "config",
            "rules",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let absent = |label| Digest::absent(DigestKind::ProviderOutput, label);

        derive_type_value_alias_with_cache_stats(
            db,
            &input_snapshot,
            AnalysisKernel::provider_manifests()
                .iter()
                .find(|manifest| manifest.id == TYPE_VALUE_ALIAS_PROVIDER_ID)
                .expect("type/value/alias manifest should exist"),
            absent("semantic_mir"),
            absent("cfg"),
            absent("calls"),
            absent("abstract_domains"),
            absent("direct_summaries"),
            absent("entrypoints"),
            absent("extensions"),
            absent("symbol_graph"),
            absent("module_topology"),
            Vec::new(),
        )
    }

    fn accepted_extension_fact(
        family: &str,
        stable_key: &str,
        payload_labels: &[&str],
    ) -> AcceptedExtensionFact {
        AcceptedExtensionFact {
            extension_id: "demo".to_string(),
            provider_id: "precision".to_string(),
            fact_family: family.to_string(),
            stable_key: crate::core::stable_key_for_test(stable_key),
            binding_refs: vec!["file:src/app.ts".to_string()],
            precision: ExtensionFactPrecision::Heuristic,
            confidence: ExtensionFactConfidence::Medium,
            status: ExtensionFactStatus::Accepted,
            evidence: vec!["reviewed".to_string()],
            payload_labels: payload_labels
                .iter()
                .map(|label| (*label).to_string())
                .collect(),
            payload_digest: format!("payload:{stable_key}"),
        }
    }
}
