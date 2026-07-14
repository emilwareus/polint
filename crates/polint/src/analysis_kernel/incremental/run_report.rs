#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "validated-run metadata is an internal store-planning boundary"
    )
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use rayon::prelude::*;
use serde::Serialize;

use super::demand::DemandQueryTrace;
#[cfg(test)]
use super::query_dependency_edges;
use super::{
    CacheNode, CacheStats, ConfigIdentity, DemandCacheStatus, DependencyEdge, DependencyIndex,
    DependencyKind, DiagnosticKey, Digest, DigestKind, GenerationIdentity,
    INPUT_SNAPSHOT_SCHEMA_VERSION, InputDependencyKey, InputSnapshot, LayerKey, LayerRunMetadata,
    PrecisionTier, ProviderOutputMeta, ProviderValidationStatus, QueryKey, RunIdentity,
    RunManifestKey, ShapeKind, SummaryKey, WorkspaceIdentity, query_dependency_edges_shared,
};
#[cfg(test)]
use crate::analysis::summaries::provider::SccClosureDebugSnapshot;
use crate::analysis_kernel::metadata::FinalizedCanonicalFactRows;
use crate::analysis_kernel::store::StoreOutcome;
#[cfg(test)]
use crate::analysis_kernel::store::StoreStatus;
use crate::analysis_kernel::validation::{
    ValidationEvent, ValidationEventKind, ValidationEventStatus,
};
use crate::analysis_kernel::{ProviderManifest, StableFactMetaRow};

type CanonicalQueryIdentity = Arc<QueryKey>;

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
    run_manifest_key: RunManifestKey,
    diagnostic_keys: Vec<DiagnosticKey>,
    declared_dependency_edges: Vec<DependencyEdge>,
    dependency_index: DependencyIndex,
    fact_rows: Vec<StableFactMetaRow>,
    validation_events: Vec<ValidationEvent>,
    identities: CanonicalRunIdentities,
}

pub(in crate::analysis_kernel) struct PreparedValidatedRunMetadata {
    input_snapshot: InputSnapshot,
    provider_manifests: Vec<CanonicalProviderManifest>,
    provider_outputs: Vec<CanonicalProviderOutput>,
    summary_keys: Vec<SummaryKey>,
    query_rows: Vec<CanonicalQueryRow>,
    run_manifest_key: RunManifestKey,
    diagnostic_keys: Vec<DiagnosticKey>,
    declared_dependency_edges: Vec<DependencyEdge>,
    dependency_index: DependencyIndex,
    dependency_digest: Digest,
    canonical_dependency_proof: CanonicalDependencyIndexProof,
    validation_events: Vec<ValidationEvent>,
}

struct CanonicalDependencyIndexProof {
    schema_version: String,
    edge_count: usize,
    digest: Digest,
}

impl CanonicalDependencyIndexProof {
    // The proof is sealed inside this module and is constructed at the same
    // point as the sorted, deduplicated persistence index and its digest. It
    // lets the optimized handoff verify that exact construction without
    // cloning every dependency edge into a second adjacency index.
    fn from_canonical_index(index: &DependencyIndex, digest: &Digest) -> Self {
        Self {
            schema_version: index.schema_version.clone(),
            edge_count: index.canonical_edges().len(),
            digest: digest.clone(),
        }
    }

    fn matches(&self, index: &DependencyIndex, digest: Option<&Digest>) -> bool {
        index.schema_version == self.schema_version
            && index.canonical_edges().len() == self.edge_count
            && digest == Some(&self.digest)
    }
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

#[derive(Serialize)]
struct CanonicalProviderOutputIdentity<'a> {
    provider_id: &'a str,
    provider_version: &'a str,
    schema_version: &'a str,
    output_digest: &'a Digest,
    precision: PrecisionTier,
    validation: ProviderValidationStatus,
    dependency_inputs: &'a [Digest],
    layer_count: usize,
}

