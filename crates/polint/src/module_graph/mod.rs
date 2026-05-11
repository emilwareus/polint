pub(crate) mod go;
pub(crate) mod model;
pub(crate) mod paths;
pub(crate) mod query;
pub(crate) mod ts;

use crate::analysis_plan::AnalysisPlan;
use crate::config::LoadedConfig;
use crate::core::{
    AnalysisDb, CapabilitySupport, CapabilitySupportStatus, CapabilitySupportView, ImportFact,
    ResolutionStatus, ResolvedImportId,
};
use crate::diagnostics::{Diagnostic, TextRange};
use model::{ModuleGraphBuilder, ResolverInput, sort_packages};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct ModuleGraphDerivation {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: Vec<CapabilitySupport>,
}

impl ModuleGraphDerivation {
    pub(crate) fn support_view(&self, base: &CapabilitySupportView) -> CapabilitySupportView {
        let mut entries = base.entries().to_vec();
        for override_entry in &self.capability_support {
            if let Some(existing) = entries.iter_mut().find(|entry| {
                entry.capability == override_entry.capability
                    && entry.language == override_entry.language
            }) {
                *existing = override_entry.clone();
            } else {
                entries.push(override_entry.clone());
            }
        }
        CapabilitySupportView::new(entries)
    }
}

pub(crate) fn derive_requested_module_graph(
    db: &mut AnalysisDb,
    loaded: &LoadedConfig,
    plan: &AnalysisPlan,
) -> ModuleGraphDerivation {
    if !plan.requests_any_capability(&["resolved_imports", "module_graph"]) {
        return ModuleGraphDerivation::default();
    }

    let mut builder = ModuleGraphBuilder::new(db);
    let has_graph_inputs =
        !db.files().is_empty() || !db.packages().is_empty() || !db.imports().is_empty();
    let root_module = has_graph_inputs.then(|| builder.ensure_module_node("."));
    let mut file_nodes = BTreeMap::new();
    let mut files = db.files().iter().collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    for file in &files {
        file_nodes.insert(file.id, builder.ensure_file_node(file.id));
    }

    let mut package_nodes_by_file = BTreeMap::new();
    for package in sort_packages(db.packages(), db) {
        let package_node = builder.ensure_package_node(package);
        let file_node = builder.ensure_file_node(package.file);
        if let Some(root_module) = root_module {
            builder.link_module_contains(root_module, package_node);
        }
        builder.link_contains(package_node, file_node);
        package_nodes_by_file.insert(package.file, package_node);
    }

    for file in &files {
        if !package_nodes_by_file.contains_key(&file.id) {
            let file_node = builder.ensure_file_node(file.id);
            if let Some(root_module) = root_module {
                builder.link_module_contains(root_module, file_node);
            }
        }
    }

    let mut imports = db.imports().iter().collect::<Vec<_>>();
    imports.sort_by(|left, right| import_order(left, right, db));

    let mut resolved_imports = Vec::with_capacity(imports.len());
    let go_metadata = go::GoPackageIndex::default();
    let mut saw_setup_missing = false;
    for import in imports {
        let owner = package_nodes_by_file
            .get(&import.file)
            .copied()
            .or_else(|| file_nodes.get(&import.file).copied())
            .or(root_module)
            .unwrap_or_else(|| builder.ensure_module_node("."));
        let index = resolved_imports.len();
        let input = ResolverInput {
            root: loaded.root.as_path(),
            db,
            import,
            owner_module: root_module,
            owner_package: package_nodes_by_file.get(&import.file).copied(),
        };
        let draft = if import.language.is_ts_family() {
            ts::resolve_ts_import(input)
        } else if matches!(import.language, crate::core::Language::Go) {
            go::resolve_go_import(input, &go_metadata)
        } else {
            model::ResolvedImportDraft::unsupported_language()
        };
        let fact = builder.apply_resolved_import_draft_with_id(
            import,
            owner,
            draft,
            ResolvedImportId(index as u64),
        );
        saw_setup_missing |= fact.status == ResolutionStatus::SetupMissing;
        resolved_imports.push(fact);
    }

    let output = builder.finish();
    db.replace_module_graph_facts(resolved_imports, output.nodes, output.edges);

    let capability_support = setup_missing_support(plan, saw_setup_missing);
    let diagnostics = setup_missing_diagnostics(&capability_support);
    ModuleGraphDerivation {
        diagnostics,
        capability_support,
    }
}

