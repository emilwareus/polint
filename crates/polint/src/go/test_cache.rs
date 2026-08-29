//! Test-only filesystem [`AnalysisCache`] fixture for Go adapter integration tests.

use std::fs;
use std::path::{Path, PathBuf};

use crate::analysis_api::{
    AnalysisCache, Digest, DigestKind, FileCacheKeyParts, FileCacheReadOutcome,
    FileCacheReadStatus, LayerCacheKeyParts, LayerCacheKind, LayerCachePrecision,
    LayerCacheReadOutcome, LayerCacheReadStatus, LayerCacheWriteStatus,
};
use serde::{Deserialize, Serialize};

use crate::go::hash::stable_hash;
use crate::go::repo_fs;

const FILE_CACHE_VERSION: &str = "v1";
const CACHE_MAX_BYTES: u64 = 16 * 1_048_576;

/// Disk-backed analysis cache used only by `polint-go` unit tests.
#[derive(Debug, Clone)]
pub(crate) struct FsAnalysisCache {
    root: PathBuf,
    enabled: bool,
}

impl FsAnalysisCache {
    pub(crate) fn new(root: impl AsRef<Path>, enabled: bool) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            enabled,
        }
    }

    fn layer_cache_dir(&self) -> PathBuf {
        if self.root.file_name().and_then(|name| name.to_str()) == Some("analysis")
            && let Some(parent) = self.root.parent()
        {
            return parent.join("layers");
        }
        self.root.join("layers")
    }

    fn manifests_dir(&self) -> PathBuf {
        self.layer_cache_dir().join("manifests")
    }

    fn blobs_dir(&self) -> PathBuf {
        self.layer_cache_dir().join("blobs")
    }

    fn file_path(&self, key: &FileCacheKeyParts) -> PathBuf {
        self.root.join(format!("{}.json", file_stable_id(key)))
    }

    fn manifest_path(&self, key: &LayerCacheKeyParts) -> PathBuf {
        self.manifests_dir()
            .join(format!("{}.json", layer_key_hash(key)))
    }

    fn blob_path(&self, payload_digest: &Digest) -> PathBuf {
        self.blobs_dir()
            .join(format!("{}.json", payload_digest.value))
    }
}

impl AnalysisCache for FsAnalysisCache {
    fn read_file_json(&self, key: &FileCacheKeyParts) -> FileCacheReadOutcome {
        if !self.enabled {
            return FileCacheReadOutcome {
                status: FileCacheReadStatus::Disabled,
                value: None,
            };
        }
        let path = self.file_path(key);
        match read_bytes(&path) {
            ReadBytes::Missing => FileCacheReadOutcome {
                status: FileCacheReadStatus::Miss,
                value: None,
            },
            ReadBytes::Ok(bytes) => FileCacheReadOutcome {
                status: FileCacheReadStatus::Hit,
                value: Some(bytes),
            },
            ReadBytes::Invalid => {
                let _ = fs::remove_file(&path);
                FileCacheReadOutcome {
                    status: FileCacheReadStatus::InvalidEvicted,
                    value: None,
                }
            }
        }
    }

