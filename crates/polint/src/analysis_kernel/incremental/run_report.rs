#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "validated-run metadata is an internal store-planning boundary"
    )
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Serialize;

use super::demand::DemandQueryTrace;
use super::{
    CacheNode, CacheStats, ConfigIdentity, DemandCacheStatus, DependencyIndex, Digest, DigestKind,
    GenerationIdentity, INPUT_SNAPSHOT_SCHEMA_VERSION, InputSnapshot, LayerRunMetadata,
    PrecisionTier, ProviderOutputMeta, ProviderValidationStatus, QueryKey, RunIdentity, SummaryKey,
    WorkspaceIdentity, query_dependency_edges,
};
#[cfg(test)]
use crate::analysis::summaries::provider::SccClosureDebugSnapshot;
use crate::analysis_kernel::validation::{
    ValidationEvent, ValidationEventKind, ValidationEventStatus,
};
use crate::analysis_kernel::{ProviderManifest, StableFactMetaRow, StoreStatus};

type CanonicalQueryIdentity = QueryKey;

/// The semantic metadata from one globally validated kernel run.
///
/// This is the only handoff the private store planner needs. Its fields are
/// deliberately private so persistence remains a consumer of the canonical
/// kernel vocabulary rather than a second source of identities.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the private store consumes the validated-run handoff at its planning boundary"
    )
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis_kernel) struct ValidatedRunMetadata {
    input_snapshot: InputSnapshot,
    provider_manifests: Vec<CanonicalProviderManifest>,
    provider_outputs: Vec<CanonicalProviderOutput>,
    summary_keys: Vec<SummaryKey>,
    query_rows: Vec<CanonicalQueryRow>,
    dependency_index: DependencyIndex,
    fact_rows: Vec<StableFactMetaRow>,
    validation_events: Vec<ValidationEvent>,
    identities: CanonicalRunIdentities,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "provider-manifest rows are consumed by the private store planner"
    )
)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(in crate::analysis_kernel) struct CanonicalProviderManifest {
    provider_id: String,
    provider_version: String,
    provider_kind: String,
    language_scope: String,
    cache_policy: String,
    precision_ceiling: String,
    schema_versions: Vec<String>,
    inputs: Vec<String>,
    outputs: Vec<String>,
    manifest_digest: Digest,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "provider-output rows are consumed by the private store planner"
    )
)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(in crate::analysis_kernel) struct CanonicalProviderOutput {
    provider_id: String,
    provider_version: String,
    schema_version: String,
    output_digest: Digest,
    precision: PrecisionTier,
    validation: ProviderValidationStatus,
    dependency_inputs: Vec<Digest>,
    layers: Vec<LayerRunMetadata>,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "semantic query rows are consumed by the private store planner"
    )
)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(in crate::analysis_kernel) struct CanonicalQueryRow {
    query_key: CanonicalQueryIdentity,
    result_digest: Digest,
    precision: PrecisionTier,
    provenance: String,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "canonical identities are consumed by the private store planner"
    )
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis_kernel) struct CanonicalRunIdentities {
    workspace: WorkspaceIdentity,
    full_config: ConfigIdentity,
    input_snapshot: Digest,
    provider_manifest: Digest,
    provider_output: Digest,
    layer: Digest,
    summary: Digest,
    query: Digest,
    fact: Digest,
    dependency: Digest,
    validation: Digest,
    run: RunIdentity,
    generation: GenerationIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis_kernel) struct ValidatedRunMetadataError {
    message: String,
}

impl ValidatedRunMetadataError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidatedRunMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ValidatedRunMetadataError {}

impl ValidatedRunMetadata {
    pub(in crate::analysis_kernel) fn from_finalized_run(
        report: &KernelRunReport,
        manifests: &[ProviderManifest],
        fact_rows: Vec<StableFactMetaRow>,
    ) -> Result<Self, ValidatedRunMetadataError> {
        if report.input_snapshot.schema_version != INPUT_SNAPSHOT_SCHEMA_VERSION {
            return Err(ValidatedRunMetadataError::new(format!(
                "validated run uses unsupported input snapshot schema `{}`",
                report.input_snapshot.schema_version
            )));
        }

        let provider_manifests = canonical_provider_manifests(&report.input_snapshot, manifests)?;
        let provider_outputs = canonical_provider_outputs(&report.provider_outputs, manifests)?;
        let query_rows = report
            .demand_query_trace
            .semantic_projections()
            .into_iter()
            .map(|projection| CanonicalQueryRow {
                query_key: projection.query_key.clone(),
                result_digest: projection.result_digest.clone(),
                precision: projection.precision_tier,
                provenance: projection.provenance.to_string(),
            })
            .collect::<Vec<_>>();
        let summary_keys = Vec::new();
        let fact_rows = canonical_fact_rows(fact_rows)?;
        let validation_events = canonical_validation_events(report.validation_events.clone())?;
        let dependency_index = dependency_index_for(&provider_outputs, &query_rows);
        let identities = CanonicalRunIdentities::from_semantic_rows(
            &report.input_snapshot,
            &provider_manifests,
            &provider_outputs,
            &summary_keys,
            &query_rows,
            &fact_rows,
            &dependency_index,
            &validation_events,
        )?;

        let metadata = Self {
            input_snapshot: report.input_snapshot.clone(),
            provider_manifests,
            provider_outputs,
            summary_keys,
            query_rows,
            dependency_index,
            fact_rows,
            validation_events,
            identities,
        };
        metadata.validate_integrity()?;
        Ok(metadata)
    }

    pub(in crate::analysis_kernel) fn input_snapshot(&self) -> &InputSnapshot {
        &self.input_snapshot
    }

    pub(in crate::analysis_kernel) fn provider_manifests(&self) -> &[CanonicalProviderManifest] {
        &self.provider_manifests
    }

    pub(in crate::analysis_kernel) fn provider_outputs(&self) -> &[CanonicalProviderOutput] {
        &self.provider_outputs
    }

    pub(in crate::analysis_kernel) fn layers(&self) -> impl Iterator<Item = &LayerRunMetadata> {
        self.provider_outputs
            .iter()
            .flat_map(|provider| provider.layers.iter())
    }

    pub(in crate::analysis_kernel) fn summary_keys(&self) -> &[SummaryKey] {
        &self.summary_keys
    }

    pub(in crate::analysis_kernel) fn query_rows(&self) -> &[CanonicalQueryRow] {
        &self.query_rows
    }

    pub(in crate::analysis_kernel) fn dependency_index(&self) -> &DependencyIndex {
        &self.dependency_index
    }

    pub(in crate::analysis_kernel) fn fact_rows(&self) -> &[StableFactMetaRow] {
        &self.fact_rows
    }

    pub(in crate::analysis_kernel) fn validation_events(&self) -> &[ValidationEvent] {
        &self.validation_events
    }

    pub(in crate::analysis_kernel) fn identities(&self) -> &CanonicalRunIdentities {
        &self.identities
    }

