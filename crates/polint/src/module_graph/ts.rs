use crate::core::{
    AnalysisDb, FileId, Language, ModuleEdgeKind, ModuleNodeId, ResolutionPrecision,
    ResolutionStatus, UnresolvedReason,
};
use crate::module_graph::model::{ModuleNodeDraft, ResolvedImportDraft, ResolverInput};
use crate::module_graph::paths::normalize_path;
use crate::ts::DYNAMIC_IMPORT_SPECIFIER;
use oxc_resolver::{ResolveError, ResolveOptions, Resolver, TsconfigDiscovery};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static RESOLVER_CONTEXT_CONSTRUCTIONS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Debug)]
pub(crate) struct TsResolverContext {
    resolver: Resolver,
    root: PathBuf,
    file_by_absolute_normalized_path: BTreeMap<PathBuf, FileId>,
    pub(crate) owner_module: Option<ModuleNodeId>,
}

impl TsResolverContext {
    pub(crate) fn new(root: &Path, db: &AnalysisDb, owner_module: Option<ModuleNodeId>) -> Self {
        #[cfg(test)]
        RESOLVER_CONTEXT_CONSTRUCTIONS.with(|count| count.set(count.get() + 1));

        let root = normalize_path(root).unwrap_or_else(|| root.to_path_buf());
        let file_by_absolute_normalized_path = db
            .files()
            .iter()
            .filter_map(|file| {
                let absolute = if file.path.is_absolute() {
                    file.path.clone()
                } else {
                    root.join(&file.relative_path)
                };
                normalize_path(&absolute).map(|path| (path, file.id))
            })
            .collect();

        Self {
            resolver: Resolver::new(resolve_options()),
            root,
            file_by_absolute_normalized_path,
            owner_module,
        }
    }
}

pub(crate) fn resolve_ts_import(input: ResolverInput<'_>) -> ResolvedImportDraft {
    let _ = (input.root, input.owner_module, input.owner_package);
    if !input.import.language.is_ts_family() {
        return ResolvedImportDraft::unsupported_language();
    }
    if input.import.path == DYNAMIC_IMPORT_SPECIFIER {
        return ResolvedImportDraft {
            target: None,
            status: ResolutionStatus::Dynamic,
            precision: ResolutionPrecision::None,
            reason: Some(UnresolvedReason::DynamicExpression),
            edge_kind: None,
        };
    }

    let Some(context) = input.ts_resolver else {
        return ResolvedImportDraft::setup_missing();
    };
    let _owner_module = input.owner_module.or(context.owner_module);
    let Some(importer) = input.db.file(input.import.file) else {
        return ResolvedImportDraft::unresolved(UnresolvedReason::NotFound);
    };
    let importer_path = if importer.path.is_absolute() {
        importer.path.clone()
    } else {
        context.root.join(&importer.relative_path)
    };
    let Some(importer_path) = normalize_path(&importer_path) else {
        return ResolvedImportDraft::unresolved(UnresolvedReason::NotFound);
    };

    match context
        .resolver
        .resolve_file(&importer_path, input.import.path.as_str())
    {
        Ok(resolution) => resolved_path_draft(context, input, resolution.path()),
        Err(ResolveError::Builtin { resolved, .. }) => {
            external_draft(resolved, input.import.language)
        }
        Err(ResolveError::NotFound(_) | ResolveError::MatchedAliasNotFound(_, _)) => {
            if is_external_package_specifier(&input.import.path) {
                external_draft(input.import.path.clone(), input.import.language)
            } else {
                ResolvedImportDraft::unresolved(UnresolvedReason::NotFound)
            }
        }
        Err(
            ResolveError::TsconfigNotFound(_)
            | ResolveError::TsconfigSelfReference(_)
            | ResolveError::TsconfigCircularExtend(_)
            | ResolveError::Json(_)
            | ResolveError::IOError(_),
        ) => ResolvedImportDraft::setup_missing(),
        Err(_) => {
            if is_external_package_specifier(&input.import.path) {
                external_draft(input.import.path.clone(), input.import.language)
            } else {
                ResolvedImportDraft::unresolved(UnresolvedReason::ResolverError)
            }
        }
    }
}

