#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 24 establishes layer-cache persistence before providers read or write cached layers."
    )
)]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use super::dependency_index::{CacheNode, DEPENDENCY_INDEX_SCHEMA, DependencyEdge};
use super::digest::{Digest, DigestKind};
use super::keys::{LayerKey, LayerKind, PrecisionTier};
use crate::cache::stable_hash;

pub(crate) const LAYER_CACHE_MANIFEST_SCHEMA: &str = "polint-layer-cache-manifest-1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LayerCacheManifest {
    pub(crate) manifest_schema: String,
    pub(crate) dependency_index_schema: String,
    pub(crate) key: LayerKey,
    pub(crate) output_digest: Digest,
    pub(crate) payload_digest: Digest,
    pub(crate) created_by_polint: String,
    pub(crate) dependencies: Vec<DependencyEdge>,
    pub(crate) precision: PrecisionTier,
    pub(crate) validation: String,
    pub(crate) warnings: Vec<String>,
}

impl LayerCacheManifest {
    pub(crate) fn new(
        key: LayerKey,
        output_digest: Digest,
        payload_digest: Digest,
        mut dependencies: Vec<DependencyEdge>,
        precision: PrecisionTier,
        validation: impl Into<String>,
        mut warnings: Vec<String>,
    ) -> Self {
        dependencies.sort();
        dependencies.dedup();
        warnings.sort();
        warnings.dedup();

        Self {
            manifest_schema: LAYER_CACHE_MANIFEST_SCHEMA.to_string(),
            dependency_index_schema: DEPENDENCY_INDEX_SCHEMA.to_string(),
            key,
            output_digest,
            payload_digest,
            created_by_polint: env!("CARGO_PKG_VERSION").to_string(),
            dependencies,
            precision,
            validation: validation.into(),
            warnings,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LayerCacheStore {
    root: PathBuf,
    enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LayerCacheReadStatus {
    Hit,
    Miss,
    InvalidEvicted,
    BypassedDisabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LayerCacheReadOutcome<T> {
    pub(crate) status: LayerCacheReadStatus,
    pub(crate) manifest: Option<LayerCacheManifest>,
    pub(crate) output_digest: Option<Digest>,
    pub(crate) payload_digest: Option<Digest>,
    pub(crate) value: Option<T>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LayerCacheWriteStatus {
    Written,
    BypassedDisabled,
}

impl LayerCacheStore {
    pub(crate) fn new(root: impl AsRef<Path>, enabled: bool) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            enabled,
        }
    }

    pub(crate) fn payload_digest_for_json<T>(value: &T) -> Result<Digest>
    where
        T: Serialize,
    {
        let payload = serde_json::to_vec(value)?;
        payload_digest_for_bytes(&payload)
    }

    pub(crate) fn read_json<T>(&self, key: &LayerKey) -> LayerCacheReadOutcome<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.read_json_validated(key, |_, _| true)
    }

    pub(crate) fn read_json_validated<T, F>(
        &self,
        key: &LayerKey,
        validator: F,
    ) -> LayerCacheReadOutcome<T>
    where
        T: for<'de> Deserialize<'de>,
        F: FnOnce(&T, &LayerCacheManifest) -> bool,
    {
        if !self.enabled {
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::BypassedDisabled);
        }

        let Ok(manifest_path) = self.manifest_path(key) else {
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::Miss);
        };
        if !manifest_path.exists() {
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::Miss);
        }

        let Some(manifest_path) = self.managed_existing_file(&manifest_path, &self.manifests_dir())
        else {
            evict_file(&manifest_path);
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::InvalidEvicted);
        };
        let Ok(raw_manifest) = fs::read_to_string(&manifest_path) else {
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::Miss);
        };
        let Ok(manifest) = serde_json::from_str::<LayerCacheManifest>(&raw_manifest) else {
            evict_file(&manifest_path);
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::InvalidEvicted);
        };
        if !self.manifest_metadata_is_supported(&manifest, key) {
            evict_file(&manifest_path);
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::InvalidEvicted);
        }

        let Some(blob_path) = self.blob_path(&manifest.payload_digest) else {
            evict_file(&manifest_path);
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::InvalidEvicted);
        };
        let Some(blob_path) = self.managed_existing_file(&blob_path, &self.blobs_dir()) else {
            evict_file(&manifest_path);
            evict_file(&blob_path);
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::InvalidEvicted);
        };
        let Ok(payload_bytes) = fs::read(&blob_path) else {
            evict_file(&manifest_path);
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::InvalidEvicted);
        };
        let Ok(payload_digest) = payload_digest_for_bytes(&payload_bytes) else {
            evict_file(&manifest_path);
            evict_file(&blob_path);
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::InvalidEvicted);
        };
        if payload_digest != manifest.payload_digest {
            evict_file(&manifest_path);
            evict_file(&blob_path);
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::InvalidEvicted);
        }
        let Ok(value) = serde_json::from_slice::<T>(&payload_bytes) else {
            evict_file(&manifest_path);
            evict_file(&blob_path);
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::InvalidEvicted);
        };
        if !validator(&value, &manifest) {
            evict_file(&manifest_path);
            return LayerCacheReadOutcome::without_value(LayerCacheReadStatus::InvalidEvicted);
        }

        LayerCacheReadOutcome {
            status: LayerCacheReadStatus::Hit,
            output_digest: Some(manifest.output_digest.clone()),
            payload_digest: Some(payload_digest),
            manifest: Some(manifest),
            value: Some(value),
        }
    }

    pub(crate) fn write_json<T>(
        &self,
        manifest: &LayerCacheManifest,
        value: &T,
    ) -> Result<LayerCacheWriteStatus>
    where
        T: Serialize,
    {
        if manifest.manifest_schema != LAYER_CACHE_MANIFEST_SCHEMA {
            return Err(anyhow!("unsupported layer cache manifest schema"));
        }
        let payload_digest = Self::payload_digest_for_json(value)?;
        if payload_digest != manifest.payload_digest {
            return Err(anyhow!("layer cache payload digest mismatch"));
        }
        self.write_json_inner(&manifest.key, manifest, value)
    }

    fn write_json_inner<T>(
        &self,
        key: &LayerKey,
        manifest: &LayerCacheManifest,
        value: &T,
    ) -> Result<LayerCacheWriteStatus>
    where
        T: Serialize,
    {
        if !self.enabled {
            return Ok(LayerCacheWriteStatus::BypassedDisabled);
        }

        let payload_path = self
            .blob_path(&manifest.payload_digest)
            .ok_or_else(|| anyhow!("invalid layer cache payload digest path"))?;
        let manifest_path = self.manifest_path(key)?;
        fs::create_dir_all(self.blobs_dir())?;
        fs::create_dir_all(self.manifests_dir())?;
        self.ensure_managed_dir(&self.blobs_dir())?;
        self.ensure_managed_dir(&self.manifests_dir())?;

        let payload = serde_json::to_vec(value)?;
        let payload_tmp = payload_path.with_extension("json.tmp");
        fs::write(&payload_tmp, payload)
            .with_context(|| format!("failed to write layer payload {}", payload_tmp.display()))?;
        fs::rename(&payload_tmp, &payload_path).with_context(|| {
            format!("failed to rename layer payload {}", payload_path.display())
        })?;

        self.write_manifest_last(&manifest_path, manifest)?;
        Ok(LayerCacheWriteStatus::Written)
    }

    fn write_manifest_last(
        &self,
        manifest_path: &Path,
        manifest: &LayerCacheManifest,
    ) -> Result<()> {
        let mut normalized = manifest.clone();
        normalized.dependencies.sort();
        normalized.dependencies.dedup();
        normalized.warnings.sort();
        normalized.warnings.dedup();

        let manifest_tmp = manifest_path.with_extension("json.tmp");
        fs::write(&manifest_tmp, serde_json::to_vec(&normalized)?).with_context(|| {
            format!("failed to write layer manifest {}", manifest_tmp.display())
        })?;
        fs::rename(&manifest_tmp, manifest_path).with_context(|| {
            format!(
                "failed to rename layer manifest {}",
                manifest_path.display()
            )
        })?;
        Ok(())
    }

    fn blobs_dir(&self) -> PathBuf {
        self.root.join("blobs")
    }

    fn manifests_dir(&self) -> PathBuf {
        self.root.join("manifests")
    }

    fn blob_path(&self, digest: &Digest) -> Option<PathBuf> {
        if is_safe_digest_file_component(&digest.value) {
            Some(self.blobs_dir().join(format!("{}.json", digest.value)))
        } else {
            None
        }
    }

    fn manifest_path(&self, key: &LayerKey) -> Result<PathBuf> {
        let key_json = serde_json::to_string(key)?;
        Ok(self
            .manifests_dir()
            .join(format!("{}.json", stable_hash(&[&key_json]))))
    }

    fn manifest_metadata_is_supported(
        &self,
        manifest: &LayerCacheManifest,
        key: &LayerKey,
    ) -> bool {
        manifest.manifest_schema == LAYER_CACHE_MANIFEST_SCHEMA
            && manifest.dependency_index_schema == DEPENDENCY_INDEX_SCHEMA
            && manifest.key == *key
            && manifest.output_digest.kind == DigestKind::ProviderOutput
            && is_safe_digest_file_component(&manifest.output_digest.value)
            && manifest.payload_digest.kind == DigestKind::LayerOutput
            && is_safe_digest_file_component(&manifest.payload_digest.value)
            && is_supported_validation_label(&manifest.validation)
            && dependencies_match_layer_key(manifest)
    }

    fn managed_existing_file(&self, path: &Path, managed_dir: &Path) -> Option<PathBuf> {
        let metadata = fs::symlink_metadata(path).ok()?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return None;
        }
        let canonical_root = self.root.canonicalize().ok()?;
        let canonical_managed_dir = managed_dir.canonicalize().ok()?;
        let canonical_path = path.canonicalize().ok()?;
        if canonical_managed_dir.starts_with(&canonical_root)
            && canonical_path.starts_with(&canonical_managed_dir)
        {
            Some(canonical_path)
        } else {
            None
        }
    }

    fn ensure_managed_dir(&self, dir: &Path) -> Result<()> {
        let metadata = fs::symlink_metadata(dir)
            .with_context(|| format!("failed to inspect layer cache dir {}", dir.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(anyhow!("layer cache dir escapes managed cache root"));
        }
        let canonical_root = self.root.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize layer cache root {}",
                self.root.display()
            )
        })?;
        let canonical_dir = dir
            .canonicalize()
            .with_context(|| format!("failed to canonicalize layer cache dir {}", dir.display()))?;
        if !canonical_dir.starts_with(canonical_root) {
            return Err(anyhow!("layer cache dir escapes managed cache root"));
        }
        Ok(())
    }

    #[cfg(test)]
    fn blobs_dir_for_test(&self) -> PathBuf {
        self.blobs_dir()
    }

    #[cfg(test)]
    fn manifest_path_for_test(&self, key: &LayerKey) -> PathBuf {
        self.manifest_path(key).expect("manifest path")
    }

    #[cfg(test)]
    fn write_json_for_key_without_repair_for_test<T>(
        &self,
        key: &LayerKey,
        manifest: &LayerCacheManifest,
        value: &T,
    ) -> Result<LayerCacheWriteStatus>
    where
        T: Serialize,
    {
        self.write_json_inner(key, manifest, value)
    }

    #[cfg(test)]
    fn write_json_without_repair_for_test<T>(
        &self,
        manifest: &LayerCacheManifest,
        value: &T,
    ) -> Result<LayerCacheWriteStatus>
    where
        T: Serialize,
    {
        self.write_json_inner(&manifest.key, manifest, value)
    }
}