    fn validate_integrity(&self) -> Result<(), ValidatedRunMetadataError> {
        if self.input_snapshot.schema_version != INPUT_SNAPSHOT_SCHEMA_VERSION {
            return Err(ValidatedRunMetadataError::new(
                "validated-run input snapshot schema is not current",
            ));
        }
        if self.input_snapshot.files.iter().any(|file| {
            std::path::Path::new(&file.relative_path).is_absolute()
                || file
                    .relative_path
                    .as_bytes()
                    .get(1)
                    .is_some_and(|byte| *byte == b':')
        }) {
            return Err(ValidatedRunMetadataError::new(
                "validated-run input snapshot contains an absolute file path",
            ));
        }
        if !self.summary_keys.is_empty() {
            return Err(ValidatedRunMetadataError::new(
                "validated-run summary slot must remain honestly empty",
            ));
        }
        require_strictly_sorted(&self.provider_manifests, "provider manifests")?;
        require_strictly_sorted(&self.provider_outputs, "provider outputs")?;
        require_strictly_sorted(&self.query_rows, "query rows")?;
        for provider in &self.provider_outputs {
            require_strictly_sorted(&provider.layers, "provider layers")?;
        }

        validate_provider_relationships(
            &self.input_snapshot,
            &self.provider_manifests,
            &self.provider_outputs,
        )?;
        if canonical_fact_rows(self.fact_rows.clone())? != self.fact_rows {
            return Err(ValidatedRunMetadataError::new(
                "validated-run fact metadata rows are not canonical",
            ));
        }
        if canonical_validation_events(self.validation_events.clone())? != self.validation_events {
            return Err(ValidatedRunMetadataError::new(
                "validated-run validation events are not canonical",
            ));
        }

        let expected_dependency_index =
            dependency_index_for(&self.provider_outputs, &self.query_rows);
        if self.dependency_index != expected_dependency_index {
            return Err(ValidatedRunMetadataError::new(
                "validated-run dependency index does not match canonical layer and query declarations",
            ));
        }

        let expected_identities = CanonicalRunIdentities::from_semantic_rows(
            &self.input_snapshot,
            &self.provider_manifests,
            &self.provider_outputs,
            &self.summary_keys,
            &self.query_rows,
            &self.fact_rows,
            &self.dependency_index,
            &self.validation_events,
        )?;
        if self.identities != expected_identities {
            return Err(ValidatedRunMetadataError::new(
                "validated-run identities do not match their canonical semantic rows",
            ));
        }
        Ok(())
    }
}

impl CanonicalProviderManifest {
    pub(in crate::analysis_kernel) fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub(in crate::analysis_kernel) fn provider_version(&self) -> &str {
        &self.provider_version
    }

    pub(in crate::analysis_kernel) fn provider_kind(&self) -> &str {
        &self.provider_kind
    }

    pub(in crate::analysis_kernel) fn language_scope(&self) -> &str {
        &self.language_scope
    }

    pub(in crate::analysis_kernel) fn cache_policy(&self) -> &str {
        &self.cache_policy
    }

    pub(in crate::analysis_kernel) fn precision_ceiling(&self) -> &str {
        &self.precision_ceiling
    }

    pub(in crate::analysis_kernel) fn schema_versions(&self) -> &[String] {
        &self.schema_versions
    }

    pub(in crate::analysis_kernel) fn inputs(&self) -> &[String] {
        &self.inputs
    }

    pub(in crate::analysis_kernel) fn outputs(&self) -> &[String] {
        &self.outputs
    }

    pub(in crate::analysis_kernel) fn manifest_digest(&self) -> &Digest {
        &self.manifest_digest
    }
}

impl CanonicalProviderOutput {
    pub(in crate::analysis_kernel) fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub(in crate::analysis_kernel) fn provider_version(&self) -> &str {
        &self.provider_version
    }

    pub(in crate::analysis_kernel) fn schema_version(&self) -> &str {
        &self.schema_version
    }

    pub(in crate::analysis_kernel) fn output_digest(&self) -> &Digest {
        &self.output_digest
    }

    pub(in crate::analysis_kernel) fn precision(&self) -> PrecisionTier {
        self.precision
    }

    pub(in crate::analysis_kernel) fn validation(&self) -> ProviderValidationStatus {
        self.validation
    }

    pub(in crate::analysis_kernel) fn dependency_inputs(&self) -> &[Digest] {
        &self.dependency_inputs
    }

    pub(in crate::analysis_kernel) fn layers(&self) -> &[LayerRunMetadata] {
        &self.layers
    }
}

impl CanonicalQueryRow {
    pub(in crate::analysis_kernel) fn query_key(&self) -> &CanonicalQueryIdentity {
        &self.query_key
    }

    pub(in crate::analysis_kernel) fn result_digest(&self) -> &Digest {
        &self.result_digest
    }

    pub(in crate::analysis_kernel) fn precision(&self) -> PrecisionTier {
        self.precision
    }

    pub(in crate::analysis_kernel) fn provenance(&self) -> &str {
        &self.provenance
    }
}

impl CanonicalRunIdentities {
    #[expect(
        clippy::too_many_arguments,
        reason = "generation identity construction keeps every semantic family explicit"
    )]
    fn from_semantic_rows(
        input_snapshot: &InputSnapshot,
        provider_manifests: &[CanonicalProviderManifest],
        provider_outputs: &[CanonicalProviderOutput],
        summary_keys: &[SummaryKey],
        query_rows: &[CanonicalQueryRow],
        fact_rows: &[StableFactMetaRow],
        dependency_index: &DependencyIndex,
        validation_events: &[ValidationEvent],
    ) -> Result<Self, ValidatedRunMetadataError> {
        let input_snapshot_digest = input_snapshot.semantic_digest();
        let provider_manifest = Digest::from_unordered(
            DigestKind::ProviderManifest,
            "provider_manifest_rows",
            provider_manifests
                .iter()
                .map(|row| row.manifest_digest.clone())
                .collect(),
        );
        let provider_output = serialized_rows_digest(
            DigestKind::ProviderOutput,
            "provider_output_rows",
            provider_outputs,
        );
        let layers = provider_outputs
            .iter()
            .flat_map(|provider| provider.layers.iter().cloned())
            .collect::<Vec<_>>();
        let layer = serialized_rows_digest(DigestKind::Layer, "layer_rows", &layers);
        let summary = serialized_rows_digest(DigestKind::Summary, "summary_rows", summary_keys);
        let query = serialized_rows_digest(DigestKind::Query, "query_rows", query_rows);
        let fact = fact_rows_digest(fact_rows);
        let dependency = serialized_rows_digest(
            DigestKind::Dependency,
            "dependency_rows",
            dependency_index.canonical_edges(),
        );
        let validation = serialized_rows_digest(
            DigestKind::ValidationEvent,
            "validation_rows",
            validation_events,
        );
        let run = RunIdentity::new(
            &input_snapshot.workspace_identity,
            &input_snapshot.config_identity,
            &input_snapshot_digest,
            &provider_manifest,
        )
        .map_err(|error| ValidatedRunMetadataError::new(error.to_string()))?;
        let generation = GenerationIdentity::new(
            &run,
            &[
                provider_output.clone(),
                layer.clone(),
                summary.clone(),
                query.clone(),
                fact.clone(),
                dependency.clone(),
                validation.clone(),
            ],
        )
        .map_err(|error| ValidatedRunMetadataError::new(error.to_string()))?;

        Ok(Self {
            workspace: input_snapshot.workspace_identity.clone(),
            full_config: input_snapshot.config_identity.clone(),
            input_snapshot: input_snapshot_digest,
            provider_manifest,
            provider_output,
            layer,
            summary,
            query,
            fact,
            dependency,
            validation,
            run,
            generation,
        })
    }

    pub(in crate::analysis_kernel) fn workspace(&self) -> &WorkspaceIdentity {
        &self.workspace
    }

    pub(in crate::analysis_kernel) fn full_config(&self) -> &ConfigIdentity {
        &self.full_config
    }

    pub(in crate::analysis_kernel) fn input_snapshot(&self) -> &Digest {
        &self.input_snapshot
    }

    pub(in crate::analysis_kernel) fn provider_manifest(&self) -> &Digest {
        &self.provider_manifest
    }

    pub(in crate::analysis_kernel) fn provider_output(&self) -> &Digest {
        &self.provider_output
    }

    pub(in crate::analysis_kernel) fn layer(&self) -> &Digest {
        &self.layer
    }

    pub(in crate::analysis_kernel) fn summary(&self) -> &Digest {
        &self.summary
    }

    pub(in crate::analysis_kernel) fn query(&self) -> &Digest {
        &self.query
    }

    pub(in crate::analysis_kernel) fn fact(&self) -> &Digest {
        &self.fact
    }

    pub(in crate::analysis_kernel) fn dependency(&self) -> &Digest {
        &self.dependency
    }

    pub(in crate::analysis_kernel) fn validation(&self) -> &Digest {
        &self.validation
    }

    pub(in crate::analysis_kernel) fn run(&self) -> &RunIdentity {
        &self.run
    }

    pub(in crate::analysis_kernel) fn generation(&self) -> &GenerationIdentity {
        &self.generation
    }
}

