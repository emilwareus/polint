pub(crate) mod go;
pub(crate) mod model;
pub(crate) mod paths;
pub(crate) mod query;
pub(crate) mod ts;

use crate::analysis_plan::AnalysisPlan;
use crate::config::LoadedConfig;
use crate::core::{
    AnalysisDb, CapabilitySupport, CapabilitySupportStatus, CapabilitySupportView, FileId,
    ImportFact, Language, ModuleNodeId, ResolutionStatus, ResolvedImportId, SourceFile,
};
use crate::diagnostics::{Diagnostic, TextRange};
use model::{ModuleGraphBuilder, ResolverInput, sort_packages};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const MODULE_GRAPH_TRIGGER_CAPABILITIES: &[&str] =
    &["resolved_imports", "module_graph", "symbols", "references"];

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
    if !plan.requests_any_capability(MODULE_GRAPH_TRIGGER_CAPABILITIES) {
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
    let mut file_owner_modules =
        seed_ts_project_module_nodes(&mut builder, loaded.root.as_path(), &files);
    let has_go_inputs = files
        .iter()
        .any(|file| file.language == crate::core::Language::Go)
        || db
            .imports()
            .iter()
            .any(|import| import.language == crate::core::Language::Go);
    let go_metadata = if has_go_inputs {
        go::GoPackageIndex::load(loaded, db)
    } else {
        go::GoPackageIndex::default()
    };
    let go_ownership = go::seed_go_module_nodes(&mut builder, &go_metadata);
    for (file, module) in go_ownership.file_owner_modules() {
        file_owner_modules.insert(file, module);
    }

    let mut package_nodes_by_file = BTreeMap::new();
    for package in sort_packages(db.packages(), db) {
        let package_node = if package.language == crate::core::Language::Go {
            go_ownership
                .package_node_for_file(package.file)
                .unwrap_or_else(|| builder.ensure_package_node(package))
        } else {
            builder.ensure_package_node(package)
        };
        let file_node = builder.ensure_file_node(package.file);
        if let Some(owner_module) = file_owner_modules
            .get(&package.file)
            .copied()
            .or(root_module)
        {
            builder.link_module_contains(owner_module, package_node);
        }
        builder.link_contains(package_node, file_node);
        package_nodes_by_file.insert(package.file, package_node);
    }
    for (file, package_node) in go_ownership.package_nodes_by_file() {
        package_nodes_by_file.entry(file).or_insert(package_node);
    }

    for file in &files {
        if !package_nodes_by_file.contains_key(&file.id) {
            let file_node = builder.ensure_file_node(file.id);
            if let Some(owner_module) = file_owner_modules.get(&file.id).copied().or(root_module) {
                builder.link_module_contains(owner_module, file_node);
            }
        }
    }

    let mut imports = db.imports().iter().collect::<Vec<_>>();
    imports.sort_by(|left, right| import_order(left, right, db));
    let ts_resolver_context = imports
        .iter()
        .any(|import| import.language.is_ts_family())
        .then(|| ts::TsResolverContext::new(loaded.root.as_path(), db, root_module));

    let mut resolved_imports = Vec::with_capacity(imports.len());
    let mut saw_setup_missing = false;
    let mut setup_missing_reason = None;
    for import in imports {
        let owner_module = file_owner_modules
            .get(&import.file)
            .copied()
            .or(root_module);
        let default_owner = package_nodes_by_file
            .get(&import.file)
            .copied()
            .or_else(|| file_nodes.get(&import.file).copied())
            .or(owner_module)
            .unwrap_or_else(|| builder.ensure_module_node("."));
        let index = resolved_imports.len();
        let input = ResolverInput {
            root: loaded.root.as_path(),
            db,
            import,
            ts_resolver: ts_resolver_context.as_ref(),
            owner_module,
            owner_package: package_nodes_by_file.get(&import.file).copied(),
        };
        let draft = if import.language.is_ts_family() {
            ts::resolve_ts_import(input)
        } else if matches!(import.language, crate::core::Language::Go) {
            go::resolve_go_import(input, &go_metadata)
        } else {
            model::ResolvedImportDraft::unsupported_language()
        };
        if draft.status == ResolutionStatus::SetupMissing && setup_missing_reason.is_none() {
            setup_missing_reason = Some(setup_missing_reason_for_import(
                import.language,
                &go_metadata,
                has_go_inputs,
            ));
        }
        let owner = if import.language.is_ts_family() && draft.status == ResolutionStatus::External
        {
            owner_module.unwrap_or(default_owner)
        } else {
            default_owner
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

    let capability_support =
        setup_missing_support(plan, saw_setup_missing, setup_missing_reason.as_deref());
    let diagnostics = setup_missing_diagnostics(&capability_support);
    ModuleGraphDerivation {
        diagnostics,
        capability_support,
    }
}

fn setup_missing_reason_for_import(
    language: Language,
    go_metadata: &go::GoPackageIndex,
    has_go_inputs: bool,
) -> String {
    if matches!(language, Language::Go) && has_go_inputs {
        return go_metadata
            .setup_missing_reason()
            .unwrap_or("Go package metadata was not loaded.")
            .to_string();
    }
    if language.is_ts_family() {
        return "TypeScript/JavaScript resolver setup failed; check tsconfig.json or package.json."
            .to_string();
    }
    "Resolver setup is required to resolve requested module relationships.".to_string()
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

fn seed_ts_project_module_nodes(
    builder: &mut ModuleGraphBuilder,
    root: &Path,
    files: &[&SourceFile],
) -> BTreeMap<FileId, ModuleNodeId> {
    let mut module_by_root = BTreeMap::<PathBuf, ModuleNodeId>::new();
    let mut owner_by_file = BTreeMap::new();

    for file in files.iter().filter(|file| file.language.is_ts_family()) {
        let absolute_file = if file.path.is_absolute() {
            file.path.clone()
        } else {
            root.join(&file.relative_path)
        };
        let Some(module_root) = find_ts_project_root(root, &absolute_file) else {
            continue;
        };
        let module = if let Some(module) = module_by_root.get(&module_root).copied() {
            module
        } else {
            let label = ts_project_module_label(root, &module_root);
            let module = builder.ensure_module_node(label);
            module_by_root.insert(module_root, module);
            module
        };
        owner_by_file.insert(file.id, module);
    }

    owner_by_file
}

fn find_ts_project_root(root: &Path, file_path: &Path) -> Option<PathBuf> {
    let root = paths::normalize_path(root)?;
    let mut current = paths::normalize_path(file_path.parent()?)?;

    loop {
        if current.join("tsconfig.json").is_file() || current.join("package.json").is_file() {
            return Some(current);
        }
        if current == root || !current.starts_with(&root) || !current.pop() {
            return None;
        }
    }
}

fn ts_project_module_label(root: &Path, module_root: &Path) -> String {
    package_json_name(&module_root.join("package.json")).unwrap_or_else(|| {
        paths::normalize_repo_relative_path(root, module_root).unwrap_or_else(|| ".".to_string())
    })
}

fn package_json_name(path: &Path) -> Option<String> {
    let source = fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&source).ok()?;
    value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

fn setup_missing_support(
    plan: &AnalysisPlan,
    saw_setup_missing: bool,
    setup_missing_reason: Option<&str>,
) -> Vec<CapabilitySupport> {
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
                reason: Some(
                    setup_missing_reason
                        .unwrap_or(
                            "Resolver setup is required to resolve requested module relationships.",
                        )
                        .to_string(),
                ),
                hint: Some("Check language resolver configuration such as tsconfig.json, package.json, or Go package metadata.".to_string()),
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
        "Capability `{}` needs language resolver setup before this rule can run; see {docs_path}.",
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
    fn module_graph_derives_for_symbol_capabilities() {
        for capability in ["symbols", "references"] {
            let temp = tempfile::tempdir().expect("tempdir");
            let mut db = AnalysisDb::new();
            let app = add_file(
                &mut db,
                temp.path(),
                "src/app.ts",
                "import tokens from './tokens';\n",
            );
            add_file(
                &mut db,
                temp.path(),
                "src/tokens.ts",
                "export const tokens = {};\n",
            );
            push_ts_import(&mut db, app, "./tokens", 0);

            derive_requested_module_graph(
                &mut db,
                &loaded_config_for(temp.path()),
                &AnalysisPlan::from_capability_names_for_test(&[capability]),
            );

            assert_eq!(
                db.resolved_imports().len(),
                1,
                "{capability} should trigger module graph derivation"
            );
            assert_eq!(db.resolved_imports()[0].status, ResolutionStatus::Resolved);
        }
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
        assert!(db.module_nodes().iter().any(|node| {
            node.kind == ModuleNodeKind::Package && node.label == "go:internal/payments:payments"
        }));
        let root = db
            .module_nodes()
            .iter()
            .find(|node| node.kind == ModuleNodeKind::Module && node.label == ".")
            .map(|node| node.id)
            .expect("root module node exists");
        let package = db
            .module_nodes()
            .iter()
            .find(|node| {
                node.kind == ModuleNodeKind::Package
                    && node.label == "go:internal/payments:payments"
            })
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
    fn module_graph_provider_keeps_same_name_go_packages_in_different_directories_distinct() {
        let temp = tempfile::tempdir().expect("tempdir");
        let loaded = loaded_config_for(temp.path());
        let mut db = AnalysisDb::new();
        let api = add_file(&mut db, temp.path(), "cmd/api/main.go", "package main\n");
        let worker = add_file(&mut db, temp.path(), "cmd/worker/main.go", "package main\n");
        db.push_package(PackageFact {
            id: PackageId(99),
            file: api,
            name: "main".to_string(),
            span: span(api, 0),
            language: Language::Go,
        });
        db.push_package(PackageFact {
            id: PackageId(99),
            file: worker,
            name: "main".to_string(),
            span: span(worker, 0),
            language: Language::Go,
        });

        derive_requested_module_graph(
            &mut db,
            &loaded,
            &AnalysisPlan::from_capability_names_for_test(&["module_graph"]),
        );

        let api_package = db
            .module_nodes()
            .iter()
            .find(|node| node.kind == ModuleNodeKind::Package && node.label == "go:cmd/api:main")
            .map(|node| node.id)
            .expect("api package node exists");
        let worker_package = db
            .module_nodes()
            .iter()
            .find(|node| node.kind == ModuleNodeKind::Package && node.label == "go:cmd/worker:main")
            .map(|node| node.id)
            .expect("worker package node exists");

        assert_ne!(api_package, worker_package);
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
    fn module_graph_ts_resolution_keeps_missing_matched_alias_unresolved_not_found() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_file(
            temp.path(),
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@domain/*":["src/domain/*"]}}}"#,
        );
        let mut db = AnalysisDb::new();
        let app = add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "import missing from '@domain/missing';\n",
        );
        push_ts_import(&mut db, app, "@domain/missing", 0);

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
    fn module_graph_ts_resolution_keeps_missing_alias_from_commented_tsconfig_unresolved_not_found()
    {
        let temp = tempfile::tempdir().expect("tempdir");
        write_file(
            temp.path(),
            "tsconfig.json",
            r#"{
  // Repo-local aliases are often documented inline.
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {"@domain/*": ["src/domain/*"]}
  }
}"#,
        );
        let mut db = AnalysisDb::new();
        let app = add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "import missing from '@domain/missing';\n",
        );
        push_ts_import(&mut db, app, "@domain/missing", 0);

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
    fn module_graph_ts_resolution_keeps_missing_alias_from_extended_tsconfig_unresolved_not_found()
    {
        let temp = tempfile::tempdir().expect("tempdir");
        write_file(
            temp.path(),
            "tsconfig.base.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@domain/*":["src/domain/*"]}}}"#,
        );
        write_file(
            temp.path(),
            "tsconfig.json",
            r#"{"extends":"./tsconfig.base.json","compilerOptions":{"strict":true}}"#,
        );
        let mut db = AnalysisDb::new();
        let app = add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "import missing from '@domain/missing';\n",
        );
        push_ts_import(&mut db, app, "@domain/missing", 0);

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
    fn module_graph_ts_setup_missing_reports_ts_reason_without_go_inputs() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_file(temp.path(), "tsconfig.json", "{ invalid json");
        let mut db = AnalysisDb::new();
        let app = add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "import dependency from './dependency';\n",
        );
        push_ts_import(&mut db, app, "./dependency", 0);

        let derivation = derive_requested_module_graph(
            &mut db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports"]),
        );

        assert_eq!(
            db.resolved_imports()[0].status,
            ResolutionStatus::SetupMissing
        );
        let reason = derivation.capability_support[0]
            .reason
            .as_deref()
            .expect("setup missing reason");
        assert!(reason.contains("TypeScript/JavaScript"), "{reason}");
        assert!(!reason.contains("Go package metadata"), "{reason}");
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
