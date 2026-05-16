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
    pub(crate) name: &'static str,
    pub(crate) version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrecisionCeiling {
    Exact,
    Syntax,
    SetupAware,
}

pub(crate) fn provider_manifests() -> &'static [ProviderManifest] {
    PROVIDER_MANIFESTS
}

const SOURCE_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "source-files-1",
    version: 1,
}];

const GO_SYNTAX_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "go-facts-v2",
    version: 2,
}];

const TS_SYNTAX_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "ts-facts-v1",
    version: 1,
}];

const MODULE_GRAPH_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "module-graph-facts-1",
    version: 1,
}];

const SYMBOL_GRAPH_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "symbol-graph-facts-1",
    version: 1,
}];

const METRICS_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "metrics-facts-1",
    version: 1,
}];

const PROVIDER_MANIFESTS: &[ProviderManifest] = &[
    ProviderManifest {
        id: "polint.source",
        kind: ProviderKind::SourceDiscovery,
        inputs: &["workspace_config", "file_discovery"],
        outputs: &["source_files"],
        language_scope: LanguageScope::Workspace,
        cache_policy: CachePolicy::NoCache,
        schema_versions: SOURCE_SCHEMA,
        precision_ceiling: PrecisionCeiling::Exact,
    },
    ProviderManifest {
        id: "polint.go.syntax",
        kind: ProviderKind::LanguageSyntax,
        inputs: &["source_files"],
        outputs: &[
            "packages",
            "functions",
            "imports",
            "go_tests",
            "branch_obligations",
        ],
        language_scope: LanguageScope::Go,
        cache_policy: CachePolicy::ExistingFileFactCache {
            schema: "go-facts-v2",
        },
        schema_versions: GO_SYNTAX_SCHEMA,
        precision_ceiling: PrecisionCeiling::Syntax,
    },
    ProviderManifest {
        id: "polint.ts.syntax",
        kind: ProviderKind::LanguageSyntax,
        inputs: &["source_files"],
        outputs: &[
            "functions",
            "imports",
            "ts_components",
            "ts_classes",
            "string_literals",
            "jsx_attributes",
        ],
        language_scope: LanguageScope::TypeScriptJavaScript,
        cache_policy: CachePolicy::ExistingFileFactCache {
            schema: "ts-facts-v1",
        },
        schema_versions: TS_SYNTAX_SCHEMA,
        precision_ceiling: PrecisionCeiling::Syntax,
    },
    ProviderManifest {
        id: "polint.module_graph",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &["source_files", "packages", "imports"],
        outputs: &["resolved_imports", "module_nodes", "module_edges"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: MODULE_GRAPH_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.symbol_graph",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "packages",
            "imports",
            "resolved_imports",
            "module_nodes",
            "module_edges",
            "functions",
        ],
        outputs: &["symbols", "definitions", "references"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: SYMBOL_GRAPH_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.metrics",
        kind: ProviderKind::MetricsDerived,
        inputs: &["source_files", "functions"],
        outputs: &["file_metrics", "function_metrics", "complexity_metrics"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: METRICS_SCHEMA,
        precision_ceiling: PrecisionCeiling::Syntax,
    },
];

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