fn canonical_provider_manifests(
    input_snapshot: &InputSnapshot,
    manifests: &[ProviderManifest],
) -> Result<Vec<CanonicalProviderManifest>, ValidatedRunMetadataError> {
    let mut seen = BTreeSet::new();
    let mut rows = Vec::with_capacity(manifests.len());
    for manifest in manifests {
        if !seen.insert(manifest.id) {
            return Err(ValidatedRunMetadataError::new(format!(
                "duplicate provider manifest `{}`",
                manifest.id
            )));
        }
        let schema_snapshot = input_snapshot
            .provider_schemas
            .iter()
            .find(|snapshot| snapshot.provider_id == manifest.id)
            .ok_or_else(|| {
                ValidatedRunMetadataError::new(format!(
                    "input snapshot is missing provider schema `{}`",
                    manifest.id
                ))
            })?;
        let mut schema_versions = manifest
            .schema_versions
            .iter()
            .map(|schema| format!("{}:{}", schema.name, schema.version))
            .collect::<Vec<_>>();
        schema_versions.sort();
        let mut inputs = manifest
            .inputs
            .iter()
            .map(|input| (*input).to_string())
            .collect::<Vec<_>>();
        inputs.sort();
        let mut outputs = manifest
            .outputs
            .iter()
            .map(|output| (*output).to_string())
            .collect::<Vec<_>>();
        outputs.sort();
        let row = CanonicalProviderManifest {
            provider_id: manifest.id.to_string(),
            provider_version: manifest.provider_version().to_string(),
            provider_kind: manifest.kind.label().to_string(),
            language_scope: manifest.language_scope.label().to_string(),
            cache_policy: manifest.cache_policy.label().into_owned(),
            precision_ceiling: manifest.precision_ceiling.label().to_string(),
            schema_versions,
            inputs,
            outputs,
            manifest_digest: schema_snapshot.provider_manifest_digest.clone(),
        };
        if provider_manifest_row_digest(&row) != row.manifest_digest
            || schema_snapshot.schema_versions != row.schema_versions
            || schema_snapshot.language_scope != row.language_scope
            || schema_snapshot.cache_policy != row.cache_policy
            || schema_snapshot.precision_ceiling != row.precision_ceiling
        {
            return Err(ValidatedRunMetadataError::new(format!(
                "provider manifest `{}` disagrees with its input snapshot row",
                manifest.id
            )));
        }
        rows.push(row);
    }
    rows.sort();
    if input_snapshot.provider_schemas.len() != rows.len() {
        return Err(ValidatedRunMetadataError::new(
            "input snapshot provider schema set does not match provider manifests",
        ));
    }
    Ok(rows)
}

fn canonical_provider_outputs(
    outputs: &[ProviderOutputMeta],
    manifests: &[ProviderManifest],
) -> Result<Vec<CanonicalProviderOutput>, ValidatedRunMetadataError> {
    let manifests_by_id = manifests
        .iter()
        .map(|manifest| (manifest.id, manifest))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut rows = Vec::with_capacity(outputs.len());
    for output in outputs {
        if !seen.insert(output.provider_id.as_str()) {
            return Err(ValidatedRunMetadataError::new(format!(
                "duplicate provider output `{}`",
                output.provider_id
            )));
        }
        let manifest = manifests_by_id
            .get(output.provider_id.as_str())
            .ok_or_else(|| {
                ValidatedRunMetadataError::new(format!(
                    "provider output `{}` has no manifest",
                    output.provider_id
                ))
            })?;
        let mut expected_dependencies = dependency_inputs_from_manifest(manifest);
        expected_dependencies.sort();
        if output.provider_version != manifest.provider_version()
            || output.schema_version != manifest.primary_schema_label()
            || output.precision != PrecisionTier::from_ceiling(manifest.precision_ceiling)
            || output.dependency_inputs != expected_dependencies
            || output.output_digest.kind != DigestKind::ProviderOutput
        {
            return Err(ValidatedRunMetadataError::new(format!(
                "provider output `{}` disagrees with its manifest",
                output.provider_id
            )));
        }
        if output
            .layers
            .iter()
            .any(|layer| layer.key.provider_id != output.provider_id)
        {
            return Err(ValidatedRunMetadataError::new(format!(
                "provider output `{}` owns a layer from another provider",
                output.provider_id
            )));
        }
        let projection = output.semantic_projection();
        rows.push(CanonicalProviderOutput {
            provider_id: projection.provider_id.to_string(),
            provider_version: projection.provider_version.to_string(),
            schema_version: projection.schema_version.to_string(),
            output_digest: projection.output_digest.clone(),
            precision: projection.precision,
            validation: projection.validation,
            dependency_inputs: projection.dependency_inputs.to_vec(),
            layers: projection.layers.to_vec(),
        });
    }
    if seen.len() != manifests.len() {
        return Err(ValidatedRunMetadataError::new(
            "provider output set does not match provider manifests",
        ));
    }
    rows.sort();
    Ok(rows)
}