fn import_order(left: &ImportFact, right: &ImportFact, db: &AnalysisDb) -> std::cmp::Ordering {
    (
        db.path_for(left.file),
        left.span.start_byte,
        left.path.as_str(),
        left.id,
    )
        .cmp(&(
            db.path_for(right.file),
            right.span.start_byte,
            right.path.as_str(),
            right.id,
        ))
}

fn setup_missing_support(plan: &AnalysisPlan, saw_setup_missing: bool) -> Vec<CapabilitySupport> {
    if !saw_setup_missing {
        return Vec::new();
    }

    ["resolved_imports", "module_graph"]
        .into_iter()
        .filter(|capability| plan.requests_capability(capability))
        .filter_map(|capability| {
            let base = plan
                .support_view()
                .entries()
                .iter()
                .find(|entry| entry.capability == capability)?;
            Some(CapabilitySupport {
                capability: capability.to_string(),
                language: None,
                status: CapabilitySupportStatus::SetupMissing,
                rules: base.rules.clone(),
                reason: Some("Go package metadata is required to resolve Go imports.".to_string()),
                hint: Some(
                    "Run polint in a Go module with package metadata available.".to_string(),
                ),
                docs_path: Some(
                    "docs/roadmap/12_RESOLVED_IMPORTS_MODULE_GRAPH_ARCHITECTURE.md".to_string(),
                ),
            })
        })
        .collect()
}

fn setup_missing_diagnostics(support: &[CapabilitySupport]) -> Vec<Diagnostic> {
    support
        .iter()
        .flat_map(|entry| {
            entry
                .rules
                .iter()
                .map(|rule_id| setup_missing_diagnostic(entry, rule_id))
        })
        .collect()
}

fn setup_missing_diagnostic(entry: &CapabilitySupport, rule_id: &str) -> Diagnostic {
    let docs_path = entry
        .docs_path
        .as_deref()
        .unwrap_or("docs/roadmap/12_RESOLVED_IMPORTS_MODULE_GRAPH_ARCHITECTURE.md");
    Diagnostic::error(
        "polint/capability",
        "<workspace>",
        TextRange::point(1, 1),
        format!(
            "Rule `{rule_id}` requested capability `{}`, but required setup is missing.",
            entry.capability
        ),
    )
    .with_evidence("rule", rule_id.to_string())
    .with_evidence("capability", entry.capability.clone())
    .with_evidence("status", "setup_missing")
    .with_help(format!(
        "Capability `{}` needs Go package metadata before this rule can run; see {docs_path}.",
        entry.capability
    ))
}

