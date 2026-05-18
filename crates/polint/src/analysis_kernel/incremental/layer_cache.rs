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

    fn dependency(layer_key: &LayerKey) -> DependencyEdge {
        DependencyEdge {
            from: CacheNode::Layer(layer_key.clone()),
            to: CacheNode::Input("src/main.ts".to_string()),
            kind: DependencyKind::Input,
            required_shape: ShapeKind::Content,
        }
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

    #[test]
    fn write_json_publishes_payload_before_manifest_and_read_returns_output_digest() {
        let temp = tempfile::tempdir().unwrap();
        let store = LayerCacheStore::new(temp.path().join("layers"), true);
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
        let temp = tempfile::tempdir().unwrap();
        let store = LayerCacheStore::new(temp.path().join("layers"), true);
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
        let temp = tempfile::tempdir().unwrap();
        let store = LayerCacheStore::new(temp.path().join("layers"), true);
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
            .write_json_without_repair_for_test(&manifest, &payload)
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
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("layers");
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
}
