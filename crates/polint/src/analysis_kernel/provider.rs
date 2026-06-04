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

const CFG_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "cfg-facts-1",
    version: 1,
}];

const CALLS_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "calls-facts-1",
    version: 1,
}];

const GO_SEMANTIC_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::go::semantic::cache_key::GO_SEMANTIC_SCHEMA_LABEL,
    version: 1,
}];

const IDENTITY_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::analysis::identity::cache_key::IDENTITY_SCHEMA_LABEL,
    version: 1,
}];

const ABSTRACT_DOMAINS_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "abstract-domain-facts-1",
    version: 1,
}];

const DIRECT_SUMMARIES_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::analysis::summaries::cache_key::DIRECT_SUMMARIES_SCHEMA_LABEL,
    version: 1,
}];

const ENTRYPOINTS_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "entrypoints-facts-1",
    version: 1,
}];

const REACHABILITY_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::analysis::reachability::cache_key::REACHABILITY_SCHEMA_LABEL,
    version: 1,
}];

const TYPE_VALUE_ALIAS_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::analysis::types::cache_key::TYPE_VALUE_ALIAS_SCHEMA_LABEL,
    version: 1,
}];

const SEMANTIC_GRAPH_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::analysis::semantic_graph::cache_key::SEMANTIC_GRAPH_SCHEMA_LABEL,
    version: 1,
}];

const SOLVER_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::analysis::solver::cache_key::SOLVER_SCHEMA_LABEL,
    version: 1,
}];

const REFINED_CALLS_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::analysis::refined_calls::cache_key::REFINED_CALLS_SCHEMA_LABEL,
    version: 1,
}];

const DATA_FLOW_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::analysis::data_flow::cache_key::DATA_FLOW_SCHEMA_LABEL,
    version: 1,
}];

const EVIDENCE_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::analysis::evidence::cache_key::EVIDENCE_SCHEMA_LABEL,
    version: 1,
}];