impl<T> LayerCacheReadOutcome<T> {
    fn without_value(status: LayerCacheReadStatus) -> Self {
        Self {
            status,
            manifest: None,
            output_digest: None,
            payload_digest: None,
            value: None,
        }
    }
}

fn payload_digest_for_bytes(payload: &[u8]) -> Result<Digest> {
    let payload = std::str::from_utf8(payload).context("layer cache payload was not UTF-8")?;
    Ok(Digest::from_parts(
        DigestKind::LayerOutput,
        "layer_cache_payload",
        &[payload],
    ))
}

fn is_safe_digest_file_component(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_supported_validation_label(value: &str) -> bool {
    value == "native_trusted"
}

fn dependencies_match_layer_key(manifest: &LayerCacheManifest) -> bool {
    let requires_dependencies = matches!(
        manifest.key.layer_kind,
        LayerKind::ModuleGraph | LayerKind::SymbolGraph | LayerKind::Metrics
    );
    if requires_dependencies && manifest.dependencies.is_empty() {
        return false;
    }
    manifest.dependencies.iter().all(|edge| {
        matches!(
            &edge.from,
            CacheNode::Layer(layer_key) if layer_key == &manifest.key
        )
    })
}

fn evict_file(path: &Path) {
    let _ = fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{
        CacheNode, DependencyEdge, DependencyKind, Digest, DigestKind, LayerKey, PrecisionTier,
        ShapeKind,
    };

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Payload {
        items: Vec<String>,
    }

    fn digest(kind: DigestKind, label: &str) -> Digest {
        Digest::from_parts(kind, label, &[label])
    }

    fn key() -> LayerKey {
        LayerKey::new(
            crate::analysis_kernel::incremental::keys::LayerKind::TsSyntax,
            "polint.ts.syntax",
            "1",
            "ts-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::TsJsLifecycle, "none"),
            Digest::absent(DigestKind::Config, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![digest(DigestKind::SourceText, "src/main.ts")],
            Vec::new(),
            Vec::new(),
        )
    }

    fn derived_key() -> LayerKey {
        LayerKey::new(
            crate::analysis_kernel::incremental::keys::LayerKind::ModuleGraph,
            "polint.module_graph",
            "1",
            "module-graph-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::Config, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![digest(DigestKind::SourceText, "src/main.ts")],
            vec![digest(DigestKind::DependencyLayer, "polint.ts.syntax")],
            Vec::new(),
        )
    }

    fn dependency(layer_key: &LayerKey) -> DependencyEdge {
        DependencyEdge {
            from: CacheNode::Layer(layer_key.clone()),
            to: CacheNode::Input("src/main.ts".to_string()),
            kind: DependencyKind::Input,
            required_shape: ShapeKind::Content,
        }
    }

    fn scratch_dir() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("scratch directory")
    }

    fn manifest_for_payload(layer_key: LayerKey, payload: &Payload) -> LayerCacheManifest {
        LayerCacheManifest::new(
            layer_key.clone(),
            digest(DigestKind::ProviderOutput, "output"),
            LayerCacheStore::payload_digest_for_json(payload).expect("payload digest"),
            vec![dependency(&layer_key)],
            PrecisionTier::Syntax,
            "native_trusted",
            Vec::new(),
        )
    }

    fn write_manifest_only(
        store: &LayerCacheStore,
        layer_key: &LayerKey,
        manifest: &LayerCacheManifest,
    ) {
        std::fs::create_dir_all(store.manifests_dir()).unwrap();
        std::fs::write(
            store.manifest_path_for_test(layer_key),
            serde_json::to_vec(manifest).expect("manifest serializes"),
        )
        .unwrap();
    }

    #[test]
    fn write_json_publishes_payload_before_manifest_and_read_returns_output_digest() {
        let scratch = scratch_dir();
        let store = LayerCacheStore::new(scratch.path().join("layers"), true);
        let payload = Payload {
            items: vec!["a".to_string(), "b".to_string()],
        };
        let layer_key = key();
        let manifest = manifest_for_payload(layer_key.clone(), &payload);

        let status = store.write_json(&manifest, &payload).unwrap();
        let outcome: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(status, LayerCacheWriteStatus::Written);
        assert_eq!(outcome.status, LayerCacheReadStatus::Hit);
        assert_eq!(outcome.value, Some(payload));
        assert_eq!(outcome.output_digest, Some(manifest.output_digest));
        assert!(
            store
                .blobs_dir_for_test()
                .join(format!("{}.json", manifest.payload_digest.value))
                .exists()
        );
        assert!(store.manifest_path_for_test(&layer_key).exists());
    }

    #[test]
    fn corrupt_payload_manifest_and_mismatches_return_controlled_invalid_reads() {
        let scratch = scratch_dir();
        let store = LayerCacheStore::new(scratch.path().join("layers"), true);
        let payload = Payload {
            items: vec!["a".to_string()],
        };
        let layer_key = key();
        let manifest = manifest_for_payload(layer_key.clone(), &payload);
        store.write_json(&manifest, &payload).unwrap();
        std::fs::write(
            store
                .blobs_dir_for_test()
                .join(format!("{}.json", manifest.payload_digest.value)),
            "{broken",
        )
        .unwrap();

        let corrupt_payload: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(corrupt_payload.status, LayerCacheReadStatus::InvalidEvicted);

        let manifest = manifest_for_payload(layer_key.clone(), &payload);
        store.write_json(&manifest, &payload).unwrap();
        std::fs::write(store.manifest_path_for_test(&layer_key), "{broken").unwrap();

        let corrupt_manifest: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(
            corrupt_manifest.status,
            LayerCacheReadStatus::InvalidEvicted
        );
    }

    #[test]
    fn mismatched_manifest_key_schema_payload_digest_and_validator_do_not_hit() {
        let scratch = scratch_dir();
        let store = LayerCacheStore::new(scratch.path().join("layers"), true);
        let payload = Payload {
            items: vec!["a".to_string()],
        };
        let layer_key = key();
        let mut manifest = manifest_for_payload(layer_key.clone(), &payload);
        manifest.manifest_schema = "old-schema".to_string();
        store
            .write_json_without_repair_for_test(&manifest, &payload)
            .unwrap();

        let schema_mismatch: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(schema_mismatch.status, LayerCacheReadStatus::InvalidEvicted);

        let mut manifest = manifest_for_payload(layer_key.clone(), &payload);
        manifest.key.provider_id = "other.provider".to_string();
        store
            .write_json_for_key_without_repair_for_test(&layer_key, &manifest, &payload)
            .unwrap();

        let key_mismatch: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(key_mismatch.status, LayerCacheReadStatus::InvalidEvicted);

        let mut manifest = manifest_for_payload(layer_key.clone(), &payload);
        manifest.payload_digest = digest(DigestKind::LayerOutput, "wrong-payload");
        store
            .write_json_without_repair_for_test(&manifest, &payload)
            .unwrap();

        let payload_mismatch: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(
            payload_mismatch.status,
            LayerCacheReadStatus::InvalidEvicted
        );

        let manifest = manifest_for_payload(layer_key.clone(), &payload);
        store.write_json(&manifest, &payload).unwrap();
        let validator_mismatch: LayerCacheReadOutcome<Payload> =
            store.read_json_validated(&layer_key, |_, _| false);

        assert_eq!(
            validator_mismatch.status,
            LayerCacheReadStatus::InvalidEvicted
        );
    }

    #[test]
    fn manifest_json_contains_schema_and_no_forbidden_identity_fields() {
        let payload = Payload {
            items: vec!["a".to_string()],
        };
        let manifest = manifest_for_payload(key(), &payload);
        let json = serde_json::to_value(manifest).expect("manifest should serialize");

        assert_eq!(json["manifest_schema"], "polint-layer-cache-manifest-1");
        for forbidden in [
            concat!("raw_", "source"),
            concat!("source", "_text"),
            concat!("created", "_at"),
            concat!("m", "time"),
            concat!("run", "_id"),
            concat!("temp", "dir"),
            concat!("abs", "olute"),
        ] {
            assert!(
                json.get(forbidden).is_none(),
                "unexpected field {forbidden}"
            );
        }
    }

    #[test]
    fn disabled_store_bypasses_reads_and_writes_without_filesystem_access() {
        let scratch = scratch_dir();
        let root = scratch.path().join("layers");
        let store = LayerCacheStore::new(&root, false);
        let payload = Payload {
            items: vec!["a".to_string()],
        };
        let layer_key = key();
        let manifest = manifest_for_payload(layer_key.clone(), &payload);

        let write = store.write_json(&manifest, &payload).unwrap();
        let read: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(write, LayerCacheWriteStatus::BypassedDisabled);
        assert_eq!(read.status, LayerCacheReadStatus::BypassedDisabled);
        assert!(!root.exists());
    }

    #[test]
    fn rejects_path_traversal_payload_digest() {
        let scratch = scratch_dir();
        let store = LayerCacheStore::new(scratch.path().join("layers"), true);
        let payload = Payload {
            items: vec!["a".to_string()],
        };
        let layer_key = key();
        let mut manifest = manifest_for_payload(layer_key.clone(), &payload);
        manifest.payload_digest.value = "../outside".to_string();
        write_manifest_only(&store, &layer_key, &manifest);

        let outcome: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(outcome.status, LayerCacheReadStatus::InvalidEvicted);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_payload() {
        let scratch = scratch_dir();
        let store = LayerCacheStore::new(scratch.path().join("layers"), true);
        let payload = Payload {
            items: vec!["a".to_string()],
        };
        let layer_key = key();
        let manifest = manifest_for_payload(layer_key.clone(), &payload);
        store.write_json(&manifest, &payload).unwrap();
        let blob_path = store
            .blobs_dir_for_test()
            .join(format!("{}.json", manifest.payload_digest.value));
        let outside_payload = scratch.path().join("outside-payload.json");
        std::fs::write(
            &outside_payload,
            serde_json::to_vec(&payload).expect("payload serializes"),
        )
        .unwrap();
        std::fs::remove_file(&blob_path).unwrap();
        std::os::unix::fs::symlink(&outside_payload, &blob_path).unwrap();

        let outcome: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(outcome.status, LayerCacheReadStatus::InvalidEvicted);
    }

    #[test]
    fn schema_mismatch_output_digest_mismatch_dependency_index_schema_mismatch_and_missing_dependency_return_invalid()
     {
        let scratch = scratch_dir();
        let store = LayerCacheStore::new(scratch.path().join("layers"), true);
        let payload = Payload {
            items: vec!["a".to_string()],
        };
        let layer_key = derived_key();
        let mut manifest = manifest_for_payload(layer_key.clone(), &payload);
        manifest.dependency_index_schema = "old-dependency-index".to_string();
        store.write_json(&manifest, &payload).unwrap();

        let dependency_index_schema_mismatch: LayerCacheReadOutcome<Payload> =
            store.read_json(&layer_key);

        assert_eq!(
            dependency_index_schema_mismatch.status,
            LayerCacheReadStatus::InvalidEvicted
        );

        let mut manifest = manifest_for_payload(layer_key.clone(), &payload);
        manifest.output_digest = digest(DigestKind::ProviderOutput, "wrong-output");
        store
            .write_json_without_repair_for_test(&manifest, &payload)
            .unwrap();

        let output_digest_mismatch: LayerCacheReadOutcome<Payload> = store
            .read_json_validated(&layer_key, |_, manifest| {
                manifest.output_digest == digest(DigestKind::ProviderOutput, "output")
            });

        assert_eq!(
            output_digest_mismatch.status,
            LayerCacheReadStatus::InvalidEvicted
        );

        let mut manifest = manifest_for_payload(layer_key.clone(), &payload);
        manifest.dependencies.clear();
        store.write_json(&manifest, &payload).unwrap();

        let missing_dependency: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(
            missing_dependency.status,
            LayerCacheReadStatus::InvalidEvicted
        );
    }

    #[test]
    fn deserialization_wrong_shape_and_corrupt_payload_return_invalid() {
        let scratch = scratch_dir();
        let store = LayerCacheStore::new(scratch.path().join("layers"), true);
        let payload = Payload {
            items: vec!["a".to_string()],
        };
        let layer_key = key();
        let manifest = manifest_for_payload(layer_key.clone(), &payload);
        store.write_json(&manifest, &payload).unwrap();
        std::fs::write(
            store
                .blobs_dir_for_test()
                .join(format!("{}.json", manifest.payload_digest.value)),
            r#"{"items":"not-a-list"}"#,
        )
        .unwrap();

        let wrong_shape: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(wrong_shape.status, LayerCacheReadStatus::InvalidEvicted);

        let manifest = manifest_for_payload(layer_key.clone(), &payload);
        store.write_json(&manifest, &payload).unwrap();
        std::fs::write(
            store
                .blobs_dir_for_test()
                .join(format!("{}.json", manifest.payload_digest.value)),
            b"\xff\xfe\xfd",
        )
        .unwrap();

        let corrupt_payload: LayerCacheReadOutcome<Payload> = store.read_json(&layer_key);

        assert_eq!(corrupt_payload.status, LayerCacheReadStatus::InvalidEvicted);
    }
}
