#![allow(
    dead_code,
    reason = "Extension layer cache keys are consumed by cache persistence and eval fixtures after the initial constructor lands."
)]

use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{Digest, DigestKind, LayerKey, LayerKind};

pub(crate) const EXTENSION_FACTS_SCHEMA_LABEL: &str = "extension-facts-1";
pub(crate) const EXTENSION_PROTOCOL_IDENTITY: &str = "polint-extension-provider-run-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtensionLayerKeyInput {
    pub(crate) extension_id: String,
    pub(crate) provider_id: String,
    pub(crate) source_digest: Digest,
    pub(crate) dependency_digest: Digest,
    pub(crate) manifest_digest: Digest,
    pub(crate) options_digest: Digest,
    pub(crate) declared_read_digest: Digest,
    pub(crate) input_fact_digests: Vec<Digest>,
    pub(crate) dependency_layer_digests: Vec<Digest>,
}

pub(crate) fn extension_layer_key(
    manifest: &ProviderManifest,
    input: ExtensionLayerKeyInput,
) -> LayerKey {
    let provider_identity = format!(
        "polint.extension.{}.{}",
        input.extension_id, input.provider_id
    );
    let parameter_digest = Digest::from_parts(
        DigestKind::ProviderParameters,
        "extension_provider_parameters",
        &[
            &provider_identity,
            EXTENSION_PROTOCOL_IDENTITY,
            EXTENSION_FACTS_SCHEMA_LABEL,
        ],
    );
    let toolchain_digest = Digest::absent(DigestKind::ToolInvocation, "extension_toolchain");
    let config_digest = input.options_digest;
    let mut extension_digests = vec![
        input.source_digest,
        input.dependency_digest,
        input.manifest_digest,
        Digest::from_parts(
            DigestKind::ExtensionCode,
            "extension_protocol",
            &[EXTENSION_PROTOCOL_IDENTITY],
        ),
    ];
    extension_digests.sort();

    let mut input_digests = vec![input.declared_read_digest];
    input_digests.extend(input.input_fact_digests);
    input_digests.sort();

    LayerKey::new(
        LayerKind::Extension,
        provider_identity,
        manifest.provider_version(),
        EXTENSION_FACTS_SCHEMA_LABEL,
        parameter_digest,
        Digest::absent(DigestKind::ProviderParameters, "extension_lifecycle"),
        config_digest,
        toolchain_digest,
        input_digests,
        input.dependency_layer_digests,
        extension_digests,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;

    fn manifest() -> &'static ProviderManifest {
        AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.extensions")
            .expect("extension manifest exists")
    }

    fn digest(kind: DigestKind, label: &str, value: &str) -> Digest {
        Digest::from_parts(kind, label, &[value])
    }

    fn input() -> ExtensionLayerKeyInput {
        ExtensionLayerKeyInput {
            extension_id: "demo".to_string(),
            provider_id: "routes".to_string(),
            source_digest: digest(DigestKind::ExtensionCode, "source", "source-v1"),
            dependency_digest: digest(DigestKind::ExtensionCode, "deps", "deps-v1"),
            manifest_digest: digest(DigestKind::ExtensionCode, "manifest", "manifest-v1"),
            options_digest: digest(DigestKind::RuleOptions, "options", "options-v1"),
            declared_read_digest: digest(DigestKind::ProviderParameters, "reads", "reads-v1"),
            input_fact_digests: vec![digest(DigestKind::ProviderOutput, "facts", "facts-v1")],
            dependency_layer_digests: vec![digest(
                DigestKind::DependencyLayer,
                "dependency",
                "dependency-v1",
            )],
        }
    }

    #[test]
    fn extension_layer_key_changes_for_source_manifest_protocol_reads_and_options() {
        let base = extension_layer_key(manifest(), input());
        let mut source_changed = input();
        source_changed.source_digest = digest(DigestKind::ExtensionCode, "source", "source-v2");
        let mut manifest_changed = input();
        manifest_changed.manifest_digest =
            digest(DigestKind::ExtensionCode, "manifest", "manifest-v2");
        let mut reads_changed = input();
        reads_changed.declared_read_digest =
            digest(DigestKind::ProviderParameters, "reads", "reads-v2");
        let mut options_changed = input();
        options_changed.options_digest = digest(DigestKind::RuleOptions, "options", "options-v2");

        assert_ne!(base, extension_layer_key(manifest(), source_changed));
        assert_ne!(base, extension_layer_key(manifest(), manifest_changed));
        assert_ne!(base, extension_layer_key(manifest(), reads_changed));
        assert_ne!(base, extension_layer_key(manifest(), options_changed));
        assert!(base.extension_digests.iter().any(|digest| {
            digest.kind == DigestKind::ExtensionCode
                && *digest != Digest::absent(DigestKind::ExtensionCode, "extension_digest_absent")
        }));
    }

    #[test]
    fn identical_extension_layer_inputs_produce_identical_keys() {
        assert_eq!(
            extension_layer_key(manifest(), input()),
            extension_layer_key(manifest(), input())
        );
    }
}