fn validate_provider_relationships(
    input_snapshot: &InputSnapshot,
    manifests: &[CanonicalProviderManifest],
    outputs: &[CanonicalProviderOutput],
) -> Result<(), ValidatedRunMetadataError> {
    if input_snapshot.provider_schemas.len() != manifests.len() || outputs.len() != manifests.len()
    {
        return Err(ValidatedRunMetadataError::new(
            "validated-run provider families have different cardinalities",
        ));
    }
    for manifest in manifests {
        if provider_manifest_row_digest(manifest) != manifest.manifest_digest {
            return Err(ValidatedRunMetadataError::new(format!(
                "provider manifest `{}` has an invalid digest",
                manifest.provider_id
            )));
        }
        let output = outputs
            .iter()
            .find(|output| output.provider_id == manifest.provider_id)
            .ok_or_else(|| {
                ValidatedRunMetadataError::new(format!(
                    "provider manifest `{}` has no output",
                    manifest.provider_id
                ))
            })?;
        let schema_snapshot = input_snapshot
            .provider_schemas
            .iter()
            .find(|snapshot| snapshot.provider_id == manifest.provider_id)
            .ok_or_else(|| {
                ValidatedRunMetadataError::new(format!(
                    "provider manifest `{}` has no input snapshot schema",
                    manifest.provider_id
                ))
            })?;
        let mut expected_dependencies = manifest
            .inputs
            .iter()
            .map(|input| {
                Digest::from_parts(DigestKind::DependencyLayer, "dependency_input", &[input])
            })
            .collect::<Vec<_>>();
        expected_dependencies.sort();
        if output.provider_version != manifest.provider_version
            || output.schema_version != manifest.schema_versions.join(",")
            || output.precision.label() != manifest.precision_ceiling
            || output.dependency_inputs != expected_dependencies
            || schema_snapshot.schema_versions != manifest.schema_versions
            || schema_snapshot.language_scope != manifest.language_scope
            || schema_snapshot.cache_policy != manifest.cache_policy
            || schema_snapshot.precision_ceiling != manifest.precision_ceiling
            || schema_snapshot.provider_manifest_digest != manifest.manifest_digest
            || output
                .layers
                .iter()
                .any(|layer| layer.key.provider_id != manifest.provider_id)
        {
            return Err(ValidatedRunMetadataError::new(format!(
                "provider output `{}` has inconsistent canonical identity",
                output.provider_id
            )));
        }
    }
    Ok(())
}

fn require_strictly_sorted<T: Ord>(
    rows: &[T],
    family: &str,
) -> Result<(), ValidatedRunMetadataError> {
    if rows.windows(2).any(|pair| pair[0] >= pair[1]) {
        Err(ValidatedRunMetadataError::new(format!(
            "validated-run {family} are not sorted and unique"
        )))
    } else {
        Ok(())
    }
}

fn provider_manifest_row_digest(row: &CanonicalProviderManifest) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", row.provider_id),
        format!("provider_version={}", row.provider_version),
        format!("provider_kind={}", row.provider_kind),
        format!("language_scope={}", row.language_scope),
        format!("cache_policy={}", row.cache_policy),
        format!("precision_ceiling={}", row.precision_ceiling),
    ];
    parts.extend(
        row.schema_versions
            .iter()
            .map(|schema| format!("schema_version={schema}")),
    );
    parts.extend(row.inputs.iter().map(|input| format!("input={input}")));
    parts.extend(row.outputs.iter().map(|output| format!("output={output}")));
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderManifest, "provider_manifest", &refs)
}

fn canonical_fact_rows(
    mut rows: Vec<StableFactMetaRow>,
) -> Result<Vec<StableFactMetaRow>, ValidatedRunMetadataError> {
    rows.sort();
    for pair in rows.windows(2) {
        if pair[0].family == pair[1].family
            && pair[0].stable_key == pair[1].stable_key
            && pair[0] != pair[1]
        {
            return Err(ValidatedRunMetadataError::new(format!(
                "conflicting finalized fact metadata for {} `{}`",
                pair[0].family.label(),
                pair[0].stable_key
            )));
        }
    }
    rows.dedup();
    Ok(rows)
}

fn canonical_validation_events(
    mut events: Vec<ValidationEvent>,
) -> Result<Vec<ValidationEvent>, ValidatedRunMetadataError> {
    events.sort_by_key(|event| event.kind);
    if events.len() != ValidationEventKind::ALL.len()
        || events
            .iter()
            .map(|event| event.kind)
            .ne(ValidationEventKind::ALL)
    {
        return Err(ValidatedRunMetadataError::new(
            "validated run must contain each required validation event exactly once",
        ));
    }
    for event in &events[..events.len() - 1] {
        let expected_status = validation_status_for_issue_count(event.issue_count);
        let expected_digest =
            validation_event_digest(event.kind, expected_status, event.issue_count);
        if event.status != expected_status || event.digest != expected_digest {
            return Err(ValidatedRunMetadataError::new(format!(
                "validation event `{}` is internally inconsistent",
                event.kind.label()
            )));
        }
    }
    let stage_events = &events[..events.len() - 1];
    let global = events
        .last()
        .expect("validation event vocabulary is non-empty");
    let issue_count = stage_events.iter().fold(0_u64, |count, event| {
        count.saturating_add(event.issue_count)
    });
    let status = validation_status_for_issue_count(issue_count);
    let mut builder = validation_event_digest_builder(global.kind, status, issue_count);
    for event in stage_events {
        builder.labeled_part("stage_kind", event.kind.label());
        builder.labeled_part("stage_status", event.status.label());
        builder.labeled_part("stage_issue_count", &event.issue_count.to_string());
        builder.labeled_part("stage_digest", &event.digest.value);
    }
    if global.kind != ValidationEventKind::GlobalFactValidation
        || global.issue_count != issue_count
        || global.status != status
        || global.digest != builder.finish()
    {
        return Err(ValidatedRunMetadataError::new(
            "global validation event does not aggregate the required stages",
        ));
    }
    Ok(events)
}

fn validation_status_for_issue_count(issue_count: u64) -> ValidationEventStatus {
    if issue_count == 0 {
        ValidationEventStatus::Passed
    } else {
        ValidationEventStatus::Failed
    }
}

fn validation_event_digest(
    kind: ValidationEventKind,
    status: ValidationEventStatus,
    issue_count: u64,
) -> Digest {
    validation_event_digest_builder(kind, status, issue_count).finish()
}

fn validation_event_digest_builder(
    kind: ValidationEventKind,
    status: ValidationEventStatus,
    issue_count: u64,
) -> super::DigestBuilder {
    let mut builder = Digest::builder(DigestKind::ValidationEvent, "fact_validation_event");
    builder.labeled_part("kind", kind.label());
    builder.labeled_part("status", status.label());
    builder.labeled_part("issue_count", &issue_count.to_string());
    builder
}

fn dependency_index_for(
    provider_outputs: &[CanonicalProviderOutput],
    query_rows: &[CanonicalQueryRow],
) -> DependencyIndex {
    let layer_edges = provider_outputs.iter().flat_map(|provider| {
        provider.layers.iter().flat_map(|layer| {
            let from = CacheNode::Layer(layer.key.clone());
            layer.dependencies.iter().cloned().map(move |mut edge| {
                edge.from = from.clone();
                edge
            })
        })
    });
    let query_edges = query_rows
        .iter()
        .flat_map(|row| query_dependency_edges(&row.query_key));
    DependencyIndex::from_edges(layer_edges.chain(query_edges).collect())
}