#[derive(Serialize)]
struct CanonicalLayerIdentity<'a> {
    key: &'a LayerKey,
    output_digest: &'a Digest,
    payload_digest: &'a Digest,
    precision: PrecisionTier,
    validation: ProviderValidationStatus,
    edge_count: usize,
    warning_codes: &'a [String],
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
        input_snapshot: &InputSnapshot,
        provider_output_rows: &[ProviderOutputMeta],
        demand_query_trace: &DemandQueryTrace,
        validation_event_rows: &[ValidationEvent],
        manifests: &[ProviderManifest],
        fact_rows: Vec<StableFactMetaRow>,
    ) -> Result<Self, ValidatedRunMetadataError> {
        let fact_rows = canonical_fact_rows(fact_rows)?;
        crate::analysis_kernel::store::record_handoff_materialization();
        let prepared = Self::prepare_canonical_finalized_parts(
            input_snapshot,
            provider_output_rows,
            demand_query_trace,
            validation_event_rows,
            manifests,
            true,
        )?;
        Self::finish_canonical_finalized_parts(prepared, fact_rows, None, true)
    }

    pub(in crate::analysis_kernel) fn prepare_finalized_canonical_run(
        input_snapshot: &InputSnapshot,
        provider_output_rows: &[ProviderOutputMeta],
        demand_query_trace: &DemandQueryTrace,
        validation_event_rows: &[ValidationEvent],
        manifests: &[ProviderManifest],
    ) -> Result<PreparedValidatedRunMetadata, ValidatedRunMetadataError> {
        Self::prepare_canonical_finalized_parts(
            input_snapshot,
            provider_output_rows,
            demand_query_trace,
            validation_event_rows,
            manifests,
            false,
        )
    }

    pub(in crate::analysis_kernel) fn finish_prepared_canonical_run(
        prepared: PreparedValidatedRunMetadata,
        finalized_facts: FinalizedCanonicalFactRows,
    ) -> Result<Self, ValidatedRunMetadataError> {
        let (fact_rows, fact_digest) = finalized_facts.into_parts();
        Self::finish_canonical_finalized_parts(prepared, fact_rows, Some(fact_digest), false)
    }

    fn prepare_canonical_finalized_parts(
        input_snapshot: &InputSnapshot,
        provider_output_rows: &[ProviderOutputMeta],
        demand_query_trace: &DemandQueryTrace,
        validation_event_rows: &[ValidationEvent],
        manifests: &[ProviderManifest],
        verify_reconstruction: bool,
    ) -> Result<PreparedValidatedRunMetadata, ValidatedRunMetadataError> {
        if input_snapshot.schema_version != INPUT_SNAPSHOT_SCHEMA_VERSION {
            return Err(ValidatedRunMetadataError::new(format!(
                "validated run uses unsupported input snapshot schema `{}`",
                input_snapshot.schema_version
            )));
        }

        let provider_manifests = canonical_provider_manifests(input_snapshot, manifests)?;
        let provider_outputs = canonical_provider_outputs(provider_output_rows, manifests)?;
        let query_rows = demand_query_trace
            .semantic_projections()
            .into_iter()
            .map(|projection| CanonicalQueryRow {
                query_key: Arc::clone(projection.query_key),
                result_digest: projection.result_digest.clone(),
                precision: projection.precision_tier,
                provenance: projection.provenance.to_string(),
            })
            .collect::<Vec<_>>();
        let summary_keys = Vec::new();
        let diagnostic_keys = Vec::new();
        let declared_dependency_edges = Vec::new();
        let validation_events = canonical_validation_events(validation_event_rows.to_vec())?;
        let run_manifest_key = run_manifest_key_for(input_snapshot, &provider_manifests)?;
        let dependency_index = dependency_index_for(
            input_snapshot,
            &provider_outputs,
            &query_rows,
            &run_manifest_key,
            &diagnostic_keys,
            &declared_dependency_edges,
            verify_reconstruction,
        );
        let dependency_digest = dependency_rows_digest(dependency_index.canonical_edges());
        let canonical_dependency_proof = CanonicalDependencyIndexProof::from_canonical_index(
            &dependency_index,
            &dependency_digest,
        );
        Ok(PreparedValidatedRunMetadata {
            input_snapshot: input_snapshot.clone(),
            provider_manifests,
            provider_outputs,
            summary_keys,
            query_rows,
            run_manifest_key,
            diagnostic_keys,
            declared_dependency_edges,
            dependency_index,
            dependency_digest,
            canonical_dependency_proof,
            validation_events,
        })
    }

    fn finish_canonical_finalized_parts(
        prepared: PreparedValidatedRunMetadata,
        fact_rows: Vec<StableFactMetaRow>,
        fact_digest: Option<Digest>,
        verify_reconstruction: bool,
    ) -> Result<Self, ValidatedRunMetadataError> {
        let PreparedValidatedRunMetadata {
            input_snapshot,
            provider_manifests,
            provider_outputs,
            summary_keys,
            query_rows,
            run_manifest_key,
            diagnostic_keys,
            declared_dependency_edges,
            dependency_index,
            dependency_digest,
            canonical_dependency_proof,
            validation_events,
        } = prepared;
        let identities = CanonicalRunIdentities::from_semantic_rows(
            &input_snapshot,
            &provider_manifests,
            &provider_outputs,
            &summary_keys,
            &query_rows,
            &fact_rows,
            fact_digest.as_ref(),
            &dependency_index,
            Some(&dependency_digest),
            &validation_events,
        )?;

        let metadata = Self {
            input_snapshot,
            provider_manifests,
            provider_outputs,
            summary_keys,
            query_rows,
            run_manifest_key,
            diagnostic_keys,
            declared_dependency_edges,
            dependency_index,
            fact_rows,
            validation_events,
            identities,
        };
        if verify_reconstruction {
            metadata.validate_integrity()?;
        } else {
            let fact_digest = fact_digest.as_ref().ok_or_else(|| {
                ValidatedRunMetadataError::new(
                    "optimized validated-run handoff is missing its canonical fact digest",
                )
            })?;
            metadata.validate_integrity_with_canonical_proofs(
                fact_digest,
                &dependency_digest,
                &canonical_dependency_proof,
            )?;
        }
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

    pub(in crate::analysis_kernel) fn run_manifest_key(&self) -> &RunManifestKey {
        &self.run_manifest_key
    }

    pub(in crate::analysis_kernel) fn diagnostic_keys(&self) -> &[DiagnosticKey] {
        &self.diagnostic_keys
    }

    pub(in crate::analysis_kernel) fn dependency_index(&self) -> &DependencyIndex {
        &self.dependency_index
    }

    pub(in crate::analysis_kernel) fn take_dependency_index(&mut self) -> DependencyIndex {
        std::mem::take(&mut self.dependency_index)
    }

    pub(in crate::analysis_kernel) fn fact_rows(&self) -> &[StableFactMetaRow] {
        &self.fact_rows
    }

    pub(in crate::analysis_kernel) fn take_fact_rows(&mut self) -> Vec<StableFactMetaRow> {
        std::mem::take(&mut self.fact_rows)
    }

    #[cfg(test)]
    pub(crate) fn corrupt_first_fact_stable_key_for_store_test(&mut self) {
        self.fact_rows
            .first_mut()
            .expect("store fixture has fact metadata")
            .stable_key = String::new().into();
    }

    #[cfg(test)]
    pub(crate) fn corrupt_first_fact_producer_for_store_test(&mut self) {
        self.fact_rows
            .first_mut()
            .expect("store fixture has fact metadata")
            .producer_id = "unknown.provider".into();
    }

    #[cfg(test)]
    pub(crate) fn corrupt_first_file_path_for_store_test(&mut self) {
        self.input_snapshot
            .files
            .first_mut()
            .expect("store fixture has an input file")
            .relative_path = "/private/not-repository-relative.go".to_string();
    }

    pub(in crate::analysis_kernel) fn validation_events(&self) -> &[ValidationEvent] {
        &self.validation_events
    }

    pub(in crate::analysis_kernel) fn identities(&self) -> &CanonicalRunIdentities {
        &self.identities
    }

    pub(in crate::analysis_kernel) fn validate_integrity(
        &self,
    ) -> Result<(), ValidatedRunMetadataError> {
        self.validate_integrity_inner(None, None, false, None)
    }

    fn validate_integrity_with_canonical_proofs(
        &self,
        fact_digest: &Digest,
        dependency_digest: &Digest,
        dependency_proof: &CanonicalDependencyIndexProof,
    ) -> Result<(), ValidatedRunMetadataError> {
        if fact_digest.kind != DigestKind::FactMetadata
            || dependency_digest.kind != DigestKind::Dependency
        {
            return Err(ValidatedRunMetadataError::new(
                "optimized validated-run handoff has a wrong-purpose canonical digest",
            ));
        }
        self.validate_integrity_inner(
            Some(fact_digest),
            Some(dependency_digest),
            true,
            Some(dependency_proof),
        )
    }

    fn validate_integrity_inner(
        &self,
        fact_digest: Option<&Digest>,
        dependency_digest: Option<&Digest>,
        fact_rows_are_canonical: bool,
        canonical_dependency_proof: Option<&CanonicalDependencyIndexProof>,
    ) -> Result<(), ValidatedRunMetadataError> {
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
        require_strictly_sorted(&self.diagnostic_keys, "diagnostic keys")?;
        require_strictly_sorted(&self.declared_dependency_edges, "declared dependency edges")?;
        for provider in &self.provider_outputs {
            require_strictly_sorted(&provider.layers, "provider layers")?;
        }

        validate_provider_relationships(
            &self.input_snapshot,
            &self.provider_manifests,
            &self.provider_outputs,
        )?;
        if !fact_rows_are_canonical {
            validate_canonical_fact_rows(&self.fact_rows)?;
        }
        if canonical_validation_events(self.validation_events.clone())? != self.validation_events {
            return Err(ValidatedRunMetadataError::new(
                "validated-run validation events are not canonical",
            ));
        }

        let expected_run_manifest_key =
            run_manifest_key_for(&self.input_snapshot, &self.provider_manifests)?;
        if self.run_manifest_key != expected_run_manifest_key {
            return Err(ValidatedRunMetadataError::new(
                "validated-run manifest key does not match canonical run identity",
            ));
        }
        let dependency_index_matches = if let Some(proof) = canonical_dependency_proof {
            proof.matches(&self.dependency_index, dependency_digest)
        } else {
            // General callers can supply an independently assembled handoff, so
            // retain the full reconstruction check when no sealed proof exists.
            let expected_dependency_index = dependency_index_for(
                &self.input_snapshot,
                &self.provider_outputs,
                &self.query_rows,
                &self.run_manifest_key,
                &self.diagnostic_keys,
                &self.declared_dependency_edges,
                true,
            );
            self.dependency_index == expected_dependency_index
        };
        if !dependency_index_matches {
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
            fact_digest,
            &self.dependency_index,
            dependency_digest,
            &self.validation_events,
        )?;
        if self.identities != expected_identities {
            return Err(ValidatedRunMetadataError::new(
                "validated-run identities do not match their canonical semantic rows",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn with_dependency_fixture(
        mut self,
        mut diagnostic_keys: Vec<DiagnosticKey>,
        mut declared_dependency_edges: Vec<DependencyEdge>,
    ) -> Result<Self, ValidatedRunMetadataError> {
        diagnostic_keys.sort();
        diagnostic_keys.dedup();
        declared_dependency_edges.sort();
        declared_dependency_edges.dedup();
        self.diagnostic_keys = diagnostic_keys;
        self.declared_dependency_edges = declared_dependency_edges;
        self.dependency_index = dependency_index_for(
            &self.input_snapshot,
            &self.provider_outputs,
            &self.query_rows,
            &self.run_manifest_key,
            &self.diagnostic_keys,
            &self.declared_dependency_edges,
            true,
        );
        self.identities = CanonicalRunIdentities::from_semantic_rows(
            &self.input_snapshot,
            &self.provider_manifests,
            &self.provider_outputs,
            &self.summary_keys,
            &self.query_rows,
            &self.fact_rows,
            None,
            &self.dependency_index,
            None,
            &self.validation_events,
        )?;
        self.validate_integrity()?;
        Ok(self)
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
    pub(in crate::analysis_kernel) fn query_key(&self) -> &QueryKey {
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
        fact_digest: Option<&Digest>,
        dependency_index: &DependencyIndex,
        dependency_digest: Option<&Digest>,
        validation_events: &[ValidationEvent],
    ) -> Result<Self, ValidatedRunMetadataError> {
        let input_snapshot_digest = input_snapshot.semantic_digest();
        let (provider_manifest, run) = run_identity_for(input_snapshot, provider_manifests)?;
        let provider_output_rows = provider_outputs
            .iter()
            .map(|provider| CanonicalProviderOutputIdentity {
                provider_id: &provider.provider_id,
                provider_version: &provider.provider_version,
                schema_version: &provider.schema_version,
                output_digest: &provider.output_digest,
                precision: provider.precision,
                validation: provider.validation,
                dependency_inputs: &provider.dependency_inputs,
                layer_count: provider.layers.len(),
            })
            .collect::<Vec<_>>();
        let provider_output = serialized_rows_digest(
            DigestKind::ProviderOutput,
            "provider_output_rows",
            &provider_output_rows,
        );
        let layer_rows = provider_outputs
            .iter()
            .flat_map(|provider| provider.layers.iter())
            .map(|layer| CanonicalLayerIdentity {
                key: &layer.key,
                output_digest: &layer.output_digest,
                payload_digest: &layer.payload_digest,
                precision: layer.precision,
                validation: layer.validation,
                edge_count: layer.dependencies.len(),
                warning_codes: &layer.warning_codes,
            })
            .collect::<Vec<_>>();
        // Each normalized family contributes once: provider rows retain layer
        // cardinality, layer rows retain edge cardinality, and concrete edges
        // belong to the dependency aggregate composed into generation identity.
        let layer = serialized_rows_digest(DigestKind::Layer, "layer_rows", &layer_rows);
        let summary = serialized_rows_digest(DigestKind::Summary, "summary_rows", summary_keys);
        let query = serialized_rows_digest(DigestKind::Query, "query_rows", query_rows);
        let fact = fact_digest
            .cloned()
            .unwrap_or_else(|| fact_rows_digest(fact_rows));
        let dependency = dependency_digest
            .cloned()
            .unwrap_or_else(|| dependency_rows_digest(dependency_index.canonical_edges()));
        let validation = serialized_rows_digest(
            DigestKind::ValidationEvent,
            "validation_rows",
            validation_events,
        );
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

fn run_identity_for(
    input_snapshot: &InputSnapshot,
    provider_manifests: &[CanonicalProviderManifest],
) -> Result<(Digest, RunIdentity), ValidatedRunMetadataError> {
    let input_snapshot_digest = input_snapshot.semantic_digest();
    let provider_manifest = Digest::from_unordered(
        DigestKind::ProviderManifest,
        "provider_manifest_rows",
        provider_manifests
            .iter()
            .map(|row| row.manifest_digest.clone())
            .collect(),
    );
    let run = RunIdentity::new(
        &input_snapshot.workspace_identity,
        &input_snapshot.config_identity,
        &input_snapshot_digest,
        &provider_manifest,
    )
    .map_err(|error| ValidatedRunMetadataError::new(error.to_string()))?;
    Ok((provider_manifest, run))
}

fn run_manifest_key_for(
    input_snapshot: &InputSnapshot,
    provider_manifests: &[CanonicalProviderManifest],
) -> Result<RunManifestKey, ValidatedRunMetadataError> {
    let (_, run) = run_identity_for(input_snapshot, provider_manifests)?;
    Ok(RunManifestKey::new(
        run,
        input_snapshot.config_identity.clone(),
    ))
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

fn validate_canonical_fact_rows(
    rows: &[StableFactMetaRow],
) -> Result<(), ValidatedRunMetadataError> {
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
        if pair[0] >= pair[1] {
            return Err(ValidatedRunMetadataError::new(
                "validated-run fact metadata rows are not canonical",
            ));
        }
    }
    Ok(())
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
    input_snapshot: &InputSnapshot,
    provider_outputs: &[CanonicalProviderOutput],
    query_rows: &[CanonicalQueryRow],
    run_manifest_key: &RunManifestKey,
    diagnostic_keys: &[DiagnosticKey],
    declared_dependency_edges: &[DependencyEdge],
    populate_adjacency: bool,
) -> DependencyIndex {
    let layer_edges = provider_outputs.iter().flat_map(|provider| {
        provider.layers.iter().flat_map(|layer| {
            let from = CacheNode::layer(layer.key.clone());
            layer.dependencies.iter().cloned().map(move |mut edge| {
                edge.from = from.clone();
                edge
            })
        })
    });
    let query_edges = query_rows
        .iter()
        .flat_map(|row| query_dependency_edges_shared(Arc::clone(&row.query_key)));
    let boundary_edges = run_boundary_edges(input_snapshot, run_manifest_key, diagnostic_keys);
    let edges = layer_edges
        .chain(query_edges)
        .chain(boundary_edges)
        .chain(declared_dependency_edges.iter().cloned())
        .collect();
    if populate_adjacency {
        DependencyIndex::from_edges(edges)
    } else {
        DependencyIndex::from_edges_for_persistence(edges)
    }
}

fn run_boundary_edges(
    input_snapshot: &InputSnapshot,
    run_manifest_key: &RunManifestKey,
    diagnostic_keys: &[DiagnosticKey],
) -> Vec<DependencyEdge> {
    let config = InputDependencyKey::config(
        input_snapshot.config.name.clone(),
        input_snapshot.config_identity.digest().clone(),
        input_snapshot.config.status,
    )
    .expect("canonical config component has a config digest");
    let mut edges = vec![DependencyEdge {
        from: CacheNode::RunManifest(run_manifest_key.clone()),
        to: CacheNode::DependencyInput(config),
        kind: DependencyKind::Config,
        required_shape: ShapeKind::Unknown,
    }];

    for diagnostic in diagnostic_keys {
        for component in &input_snapshot.rules {
            let (input, required_shape) = match component.digest.kind {
                DigestKind::RuleCode if component.digest == diagnostic.rule_code_digest => (
                    InputDependencyKey::rule_code(
                        component.name.clone(),
                        component.digest.clone(),
                        component.status,
                    )
                    .expect("canonical rule-code component has a rule-code digest"),
                    ShapeKind::RuleCode,
                ),
                DigestKind::RuleOptions if component.digest == diagnostic.options_digest => (
                    InputDependencyKey::rule_options(
                        component.name.clone(),
                        component.digest.clone(),
                        component.status,
                    )
                    .expect("canonical rule-options component has a rule-options digest"),
                    ShapeKind::RuleOptions,
                ),
                _ => continue,
            };
            edges.push(DependencyEdge {
                from: CacheNode::Diagnostic(diagnostic.clone()),
                to: CacheNode::DependencyInput(input),
                kind: DependencyKind::Rule,
                required_shape,
            });
        }
    }
    edges
}

fn serialized_rows_digest<T: Serialize + Sync>(
    kind: DigestKind,
    label: &'static str,
    rows: &[T],
) -> Digest {
    let row_digests = rows
        .par_iter()
        .map(|row| {
            let encoded = serde_json::to_string(row)
                .expect("canonical in-memory semantic rows must serialize");
            Digest::from_parts(kind, label, &[&encoded])
        })
        .collect();
    Digest::from_unordered(kind, label, row_digests)
}

fn dependency_rows_digest(rows: &[DependencyEdge]) -> Digest {
    const LABEL: &str = "dependency_rows";

    let mut encoded_from_nodes = Vec::<String>::new();
    let mut row_from_ordinals = Vec::with_capacity(rows.len());
    let mut prior_from = None::<(&CacheNode, usize)>;
    for row in rows {
        let from_ordinal = prior_from
            .filter(|(node, _)| same_dependency_endpoint(node, &row.from))
            .map(|(_, ordinal)| ordinal)
            .unwrap_or_else(|| {
                let ordinal = encoded_from_nodes.len();
                encoded_from_nodes.push(
                    serde_json::to_string(&row.from)
                        .expect("canonical dependency endpoint serializes"),
                );
                prior_from = Some((&row.from, ordinal));
                ordinal
            });
        row_from_ordinals.push(from_ordinal);
    }

    let row_digests = rows
        .par_iter()
        .zip(&row_from_ordinals)
        .map(|(row, from_ordinal)| {
            let from = encoded_from_nodes
                .get(*from_ordinal)
                .expect("from endpoint was encoded");
            let to =
                serde_json::to_string(&row.to).expect("canonical dependency endpoint serializes");
            Digest::fingerprint_from_fragments(
                DigestKind::Dependency,
                LABEL,
                &[
                    "{\"from\":",
                    from,
                    ",\"to\":",
                    &to,
                    ",\"kind\":\"",
                    row.kind.label(),
                    "\",\"required_shape\":\"",
                    row.required_shape.label(),
                    "\"}",
                ],
            )
        })
        .collect();
    Digest::from_unordered_same_kind_fingerprints(
        DigestKind::Dependency,
        LABEL,
        DigestKind::Dependency,
        row_digests,
    )
}

fn same_dependency_endpoint(left: &CacheNode, right: &CacheNode) -> bool {
    match (left, right) {
        (CacheNode::Query(left), CacheNode::Query(right)) => {
            Arc::ptr_eq(left, right) || left == right
        }
        _ => left == right,
    }
}

fn fact_rows_digest(rows: &[StableFactMetaRow]) -> Digest {
    let row_digests = rows
        .iter()
        .map(|row| {
            let stable_key = row.stable_key.decoded();
            Digest::from_parts(
                DigestKind::FactMetadata,
                "fact_metadata_row",
                &[
                    row.family.label(),
                    &stable_key,
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
    store_outcome: StoreOutcome,
    #[cfg(test)]
    pub(crate) scc_closure_debug: Option<SccClosureDebugSnapshot>,
}

impl KernelRunReport {
    pub(in crate::analysis_kernel) fn new(
        input_snapshot: InputSnapshot,
        provider_outputs: Vec<ProviderOutputMeta>,
        demand_query_trace: DemandQueryTrace,
        validation_events: Vec<ValidationEvent>,
        store_outcome: StoreOutcome,
    ) -> Self {
        let mut cache_stats = aggregate_cache_stats(&provider_outputs);
        aggregate_demand_query_stats(&demand_query_trace, &mut cache_stats);

        Self {
            input_snapshot,
            provider_outputs,
            cache_stats,
            demand_query_trace,
            validation_events,
            store_outcome,
            #[cfg(test)]
            scc_closure_debug: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        input_snapshot: InputSnapshot,
        provider_outputs: Vec<ProviderOutputMeta>,
        demand_query_trace: DemandQueryTrace,
        validation_events: Vec<ValidationEvent>,
        store_status: StoreStatus,
    ) -> Self {
        Self::new(
            input_snapshot,
            provider_outputs,
            demand_query_trace,
            validation_events,
            StoreOutcome {
                status: store_status,
                statistics: None,
            },
        )
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
        &self.store_outcome.status
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
        let stable_key = row.stable_key.decoded();
        builder.labeled_part("fact_family", row.family.label());
        builder.labeled_part("stable_key", &stable_key);
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

    fn validated_run_from_report(
        report: &KernelRunReport,
        manifests: &[ProviderManifest],
        fact_rows: Vec<StableFactMetaRow>,
    ) -> Result<ValidatedRunMetadata, ValidatedRunMetadataError> {
        ValidatedRunMetadata::from_finalized_run(
            &report.input_snapshot,
            &report.provider_outputs,
            &report.demand_query_trace,
            &report.validation_events,
            manifests,
            fact_rows,
        )
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
            query_key: query_key.into(),
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
            stable_key: stable_key.into(),
            producer_id: manifest.id.into(),
            layer_id: manifest.id.into(),
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
        row.stable_key = "fact:changed".into();
        mutations.push(row);
        let mut row = base.clone();
        row.producer_id = "polint.changed.producer".into();
        mutations.push(row);
        let mut row = base.clone();
        row.layer_id = "polint.changed.layer".into();
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
    fn cached_dependency_digest_matches_full_row_serialization() {
        let (report, fact_rows) = finalized_run_fixture();
        let report = report_with_query_rows(report);
        let metadata =
            validated_run_from_report(&report, AnalysisKernel::provider_manifests(), fact_rows)
                .expect("fixture handoff is valid");
        let rows = metadata.dependency_index().canonical_edges();

        assert_eq!(
            dependency_rows_digest(rows),
            serialized_rows_digest(DigestKind::Dependency, "dependency_rows", rows)
        );
    }

    #[test]
    fn validated_run_metadata_is_identical_across_twenty_four_permutations() {
        let (report, fact_rows) = finalized_run_fixture();
        let report = report_with_query_rows(report);
        let manifests = AnalysisKernel::provider_manifests().to_vec();
        let baseline = validated_run_from_report(&report, &manifests, fact_rows.clone())
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

            let candidate =
                validated_run_from_report(&candidate_report, &candidate_manifests, candidate_facts)
                    .expect("permuted handoff is valid");
            assert_eq!(candidate, baseline, "permutation {permutation}");
        }
    }

    #[test]
    fn telemetry_mutations_preserve_every_semantic_identity_family() {
        let (report, fact_rows) = finalized_run_fixture();
        let report = report_with_query_rows(report);
        let manifests = AnalysisKernel::provider_manifests();
        let baseline = validated_run_from_report(&report, manifests, fact_rows.clone())
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
        let changed = validated_run_from_report(&changed_report, manifests, fact_rows)
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
        let metadata =
            validated_run_from_report(&report, AnalysisKernel::provider_manifests(), fact_rows)
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
    fn canonical_dependency_proof_binds_schema_count_and_digest() {
        let index = DependencyIndex::from_edges_for_persistence(Vec::new());
        let digest = dependency_rows_digest(index.canonical_edges());
        let proof = CanonicalDependencyIndexProof::from_canonical_index(&index, &digest);
        assert!(proof.matches(&index, Some(&digest)));

        let wrong_digest = Digest::from_parts(DigestKind::Dependency, "wrong", &["digest"]);
        assert!(!proof.matches(&index, Some(&wrong_digest)));

        let mut wrong_schema = index;
        wrong_schema.schema_version.push_str(".tampered");
        assert!(!proof.matches(&wrong_schema, Some(&digest)));

        let query = dependency_free_test_query_key(
            "proof-count",
            "1",
            Digest::from_parts(DigestKind::QueryParameters, "proof", &["parameters"]),
            Digest::from_parts(DigestKind::Budget, "proof", &["budget"]),
            PrecisionTier::SetupAware,
        );
        let wrong_count =
            DependencyIndex::from_edges_for_persistence(query_dependency_edges(&query));
        assert!(!proof.matches(&wrong_count, Some(&digest)));
    }

    #[test]
    fn handoff_retains_payload_digests_and_excludes_forbidden_content_fields() {
        let (report, fact_rows) = finalized_run_fixture();
        let metadata =
            validated_run_from_report(&report, AnalysisKernel::provider_manifests(), fact_rows)
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
            validated_run_from_report(
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
            validated_run_from_report(
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
