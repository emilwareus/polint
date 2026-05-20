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

impl ProviderManifest {
    pub(crate) fn provider_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    pub(crate) fn primary_schema_label(&self) -> String {
        let mut labels = self
            .schema_versions
            .iter()
            .map(|schema| format!("{}:{}", schema.name, schema.version))
            .collect::<Vec<_>>();
        labels.sort();
        labels.join(",")
    }

    pub(crate) fn language_scope_label(&self) -> &'static str {
        match self.language_scope {
            LanguageScope::Workspace => "workspace",
            LanguageScope::Go => "go",
            LanguageScope::TypeScriptJavaScript => "typescript_javascript",
            LanguageScope::MultiLanguage => "multi_language",
        }
    }

    pub(crate) fn cache_policy_label(&self) -> String {
        match self.cache_policy {
            CachePolicy::NoCache => "no_cache".to_string(),
            CachePolicy::ExistingFileFactCache { schema } => {
                format!("existing_file_fact_cache:{schema}")
            }
            CachePolicy::InMemoryDerived => "in_memory_derived".to_string(),
        }
    }
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
    provider_manifests()
        .iter()
        .map(|manifest| manifest.id)
        .collect()
}

#[cfg(test)]
pub(crate) fn provider_order_report_for_test() -> Vec<ProviderOrderRow> {
    provider_manifests()
        .iter()
        .map(|manifest| ProviderOrderRow {
            id: manifest.id,
            kind: provider_kind_label(manifest.kind),
            language_scope: language_scope_label(manifest.language_scope),
            inputs: manifest.inputs.to_vec(),
            outputs: manifest.outputs.to_vec(),
        })
        .collect()
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

#[cfg(test)]
fn provider_kind_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::SourceDiscovery => "source_discovery",
        ProviderKind::LanguageSyntax => "language_syntax",
        ProviderKind::WholeRepoDerived => "whole_repo_derived",
        ProviderKind::MetricsDerived => "metrics_derived",
    }
}

#[cfg(test)]
fn language_scope_label(scope: LanguageScope) -> &'static str {
    match scope {
        LanguageScope::Workspace => "workspace",
        LanguageScope::Go => "go",
        LanguageScope::TypeScriptJavaScript => "typescript_javascript",
        LanguageScope::MultiLanguage => "multi_language",
    }
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
    name: "module-graph-facts-2",
    version: 2,
}];

const SYMBOL_GRAPH_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "symbol-graph-facts-2",
    version: 2,
}];

const MODULE_TOPOLOGY_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "module-topology-facts-1",
    version: 1,
}];

