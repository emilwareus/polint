//! Cross-crate analysis contracts for polint.
//!
//! Depends only on `polint-core` and `polint-ir`. Must not import concrete analyses or frontends.

mod digest;
mod fact_store;
mod metadata;
mod provider;
mod source_file;

pub use digest::{CacheStats, Digest, DigestBuilder, DigestKind};
pub use fact_store::{FactStore, FactStoreEntry};
pub use metadata::{
    FactConfidence, FactFamily, FactMeta, FactMetaInsert, FactMetaStore, FactPrecision, FactRef,
    MissingFactMeta, StableKeyConflict, StableKeyOwner, ValidationStatus, stable_key_from_parts,
    stable_key_text_from_parts,
};
pub use provider::{
    CachePolicy, FactDatabase, HostAttachment, NullHostAttachment, PrecisionCeiling, Provider,
    ProviderCtx, ProviderHostServices, ProviderKind, ProviderManifest, ProviderRunResult,
    SchemaVersion,
};
pub use source_file::SourceFile;

/// MIR identifiers shared with analysis contracts.
pub use polint_ir::{MirBodyId, PlaceId};