fn resolved_path_draft(
    context: &TsResolverContext,
    input: ResolverInput<'_>,
    resolved_path: &Path,
) -> ResolvedImportDraft {
    let Some(normalized_path) = normalize_path(resolved_path) else {
        return ResolvedImportDraft::unresolved(UnresolvedReason::NotFound);
    };
    if let Some(file) = context
        .file_by_absolute_normalized_path
        .get(&normalized_path)
        .copied()
    {
        return ResolvedImportDraft {
            target: Some(ModuleNodeDraft::file(
                file,
                input.db.path_for(file),
                input.import.language,
            )),
            status: ResolutionStatus::Resolved,
            precision: ResolutionPrecision::ExactFile,
            reason: None,
            edge_kind: Some(ModuleEdgeKind::Imports),
        };
    }

    if !normalized_path.starts_with(&context.root)
        || is_external_package_specifier(&input.import.path)
    {
        external_draft(input.import.path.clone(), input.import.language)
    } else {
        ResolvedImportDraft::unresolved(UnresolvedReason::NotFound)
    }
}

fn external_draft(label: String, language: Language) -> ResolvedImportDraft {
    ResolvedImportDraft {
        target: Some(ModuleNodeDraft::external(label, Some(language))),
        status: ResolutionStatus::External,
        precision: ResolutionPrecision::ExternalPackage,
        reason: None,
        edge_kind: Some(ModuleEdgeKind::DependsOn),
    }
}

fn is_external_package_specifier(specifier: &str) -> bool {
    !specifier.starts_with('.')
        && !specifier.starts_with('/')
        && !specifier.starts_with('#')
        && !specifier.starts_with("@/")
}

fn resolve_options() -> ResolveOptions {
    ResolveOptions {
        tsconfig: Some(TsconfigDiscovery::Auto),
        extensions: vec![
            ".ts".into(),
            ".tsx".into(),
            ".js".into(),
            ".jsx".into(),
            ".json".into(),
            ".node".into(),
        ],
        extension_alias: vec![
            (
                ".js".into(),
                vec![".js".into(), ".ts".into(), ".tsx".into()],
            ),
            (".jsx".into(), vec![".jsx".into(), ".tsx".into()]),
            (".mjs".into(), vec![".mjs".into(), ".mts".into()]),
            (".cjs".into(), vec![".cjs".into(), ".cts".into()]),
        ],
        condition_names: vec![
            "import".into(),
            "require".into(),
            "node".into(),
            "default".into(),
        ],
        main_fields: vec!["module".into(), "browser".into(), "main".into()],
        exports_fields: vec![vec!["exports".into()]],
        imports_fields: vec![vec!["imports".into()]],
        builtin_modules: true,
        symlinks: false,
        ..ResolveOptions::default()
    }
}

