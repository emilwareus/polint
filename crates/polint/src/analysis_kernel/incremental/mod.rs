mod change_set;
mod demand;
mod dependency_index;
mod dependency_input;
mod digest;
mod input_snapshot;
mod invalidation;
mod keys;
mod layer_cache;
mod quarantine;
mod run_report;
mod stats;

#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "Unit tests exercise only selected re-exported cache vocabulary terms."
    )
)]
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "layer manifest reuse retains the complete conservative invalidation classification"
    )
)]
pub(crate) use change_set::{ChangeKind, ChangeSet, ChangeSetRow};
#[cfg(test)]
pub(crate) use demand::dependency_free_test_query_key;
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "Unit tests exercise only selected re-exported demand query vocabulary terms."
    )
)]
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "demand query vocabulary is retained for private query consumers"
    )
)]
pub(crate) use demand::{
    DemandCacheStatus, DemandQueryEngine, DemandQueryResult, DemandQueryTrace,
    DemandQueryTraceEntry,
};
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "Unit tests exercise only selected re-exported cache vocabulary terms."
    )
)]
#[cfg_attr(
    not(test),
    allow(
        unused_imports,
        reason = "the crate-private dependency vocabulary is shared across cache and persistence boundaries"
    )
)]
pub(crate) use dependency_index::{
    CacheNode, CacheNodeKind, DEPENDENCY_INDEX_SCHEMA, DependencyEdge, DependencyIndex,
    DependencyKind, RunManifestKey, ShapeKind, query_dependency_edges,
};
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "unit tests exercise typed dependency inputs in their defining module"
    )
)]
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "typed dependency inputs stay isolated with the consumers that validate their digest purpose"
    )
)]
pub(crate) use dependency_input::{
    InputDependencyDigestKindError, InputDependencyKey, InputDependencyKind,
    UnknownInputDependencyKindLabel,
};
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "unit tests exercise identities in their defining module"
    )
)]
pub(crate) use digest::{
    ConfigIdentity, Digest, DigestBuilder, DigestKind, GenerationIdentity, RunIdentity,
    WorkspaceIdentity,
};
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "Unit tests exercise only selected re-exported cache vocabulary terms."
    )
)]
#[cfg_attr(
    not(test),
    expect(unused_imports, reason = "kept for private internal consumers")
)]
pub(crate) use input_snapshot::{
    AnalysisSettingSource, FileSnapshot, GoLifecycleSnapshot, INPUT_SNAPSHOT_SCHEMA_VERSION,
    InputComponent, InputComponentStatus, InputSnapshot, InputSnapshotIdentitySources,
    ProviderSchemaSnapshot, TsJsLifecycleSnapshot,
};
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "Unit tests exercise only selected re-exported cache vocabulary terms."
    )
)]
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "layer manifest reuse retains every conservative cache-decision action"
    )
)]
pub(crate) use invalidation::{
    DropReason, InvalidationAction, InvalidationPlan, InvalidationStats, QuarantineReason,
    RecomputeReason, VerifyReason,
};
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "Unit tests exercise only selected re-exported cache vocabulary terms."
    )
)]
#[cfg_attr(
    not(test),
    expect(unused_imports, reason = "kept for private internal consumers")
)]
pub(crate) use keys::{
    DiagnosticKey, LayerKey, LayerKind, MODULE_GRAPH_TOPOLOGY_INPUT_FILE_NAMES, PrecisionTier,
    QueryDependencyInputs, QueryKey, SummaryKey, dependency_layer_digest,
    module_graph_topology_input_digest_rows, module_graph_topology_input_digests,
    semantic_provider_parameter_digest,
};
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "Unit tests exercise only selected re-exported cache vocabulary terms."
    )
)]
#[cfg_attr(
    not(test),
    expect(unused_imports, reason = "kept for private internal consumers")
)]
pub(crate) use layer_cache::{
    LAYER_CACHE_MANIFEST_SCHEMA, LayerCacheManifest, LayerCacheReadOutcome, LayerCacheReadStatus,
    LayerCacheStore, LayerCacheWriteStatus, LayerRunMetadata, LayerSemanticProjection,
    relative_manifest_dependency_source,
};
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "Unit tests exercise only selected re-exported quarantine vocabulary terms."
    )
)]
#[cfg_attr(
    not(test),
    expect(unused_imports, reason = "kept for private internal consumers")
)]
pub(crate) use quarantine::{QuarantineEntry, QuarantinePolicy, QuarantineStore};
#[cfg(test)]
pub(crate) use run_report::provider_output_from_manifest;
#[allow(
    unused_imports,
    reason = "private sibling-store row types form the borrowed handoff vocabulary"
)]
pub(in crate::analysis_kernel) use run_report::{
    CanonicalProviderManifest, CanonicalProviderOutput, CanonicalQueryRow, CanonicalRunIdentities,
    ValidatedRunMetadata, ValidatedRunMetadataError,
};
pub(crate) use run_report::{
    KernelRunReport, provider_output_digest_from_manifest,
    provider_output_from_manifest_with_layers,
};
pub(crate) use stats::{CacheStats, ProviderOutputMeta, ProviderValidationStatus};