const SEMANTIC_MIR_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "semantic-mir-facts-1",
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
        outputs: &[
            "resolved_imports",
            "module_nodes",
            "module_edges",
            "workspace_roots",
            "topology_packages",
            "source_sets",
            "dependency_requirements",
            "resolved_dependency_edges",
            "repo_topology_overlays",
        ],
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
        outputs: &[
            "symbols",
            "definitions",
            "references",
            "scopes",
            "semantic_imports",
            "exports",
            "aliases",
            "resolution_facts",
            "generated_symbols",
            "stable_exports",
        ],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: SYMBOL_GRAPH_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.module_topology",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "imports",
            "resolved_imports",
            "module_nodes",
            "module_edges",
            "workspace_roots",
            "topology_packages",
            "source_sets",
            "dependency_requirements",
            "resolved_dependency_edges",
            "semantic_imports",
        ],
        outputs: &["import_to_package_edges"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: MODULE_TOPOLOGY_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.semantic_mir",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "functions",
            "symbols",
            "references",
            "scopes",
            "semantic_imports",
            "import_to_package_edges",
        ],
        outputs: &[
            "mir_bodies",
            "mir_operations",
            "places",
            "unsupported_semantics",
        ],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: SEMANTIC_MIR_SCHEMA,
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
    use std::path::{Path, PathBuf};

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
                "polint.module_topology",
                "polint.semantic_mir",
                "polint.metrics",
            ]
        );
    }

    #[test]
    fn symbol_graph_manifest_declares_semantic_outputs_without_reordering_providers() {
        assert_eq!(
            provider_order_for_test(),
            vec![
                "polint.source",
                "polint.go.syntax",
                "polint.ts.syntax",
                "polint.module_graph",
                "polint.symbol_graph",
                "polint.module_topology",
                "polint.semantic_mir",
                "polint.metrics",
            ]
        );
        let manifest = provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.symbol_graph")
            .expect("symbol graph manifest should exist");

        assert_eq!(
            manifest.outputs,
            &[
                "symbols",
                "definitions",
                "references",
                "scopes",
                "semantic_imports",
                "exports",
                "aliases",
                "resolution_facts",
                "generated_symbols",
                "stable_exports",
            ]
        );
        assert_eq!(
            manifest.schema_versions,
            &[SchemaVersion {
                name: "symbol-graph-facts-2",
                version: 2,
            }]
        );
    }

    #[test]
    fn module_graph_manifest_declares_base_topology_outputs_without_reordering_providers() {
        assert_eq!(
            provider_order_for_test(),
            vec![
                "polint.source",
                "polint.go.syntax",
                "polint.ts.syntax",
                "polint.module_graph",
                "polint.symbol_graph",
                "polint.module_topology",
                "polint.metrics",
            ]
        );
        let manifest = provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.module_graph")
            .expect("module graph manifest should exist");

        assert_eq!(
            manifest.outputs,
            &[
                "resolved_imports",
                "module_nodes",
                "module_edges",
                "workspace_roots",
                "topology_packages",
                "source_sets",
                "dependency_requirements",
                "resolved_dependency_edges",
                "repo_topology_overlays",
            ]
        );
        assert!(!manifest.outputs.contains(&"import_to_package_edges"));
        assert_eq!(
            manifest.schema_versions,
            &[SchemaVersion {
                name: "module-graph-facts-2",
                version: 2,
            }]
        );
    }

    #[test]
    fn topology_outputs_are_not_sdk_prelude_exports() {
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let prelude = read_source(&crate_root.join("src/sdk/mod.rs"));

        for term in [
            ["Packages", "<'_"].concat(),
            ["Dependencies", "<'_"].concat(),
            ["SourceSets", "<'_"].concat(),
            ["RepoTopology", "<'_"].concat(),
        ] {
            assert!(
                !prelude.contains(&term),
                "unexpected topology SDK prelude export `{term}`"
            );
        }
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
                    outputs: vec![
                        "resolved_imports",
                        "module_nodes",
                        "module_edges",
                        "workspace_roots",
                        "topology_packages",
                        "source_sets",
                        "dependency_requirements",
                        "resolved_dependency_edges",
                        "repo_topology_overlays",
                    ],
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
                    outputs: vec![
                        "symbols",
                        "definitions",
                        "references",
                        "scopes",
                        "semantic_imports",
                        "exports",
                        "aliases",
                        "resolution_facts",
                        "generated_symbols",
                        "stable_exports",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.module_topology",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "imports",
                        "resolved_imports",
                        "module_nodes",
                        "module_edges",
                        "workspace_roots",
                        "topology_packages",
                        "source_sets",
                        "dependency_requirements",
                        "resolved_dependency_edges",
                        "semantic_imports",
                    ],
                    outputs: vec!["import_to_package_edges"],
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

    #[test]
    fn provider_manifests_are_not_public_sdk_runner_or_cli_contract() {
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let lib = read_source(&crate_root.join("src/lib.rs"));
        assert!(!lib.contains("pub mod analysis_kernel"));

        assert_no_manifest_contract_terms(&crate_root.join("src/runner/mod.rs"));
        assert_no_manifest_contract_terms(&crate_root.join("src/cli/mod.rs"));
        for source_path in rust_sources_under(&crate_root.join("src/sdk")) {
            assert_no_manifest_contract_terms(&source_path);
        }
    }

    fn assert_no_manifest_contract_terms(path: &Path) {
        let source = read_source(path);
        for term in ["ProviderManifest", "provider_order", "provider_manifests"] {
            assert!(
                !source.contains(term),
                "unexpected manifest contract term `{term}` in {}",
                path.display()
            );
        }
    }

    fn rust_sources_under(dir: &Path) -> Vec<PathBuf> {
        let mut sources = Vec::new();
        collect_rust_sources(dir, &mut sources);
        sources.sort();
        sources
    }

    fn collect_rust_sources(dir: &Path, sources: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read sdk source directory") {
            let entry = entry.expect("read sdk source entry");
            let path = entry.path();
            if path.is_dir() {
                collect_rust_sources(&path, sources);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }

    fn read_source(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|error| {
            panic!("read {}: {error}", path.display());
        })
    }

    #[test]
    fn semantic_mir_manifest_declares_private_provider_contract() {
        let manifest = provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.semantic_mir")
            .expect("semantic MIR manifest should exist");

        assert_eq!(manifest.primary_schema_label(), "semantic-mir-facts-1:1");
        assert_eq!(manifest.language_scope, LanguageScope::MultiLanguage);
        assert_eq!(manifest.cache_policy, CachePolicy::InMemoryDerived);
        assert_eq!(manifest.precision_ceiling, PrecisionCeiling::SetupAware);
        assert!(manifest.inputs.contains(&"functions"));
        assert!(manifest.inputs.contains(&"symbols"));
        assert!(manifest.inputs.contains(&"references"));
        assert!(manifest.inputs.contains(&"scopes"));
        assert!(manifest.inputs.contains(&"semantic_imports"));
        assert!(manifest.inputs.contains(&"import_to_package_edges"));
        assert!(manifest.outputs.contains(&"mir_bodies"));
        assert!(manifest.outputs.contains(&"mir_operations"));
        assert!(manifest.outputs.contains(&"places"));
        assert!(manifest.outputs.contains(&"unsupported_semantics"));
    }
}