    fn write_file_json(&self, key: &FileCacheKeyParts, bytes: &[u8]) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        let path = self.file_path(key);
        repo_fs::write_file_atomic_no_symlink(&path, bytes).map_err(|error| error.to_string())
    }

    fn read_layer_json(
        &self,
        key: &LayerCacheKeyParts,
        validate: &mut dyn FnMut(&[u8], Option<&Digest>) -> bool,
    ) -> LayerCacheReadOutcome {
        if !self.enabled {
            return LayerCacheReadOutcome {
                status: LayerCacheReadStatus::BypassedDisabled,
                output_digest: None,
                payload_digest: None,
                value: None,
            };
        }

        let manifest_path = self.manifest_path(key);
        let raw_manifest = match read_bytes(&manifest_path) {
            ReadBytes::Missing => {
                return LayerCacheReadOutcome {
                    status: LayerCacheReadStatus::Miss,
                    output_digest: None,
                    payload_digest: None,
                    value: None,
                };
            }
            ReadBytes::Invalid => {
                let _ = fs::remove_file(&manifest_path);
                return LayerCacheReadOutcome {
                    status: LayerCacheReadStatus::InvalidEvicted,
                    output_digest: None,
                    payload_digest: None,
                    value: None,
                };
            }
            ReadBytes::Ok(bytes) => bytes,
        };

        let Ok(manifest) = serde_json::from_slice::<LayerManifest>(&raw_manifest) else {
            let _ = fs::remove_file(&manifest_path);
            return LayerCacheReadOutcome {
                status: LayerCacheReadStatus::InvalidEvicted,
                output_digest: None,
                payload_digest: None,
                value: None,
            };
        };

        let blob_path = self.blob_path(&manifest.payload_digest);
        let payload_bytes = match read_bytes(&blob_path) {
            ReadBytes::Ok(bytes) => bytes,
            ReadBytes::Missing | ReadBytes::Invalid => {
                let _ = fs::remove_file(&manifest_path);
                return LayerCacheReadOutcome {
                    status: LayerCacheReadStatus::InvalidEvicted,
                    output_digest: None,
                    payload_digest: None,
                    value: None,
                };
            }
        };

        if !validate(&payload_bytes, Some(&manifest.output_digest)) {
            let _ = fs::remove_file(&manifest_path);
            return LayerCacheReadOutcome {
                status: LayerCacheReadStatus::InvalidEvicted,
                output_digest: None,
                payload_digest: None,
                value: None,
            };
        }

        LayerCacheReadOutcome {
            status: LayerCacheReadStatus::Hit,
            output_digest: Some(manifest.output_digest),
            payload_digest: Some(manifest.payload_digest),
            value: Some(payload_bytes),
        }
    }

    fn write_layer_json(
        &self,
        key: &LayerCacheKeyParts,
        output_digest: &Digest,
        payload_digest: &Digest,
        _precision: LayerCachePrecision,
        _validation: &str,
        payload: &[u8],
    ) -> Result<LayerCacheWriteStatus, String> {
        if !self.enabled {
            return Ok(LayerCacheWriteStatus::BypassedDisabled);
        }

        let blob_path = self.blob_path(payload_digest);
        repo_fs::write_file_atomic_no_symlink(&blob_path, payload)
            .map_err(|error| error.to_string())?;

        let manifest = LayerManifest {
            output_digest: output_digest.clone(),
            payload_digest: payload_digest.clone(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
        let manifest_path = self.manifest_path(key);
        repo_fs::write_file_atomic_no_symlink(&manifest_path, manifest_bytes)
            .map_err(|error| error.to_string())?;

        Ok(LayerCacheWriteStatus::Written)
    }

    fn payload_digest_for_json_bytes(&self, payload: &[u8]) -> Result<Digest, String> {
        let payload = std::str::from_utf8(payload)
            .map_err(|_| "layer cache payload was not UTF-8".to_string())?;
        Ok(Digest::from_parts(
            DigestKind::LayerOutput,
            "layer_cache_payload",
            &[payload],
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LayerManifest {
    output_digest: Digest,
    payload_digest: Digest,
}

#[derive(Serialize)]
struct LayerKeySerializable<'a> {
    layer_kind: &'a str,
    provider_id: &'a str,
    provider_version: &'a str,
    schema_version: &'a str,
    parameter_digest: &'a Digest,
    lifecycle_digest: &'a Digest,
    config_digest: &'a Digest,
    toolchain_digest: &'a Digest,
    input_digests: &'a [Digest],
}

fn file_stable_id(key: &FileCacheKeyParts) -> String {
    let file_hash = stable_hash(&[key.relative_path.as_str(), key.content_hash.as_str()]);
    stable_hash(&[
        file_hash.as_str(),
        key.config_hash.as_str(),
        key.rule_hash.as_str(),
        key.plan_hash.as_str(),
        FILE_CACHE_VERSION,
        key.schema.as_str(),
        key.parser_identity.as_str(),
    ])
}

fn layer_key_hash(key: &LayerCacheKeyParts) -> String {
    let layer_kind = match key.layer_kind {
        LayerCacheKind::GoSyntax => "go_syntax",
        LayerCacheKind::TsSyntax => "ts_syntax",
    };
    let serializable = LayerKeySerializable {
        layer_kind,
        provider_id: &key.provider_id,
        provider_version: &key.provider_version,
        schema_version: &key.schema_version,
        parameter_digest: &key.parameter_digest,
        lifecycle_digest: &key.lifecycle_digest,
        config_digest: &key.config_digest,
        toolchain_digest: &key.toolchain_digest,
        input_digests: &key.input_digests,
    };
    let json = serde_json::to_string(&serializable).expect("layer key serializes");
    stable_hash(&[json.as_str()])
}

enum ReadBytes {
    Missing,
    Ok(Vec<u8>),
    Invalid,
}

fn read_bytes(path: &Path) -> ReadBytes {
    if repo_fs::ensure_no_symlink_ancestors(path).is_err() {
        return ReadBytes::Invalid;
    }
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return ReadBytes::Missing;
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return ReadBytes::Invalid;
    }
    match repo_fs::read_file_with_limit(path, CACHE_MAX_BYTES) {
        Ok(bytes) => ReadBytes::Ok(bytes),
        Err(error) if error.is_not_found() => ReadBytes::Missing,
        Err(_) => ReadBytes::Invalid,
    }
}
