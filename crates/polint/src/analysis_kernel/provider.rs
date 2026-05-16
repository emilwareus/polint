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

#[cfg(test)]
pub(crate) fn provider_order_for_test() -> Vec<&'static str> {
    Vec::new()
}

#[cfg(test)]
pub(crate) fn provider_order_report_for_test() -> Vec<ProviderOrderRow> {
    Vec::new()
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderOrderRow {
    pub(crate) id: &'static str,
    pub(crate) kind: &'static str,
    pub(crate) language_scope: &'static str,
    pub(crate) inputs: Vec<&'static str>,
    pub(crate) outputs: Vec<&'static str>,
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

    #[test]
    fn provider_order_matches_behavior_preserving_kernel_sequence() {
        assert_eq!(
            provider_order_for_test(),
            vec![
                "polint.source",
                "polint.go.syntax",
                "polint.ts.syntax",
                "polint.module_graph",
                "polint.symbol_graph",
                "polint.metrics",
            ]
        );
    }

    #[test]
    fn provider_manifest_dependencies_are_deterministic_metadata() {
        let report = provider_order_report_for_test();

        assert_eq!(
            report,
            vec![
                ProviderOrderRow {
                    id: "polint.source",
                    kind: "source_discovery",
                    language_scope: "workspace",
                    inputs: vec!["workspace_config", "file_discovery"],
                    outputs: vec!["source_files"],
                },
                ProviderOrderRow {
                    id: "polint.go.syntax",
                    kind: "language_syntax",
                    language_scope: "go",
                    inputs: vec!["source_files"],
                    outputs: vec![
                        "packages",
                        "functions",
                        "imports",
                        "go_tests",
                        "branch_obligations",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.ts.syntax",
                    kind: "language_syntax",
                    language_scope: "typescript_javascript",
                    inputs: vec!["source_files"],
                    outputs: vec![
                        "functions",
                        "imports",
                        "ts_components",
                        "ts_classes",
                        "string_literals",
                        "jsx_attributes",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.module_graph",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec!["source_files", "packages", "imports"],
                    outputs: vec!["resolved_imports", "module_nodes", "module_edges"],
                },
                ProviderOrderRow {
                    id: "polint.symbol_graph",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "packages",
                        "imports",
                        "resolved_imports",
                        "module_nodes",
                        "module_edges",
                        "functions",
                    ],
                    outputs: vec!["symbols", "definitions", "references"],
                },
                ProviderOrderRow {
                    id: "polint.metrics",
                    kind: "metrics_derived",
                    language_scope: "multi_language",
                    inputs: vec!["source_files", "functions"],
                    outputs: vec!["file_metrics", "function_metrics", "complexity_metrics"],
                },
            ]
        );
    }

    #[test]
    fn provider_order_report_for_test_is_path_stable() {
        let report = provider_order_report_for_test();
        let rendered = format!("{report:?}");

        assert!(!rendered.contains(env!("CARGO_MANIFEST_DIR")));
        assert!(!rendered.contains('/'));
        assert!(!rendered.contains('\\'));
        assert!(!rendered.contains("202"));
    }
}