#[cfg(test)]
mod tests {
    use super::model::ModuleGraphBuilder;
    use super::{derive_requested_module_graph, paths, query, ts};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, CapabilitySupport, CapabilitySupportStatus, CapabilitySupportView, ImportFact,
        ImportId, Language, ModuleEdge, ModuleEdgeId, ModuleEdgeKind, ModuleNodeId, ModuleNodeKind,
        PackageFact, PackageId, ResolutionPrecision, ResolutionStatus, Span, UnresolvedReason,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    fn span(file: crate::core::FileId, start_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte: start_byte + 1,
            start_line: 1,
            start_col: start_byte + 1,
            end_line: 1,
            end_col: start_byte + 2,
        }
    }

    type DeterministicSnapshot = (
        Vec<String>,
        Vec<(ModuleNodeId, ModuleNodeId, ModuleEdgeKind)>,
        Vec<(
            ImportId,
            Option<ModuleNodeId>,
            ResolutionStatus,
            ResolutionPrecision,
            Option<UnresolvedReason>,
        )>,
    );

    fn loaded_config() -> crate::config::LoadedConfig {
        let temp = tempfile::tempdir().expect("tempdir");
        load_config(temp.path()).expect("default config loads")
    }

    fn loaded_config_for(root: &Path) -> crate::config::LoadedConfig {
        load_config(root).expect("default config loads")
    }

    fn write_file(root: &Path, relative_path: &str, source: &str) -> PathBuf {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("test file has parent")).expect("mkdirs");
        fs::write(&path, source).expect("write fixture file");
        path
    }

    fn add_file(
        db: &mut AnalysisDb,
        root: &Path,
        relative_path: &str,
        source: &str,
    ) -> crate::core::FileId {
        let path = write_file(root, relative_path, source);
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn push_ts_import(
        db: &mut AnalysisDb,
        file: crate::core::FileId,
        path: &str,
        start_byte: u32,
    ) -> ImportId {
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: path.to_string(),
            span: span(file, start_byte),
            language: Language::TypeScript,
        })
    }

    fn node_label(db: &AnalysisDb, id: Option<ModuleNodeId>) -> Option<&str> {
        let id = id?;
        db.module_nodes()
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.label.as_str())
    }

    #[test]
    fn module_graph_provider_skips_work_when_relationship_capabilities_are_not_requested() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import React from 'react';\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "react".to_string(),
            span: span(file, 0),
            language: Language::TypeScript,
        });

        let derivation =
            derive_requested_module_graph(&mut db, &loaded_config(), &AnalysisPlan::empty());

        assert!(derivation.diagnostics.is_empty());
        assert!(derivation.capability_support.is_empty());
        assert!(db.resolved_imports().is_empty());
        assert!(db.module_nodes().is_empty());
        assert!(db.module_edges().is_empty());
    }

    #[test]
    fn module_graph_derivation_support_view_overrides_base_rows() {
        let derivation = super::ModuleGraphDerivation {
            diagnostics: Vec::new(),
            capability_support: vec![CapabilitySupport {
                capability: "module_graph".to_string(),
                language: None,
                status: CapabilitySupportStatus::SetupMissing,
                rules: vec!["local/graph".to_string()],
                reason: Some("setup missing".to_string()),
                hint: None,
                docs_path: None,
            }],
        };
        let base = CapabilitySupportView::new(vec![CapabilitySupport {
            capability: "module_graph".to_string(),
            language: None,
            status: CapabilitySupportStatus::Supported,
            rules: vec!["local/graph".to_string()],
            reason: None,
            hint: None,
            docs_path: None,
        }]);

        let support = derivation.support_view(&base);

        assert_eq!(
            support.status_for("module_graph"),
            Some(CapabilitySupportStatus::SetupMissing)
        );
    }

    #[test]
    fn module_graph_provider_seeds_file_package_and_module_contains_nodes() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("internal/payments/service.go"),
            "internal/payments/service.go".to_string(),
            "package payments\n".to_string(),
        );
        db.push_package(PackageFact {
            id: PackageId(99),
            file,
            name: "payments".to_string(),
            span: span(file, 0),
            language: Language::Go,
        });

        derive_requested_module_graph(
            &mut db,
            &loaded_config(),
            &AnalysisPlan::from_capability_names_for_test(&["module_graph"]),
        );

        assert!(db.module_nodes().iter().any(|node| {
            node.kind == ModuleNodeKind::File && node.label == "internal/payments/service.go"
        }));
        assert!(
            db.module_nodes().iter().any(|node| {
                node.kind == ModuleNodeKind::Package && node.label == "go:payments"
            })
        );
        let root = db
            .module_nodes()
            .iter()
            .find(|node| node.kind == ModuleNodeKind::Module && node.label == ".")
            .map(|node| node.id)
            .expect("root module node exists");
        let package = db
            .module_nodes()
            .iter()
            .find(|node| node.kind == ModuleNodeKind::Package && node.label == "go:payments")
            .map(|node| node.id)
            .expect("package node exists");
        let file_node = db
            .module_nodes()
            .iter()
            .find(|node| node.kind == ModuleNodeKind::File)
            .map(|node| node.id)
            .expect("file node exists");

        assert!(db.module_edges().iter().any(|edge| {
            edge.kind == ModuleEdgeKind::Contains && edge.from == root && edge.to == package
        }));
        assert!(db.module_edges().iter().any(|edge| {
            edge.kind == ModuleEdgeKind::Contains && edge.from == package && edge.to == file_node
        }));
    }

    #[test]
    fn module_graph_builder_links_external_dependencies_from_owner_nodes() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import React from 'react';\n".to_string(),
        );
        let mut builder = ModuleGraphBuilder::new(&db);
        let root = builder.ensure_module_node(".");
        let file_node = builder.ensure_file_node(file);
        builder.link_module_contains(root, file_node);
        let external = builder.ensure_external_node("react", Some(Language::TypeScript));
        builder.link_dependency(root, external, None, ModuleEdgeKind::DependsOn);
        let output = builder.finish();

        assert!(
            output
                .nodes
                .iter()
                .any(|node| { node.kind == ModuleNodeKind::External && node.label == "react" })
        );
        assert!(output.edges.iter().any(|edge| {
            edge.kind == ModuleEdgeKind::DependsOn && edge.from == root && edge.to == external
        }));
    }

    #[test]
    fn module_graph_provider_emits_one_resolved_import_for_each_syntax_import() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import React from 'react';\nimport x from './missing';\n".to_string(),
        );
        let first = db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "react".to_string(),
            span: span(file, 0),
            language: Language::TypeScript,
        });
        let second = db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "./missing".to_string(),
            span: span(file, 25),
            language: Language::TypeScript,
        });

        derive_requested_module_graph(
            &mut db,
            &loaded_config(),
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports"]),
        );

        let import_ids = db
            .resolved_imports()
            .iter()
            .map(|fact| fact.import)
            .collect::<Vec<_>>();
        assert_eq!(import_ids, vec![first, second]);
    }

    #[test]
    fn module_graph_provider_orders_output_deterministically() {
        fn derive() -> DeterministicSnapshot {
            let mut db = AnalysisDb::new();
            let app = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "import React from 'react';\n".to_string(),
            );
            let util = db.add_file(
                PathBuf::from("src/util.ts"),
                "src/util.ts".to_string(),
                "export const util = 1;\n".to_string(),
            );
            db.push_import(ImportFact {
                id: ImportId(99),
                file: app,
                package: None,
                path: "react".to_string(),
                span: span(app, 0),
                language: Language::TypeScript,
            });
            db.push_import(ImportFact {
                id: ImportId(99),
                file: util,
                package: None,
                path: "./missing".to_string(),
                span: span(util, 0),
                language: Language::TypeScript,
            });

            derive_requested_module_graph(
                &mut db,
                &loaded_config(),
                &AnalysisPlan::from_capability_names_for_test(&[
                    "resolved_imports",
                    "module_graph",
                ]),
            );

            (
                db.module_nodes()
                    .iter()
                    .map(|node| format!("{:?}:{}", node.kind, node.label))
                    .collect(),
                db.module_edges()
                    .iter()
                    .map(|edge| (edge.from, edge.to, edge.kind))
                    .collect(),
                db.resolved_imports()
                    .iter()
                    .map(|fact| {
                        (
                            fact.import,
                            fact.target_node,
                            fact.status,
                            fact.precision,
                            fact.reason,
                        )
                    })
                    .collect(),
            )
        }

        assert_eq!(derive(), derive());
    }

    #[test]
    fn module_graph_paths_normalize_lexically_without_escaping_repo_root() {
        assert_eq!(
            paths::normalize_repo_relative("src/./app/../app/main.ts"),
            Some("src/app/main.ts".to_string())
        );
        assert_eq!(paths::normalize_repo_relative("../escape.ts"), None);
    }

    #[test]
    fn module_graph_query_reachable_from_uses_sorted_bfs_order() {
        let tuple_edges = vec![
            (ModuleNodeId(0), ModuleNodeId(2)),
            (ModuleNodeId(0), ModuleNodeId(1)),
            (ModuleNodeId(1), ModuleNodeId(3)),
            (ModuleNodeId(2), ModuleNodeId(3)),
        ];
        let module_edges = vec![
            ModuleEdge {
                id: ModuleEdgeId(0),
                from: ModuleNodeId(0),
                to: ModuleNodeId(2),
                import: None,
                resolved_import: None,
                kind: ModuleEdgeKind::Imports,
                status: ResolutionStatus::Resolved,
            },
            ModuleEdge {
                id: ModuleEdgeId(1),
                from: ModuleNodeId(1),
                to: ModuleNodeId(0),
                import: None,
                resolved_import: None,
                kind: ModuleEdgeKind::Imports,
                status: ResolutionStatus::Resolved,
            },
        ];

        assert_eq!(
            query::reachable_from(ModuleNodeId(0), tuple_edges),
            vec![ModuleNodeId(1), ModuleNodeId(2), ModuleNodeId(3)]
        );
        assert_eq!(
            query::outgoing(&module_edges, ModuleNodeId(0))
                .map(|edge| edge.to)
                .collect::<Vec<_>>(),
            vec![ModuleNodeId(2)]
        );
        assert_eq!(
            query::incoming(&module_edges, ModuleNodeId(0))
                .map(|edge| edge.from)
                .collect::<Vec<_>>(),
            vec![ModuleNodeId(1)]
        );
    }

    #[test]
    fn module_graph_provider_keeps_unsupported_imports_visible() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("README.md"),
            "README.md".to_string(),
            "not source\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "unknown".to_string(),
            span: span(file, 0),
            language: Language::Unknown,
        });

        derive_requested_module_graph(
            &mut db,
            &loaded_config(),
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports"]),
        );

        assert_eq!(db.resolved_imports().len(), 1);
        assert_eq!(db.resolved_imports()[0].target_node, None);
        assert_eq!(
            db.resolved_imports()[0].status,
            ResolutionStatus::Unsupported
        );
        assert_eq!(
            db.resolved_imports()[0].precision,
            ResolutionPrecision::None
        );
        assert_eq!(
            db.resolved_imports()[0].reason,
            Some(UnresolvedReason::UnsupportedLanguage)
        );
    }

    #[test]
    fn module_graph_ts_resolution_resolves_relative_import_to_local_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        let component = add_file(
            &mut db,
            temp.path(),
            "src/component.ts",
            "import tokens from './tokens';\n",
        );
        add_file(
            &mut db,
            temp.path(),
            "src/tokens.ts",
            "export const tokens = {};\n",
        );
        push_ts_import(&mut db, component, "./tokens", 0);

        derive_requested_module_graph(
            &mut db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports", "module_graph"]),
        );

        let fact = &db.resolved_imports()[0];
        assert_eq!(fact.status, ResolutionStatus::Resolved);
        assert_eq!(fact.precision, ResolutionPrecision::ExactFile);
        assert_eq!(fact.reason, None);
        assert_eq!(node_label(&db, fact.target_node), Some("src/tokens.ts"));
    }

    #[test]
    fn module_graph_ts_resolution_resolves_tsconfig_path_alias_to_local_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_file(
            temp.path(),
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        );
        let mut db = AnalysisDb::new();
        let component = add_file(
            &mut db,
            temp.path(),
            "src/component.ts",
            "import tokens from '@/tokens';\n",
        );
        add_file(
            &mut db,
            temp.path(),
            "src/tokens.ts",
            "export const tokens = {};\n",
        );
        push_ts_import(&mut db, component, "@/tokens", 0);

        derive_requested_module_graph(
            &mut db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports", "module_graph"]),
        );

        let fact = &db.resolved_imports()[0];
        assert_eq!(fact.status, ResolutionStatus::Resolved);
        assert_eq!(fact.precision, ResolutionPrecision::ExactFile);
        assert_eq!(node_label(&db, fact.target_node), Some("src/tokens.ts"));
    }

    #[test]
    fn module_graph_ts_resolution_classifies_package_imports_as_external_dependencies() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_file(
            temp.path(),
            "package.json",
            r#"{"name":"frontend","dependencies":{"react":"latest","@scope/lib":"latest"}}"#,
        );
        let mut db = AnalysisDb::new();
        let app = add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "import React from 'react';\nconst lib = require('@scope/lib');\n",
        );
        push_ts_import(&mut db, app, "react", 0);
        push_ts_import(&mut db, app, "@scope/lib", 28);

        derive_requested_module_graph(
            &mut db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports", "module_graph"]),
        );

        let facts = db.resolved_imports();
        assert!(facts.iter().all(|fact| {
            fact.status == ResolutionStatus::External
                && fact.precision == ResolutionPrecision::ExternalPackage
                && fact.reason.is_none()
        }));
        assert!(
            db.module_nodes()
                .iter()
                .any(|node| node.kind == ModuleNodeKind::External && node.label == "react")
        );
        assert!(
            db.module_nodes()
                .iter()
                .any(|node| node.kind == ModuleNodeKind::External && node.label == "@scope/lib")
        );
    }

    #[test]
    fn module_graph_ts_resolution_keeps_missing_relative_import_unresolved_not_found() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        let app = add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "import missing from './missing';\n",
        );
        push_ts_import(&mut db, app, "./missing", 0);

        derive_requested_module_graph(
            &mut db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports"]),
        );

        let fact = &db.resolved_imports()[0];
        assert_eq!(fact.status, ResolutionStatus::Unresolved);
        assert_eq!(fact.precision, ResolutionPrecision::None);
        assert_eq!(fact.reason, Some(UnresolvedReason::NotFound));
        assert_eq!(fact.target_node, None);
    }

    #[test]
    fn module_graph_ts_resolution_constructs_resolver_context_once_per_run() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        let app = add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "import one from './one';\nimport two from './two';\n",
        );
        add_file(
            &mut db,
            temp.path(),
            "src/one.ts",
            "export const one = 1;\n",
        );
        add_file(
            &mut db,
            temp.path(),
            "src/two.ts",
            "export const two = 2;\n",
        );
        push_ts_import(&mut db, app, "./one", 0);
        push_ts_import(&mut db, app, "./two", 25);
        ts::reset_resolver_context_construction_count_for_test();

        derive_requested_module_graph(
            &mut db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports", "module_graph"]),
        );

        assert_eq!(ts::resolver_context_construction_count_for_test(), 1);
    }

    #[test]
    fn module_graph_ts_resolution_creates_project_module_with_contains_and_dependency_edges() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_file(
            temp.path(),
            "package.json",
            r#"{"name":"frontend","dependencies":{"react":"latest"}}"#,
        );
        write_file(
            temp.path(),
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        );
        let mut db = AnalysisDb::new();
        let app = add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "import tokens from '@/tokens';\nimport React from 'react';\n",
        );
        add_file(
            &mut db,
            temp.path(),
            "src/tokens.ts",
            "export const tokens = {};\n",
        );
        push_ts_import(&mut db, app, "@/tokens", 0);
        push_ts_import(&mut db, app, "react", 32);

        derive_requested_module_graph(
            &mut db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports", "module_graph"]),
        );

        let module = db
            .module_nodes()
            .iter()
            .find(|node| node.kind == ModuleNodeKind::Module && node.label == "frontend")
            .map(|node| node.id)
            .expect("package-named module node exists");
        let app_file = db
            .module_nodes()
            .iter()
            .find(|node| node.kind == ModuleNodeKind::File && node.label == "src/app.ts")
            .map(|node| node.id)
            .expect("file node exists");
        let external = db
            .module_nodes()
            .iter()
            .find(|node| node.kind == ModuleNodeKind::External && node.label == "react")
            .map(|node| node.id)
            .expect("external node exists");

        assert!(db.module_edges().iter().any(|edge| {
            edge.kind == ModuleEdgeKind::Contains && edge.from == module && edge.to == app_file
        }));
        assert!(db.module_edges().iter().any(|edge| {
            edge.kind == ModuleEdgeKind::DependsOn && edge.from == module && edge.to == external
        }));
    }
}
