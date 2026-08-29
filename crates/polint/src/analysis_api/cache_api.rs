//! Object-safe cache contracts for language frontends.
//!
//! The composition root (`polint`) implements [`AnalysisCache`] for its disk cache.
//! Frontends (`polint-go`, `polint-ts`) depend only on this trait.

use crate::analysis_api::digest::Digest;

/// File-level JSON cache key parts (mirrors facade `CacheKey::for_file` inputs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCacheKeyParts {
    pub relative_path: String,
    pub content_hash: String,
    pub config_hash: String,
    pub rule_hash: String,
    pub plan_hash: String,
    pub schema: String,
    /// Parser that produced the cached facts (see [`crate::analysis_api::GO_PARSER_BACKEND`]).
    /// Cached facts are only interchangeable with freshly parsed ones when the
    /// parser matches, so it is part of the identity of the entry.
    pub parser_identity: String,
}

/// Layer-cache identity parts used by syntax frontends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerCacheKeyParts {
    pub layer_kind: LayerCacheKind,
    pub provider_id: String,
    pub provider_version: String,
    pub schema_version: String,
    pub parameter_digest: Digest,
    pub lifecycle_digest: Digest,
    pub config_digest: Digest,
    pub toolchain_digest: Digest,
    pub input_digests: Vec<Digest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerCacheKind {
    GoSyntax,
    TsSyntax,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCacheReadStatus {
    Disabled,
    Miss,
    Hit,
    InvalidEvicted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCacheReadOutcome {
    pub status: FileCacheReadStatus,
    pub value: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerCacheReadStatus {
    Hit,
    Miss,
    InvalidEvicted,
    BypassedDisabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerCacheReadOutcome {
    pub status: LayerCacheReadStatus,
    pub output_digest: Option<Digest>,
    pub payload_digest: Option<Digest>,
    pub value: Option<Vec<u8>>,
}

/// Digests a cache recorded for the entry a read validator is inspecting.
///
/// `payload` is the digest of the very bytes handed to the validator: a cache
/// verifies it against the stored blob before running the validator, so a
/// validator can derive the expected output digest from it instead of
/// re-serializing the payload it just parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerCacheEntryDigests<'a> {
    pub output: Option<&'a Digest>,
    pub payload: Option<&'a Digest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerCacheWriteStatus {
    Written,
    BypassedDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerCachePrecision {
    Syntax,
    SetupAware,
    Exact,
}

/// Object-safe analysis cache used by language frontends.
pub trait AnalysisCache: Send + Sync {
    fn read_file_json(&self, key: &FileCacheKeyParts) -> FileCacheReadOutcome;
    fn write_file_json(&self, key: &FileCacheKeyParts, bytes: &[u8]) -> Result<(), String>;

    fn read_layer_json(
        &self,
        key: &LayerCacheKeyParts,
        validate: &mut dyn FnMut(&[u8], LayerCacheEntryDigests<'_>) -> bool,
    ) -> LayerCacheReadOutcome;

    fn write_layer_json(
        &self,
        key: &LayerCacheKeyParts,
        output_digest: &Digest,
        payload_digest: &Digest,
        precision: LayerCachePrecision,
        validation: &str,
        payload: &[u8],
    ) -> Result<LayerCacheWriteStatus, String>;

    fn payload_digest_for_json_bytes(&self, payload: &[u8]) -> Result<Digest, String>;
}

/// Always-disabled cache for unit tests and cache-bypass paths.
#[derive(Debug, Default, Clone, Copy)]
pub struct DisabledAnalysisCache;

impl AnalysisCache for DisabledAnalysisCache {
    fn read_file_json(&self, _key: &FileCacheKeyParts) -> FileCacheReadOutcome {
        FileCacheReadOutcome {
            status: FileCacheReadStatus::Disabled,
            value: None,
        }
    }

    fn write_file_json(&self, _key: &FileCacheKeyParts, _bytes: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn read_layer_json(
        &self,
        _key: &LayerCacheKeyParts,
        _validate: &mut dyn FnMut(&[u8], LayerCacheEntryDigests<'_>) -> bool,
    ) -> LayerCacheReadOutcome {
        LayerCacheReadOutcome {
            status: LayerCacheReadStatus::BypassedDisabled,
            output_digest: None,
            payload_digest: None,
            value: None,
        }
    }

    fn write_layer_json(
        &self,
        _key: &LayerCacheKeyParts,
        _output_digest: &Digest,
        _payload_digest: &Digest,
        _precision: LayerCachePrecision,
        _validation: &str,
        _payload: &[u8],
    ) -> Result<LayerCacheWriteStatus, String> {
        Ok(LayerCacheWriteStatus::BypassedDisabled)
    }

    fn payload_digest_for_json_bytes(&self, payload: &[u8]) -> Result<Digest, String> {
        use crate::analysis_api::digest::DigestKind;
        let payload = std::str::from_utf8(payload)
            .map_err(|_| "layer cache payload was not UTF-8".to_string())?;
        Ok(Digest::from_parts(
            DigestKind::LayerOutput,
            "layer_cache_payload",
            &[payload],
        ))
    }
}
