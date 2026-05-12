use crate::core::{
    AnalysisDb, FileId, Language, ModuleEdgeKind, ModuleNodeId, ResolutionPrecision,
    ResolutionStatus, UnresolvedReason,
};
use crate::module_graph::model::{ModuleNodeDraft, ResolvedImportDraft, ResolverInput};
use crate::module_graph::paths::normalize_path;
use crate::ts::DYNAMIC_IMPORT_SPECIFIER;
use oxc_resolver::{ResolveError, ResolveOptions, Resolver, TsconfigDiscovery};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
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
    path_aliases_by_config_dir: BTreeMap<PathBuf, Vec<String>>,
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
            path_aliases_by_config_dir: collect_ts_path_aliases(&root, db),
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
        Err(ResolveError::MatchedAliasNotFound(_, _)) => {
            ResolvedImportDraft::unresolved(UnresolvedReason::NotFound)
        }
        Err(ResolveError::NotFound(_)) => {
            if tsconfig_path_alias_matches(context, &importer_path, &input.import.path) {
                ResolvedImportDraft::unresolved(UnresolvedReason::NotFound)
            } else if is_external_package_specifier(&input.import.path) {
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

fn tsconfig_path_alias_matches(
    context: &TsResolverContext,
    importer_path: &Path,
    specifier: &str,
) -> bool {
    let Some(mut current) = importer_path.parent().and_then(normalize_path) else {
        return false;
    };
    loop {
        if let Some(patterns) = context.path_aliases_by_config_dir.get(&current) {
            return patterns
                .iter()
                .any(|pattern| ts_path_pattern_matches(pattern, specifier));
        }
        if current == context.root || !current.starts_with(&context.root) || !current.pop() {
            return false;
        }
    }
}

fn ts_path_pattern_matches(pattern: &str, specifier: &str) -> bool {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return pattern == specifier;
    };
    specifier.starts_with(prefix) && specifier.ends_with(suffix)
}

fn collect_ts_path_aliases(root: &Path, db: &AnalysisDb) -> BTreeMap<PathBuf, Vec<String>> {
    let mut aliases = BTreeMap::new();
    for file in db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
    {
        let absolute = if file.path.is_absolute() {
            file.path.clone()
        } else {
            root.join(&file.relative_path)
        };
        let Some(config_path) = nearest_tsconfig_path(root, &absolute) else {
            continue;
        };
        let Some(config_dir) = config_path.parent().and_then(normalize_path) else {
            continue;
        };
        aliases
            .entry(config_dir)
            .or_insert_with(|| read_tsconfig_path_aliases(&config_path));
    }
    aliases
}

fn nearest_tsconfig_path(root: &Path, file_path: &Path) -> Option<PathBuf> {
    let root = normalize_path(root)?;
    let mut current = normalize_path(file_path.parent()?)?;
    loop {
        let candidate = current.join("tsconfig.json");
        if candidate.is_file() {
            return Some(candidate);
        }
        if current == root || !current.starts_with(&root) || !current.pop() {
            return None;
        }
    }
}

fn read_tsconfig_path_aliases(path: &Path) -> Vec<String> {
    let mut visited = BTreeSet::new();
    read_tsconfig_path_aliases_inner(path, &mut visited)
}

fn read_tsconfig_path_aliases_inner(path: &Path, visited: &mut BTreeSet<PathBuf>) -> Vec<String> {
    let Some(path) = normalize_path(path) else {
        return Vec::new();
    };
    if !visited.insert(path.clone()) {
        return Vec::new();
    }
    let Some(config) = read_tsconfig_alias_wire(&path) else {
        return Vec::new();
    };

    if let Some(paths) = config
        .compiler_options
        .as_ref()
        .and_then(|options| options.paths.as_ref())
    {
        return sorted_ts_path_aliases(paths.keys().cloned());
    }

    let Some(config_dir) = path.parent() else {
        return Vec::new();
    };
    let mut aliases = config
        .extends
        .into_iter()
        .flat_map(TsconfigExtendsWire::into_specifiers)
        .filter_map(|specifier| resolve_tsconfig_extends_path(config_dir, &specifier))
        .flat_map(|extended_path| read_tsconfig_path_aliases_inner(&extended_path, visited))
        .collect::<Vec<_>>();
    aliases.sort();
    aliases.dedup();
    aliases
}

fn read_tsconfig_alias_wire(path: &Path) -> Option<TsconfigAliasWire> {
    let Ok(mut source) = fs::read_to_string(path) else {
        return None;
    };
    if let Some(stripped) = source.strip_prefix('\u{feff}') {
        source = stripped.to_string();
    }
    if json_strip_comments::strip(&mut source).is_err() {
        return None;
    }
    serde_json::from_str::<TsconfigAliasWire>(&source).ok()
}

fn sorted_ts_path_aliases(paths: impl Iterator<Item = String>) -> Vec<String> {
    let mut aliases = paths.collect::<Vec<_>>();
    aliases.sort();
    aliases.dedup();
    aliases
}

fn resolve_tsconfig_extends_path(config_dir: &Path, specifier: &str) -> Option<PathBuf> {
    let specifier_path = Path::new(specifier);
    if specifier_path.is_absolute() {
        return resolve_tsconfig_file_candidate(specifier_path);
    }
    if specifier.starts_with('.') {
        return resolve_tsconfig_file_candidate(&config_dir.join(specifier_path));
    }
    resolve_package_tsconfig_extends_path(config_dir, specifier)
}

fn resolve_package_tsconfig_extends_path(config_dir: &Path, specifier: &str) -> Option<PathBuf> {
    let mut current = normalize_path(config_dir)?;
    loop {
        let candidate = current.join("node_modules").join(specifier);
        if let Some(resolved) = resolve_tsconfig_file_candidate(&candidate) {
            return Some(resolved);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn resolve_tsconfig_file_candidate(base: &Path) -> Option<PathBuf> {
    let mut candidates = vec![base.to_path_buf()];
    if base.extension().and_then(|extension| extension.to_str()) != Some("json") {
        let mut with_json = base.as_os_str().to_owned();
        with_json.push(".json");
        candidates.push(PathBuf::from(with_json));
    }
    candidates.push(base.join("tsconfig.json"));

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| normalize_path(&candidate))
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TsconfigExtendsWire {
    Single(String),
    Multiple(Vec<String>),
}

impl TsconfigExtendsWire {
    fn into_specifiers(self) -> Vec<String> {
        match self {
            Self::Single(specifier) => vec![specifier],
            Self::Multiple(specifiers) => specifiers,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsconfigAliasWire {
    #[serde(default)]
    extends: Option<TsconfigExtendsWire>,
    compiler_options: Option<TsconfigCompilerOptionsWire>,
}

#[derive(Debug, Deserialize)]
struct TsconfigCompilerOptionsWire {
    paths: Option<BTreeMap<String, Vec<String>>>,
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