#[cfg(test)]
pub(crate) fn reset_resolver_context_construction_count_for_test() {
    RESOLVER_CONTEXT_CONSTRUCTIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn resolver_context_construction_count_for_test() -> usize {
    RESOLVER_CONTEXT_CONSTRUCTIONS.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::{TsResolverContext, resolve_ts_import};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, ImportFact, ImportId, Language, ModuleEdgeKind, ModuleNodeId, ModuleNodeKind,
        ResolutionPrecision, ResolutionStatus, Span, UnresolvedReason,
    };
    use crate::module_graph::derive_requested_module_graph;
    use crate::module_graph::model::ResolverInput;
    use crate::ts::DYNAMIC_IMPORT_SPECIFIER;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn module_graph_resolver_contracts_ts_without_context_is_setup_missing() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import missing from './missing';\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "./missing".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
        });
        let import = &db.imports()[0];

        let draft = resolve_ts_import(ResolverInput {
            root: Path::new("."),
            db: &db,
            import,
            ts_resolver: None,
            owner_module: None,
            owner_package: None,
        });

        assert_eq!(draft.status, ResolutionStatus::SetupMissing);
        assert_eq!(draft.reason, Some(UnresolvedReason::SetupMissing));
    }

    #[test]
    fn module_graph_ts_dynamic_resolution_marks_sentinel_as_dynamic() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("src/app.ts");
        fs::create_dir_all(path.parent().expect("test file has parent")).expect("mkdirs");
        fs::write(&path, "const mod = await import(name);\n").expect("write fixture file");
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            path,
            "src/app.ts".to_string(),
            "const mod = await import(name);\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: DYNAMIC_IMPORT_SPECIFIER.to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
        });
        let import = &db.imports()[0];
        let context = TsResolverContext::new(temp.path(), &db, None);

        let draft = resolve_ts_import(ResolverInput {
            root: temp.path(),
            db: &db,
            import,
            ts_resolver: Some(&context),
            owner_module: None,
            owner_package: None,
        });

        assert_eq!(draft.status, ResolutionStatus::Dynamic);
        assert_eq!(draft.precision, ResolutionPrecision::None);
        assert_eq!(draft.reason, Some(UnresolvedReason::DynamicExpression));
        assert_eq!(draft.target, None);
    }

    type DeterminismSnapshot = (
        Vec<(ModuleNodeKind, String)>,
        Vec<(
            ResolutionStatus,
            ResolutionPrecision,
            Option<UnresolvedReason>,
            Option<String>,
        )>,
        Vec<(String, String, ModuleEdgeKind, ResolutionStatus)>,
    );

    fn write_fixture(root: &Path, relative_path: &str, source: &str) -> PathBuf {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("test fixture path has parent")).expect("mkdirs");
        fs::write(&path, source).expect("write fixture");
        path
    }

    fn add_fixture_file(
        db: &mut AnalysisDb,
        root: &Path,
        relative_path: &str,
        source: &str,
    ) -> crate::core::FileId {
        let path = write_fixture(root, relative_path, source);
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn push_import(db: &mut AnalysisDb, file: crate::core::FileId, path: &str, offset: u32) {
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: path.to_string(),
            span: Span {
                file,
                start_byte: offset,
                end_byte: offset + 1,
                start_line: 1,
                start_col: offset + 1,
                end_line: 1,
                end_col: offset + 2,
            },
            language: Language::TypeScript,
        });
    }

    fn build_determinism_db(root: &Path) -> AnalysisDb {
        write_fixture(
            root,
            "package.json",
            r#"{"name":"frontend","dependencies":{"react":"latest"}}"#,
        );
        write_fixture(
            root,
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        );
        let mut db = AnalysisDb::new();
        let app = add_fixture_file(
            &mut db,
            root,
            "src/app.ts",
            r#"
import tokens from "@/tokens";
import React from "react";
const lazy = await import("./lazy");
const dynamic = await import(name);
"#,
        );
        add_fixture_file(
            &mut db,
            root,
            "src/tokens.ts",
            "export const tokens = {};\n",
        );
        add_fixture_file(&mut db, root, "src/lazy.ts", "export const lazy = true;\n");
        push_import(&mut db, app, "@/tokens", 0);
        push_import(&mut db, app, "react", 30);
        push_import(&mut db, app, "./lazy", 60);
        push_import(&mut db, app, DYNAMIC_IMPORT_SPECIFIER, 90);
        db
    }

    fn node_label(db: &AnalysisDb, id: ModuleNodeId) -> String {
        db.module_nodes()
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.label.clone())
            .expect("node exists")
    }

    fn derive_snapshot(root: &Path) -> DeterminismSnapshot {
        let mut db = build_determinism_db(root);
        let config = load_config(root).expect("test config loads");
        derive_requested_module_graph(
            &mut db,
            &config,
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports", "module_graph"]),
        );

        let nodes = db
            .module_nodes()
            .iter()
            .map(|node| (node.kind, node.label.clone()))
            .collect::<Vec<_>>();
        let imports = db
            .resolved_imports()
            .iter()
            .map(|fact| {
                (
                    fact.status,
                    fact.precision,
                    fact.reason,
                    fact.target_node.map(|node| node_label(&db, node)),
                )
            })
            .collect::<Vec<_>>();
        let edges = db
            .module_edges()
            .iter()
            .map(|edge| {
                (
                    node_label(&db, edge.from),
                    node_label(&db, edge.to),
                    edge.kind,
                    edge.status,
                )
            })
            .collect::<Vec<_>>();

        (nodes, imports, edges)
    }

    #[test]
    fn module_graph_ts_determinism_repeated_provider_runs_match_exact_graph_rows() {
        let temp = tempfile::tempdir().expect("tempdir");

        let first = derive_snapshot(temp.path());
        let second = derive_snapshot(temp.path());

        assert_eq!(first, second);
        assert!(
            first
                .0
                .iter()
                .any(|(kind, label)| { *kind == ModuleNodeKind::Module && label == "frontend" })
        );
        assert!(first.2.iter().any(|(from, to, kind, status)| {
            from == "frontend"
                && to == "react"
                && *kind == ModuleEdgeKind::DependsOn
                && *status == ResolutionStatus::External
        }));
    }
}
