#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderManifest {
    pub(crate) id: &'static str,
    pub(crate) kind: ProviderKind,
    pub(crate) inputs: &'static [&'static str],
    pub(crate) outputs: &'static [&'static str],
    pub(crate) language_scope: LanguageScope,
    pub(crate) cache_policy: CachePolicy,
    pub(crate) schema_versions: &'static [SchemaVersion],
    pub(crate) precision_ceiling: PrecisionCeiling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderKind {
    SourceDiscovery,
    LanguageSyntax,
    WholeRepoDerived,
    MetricsDerived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LanguageScope {
    Workspace,
    Go,
    TypeScriptJavaScript,
    MultiLanguage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CachePolicy {
    NoCache,
    ExistingFileFactCache { schema: &'static str },
    InMemoryDerived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SchemaVersion {
    pub(crate) family: &'static str,
    pub(crate) version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrecisionCeiling {
    Exact,
    Syntax,
    SetupAware,
    Heuristic,
}

pub(crate) fn provider_manifests() -> &'static [ProviderManifest] {
    todo!("provider manifest rows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_manifests_have_required_metadata() {
        for manifest in provider_manifests() {
            assert!(!manifest.id.is_empty());
            assert!(
                !manifest.inputs.is_empty()
                    || matches!(manifest.kind, ProviderKind::SourceDiscovery)
            );
            assert!(!manifest.outputs.is_empty());
            assert!(!manifest.schema_versions.is_empty());
            let _language_scope = manifest.language_scope;
            let _cache_policy = manifest.cache_policy;
            let _precision_ceiling = manifest.precision_ceiling;
        }
    }
}
