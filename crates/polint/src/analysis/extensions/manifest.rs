use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExtensionManifest {
    pub(crate) extension_id: String,
    pub(crate) trust: ExtensionTrust,
    pub(crate) budget: ExtensionBudget,
    pub(crate) providers: Vec<ExtensionProviderManifest>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct ExtensionProviderManifest {
    pub(crate) extension_id: String,
    pub(crate) provider_id: String,
    pub(crate) declared_reads: Vec<FactFamilyLabel>,
    pub(crate) declared_outputs: Vec<FactFamilyLabel>,
    pub(crate) activation_status: ExtensionActivationStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExtensionActivationStatus {
    Configured,
    Discovered,
    HandshakeOk,
    Active,
    Failed,
    ValidationFailed,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExtensionTrust {
    RepoLocal,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExtensionBudget {
    pub(crate) timeout_millis: u64,
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct FactFamilyLabel(String);

impl ExtensionManifest {
    pub(crate) fn repo_local(extension_id: impl Into<String>) -> Result<Self, ManifestError> {
        let extension_id = normalize_id(extension_id.into())?;
        Ok(Self {
            extension_id,
            trust: ExtensionTrust::RepoLocal,
            budget: ExtensionBudget::default(),
            providers: Vec::new(),
        })
    }

    pub(crate) fn with_providers(
        mut self,
        providers: Vec<ExtensionProviderManifest>,
    ) -> Result<Self, ManifestError> {
        let mut provider_ids = BTreeSet::new();
        for provider in &providers {
            if provider.extension_id != self.extension_id {
                return Err(ManifestError::MismatchedExtensionId {
                    expected: self.extension_id,
                    actual: provider.extension_id.clone(),
                });
            }
            if !provider_ids.insert(provider.provider_id.clone()) {
                return Err(ManifestError::DuplicateProvider(
                    provider.provider_id.clone(),
                ));
            }
        }
        self.providers = sorted_providers(providers);
        Ok(self)
    }
}

impl ExtensionProviderManifest {
    pub(crate) fn new(
        extension_id: impl Into<String>,
        provider_id: impl Into<String>,
        declared_reads: Vec<impl Into<String>>,
        declared_outputs: Vec<impl Into<String>>,
        activation_status: ExtensionActivationStatus,
    ) -> Result<Self, ManifestError> {
        Ok(Self {
            extension_id: normalize_id(extension_id.into())?,
            provider_id: normalize_id(provider_id.into())?,
            declared_reads: normalize_family_labels(declared_reads)?,
            declared_outputs: normalize_family_labels(declared_outputs)?,
            activation_status,
        })
    }

    pub(crate) fn stable_provider_key(&self) -> String {
        format!("{}.{}", self.extension_id, self.provider_id)
    }
}

impl FactFamilyLabel {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ManifestError> {
        Ok(Self(normalize_id(value.into())?))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ExtensionBudget {
    fn default() -> Self {
        Self {
            timeout_millis: 5_000,
            stdout_bytes: 1_048_576,
            stderr_bytes: 16_384,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ManifestError {
    Empty,
    Invalid(String),
    DuplicateProvider(String),
    MismatchedExtensionId { expected: String, actual: String },
}

fn sorted_providers(
    mut providers: Vec<ExtensionProviderManifest>,
) -> Vec<ExtensionProviderManifest> {
    providers.sort_by(|left, right| {
        left.extension_id
            .cmp(&right.extension_id)
            .then_with(|| left.provider_id.cmp(&right.provider_id))
    });
    providers
}

fn normalize_family_labels(
    labels: Vec<impl Into<String>>,
) -> Result<Vec<FactFamilyLabel>, ManifestError> {
    let mut labels = labels
        .into_iter()
        .map(FactFamilyLabel::new)
        .collect::<Result<Vec<_>, _>>()?;
    labels.sort();
    labels.dedup();
    Ok(labels)
}

fn normalize_id(value: String) -> Result<String, ManifestError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(ManifestError::Empty);
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return Err(ManifestError::Invalid(value));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_declarations_sort_by_extension_then_provider() {
        let providers = vec![
            ExtensionProviderManifest::new(
                "demo",
                "z",
                vec!["source_files"],
                vec!["extension.fact"],
                ExtensionActivationStatus::Configured,
            )
            .unwrap(),
            ExtensionProviderManifest::new(
                "demo",
                "a",
                vec!["source_files"],
                vec!["extension.fact"],
                ExtensionActivationStatus::Configured,
            )
            .unwrap(),
        ];

        let manifest = ExtensionManifest::repo_local("demo")
            .unwrap()
            .with_providers(providers)
            .unwrap();

        assert_eq!(
            manifest
                .providers
                .iter()
                .map(ExtensionProviderManifest::stable_provider_key)
                .collect::<Vec<_>>(),
            vec!["demo.a", "demo.z"]
        );
    }

    #[test]
    fn empty_ids_are_rejected() {
        assert_eq!(
            ExtensionProviderManifest::new(
                "",
                "routes",
                Vec::<String>::new(),
                Vec::<String>::new(),
                ExtensionActivationStatus::Configured,
            )
            .unwrap_err(),
            ManifestError::Empty
        );
        assert_eq!(FactFamilyLabel::new(" ").unwrap_err(), ManifestError::Empty);
    }

    #[test]
    fn declared_read_and_output_families_are_preserved_deterministically() {
        let provider = ExtensionProviderManifest::new(
            "demo",
            "routes",
            vec!["symbols", "source_files", "symbols"],
            vec!["extension.routes", "extension.calls"],
            ExtensionActivationStatus::Discovered,
        )
        .unwrap();

        assert_eq!(
            provider
                .declared_reads
                .iter()
                .map(FactFamilyLabel::as_str)
                .collect::<Vec<_>>(),
            vec!["source_files", "symbols"]
        );
        assert_eq!(
            provider
                .declared_outputs
                .iter()
                .map(FactFamilyLabel::as_str)
                .collect::<Vec<_>>(),
            vec!["extension.calls", "extension.routes"]
        );
    }
}
