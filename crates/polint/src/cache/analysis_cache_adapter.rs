//! [`crate::analysis_api::AnalysisCache`] adapter over the facade disk cache.

use std::sync::Arc;

use crate::analysis_api::{
    AnalysisCache, Digest, FileCacheKeyParts, FileCacheReadOutcome, FileCacheReadStatus,
    LayerCacheEntryDigests, LayerCacheKeyParts, LayerCacheKind, LayerCachePrecision,
    LayerCacheReadOutcome, LayerCacheReadStatus, LayerCacheWriteStatus,
};

use crate::analysis_kernel::incremental::{
    CacheNode, DependencyEdge, DependencyKind, LayerCacheManifest,
    LayerCacheReadStatus as FacadeLayerReadStatus, LayerCacheWriteStatus as FacadeLayerWriteStatus,
    LayerKey, LayerKind, PrecisionTier, ShapeKind,
};
use crate::cache::{Cache, CacheKey, CacheReadStatus};

/// Arc-wrapped adapter so [`crate::analysis_kernel::host::FacadeHostServices`] can share it.
pub(crate) type SharedAnalysisCache = Arc<CacheAnalysisCache>;

#[derive(Debug, Clone)]
pub(crate) struct CacheAnalysisCache {
    cache: Cache,
}

impl CacheAnalysisCache {
    pub(crate) fn new(cache: Cache) -> SharedAnalysisCache {
        Arc::new(Self { cache })
    }
}

impl AnalysisCache for CacheAnalysisCache {
    fn read_file_json(&self, key: &FileCacheKeyParts) -> FileCacheReadOutcome {
        let facade_key = CacheKey::for_file(
            &key.relative_path,
            &key.content_hash,
            &key.config_hash,
            &key.rule_hash,
            &key.plan_hash,
            &key.schema,
            &key.parser_identity,
        );
        let read = self
            .cache
            .read_json_with_status::<serde_json::Value>(&facade_key);
        let status = match read.status {
            CacheReadStatus::Disabled => FileCacheReadStatus::Disabled,
            CacheReadStatus::Miss => FileCacheReadStatus::Miss,
            CacheReadStatus::Hit => FileCacheReadStatus::Hit,
            CacheReadStatus::InvalidEvicted => FileCacheReadStatus::InvalidEvicted,
        };
        let value = read.value.and_then(|value| serde_json::to_vec(&value).ok());
        FileCacheReadOutcome { status, value }
    }

    fn write_file_json(&self, key: &FileCacheKeyParts, bytes: &[u8]) -> Result<(), String> {
        let facade_key = CacheKey::for_file(
            &key.relative_path,
            &key.content_hash,
            &key.config_hash,
            &key.rule_hash,
            &key.plan_hash,
            &key.schema,
            &key.parser_identity,
        );
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        self.cache
            .write_json(&facade_key, &value)
            .map_err(|error| error.to_string())
    }

    fn read_layer_json(
        &self,
        key: &LayerCacheKeyParts,
        validate: &mut dyn FnMut(&[u8], LayerCacheEntryDigests<'_>) -> bool,
    ) -> LayerCacheReadOutcome {
        let layer_key = to_layer_key(key);
        let store = self.cache.layer_cache_store();
        let read = store.read_json_bytes_validated(&layer_key, |bytes, manifest| {
            validate(
                bytes,
                LayerCacheEntryDigests {
                    output: Some(&manifest.output_digest),
                    payload: Some(&manifest.payload_digest),
                },
            )
        });
        let status = match read.status {
            FacadeLayerReadStatus::Hit => LayerCacheReadStatus::Hit,
            FacadeLayerReadStatus::Miss => LayerCacheReadStatus::Miss,
            FacadeLayerReadStatus::InvalidEvicted => LayerCacheReadStatus::InvalidEvicted,
            FacadeLayerReadStatus::BypassedDisabled => LayerCacheReadStatus::BypassedDisabled,
        };
        LayerCacheReadOutcome {
            status,
            output_digest: read.output_digest,
            payload_digest: read.payload_digest,
            value: read.value,
        }
    }

    fn write_layer_json(
        &self,
        key: &LayerCacheKeyParts,
        output_digest: &Digest,
        payload_digest: &Digest,
        precision: LayerCachePrecision,
        validation: &str,
        payload: &[u8],
    ) -> Result<LayerCacheWriteStatus, String> {
        let layer_key = to_layer_key(key);
        let precision = match precision {
            LayerCachePrecision::Syntax => PrecisionTier::Syntax,
            LayerCachePrecision::SetupAware => PrecisionTier::SetupAware,
            LayerCachePrecision::Exact => PrecisionTier::Exact,
        };
        let dependencies = layer_key
            .input_digests
            .iter()
            .map(|digest| DependencyEdge {
                from: CacheNode::Input("syntax-layer".to_string()),
                to: CacheNode::Input(format!("syntax-input:{digest}")),
                kind: DependencyKind::SourceText,
                required_shape: ShapeKind::Content,
            })
            .collect();
        let manifest = LayerCacheManifest::new(
            layer_key,
            output_digest.clone(),
            payload_digest.clone(),
            dependencies,
            precision,
            validation,
            Vec::new(),
        );
        let store = self.cache.layer_cache_store();
        match store.write_json_bytes(&manifest, payload.to_vec()) {
            Ok(FacadeLayerWriteStatus::Written) => Ok(LayerCacheWriteStatus::Written),
            Ok(FacadeLayerWriteStatus::BypassedDisabled) => {
                Ok(LayerCacheWriteStatus::BypassedDisabled)
            }
            Err(error) => Err(error.to_string()),
        }
    }

    fn payload_digest_for_json_bytes(&self, payload: &[u8]) -> Result<Digest, String> {
        crate::analysis_kernel::incremental::LayerCacheStore::payload_digest_for_json_bytes(payload)
            .map_err(|error| error.to_string())
    }
}

fn to_layer_key(key: &LayerCacheKeyParts) -> LayerKey {
    let layer_kind = match key.layer_kind {
        LayerCacheKind::GoSyntax => LayerKind::GoSyntax,
        LayerCacheKind::TsSyntax => LayerKind::TsSyntax,
    };
    LayerKey::syntax_layer_key(
        layer_kind,
        key.provider_id.clone(),
        key.provider_version.clone(),
        key.schema_version.clone(),
        key.input_digests.clone(),
        key.config_digest.clone(),
        key.lifecycle_digest.clone(),
        key.toolchain_digest.clone(),
        key.parameter_digest.clone(),
    )
}
