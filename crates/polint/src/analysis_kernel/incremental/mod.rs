mod digest;
mod input_snapshot;
mod keys;
mod stats;

pub(crate) use digest::{Digest, DigestKind};
#[expect(
    unused_imports,
    reason = "Phase 23 establishes this crate-private vocabulary before later kernel consumers wire it in."
)]
pub(crate) use input_snapshot::{
    FileSnapshot, INPUT_SNAPSHOT_SCHEMA_VERSION, InputComponent, InputComponentStatus,
    InputSnapshot, ProviderSchemaSnapshot,
};
#[expect(
    unused_imports,
    reason = "Phase 23 establishes this crate-private vocabulary before later kernel consumers wire it in."
)]
pub(crate) use keys::{DiagnosticKey, LayerKey, PrecisionTier, QueryKey, SummaryKey};
#[expect(
    unused_imports,
    reason = "Phase 23 establishes this crate-private vocabulary before later kernel consumers wire it in."
)]
pub(crate) use stats::{CacheStats, ProviderOutputMeta};
