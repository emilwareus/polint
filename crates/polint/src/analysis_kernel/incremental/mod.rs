mod dependency_index;
mod digest;
mod input_snapshot;
mod invalidation;
mod keys;
mod run_report;
mod stats;

pub(crate) use dependency_index::{
    CacheNode, DEPENDENCY_INDEX_SCHEMA, DependencyEdge, DependencyIndex, DependencyKind, ShapeKind,
};
pub(crate) use digest::{Digest, DigestKind};
#[expect(
    unused_imports,
    reason = "Phase 23 establishes this crate-private vocabulary before later kernel consumers wire it in."
)]
pub(crate) use input_snapshot::{
    FileSnapshot, GoLifecycleSnapshot, INPUT_SNAPSHOT_SCHEMA_VERSION, InputComponent,
    InputComponentStatus, InputSnapshot, ProviderSchemaSnapshot, TsJsLifecycleSnapshot,
};
pub(crate) use invalidation::{
    DropReason, InvalidationAction, InvalidationPlan, InvalidationStats, QuarantineReason,
    RecomputeReason, VerifyReason,
};
#[expect(
    unused_imports,
    reason = "Phase 23 establishes this crate-private vocabulary before later kernel consumers wire it in."
)]
pub(crate) use keys::{DiagnosticKey, LayerKey, PrecisionTier, QueryKey, SummaryKey};
pub(crate) use run_report::{
    KernelRunReport, provider_output_digest_from_manifest, provider_output_from_manifest,
};
pub(crate) use stats::{CacheStats, ProviderOutputMeta};
