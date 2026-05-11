pub(crate) mod model;
pub(crate) mod paths;
pub(crate) mod query;

#[cfg(test)]
mod tests {
    use super::model::ModuleGraphBuilder;
    use super::{derive_requested_module_graph, paths, query};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, ImportFact, ImportId, Language, ModuleEdgeKind, ModuleNodeId, ModuleNodeKind,
        PackageFact, PackageId, ResolutionPrecision, ResolutionStatus, Span, UnresolvedReason,
    };
    use std::path::PathBuf;

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
            node.kind == ModuleNodeKind::Package && node.label == "go:payments"
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

        assert!(output.nodes.iter().any(|node| {
            node.kind == ModuleNodeKind::External && node.label == "react"
        }));
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
        let edges = vec![
            (ModuleNodeId(0), ModuleNodeId(2)),
            (ModuleNodeId(0), ModuleNodeId(1)),
            (ModuleNodeId(1), ModuleNodeId(3)),
            (ModuleNodeId(2), ModuleNodeId(3)),
        ];

        assert_eq!(
            query::reachable_from(ModuleNodeId(0), edges),
            vec![ModuleNodeId(1), ModuleNodeId(2), ModuleNodeId(3)]
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
        assert_eq!(db.resolved_imports()[0].status, ResolutionStatus::Unsupported);
        assert_eq!(db.resolved_imports()[0].precision, ResolutionPrecision::None);
        assert_eq!(
            db.resolved_imports()[0].reason,
            Some(UnresolvedReason::UnsupportedLanguage)
        );
    }
}
