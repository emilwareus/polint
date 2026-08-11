//! Cross-crate analysis contracts for polint.
//!
//! Depends only on `polint-core` and `polint-ir`. Must not import concrete analyses or frontends.

mod cache_api;
mod callable_names;
mod digest;
mod fact_store;
mod metadata;
mod module_facts;
mod provider;
mod source_file;
mod syntax_facts;

pub use cache_api::{
    AnalysisCache, DisabledAnalysisCache, FileCacheKeyParts, FileCacheReadOutcome,
    FileCacheReadStatus, LayerCacheKeyParts, LayerCacheKind, LayerCachePrecision,
    LayerCacheReadOutcome, LayerCacheReadStatus, LayerCacheWriteStatus,
};
pub use callable_names::{
    ANONYMOUS_CALLABLE_PREFIX, anonymous_callable_name, is_anonymous_callable_name,
};
pub use digest::{
    CacheStats, Digest, DigestBuilder, DigestKind, FileSnapshot, GoLifecycleSnapshot,
    INPUT_SNAPSHOT_SCHEMA_VERSION, InputComponent, InputComponentStatus, InputSnapshot,
    ProviderSchemaSnapshot, TsJsLifecycleSnapshot,
};
pub use fact_store::{FactStore, FactStoreEntry};
pub use metadata::{
    FactConfidence, FactFamily, FactMeta, FactMetaInsert, FactMetaStore, FactPrecision, FactRef,
    MissingFactMeta, StableKeyConflict, StableKeyOwner, ValidationStatus, stable_key_from_parts,
    stable_key_text_from_parts,
};
pub use module_facts::{
    ModuleEdge, ModuleEdgeKind, ModuleNode, ModuleNodeKind, ResolutionPrecision, ResolutionStatus,
    ResolvedImportFact, UnresolvedReason,
};
pub use provider::{
    CachePolicy, CaptureEnrichment, FactDatabase, HostAttachment, NullCaptureEnrichment,
    NullHostAttachment, PrecisionCeiling, Provider, ProviderCtx, ProviderHostServices,
    ProviderKind, ProviderManifest, ProviderRunResult, SchemaVersion,
};
pub use source_file::SourceFile;
pub use syntax_facts::{
    BranchObligation, CachedFileAnalysis, CachedFileFacts, CoverageFact, FunctionFact, ImportFact,
    JsxAttributeFact, PackageFact, StringLiteralFact, TS_JS_MODULE_FUNCTION_NAME, TestFact,
    TsClassFact, TsComponentFact, is_synthetic_ts_js_module_function,
};

/// MIR identifiers shared with analysis contracts.
pub use polint_ir::{MirBodyId, PlaceId};