fn serialized_rows_digest<T: Serialize>(
    kind: DigestKind,
    label: &'static str,
    rows: &[T],
) -> Digest {
    let row_digests = rows
        .iter()
        .map(|row| {
            let encoded = serde_json::to_string(row)
                .expect("canonical in-memory semantic rows must serialize");
            Digest::from_parts(kind, label, &[&encoded])
        })
        .collect();
    Digest::from_unordered(kind, label, row_digests)
}

fn fact_rows_digest(rows: &[StableFactMetaRow]) -> Digest {
    let row_digests = rows
        .iter()
        .map(|row| {
            Digest::from_parts(
                DigestKind::FactMetadata,
                "fact_metadata_row",
                &[
                    row.family.label(),
                    &row.stable_key,
                    &row.producer_id,
                    &row.layer_id,
                    row.precision.label(),
                    row.confidence.label(),
                    row.validation.label(),
                    &row.payload_digest,
                ],
            )
        })
        .collect();
    Digest::from_unordered(DigestKind::FactMetadata, "fact_metadata_rows", row_digests)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KernelRunReport {
    pub(crate) input_snapshot: InputSnapshot,
    pub(crate) provider_outputs: Vec<ProviderOutputMeta>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) demand_query_trace: DemandQueryTrace,
    validation_events: Vec<ValidationEvent>,
    store_status: StoreStatus,
    #[cfg(test)]
    pub(crate) scc_closure_debug: Option<SccClosureDebugSnapshot>,
}

impl KernelRunReport {
    pub(crate) fn new(
        input_snapshot: InputSnapshot,
        provider_outputs: Vec<ProviderOutputMeta>,
        demand_query_trace: DemandQueryTrace,
        validation_events: Vec<ValidationEvent>,
        store_status: StoreStatus,
    ) -> Self {
        let mut cache_stats = aggregate_cache_stats(&provider_outputs);
        aggregate_demand_query_stats(&demand_query_trace, &mut cache_stats);

        Self {
            input_snapshot,
            provider_outputs,
            cache_stats,
            demand_query_trace,
            validation_events,
            store_status,
            #[cfg(test)]
            scc_closure_debug: None,
        }
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "demand trace is currently surfaced through test-only metadata debug output"
        )
    )]
    pub(crate) fn demand_query_trace(&self) -> &DemandQueryTrace {
        &self.demand_query_trace
    }

    #[cfg(test)]
    pub(crate) fn validation_events(&self) -> &[ValidationEvent] {
        &self.validation_events
    }

    #[cfg(test)]
    pub(crate) fn with_scc_closure_debug(mut self, debug: Option<SccClosureDebugSnapshot>) -> Self {
        self.scc_closure_debug = debug;
        self
    }

    #[cfg(test)]
    pub(crate) fn scc_closure_debug(&self) -> Option<&SccClosureDebugSnapshot> {
        self.scc_closure_debug.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn store_status(&self) -> &StoreStatus {
        &self.store_status
    }
}

pub(crate) fn provider_output_from_manifest_with_layers(
    manifest: &ProviderManifest,
    output_digest: Digest,
    layers: Vec<LayerRunMetadata>,
    cache_stats: CacheStats,
) -> ProviderOutputMeta {
    ProviderOutputMeta::new(
        manifest.id,
        manifest.provider_version(),
        manifest.primary_schema_label(),
        output_digest,
        PrecisionTier::from_ceiling(manifest.precision_ceiling),
        ProviderValidationStatus::NativeTrusted,
        dependency_inputs_from_manifest(manifest),
        layers,
        cache_stats,
    )
}

#[cfg(test)]
pub(crate) fn provider_output_from_manifest(
    manifest: &ProviderManifest,
    output_digest: Digest,
    cache_stats: CacheStats,
) -> ProviderOutputMeta {
    provider_output_from_manifest_with_layers(manifest, output_digest, Vec::new(), cache_stats)
}

pub(crate) fn provider_output_digest_from_manifest(
    manifest: &ProviderManifest,
    stable_rows: &[StableFactMetaRow],
) -> Digest {
    let schema_label = manifest.primary_schema_label();
    let language_scope = manifest.language_scope_label();
    let cache_policy = manifest.cache_policy_label();
    let precision = manifest.precision_ceiling.label();

    let mut output_families = manifest.outputs.to_vec();
    output_families.sort();
    output_families.dedup();

    let mut metadata_rows = stable_rows.to_vec();
    metadata_rows.sort();
    metadata_rows.dedup();

    let mut builder = Digest::builder(DigestKind::ProviderOutput, "provider_output");
    builder.labeled_part("provider_id", manifest.id);
    builder.labeled_part("schema_version", &schema_label);
    builder.labeled_part("language_scope", language_scope);
    builder.labeled_part("cache_policy", &cache_policy);
    builder.labeled_part("precision", precision);
    for output_family in output_families {
        builder.labeled_part("output_family", output_family);
    }
    for row in metadata_rows {
        builder.labeled_part("fact_family", row.family.label());
        builder.labeled_part("stable_key", &row.stable_key);
        builder.labeled_part("producer_id", &row.producer_id);
        builder.labeled_part("layer_id", &row.layer_id);
        builder.labeled_part("fact_precision", row.precision.label());
        builder.labeled_part("fact_confidence", row.confidence.label());
        builder.labeled_part("validation", row.validation.label());
        builder.labeled_part("payload_digest", &row.payload_digest);
    }

    builder.finish()
}

fn dependency_inputs_from_manifest(manifest: &ProviderManifest) -> Vec<Digest> {
    let mut inputs = manifest.inputs.to_vec();
    inputs.sort();
    inputs
        .into_iter()
        .map(|input| Digest::from_parts(DigestKind::DependencyLayer, "dependency_input", &[input]))
        .collect()
}

/// Aggregates demand query cache statistics into an existing `CacheStats`.
///
/// Cache hits count as hits; cache misses and freshly computed results count
/// as recomputes.
fn aggregate_demand_query_stats(trace: &DemandQueryTrace, stats: &mut CacheStats) {
    for entry in trace.entries() {
        match entry.cache_status {
            DemandCacheStatus::Hit => stats.hits += 1,
            DemandCacheStatus::Miss | DemandCacheStatus::Computed => stats.recomputes += 1,
        }
    }
}