const EXTENSIONS_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: crate::analysis::extensions::cache_key::EXTENSION_FACTS_SCHEMA_LABEL,
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
        id: "polint.cfg",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "functions",
            "mir_bodies",
            "mir_operations",
            "places",
            "unsupported_semantics",
        ],
        outputs: &[
            "cfg_functions",
            "cfg_nodes",
            "basic_blocks",
            "cfg_edges",
            "cfg_reachability",
            "cfg_dominators",
            "cfg_postdominators",
            "cfg_control_dependence",
            "unsupported_control_flow",
        ],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: CFG_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.calls",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "functions",
            "symbols",
            "references",
            "semantic_imports",
            "unsupported_semantics",
            "resolved_imports",
            "import_to_package_edges",
            "mir_bodies",
            "mir_operations",
            "places",
            "cfg_functions",
            "cfg_edges",
        ],
        outputs: &["call_sites", "call_targets", "unresolved_calls"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: CALLS_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.go.semantic",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "packages",
            "functions",
            "go.module_roots",
            "go.package_patterns",
            "go.build_tags",
            "go.include_tests",
            "go.offline",
        ],
        outputs: &[
            "go_semantic_packages",
            "go_semantic_functions",
            "go_semantic_callsites",
            "go_semantic_method_sets",
            "go_semantic_package_errors",
        ],
        language_scope: LanguageScope::Go,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: GO_SEMANTIC_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.identity",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "functions",
            "call_sites",
            "call_targets",
            "unresolved_calls",
            "go_semantic_packages",
        ],
        outputs: &["identity_records"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: IDENTITY_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.abstract_domains",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "functions",
            "mir_bodies",
            "mir_operations",
            "places",
            "unsupported_semantics",
            "cfg_functions",
            "basic_blocks",
            "cfg_edges",
            "call_sites",
            "call_targets",
            "unresolved_calls",
        ],
        outputs: &["domain_observations", "domain_events"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: ABSTRACT_DOMAINS_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.direct_summaries",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "functions",
            "mir_bodies",
            "mir_operations",
            "places",
            "unsupported_semantics",
            "cfg_functions",
            "basic_blocks",
            "cfg_edges",
            "call_sites",
            "call_targets",
            "unresolved_calls",
            "domain_observations",
            "domain_events",
        ],
        outputs: &[
            "summary_control",
            "summary_call",
            "summary_memory",
            "summary_tito",
            "summary_events",
        ],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: DIRECT_SUMMARIES_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.entrypoints",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "functions",
            "symbols",
            "references",
            "semantic_imports",
            "resolved_imports",
            "import_to_package_edges",
            "mir_bodies",
            "mir_operations",
            "places",
            "call_sites",
            "call_targets",
            "unresolved_calls",
        ],
        outputs: &[
            "entrypoints",
            "trust_boundaries",
            "dispatch_edges",
            "unresolved_framework",
        ],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: ENTRYPOINTS_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.reachability",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "functions",
            "symbols",
            "references",
            "call_sites",
            "call_targets",
            "unresolved_calls",
            "entrypoints",
            "identity_records",
            "exports",
        ],
        outputs: &["reachability_roots", "call_reachability"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: REACHABILITY_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.extensions",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "functions",
            "symbols",
            "references",
            "entrypoints",
            "trust_boundaries",
            "dispatch_edges",
            "unresolved_framework",
            "extension.providers",
        ],
        outputs: &["extension_facts", "extension_rejections"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: EXTENSIONS_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.type_value_alias",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files",
            "functions",
            "symbols",
            "references",
            "semantic_imports",
            "resolved_imports",
            "import_to_package_edges",
            "mir_bodies",
            "mir_operations",
            "places",
            "cfg_functions",
            "basic_blocks",
            "cfg_edges",
            "call_sites",
            "call_targets",
            "unresolved_calls",
            "domain_observations",
            "domain_events",
            "summary_control",
            "summary_call",
            "summary_memory",
            "summary_tito",
            "entrypoints",
            "trust_boundaries",
            "dispatch_edges",
            "unresolved_framework",
            "extension_facts",
            "extension_rejections",
        ],
        outputs: &[
            "type_facts",
            "narrowed_type_facts",
            "value_facts",
            "allocation_tokens",
            "access_paths",
            "points_to_constraints",
            "points_to_sets",
            "alias_answers",
        ],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: TYPE_VALUE_ALIAS_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.semantic_graph",
        kind: ProviderKind::WholeRepoDerived,
        // Fact families `build_semantic_graph` ACTUALLY reads today (kept in lockstep
        // with the projection so the declared read-set never overstates consumption):
        // functions/packages (syntax), scopes (symbol graph), call sites (calls),
        // value facts (type/value/alias), MIR places (semantic MIR), Go semantic rows,
        // and the private TS object-model rows refreshed inside the semantic-graph
        // provider. The producer/current-row digest of each is folded into the
        // provider output digest in `semantic_graph::provider::semantic_graph_output_digest`
        // (D-17).
        //
        // SC3 inputs with NO producer yet are intentionally ABSENT until their
        // producer lands (not silently dropped): CFG / summaries (Phase 47), accepted
        // adaptation models / `ModelEdge` (Phase 49), and solver budgets (Phase
        // 51/53). When the projection begins reading a new family, add it here AND
        // fold its producer digest in the same change.
        inputs: &[
            "functions",
            "packages",
            "scopes",
            "call_sites",
            "value_facts",
            "places",
            "go_semantic_functions",
            "go_semantic_callsites",
            "ts_object_allocations",
            "ts_property_writes",
            "ts_property_reads",
            "ts_receiver_bindings",
            "ts_prototype_links",
        ],
        outputs: &[
            "ts_object_allocations",
            "ts_property_writes",
            "ts_property_reads",
            "ts_receiver_bindings",
            "ts_prototype_links",
            "semantic_nodes",
            "semantic_edges",
            "semantic_constraints",
        ],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: SEMANTIC_GRAPH_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        // polint.solver runs in the slot Phase 44 reserved: AFTER polint.semantic_graph
        // and BEFORE polint.refined_calls (D-13). It consumes the unified
        // semantic-graph constraint vocabulary (`semantic_constraints`) plus the
        // points-to source families produced by polint.type_value_alias
        // (`points_to_constraints` / `points_to_sets`), and emits derived edges with
        // provenance. Its output digest folds those upstream digests + the SolverBudget
        // (D-15). The provider auto-enrolls in the Phase 43 determinism gate (D-14).
        id: "polint.solver",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "semantic_constraints",
            "semantic_nodes",
            "points_to_constraints",
            "points_to_sets",
        ],
        outputs: &["solver_derived_edges"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: SOLVER_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.refined_calls",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "call_sites",
            "call_targets",
            "unresolved_calls",
            "entrypoints",
            "dispatch_edges",
            "summary_call",
            "summary_events",
            "type_facts",
            "value_facts",
            "allocation_tokens",
            "points_to_sets",
            "alias_answers",
            "extension_facts",
        ],
        outputs: &["refined_call_edges"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: REFINED_CALLS_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.data_flow",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "mir_bodies",
            "mir_operations",
            "places",
            "cfg_functions",
            "cfg_nodes",
            "cfg_edges",
            "call_sites",
            "call_targets",
            "refined_call_edges",
            "summary_tito",
            "summary_events",
            "type_facts",
            "value_facts",
            "access_paths",
            "points_to_sets",
            "alias_answers",
            "entrypoints",
            "trust_boundaries",
            "extension_facts",
        ],
        outputs: &[
            "data_flow_nodes",
            "data_flow_edges",
            "data_flow_models",
            "data_flow_budgets",
        ],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: DATA_FLOW_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
    ProviderManifest {
        id: "polint.evidence",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "mir_bodies",
            "mir_operations",
            "places",
            "cfg_functions",
            "cfg_nodes",
            "cfg_edges",
            "cfg_control_dependence",
            "call_sites",
            "call_targets",
            "unresolved_calls",
            "refined_call_edges",
            "summary_tito",
            "summary_events",
            "type_facts",
            "value_facts",
            "access_paths",
            "points_to_sets",
            "alias_answers",
            "entrypoints",
            "trust_boundaries",
            "dispatch_edges",
            "extension_facts",
            "data_flow_nodes",
            "data_flow_edges",
            "data_flow_models",
            "data_flow_budgets",
        ],
        outputs: &[
            "evidence_nodes",
            "evidence_edges",
            "evidence_bundles",
            "evidence_paths",
            "evidence_slices",
            "evidence_unknowns",
            "evidence_omitted_regions",
            "evidence_replay_keys",
        ],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: EVIDENCE_SCHEMA,
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
                "polint.cfg",
                "polint.calls",
                "polint.go.semantic",
                "polint.identity",
                "polint.abstract_domains",
                "polint.direct_summaries",
                "polint.entrypoints",
                "polint.reachability",
                "polint.extensions",
                "polint.type_value_alias",
                "polint.semantic_graph",
                "polint.solver",
                "polint.refined_calls",
                "polint.data_flow",
                "polint.evidence",
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
                "polint.cfg",
                "polint.calls",
                "polint.go.semantic",
                "polint.identity",
                "polint.abstract_domains",
                "polint.direct_summaries",
                "polint.entrypoints",
                "polint.reachability",
                "polint.extensions",
                "polint.type_value_alias",
                "polint.semantic_graph",
                "polint.solver",
                "polint.refined_calls",
                "polint.data_flow",
                "polint.evidence",
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
                "polint.semantic_mir",
                "polint.cfg",
                "polint.calls",
                "polint.go.semantic",
                "polint.identity",
                "polint.abstract_domains",
                "polint.direct_summaries",
                "polint.entrypoints",
                "polint.reachability",
                "polint.extensions",
                "polint.type_value_alias",
                "polint.semantic_graph",
                "polint.solver",
                "polint.refined_calls",
                "polint.data_flow",
                "polint.evidence",
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
                    id: "polint.semantic_mir",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "functions",
                        "symbols",
                        "references",
                        "scopes",
                        "semantic_imports",
                        "import_to_package_edges",
                    ],
                    outputs: vec![
                        "mir_bodies",
                        "mir_operations",
                        "places",
                        "unsupported_semantics",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.cfg",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "functions",
                        "mir_bodies",
                        "mir_operations",
                        "places",
                        "unsupported_semantics",
                    ],
                    outputs: vec![
                        "cfg_functions",
                        "cfg_nodes",
                        "basic_blocks",
                        "cfg_edges",
                        "cfg_reachability",
                        "cfg_dominators",
                        "cfg_postdominators",
                        "cfg_control_dependence",
                        "unsupported_control_flow",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.calls",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "functions",
                        "symbols",
                        "references",
                        "semantic_imports",
                        "unsupported_semantics",
                        "resolved_imports",
                        "import_to_package_edges",
                        "mir_bodies",
                        "mir_operations",
                        "places",
                        "cfg_functions",
                        "cfg_edges",
                    ],
                    outputs: vec!["call_sites", "call_targets", "unresolved_calls"],
                },
                ProviderOrderRow {
                    id: "polint.go.semantic",
                    kind: "whole_repo_derived",
                    language_scope: "go",
                    inputs: vec![
                        "source_files",
                        "packages",
                        "functions",
                        "go.module_roots",
                        "go.package_patterns",
                        "go.build_tags",
                        "go.include_tests",
                        "go.offline",
                    ],
                    outputs: vec![
                        "go_semantic_packages",
                        "go_semantic_functions",
                        "go_semantic_callsites",
                        "go_semantic_method_sets",
                        "go_semantic_package_errors",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.identity",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "functions",
                        "call_sites",
                        "call_targets",
                        "unresolved_calls",
                        "go_semantic_packages",
                    ],
                    outputs: vec!["identity_records"],
                },
                ProviderOrderRow {
                    id: "polint.abstract_domains",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "functions",
                        "mir_bodies",
                        "mir_operations",
                        "places",
                        "unsupported_semantics",
                        "cfg_functions",
                        "basic_blocks",
                        "cfg_edges",
                        "call_sites",
                        "call_targets",
                        "unresolved_calls",
                    ],
                    outputs: vec!["domain_observations", "domain_events"],
                },
                ProviderOrderRow {
                    id: "polint.direct_summaries",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "functions",
                        "mir_bodies",
                        "mir_operations",
                        "places",
                        "unsupported_semantics",
                        "cfg_functions",
                        "basic_blocks",
                        "cfg_edges",
                        "call_sites",
                        "call_targets",
                        "unresolved_calls",
                        "domain_observations",
                        "domain_events",
                    ],
                    outputs: vec![
                        "summary_control",
                        "summary_call",
                        "summary_memory",
                        "summary_tito",
                        "summary_events",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.entrypoints",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "functions",
                        "symbols",
                        "references",
                        "semantic_imports",
                        "resolved_imports",
                        "import_to_package_edges",
                        "mir_bodies",
                        "mir_operations",
                        "places",
                        "call_sites",
                        "call_targets",
                        "unresolved_calls",
                    ],
                    outputs: vec![
                        "entrypoints",
                        "trust_boundaries",
                        "dispatch_edges",
                        "unresolved_framework",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.reachability",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "functions",
                        "symbols",
                        "references",
                        "call_sites",
                        "call_targets",
                        "unresolved_calls",
                        "entrypoints",
                        "identity_records",
                        "exports",
                    ],
                    outputs: vec!["reachability_roots", "call_reachability"],
                },
                ProviderOrderRow {
                    id: "polint.extensions",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "functions",
                        "symbols",
                        "references",
                        "entrypoints",
                        "trust_boundaries",
                        "dispatch_edges",
                        "unresolved_framework",
                        "extension.providers",
                    ],
                    outputs: vec!["extension_facts", "extension_rejections"],
                },
                ProviderOrderRow {
                    id: "polint.type_value_alias",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "source_files",
                        "functions",
                        "symbols",
                        "references",
                        "semantic_imports",
                        "resolved_imports",
                        "import_to_package_edges",
                        "mir_bodies",
                        "mir_operations",
                        "places",
                        "cfg_functions",
                        "basic_blocks",
                        "cfg_edges",
                        "call_sites",
                        "call_targets",
                        "unresolved_calls",
                        "domain_observations",
                        "domain_events",
                        "summary_control",
                        "summary_call",
                        "summary_memory",
                        "summary_tito",
                        "entrypoints",
                        "trust_boundaries",
                        "dispatch_edges",
                        "unresolved_framework",
                        "extension_facts",
                        "extension_rejections",
                    ],
                    outputs: vec![
                        "type_facts",
                        "narrowed_type_facts",
                        "value_facts",
                        "allocation_tokens",
                        "access_paths",
                        "points_to_constraints",
                        "points_to_sets",
                        "alias_answers",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.semantic_graph",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "functions",
                        "packages",
                        "scopes",
                        "call_sites",
                        "value_facts",
                        "places",
                        "go_semantic_functions",
                        "go_semantic_callsites",
                        "ts_object_allocations",
                        "ts_property_writes",
                        "ts_property_reads",
                        "ts_receiver_bindings",
                        "ts_prototype_links",
                    ],
                    outputs: vec![
                        "ts_object_allocations",
                        "ts_property_writes",
                        "ts_property_reads",
                        "ts_receiver_bindings",
                        "ts_prototype_links",
                        "semantic_nodes",
                        "semantic_edges",
                        "semantic_constraints",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.solver",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "semantic_constraints",
                        "semantic_nodes",
                        "points_to_constraints",
                        "points_to_sets",
                    ],
                    outputs: vec!["solver_derived_edges"],
                },
                ProviderOrderRow {
                    id: "polint.refined_calls",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "call_sites",
                        "call_targets",
                        "unresolved_calls",
                        "entrypoints",
                        "dispatch_edges",
                        "summary_call",
                        "summary_events",
                        "type_facts",
                        "value_facts",
                        "allocation_tokens",
                        "points_to_sets",
                        "alias_answers",
                        "extension_facts",
                    ],
                    outputs: vec!["refined_call_edges"],
                },
                ProviderOrderRow {
                    id: "polint.data_flow",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "mir_bodies",
                        "mir_operations",
                        "places",
                        "cfg_functions",
                        "cfg_nodes",
                        "cfg_edges",
                        "call_sites",
                        "call_targets",
                        "refined_call_edges",
                        "summary_tito",
                        "summary_events",
                        "type_facts",
                        "value_facts",
                        "access_paths",
                        "points_to_sets",
                        "alias_answers",
                        "entrypoints",
                        "trust_boundaries",
                        "extension_facts",
                    ],
                    outputs: vec![
                        "data_flow_nodes",
                        "data_flow_edges",
                        "data_flow_models",
                        "data_flow_budgets",
                    ],
                },
                ProviderOrderRow {
                    id: "polint.evidence",
                    kind: "whole_repo_derived",
                    language_scope: "multi_language",
                    inputs: vec![
                        "mir_bodies",
                        "mir_operations",
                        "places",
                        "cfg_functions",
                        "cfg_nodes",
                        "cfg_edges",
                        "cfg_control_dependence",
                        "call_sites",
                        "call_targets",
                        "unresolved_calls",
                        "refined_call_edges",
                        "summary_tito",
                        "summary_events",
                        "type_facts",
                        "value_facts",
                        "access_paths",
                        "points_to_sets",
                        "alias_answers",
                        "entrypoints",
                        "trust_boundaries",
                        "dispatch_edges",
                        "extension_facts",
                        "data_flow_nodes",
                        "data_flow_edges",
                        "data_flow_models",
                        "data_flow_budgets",
                    ],
                    outputs: vec![
                        "evidence_nodes",
                        "evidence_edges",
                        "evidence_bundles",
                        "evidence_paths",
                        "evidence_slices",
                        "evidence_unknowns",
                        "evidence_omitted_regions",
                        "evidence_replay_keys",
                    ],
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

    #[test]
    fn calls_manifest_declares_private_provider_contract() {
        let manifest = provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.calls")
            .expect("calls manifest should exist");

        assert_eq!(manifest.primary_schema_label(), "calls-facts-1:1");
        assert_eq!(manifest.language_scope, LanguageScope::MultiLanguage);
        assert_eq!(manifest.cache_policy, CachePolicy::InMemoryDerived);
        assert_eq!(manifest.precision_ceiling, PrecisionCeiling::SetupAware);
        for input in [
            "source_files",
            "functions",
            "symbols",
            "references",
            "semantic_imports",
            "unsupported_semantics",
            "resolved_imports",
            "import_to_package_edges",
            "mir_bodies",
            "mir_operations",
            "places",
            "cfg_functions",
            "cfg_edges",
        ] {
            assert!(manifest.inputs.contains(&input), "missing input {input}");
        }
        assert_eq!(
            manifest.outputs,
            &["call_sites", "call_targets", "unresolved_calls"]
        );
    }

    #[test]
    fn demand_query_internals_are_not_public_sdk_runner_cli_or_docs_surface() {
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));

        // Specific internal type and module names that must not appear in
        // public surfaces.
        let demand_markers = [
            "QueryContext",
            "QueryKind",
            "QueryBudget",
            "QueryTrace",
            "QuarantineSet",
            "QuarantineEntry",
            "QuarantineReason",
            "SccComponent",
            "SccGraph",
            "SccCacheEntry",
            "SccFixpointStatus",
            "DependencyRead",
            "DependencyReadKind",
            "MemoEntry",
            "demand_query_key",
            "analysis::demand",
            "DemandQuery",
            "demand::query",
            "demand::context",
            "demand::scc",
            "demand::quarantine",
        ];

        // Check SDK sources
        for source_path in rust_sources_under(&crate_root.join("src/sdk")) {
            let source = read_source(&source_path);
            for marker in &demand_markers {
                assert!(
                    !source.contains(marker),
                    "demand query internal `{marker}` leaked into SDK: {}",
                    source_path.display()
                );
            }
        }

        // Check runner
        let runner = read_source(&crate_root.join("src/runner/mod.rs"));
        for marker in &demand_markers {
            assert!(
                !runner.contains(marker),
                "demand query internal `{marker}` leaked into runner"
            );
        }

        // Check CLI
        let cli = read_source(&crate_root.join("src/cli/mod.rs"));
        for marker in &demand_markers {
            assert!(
                !cli.contains(marker),
                "demand query internal `{marker}` leaked into CLI"
            );
        }

        // Check lib.rs
        let lib = read_source(&crate_root.join("src/lib.rs"));
        assert!(
            !lib.contains("pub mod demand"),
            "demand module should not be public in lib.rs"
        );

        // Check README
        let readme_path = crate_root
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("README.md");
        if readme_path.exists() {
            let readme = read_source(&readme_path);
            for marker in &demand_markers {
                assert!(
                    !readme.contains(marker),
                    "demand query internal `{marker}` leaked into README"
                );
            }
        }

        // Check docs/facts
        let docs_facts = crate_root
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("docs/facts");
        if docs_facts.exists() {
            for source_path in rust_sources_under(&docs_facts) {
                let source = read_source(&source_path);
                for marker in &demand_markers {
                    assert!(
                        !source.contains(marker),
                        "demand query internal `{marker}` leaked into docs/facts: {}",
                        source_path.display()
                    );
                }
            }
        }
    }

    #[test]
    fn direct_summaries_provider_manifest_declares_private_outputs() {
        let manifest = provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.direct_summaries")
            .expect("direct summaries manifest should exist");

        assert_eq!(manifest.primary_schema_label(), "direct-summary-facts-1:1");
        assert_eq!(manifest.language_scope, LanguageScope::MultiLanguage);
        assert_eq!(manifest.cache_policy, CachePolicy::InMemoryDerived);
        assert_eq!(manifest.precision_ceiling, PrecisionCeiling::SetupAware);
        for input in [
            "source_files",
            "functions",
            "mir_bodies",
            "mir_operations",
            "places",
            "unsupported_semantics",
            "cfg_functions",
            "basic_blocks",
            "cfg_edges",
            "call_sites",
            "call_targets",
            "unresolved_calls",
            "domain_observations",
            "domain_events",
        ] {
            assert!(manifest.inputs.contains(&input), "missing input {input}");
        }
        assert_eq!(
            manifest.outputs,
            &[
                "summary_control",
                "summary_call",
                "summary_memory",
                "summary_tito",
                "summary_events",
            ]
        );
    }
}