fn aggregate_cache_stats(provider_outputs: &[ProviderOutputMeta]) -> CacheStats {
    let mut aggregate = CacheStats::default();
    for output in provider_outputs {
        aggregate.hits += output.cache_stats.hits;
        aggregate.misses += output.cache_stats.misses;
        aggregate.recomputes += output.cache_stats.recomputes;
        aggregate.writes += output.cache_stats.writes;
        aggregate.bypasses_disabled += output.cache_stats.bypasses_disabled;
        aggregate.invalid_evicted_reads += output.cache_stats.invalid_evicted_reads;
        aggregate.verified_reuse += output.cache_stats.verified_reuse;
        aggregate.quarantines += output.cache_stats.quarantines;
    }
    aggregate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{
        CacheStats, DemandQueryTraceEntry, DigestKind, InputComponentStatus, InputDependencyKey,
        QueryDependencyInputs, dependency_free_test_query_key,
    };
    use crate::analysis_kernel::{
        AnalysisKernel, CachePolicy, FactConfidence, FactFamily, FactPrecision, KernelInput,
        LanguageScope, PrecisionCeiling, ProviderKind, ProviderManifest, SchemaVersion,
        StableFactMetaRow, ValidationStatus,
    };
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;

    fn finalized_run_fixture() -> (KernelRunReport, Vec<StableFactMetaRow>) {
        let temp = tempfile::tempdir().expect("temporary repository");
        std::fs::write(
            temp.path().join("main.go"),
            "package main\n\nimport \"fmt\"\n\nfunc main() { fmt.Println(\"hello\") }\n",
        )
        .expect("write Go fixture");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["resolved_imports"]);
        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "validated-run-config",
            rule_digest: "validated-run-rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel fixture completes");
        let fact_rows = output
            .db
            .fact_meta()
            .stable_rows()
            .expect("finalized fact metadata is canonical");
        (output.run_report, fact_rows)
    }

    fn query_trace_entry(label: &str, cache_status: DemandCacheStatus) -> DemandQueryTraceEntry {
        let dependency = InputDependencyKey::analysis_setting(
            format!("polint.{label}"),
            Digest::from_parts(DigestKind::AnalysisSettings, label, &[label]),
            InputComponentStatus::Present,
        )
        .expect("query fixture uses an analysis-settings digest");
        let mut query_key = dependency_free_test_query_key(
            format!("query.{label}"),
            "1",
            Digest::from_parts(DigestKind::QueryParameters, label, &[label]),
            Digest::from_parts(DigestKind::Budget, label, &["bounded"]),
            PrecisionTier::SetupAware,
        );
        query_key.dependency_inputs = QueryDependencyInputs::new(vec![dependency]);
        query_key.layer_digests = vec![Digest::from_parts(
            DigestKind::ProviderOutput,
            "upstream",
            &[label],
        )];
        DemandQueryTraceEntry {
            query_key,
            result_digest: Digest::from_parts(DigestKind::ProviderOutput, "query_result", &[label]),
            precision_tier: PrecisionTier::SetupAware,
            provenance: format!("native:{label}"),
            cache_status,
            compute_duration_micros: 10,
        }
    }

    fn report_with_query_rows(mut report: KernelRunReport) -> KernelRunReport {
        let mut trace = DemandQueryTrace::default();
        for (label, status) in [
            ("alpha", DemandCacheStatus::Computed),
            ("beta", DemandCacheStatus::Hit),
            ("gamma", DemandCacheStatus::Miss),
            ("delta", DemandCacheStatus::Computed),
        ] {
            trace.record_entry(query_trace_entry(label, status));
        }
        report.demand_query_trace = trace;
        report
    }

    fn stable_fact_row(
        manifest: &ProviderManifest,
        stable_key: &str,
        payload_digest: &str,
    ) -> StableFactMetaRow {
        StableFactMetaRow {
            family: FactFamily::Import,
            stable_key: stable_key.to_string(),
            producer_id: manifest.id.to_string(),
            layer_id: manifest.id.to_string(),
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: payload_digest.to_string(),
        }
    }

    #[test]
    fn provider_outputs_are_constructed_in_manifest_order() {
        let provider_outputs = crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| {
                let row = stable_fact_row(manifest, "fixture:fact", "fixture:payload");
                let output_digest = provider_output_digest_from_manifest(manifest, &[row]);
                provider_output_from_manifest(manifest, output_digest, CacheStats::default())
            })
            .collect::<Vec<_>>();

        assert_eq!(
            provider_outputs
                .iter()
                .map(|output| output.provider_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "polint.source",
                "polint.go.syntax",
                "polint.ts.syntax",
                "polint.module_graph",
                "polint.symbol_graph",
                "polint.module_topology",
                "polint.semantic_mir",
                "polint.cfg",
                "polint.calls",
                "polint.go.semantic",
                "polint.identity",
                "polint.abstract_domains",
                "polint.direct_summaries",
                "polint.entrypoints",
                "polint.reachability",
                "polint.extensions",
                "polint.type_value_alias",
                "polint.semantic_graph",
                "polint.solver",
                "polint.refined_calls",
                "polint.data_flow",
                "polint.evidence",
                "polint.metrics",
            ]
        );
    }

    #[test]
    fn provider_output_rows_include_manifest_identity_digest_dependencies_and_stats() {
        let mut stats = CacheStats::default();
        stats.record_miss();
        stats.record_recompute();
        stats.record_write();

        for manifest in crate::analysis_kernel::AnalysisKernel::provider_manifests() {
            let row = stable_fact_row(manifest, "fixture:fact", "fixture:payload");
            let output_digest = provider_output_digest_from_manifest(manifest, &[row]);
            let row = provider_output_from_manifest(manifest, output_digest, stats.clone());

            assert_eq!(row.provider_id, manifest.id);
            assert_eq!(row.provider_version, env!("CARGO_PKG_VERSION"));
            assert_eq!(row.schema_version, manifest.primary_schema_label());
            assert_eq!(row.output_digest.kind, DigestKind::ProviderOutput);
            assert!(matches!(
                row.precision,
                PrecisionTier::Exact | PrecisionTier::Syntax | PrecisionTier::SetupAware
            ));
            assert_eq!(row.validation, ProviderValidationStatus::NativeTrusted);
            assert_eq!(row.dependency_inputs.len(), manifest.inputs.len());
            assert_eq!(row.cache_stats, stats);
        }
    }

    #[test]
    fn provider_output_digest_is_deterministic_for_identical_inputs() {
        let manifest = &crate::analysis_kernel::AnalysisKernel::provider_manifests()[1];
        let a = stable_fact_row(manifest, "fact:a", "payload:a");
        let b = stable_fact_row(manifest, "fact:b", "payload:b");
        let first = provider_output_digest_from_manifest(manifest, &[b.clone(), a.clone()]);
        let second = provider_output_digest_from_manifest(manifest, &[a, b]);

        assert_eq!(first, second);
    }

    #[test]
    fn provider_output_digest_changes_for_every_semantic_fact_field() {
        let manifest = &crate::analysis_kernel::AnalysisKernel::provider_manifests()[1];
        let base = stable_fact_row(manifest, "fact:base", "payload:base");
        let base_digest =
            provider_output_digest_from_manifest(manifest, std::slice::from_ref(&base));
        let mut mutations = Vec::new();

        let mut row = base.clone();
        row.family = FactFamily::Function;
        mutations.push(row);
        let mut row = base.clone();
        row.stable_key = "fact:changed".to_string();
        mutations.push(row);
        let mut row = base.clone();
        row.producer_id = "polint.changed.producer".to_string();
        mutations.push(row);
        let mut row = base.clone();
        row.layer_id = "polint.changed.layer".to_string();
        mutations.push(row);
        let mut row = base.clone();
        row.precision = FactPrecision::Heuristic;
        mutations.push(row);
        let mut row = base.clone();
        row.confidence = FactConfidence::Low;
        mutations.push(row);
        let mut row = base.clone();
        row.validation = ValidationStatus::SchemaValidated;
        mutations.push(row);
        let mut row = base;
        row.payload_digest = "payload:changed".to_string();
        mutations.push(row);

        for mutation in mutations {
            assert_ne!(
                provider_output_digest_from_manifest(manifest, &[mutation]),
                base_digest
            );
        }
    }

    #[test]
    fn provider_output_digest_consumes_language_scope_and_cache_policy() {
        const SCHEMAS: &[SchemaVersion] = &[SchemaVersion {
            name: "example-facts",
            version: 1,
        }];

        let base = ProviderManifest {
            id: "polint.example",
            kind: ProviderKind::LanguageSyntax,
            inputs: &["source_files"],
            outputs: &["example_facts"],
            language_scope: LanguageScope::Go,
            cache_policy: CachePolicy::NoCache,
            schema_versions: SCHEMAS,
            precision_ceiling: PrecisionCeiling::Syntax,
        };
        let scope_changed = ProviderManifest {
            language_scope: LanguageScope::TypeScriptJavaScript,
            ..base
        };
        let policy_changed = ProviderManifest {
            cache_policy: CachePolicy::ExistingFileFactCache {
                schema: "example-facts",
            },
            ..base
        };

        let row = stable_fact_row(&base, "fact:one", "payload:one");
        let base_digest = provider_output_digest_from_manifest(&base, std::slice::from_ref(&row));

        assert_ne!(
            base_digest,
            provider_output_digest_from_manifest(&scope_changed, std::slice::from_ref(&row))
        );
        assert_ne!(
            base_digest,
            provider_output_digest_from_manifest(&policy_changed, &[row])
        );
    }

    #[test]
    fn provider_output_family_digest_source_excludes_cache_telemetry() {
        let source = include_str!("run_report.rs");
        let digest_projection = source
            .split_once("pub(crate) fn provider_output_digest_from_manifest")
            .expect("provider output digest projection exists")
            .1
            .split_once("fn dependency_inputs_from_manifest")
            .expect("provider output digest projection has a bounded source section")
            .0;

        for forbidden in [
            "cache_stats",
            "hits",
            "misses",
            "recomputes",
            "writes",
            "bypasses_disabled",
            "invalid_evicted_reads",
            "verified_reuse",
            "quarantines",
        ] {
            assert!(
                !digest_projection.contains(forbidden),
                "provider output digest must exclude `{forbidden}`"
            );
        }
    }

    #[test]
    fn validated_run_metadata_is_identical_across_twenty_four_permutations() {
        let (report, fact_rows) = finalized_run_fixture();
        let report = report_with_query_rows(report);
        let manifests = AnalysisKernel::provider_manifests().to_vec();
        let baseline =
            ValidatedRunMetadata::from_finalized_run(&report, &manifests, fact_rows.clone())
                .expect("baseline handoff is valid");

        for permutation in 0..24 {
            let mut candidate_report = report.clone();
            let provider_len = candidate_report.provider_outputs.len();
            candidate_report
                .provider_outputs
                .rotate_left(permutation % provider_len);
            let event_len = candidate_report.validation_events.len();
            candidate_report
                .validation_events
                .rotate_left(permutation % event_len);
            let entries = candidate_report.demand_query_trace.entries().to_vec();
            let mut trace = DemandQueryTrace::default();
            for entry in entries
                .iter()
                .cycle()
                .skip(permutation % entries.len())
                .take(entries.len())
            {
                trace.record_entry(entry.clone());
            }
            candidate_report.demand_query_trace = trace;

            let mut candidate_manifests = manifests.clone();
            let manifest_len = candidate_manifests.len();
            candidate_manifests.rotate_left(permutation % manifest_len);
            let mut candidate_facts = fact_rows.clone();
            if !candidate_facts.is_empty() {
                let fact_len = candidate_facts.len();
                candidate_facts.rotate_left(permutation % fact_len);
            }

            let candidate = ValidatedRunMetadata::from_finalized_run(
                &candidate_report,
                &candidate_manifests,
                candidate_facts,
            )
            .expect("permuted handoff is valid");
            assert_eq!(candidate, baseline, "permutation {permutation}");
        }
    }

    #[test]
    fn telemetry_mutations_preserve_every_semantic_identity_family() {
        let (report, fact_rows) = finalized_run_fixture();
        let report = report_with_query_rows(report);
        let manifests = AnalysisKernel::provider_manifests();
        let baseline =
            ValidatedRunMetadata::from_finalized_run(&report, manifests, fact_rows.clone())
                .expect("baseline handoff is valid");

        let mut changed_report = report;
        changed_report.cache_stats.hits = changed_report.cache_stats.hits.saturating_add(91);
        for provider in &mut changed_report.provider_outputs {
            provider.cache_stats.misses = provider.cache_stats.misses.saturating_add(17);
            provider.cache_stats.writes = provider.cache_stats.writes.saturating_add(23);
        }
        for file in &mut changed_report.input_snapshot.files {
            file.mtime_hint_present = !file.mtime_hint_present;
        }
        let mut changed_trace = DemandQueryTrace::default();
        for mut entry in changed_report.demand_query_trace.entries().iter().cloned() {
            entry.cache_status = match entry.cache_status {
                DemandCacheStatus::Computed => DemandCacheStatus::Hit,
                DemandCacheStatus::Hit | DemandCacheStatus::Miss => DemandCacheStatus::Computed,
            };
            entry.compute_duration_micros = entry.compute_duration_micros.saturating_add(999_999);
            changed_trace.record_entry(entry);
        }
        changed_report.demand_query_trace = changed_trace;
        let changed =
            ValidatedRunMetadata::from_finalized_run(&changed_report, manifests, fact_rows)
                .expect("telemetry-mutated handoff is valid");

        assert_ne!(changed.input_snapshot(), baseline.input_snapshot());
        assert_eq!(changed.provider_manifests(), baseline.provider_manifests());
        assert_eq!(changed.provider_outputs(), baseline.provider_outputs());
        assert_eq!(
            changed.layers().collect::<Vec<_>>(),
            baseline.layers().collect::<Vec<_>>()
        );
        assert_eq!(changed.summary_keys(), baseline.summary_keys());
        assert_eq!(changed.query_rows(), baseline.query_rows());
        assert_eq!(changed.dependency_index(), baseline.dependency_index());
        assert_eq!(changed.fact_rows(), baseline.fact_rows());
        assert_eq!(changed.validation_events(), baseline.validation_events());
        assert_eq!(changed.identities(), baseline.identities());
    }

    #[test]
    fn validated_run_semantic_projection_source_excludes_telemetry() {
        let source = include_str!("run_report.rs");
        let projection = source
            .split_once("pub(in crate::analysis_kernel) struct ValidatedRunMetadata")
            .expect("validated-run projection exists")
            .1
            .split_once("pub(crate) struct KernelRunReport")
            .expect("validated-run projection has a bounded source section")
            .0;

        for forbidden in [
            "cache_stats",
            "DemandCacheStatus",
            "was_cached",
            "compute_duration_micros",
            "timestamp",
            "mtime_hint_present",
        ] {
            assert!(
                !projection.contains(forbidden),
                "validated-run semantic projection must exclude `{forbidden}`"
            );
        }
    }

    #[test]
    fn dependency_handoff_uses_each_query_key_declaration_exactly() {
        let (report, fact_rows) = finalized_run_fixture();
        let report = report_with_query_rows(report);
        let metadata = ValidatedRunMetadata::from_finalized_run(
            &report,
            AnalysisKernel::provider_manifests(),
            fact_rows,
        )
        .expect("handoff is valid");

        for row in metadata.query_rows() {
            assert_eq!(row.result_digest().kind, DigestKind::ProviderOutput);
            assert_eq!(row.precision(), PrecisionTier::SetupAware);
            assert!(row.provenance().starts_with("native:"));
            for edge in query_dependency_edges(row.query_key()) {
                assert!(
                    metadata
                        .dependency_index()
                        .canonical_edges()
                        .contains(&edge),
                    "query edge must come from the declared QueryKey"
                );
            }
        }
    }

    #[test]
    fn handoff_retains_payload_digests_and_excludes_forbidden_content_fields() {
        let (report, fact_rows) = finalized_run_fixture();
        let metadata = ValidatedRunMetadata::from_finalized_run(
            &report,
            AnalysisKernel::provider_manifests(),
            fact_rows,
        )
        .expect("handoff is valid");
        assert!(!metadata.fact_rows().is_empty());
        assert!(
            metadata
                .fact_rows()
                .iter()
                .all(|row| !row.payload_digest.is_empty())
        );
        let manifest = metadata
            .provider_manifests()
            .first()
            .expect("provider manifest row");
        assert!(!manifest.provider_id().is_empty());
        assert!(!manifest.provider_version().is_empty());
        assert!(!manifest.provider_kind().is_empty());
        assert!(!manifest.language_scope().is_empty());
        assert!(!manifest.cache_policy().is_empty());
        assert!(!manifest.precision_ceiling().is_empty());
        assert!(!manifest.schema_versions().is_empty());
        let _ = manifest.inputs();
        assert!(!manifest.outputs().is_empty());
        assert_eq!(
            manifest.manifest_digest().kind,
            DigestKind::ProviderManifest
        );
        let provider = metadata
            .provider_outputs()
            .first()
            .expect("provider output row");
        assert_eq!(provider.provider_id(), manifest.provider_id());
        assert_eq!(provider.provider_version(), manifest.provider_version());
        assert_eq!(
            provider.schema_version(),
            manifest.schema_versions().join(",")
        );
        assert_eq!(provider.output_digest().kind, DigestKind::ProviderOutput);
        let _ = provider.precision();
        let _ = provider.validation();
        let _ = provider.dependency_inputs();
        let _ = provider.layers();

        let identities = metadata.identities();
        assert_eq!(identities.workspace().digest().kind, DigestKind::Workspace);
        assert_eq!(identities.full_config().digest().kind, DigestKind::Config);
        assert_eq!(identities.input_snapshot().kind, DigestKind::InputSnapshot);
        assert_eq!(
            identities.provider_manifest().kind,
            DigestKind::ProviderManifest
        );
        assert_eq!(
            identities.provider_output().kind,
            DigestKind::ProviderOutput
        );
        assert_eq!(identities.layer().kind, DigestKind::Layer);
        assert_eq!(identities.summary().kind, DigestKind::Summary);
        assert_eq!(identities.query().kind, DigestKind::Query);
        assert_eq!(identities.fact().kind, DigestKind::FactMetadata);
        assert_eq!(identities.dependency().kind, DigestKind::Dependency);
        assert_eq!(identities.validation().kind, DigestKind::ValidationEvent);
        assert_eq!(identities.run().digest().kind, DigestKind::Run);
        assert_eq!(
            identities.generation().digest().kind,
            DigestKind::Generation
        );

        let fact_json = metadata
            .fact_rows()
            .iter()
            .map(|row| {
                serde_json::json!({
                    "family": row.family.label(),
                    "stable_key": row.stable_key,
                    "producer_id": row.producer_id,
                    "layer_id": row.layer_id,
                    "precision": row.precision.label(),
                    "confidence": row.confidence.label(),
                    "validation": row.validation.label(),
                    "payload_digest": row.payload_digest,
                })
            })
            .collect::<Vec<_>>();
        let inspection = serde_json::json!({
            "input_snapshot": serde_json::to_value(metadata.input_snapshot()).expect("snapshot serializes"),
            "provider_manifests": serde_json::to_value(metadata.provider_manifests()).expect("manifests serialize"),
            "provider_outputs": serde_json::to_value(metadata.provider_outputs()).expect("outputs serialize"),
            "summary_keys": serde_json::to_value(metadata.summary_keys()).expect("summaries serialize"),
            "query_rows": serde_json::to_value(metadata.query_rows()).expect("queries serialize"),
            "dependency_index": serde_json::to_value(metadata.dependency_index()).expect("dependencies serialize"),
            "fact_rows": fact_json,
            "validation_events": serde_json::to_value(metadata.validation_events()).expect("events serialize"),
        });
        assert_forbidden_fields_absent(&inspection);
    }

    #[test]
    fn integrity_validation_rejects_incomplete_validation_and_provider_sets() {
        let (report, fact_rows) = finalized_run_fixture();
        let mut missing_event = report.clone();
        missing_event.validation_events.pop();
        assert!(
            ValidatedRunMetadata::from_finalized_run(
                &missing_event,
                AnalysisKernel::provider_manifests(),
                fact_rows.clone(),
            )
            .is_err()
        );

        let mut duplicate_provider = report;
        duplicate_provider
            .provider_outputs
            .push(duplicate_provider.provider_outputs[0].clone());
        assert!(
            ValidatedRunMetadata::from_finalized_run(
                &duplicate_provider,
                AnalysisKernel::provider_manifests(),
                fact_rows,
            )
            .is_err()
        );
    }

    fn assert_forbidden_fields_absent(value: &serde_json::Value) {
        const FORBIDDEN: &[&str] = &[
            "source_text",
            "source_bytes",
            "fact_payload",
            "payload_blob",
            "ast",
            "ast_blob",
            "mir",
            "mir_blob",
            "cfg",
            "cfg_blob",
            "summary_content",
            "summary_body",
            "graph_adjacency",
            "absolute_path",
            "sql",
            "run_id",
            "row_id",
            "raw_id",
        ];
        match value {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    assert!(
                        !FORBIDDEN.contains(&key.as_str()),
                        "forbidden field `{key}`"
                    );
                    if key == "relative_path" {
                        let path = value.as_str().expect("relative paths are strings");
                        assert!(!std::path::Path::new(path).is_absolute());
                        assert!(path.as_bytes().get(1).is_none_or(|byte| *byte != b':'));
                    }
                    assert_forbidden_fields_absent(value);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    assert_forbidden_fields_absent(value);
                }
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => {}
        }
    }
}
