use crate::core::{
    AnalysisDb, FileId, Language, ModuleEdgeKind, ModuleNodeId, ResolutionPrecision, SourceFile,
    ResolutionStatus, UnresolvedReason,
};
use crate::config::LoadedConfig;
use crate::module_graph::formats::package_json::{PackageJsonManifest, parse_package_json};
use crate::module_graph::formats::package_lock::parse_package_lock;
use crate::module_graph::model::{ModuleNodeDraft, ResolvedImportDraft, ResolverInput};
use crate::module_graph::paths::{normalize_path, normalize_repo_relative};
use crate::module_graph::topology::{
    DependencyRequirementFact, DependencyRequirementId, RepoTopologyOverlayFact,
    RepoTopologyOverlayId, RepoTopologyOverlayKind, RequirementKind, ResolvedDependencyEdgeFact,
    ResolvedDependencyEdgeId, ResolvedDependencyKind, SourceSetFact, SourceSetId, SourceSetKind,
    TopologyOutput, TopologyPackageFact, TopologyPackageId, TopologyPackageKind,
    TopologyPrecision, TopologyStatus, WorkspaceRootFact, WorkspaceRootId, WorkspaceRootKind,
};
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

#[cfg(test)]
mod topology {
    use super::collect_ts_topology;
    use crate::config::load_config;
    use crate::core::{AnalysisDb, FileId};
    use crate::module_graph::topology::{
        RepoTopologyOverlayKind, SourceSetKind, TopologyPackageKind, WorkspaceRootKind,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn collect_ts_topology_emits_js_workspace_and_member_packages() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","workspaces":["packages/*"]}"#,
        );
        write_fixture(
            temp.path(),
            "packages/ui/package.json",
            r#"{"name":"@acme/ui","version":"1.0.0"}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(
            &mut db,
            temp.path(),
            "packages/ui/src/index.ts",
            "export const ui = true;\n",
        );
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.workspace_roots.iter().any(|root| {
            root.kind == WorkspaceRootKind::JsWorkspace
                && root.root_path == "."
                && root.manifest_path.as_deref() == Some("package.json")
        }));
        assert!(output.packages.iter().any(|package| {
            package.kind == TopologyPackageKind::JsPackage
                && package.name == "@acme/ui"
                && package.path == "packages/ui"
        }));
    }

    #[test]
    fn collect_ts_topology_records_package_manager_and_lockfile_evidence() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"pnpm@9.0.0"}"#,
        );
        write_fixture(temp.path(), "pnpm-workspace.yaml", "packages:\n  - packages/*\n");
        write_fixture(temp.path(), "package-lock.json", r#"{"lockfileVersion":3}"#);
        write_fixture(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
        write_fixture(temp.path(), "yarn.lock", "# yarn lockfile\n");
        write_fixture(temp.path(), "bun.lock", "# bun lockfile\n");
        write_fixture(temp.path(), "bun.lockb", "binary");
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);
        let labels = output
            .overlays
            .iter()
            .map(|overlay| overlay.label.as_str())
            .collect::<Vec<_>>();

        assert!(labels.contains(&"packageManager:pnpm@9.0.0"));
        assert!(labels.contains(&"pnpm-workspace.yaml:packages/*"));
        assert!(labels.contains(&"lockfile:package-lock.json:package-lock-v3"));
        assert!(labels.contains(&"lockfile:pnpm-lock.yaml:pnpm-lock-present"));
        assert!(labels.contains(&"lockfile:yarn.lock:yarn-lock-present"));
        assert!(labels.contains(&"lockfile:bun.lock:bun-lock-present"));
        assert!(labels.contains(&"lockfile:bun.lockb:bun-lockb-present"));
    }

    #[test]
    fn collect_ts_topology_classifies_ts_source_sets() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(temp.path(), "package.json", r#"{"name":"root"}"#);
        let mut db = AnalysisDb::new();
        let source = add_fixture_file(&mut db, temp.path(), "src/app.ts", "export {};\n");
        let test = add_fixture_file(&mut db, temp.path(), "src/app.test.ts", "export {};\n");
        let spec = add_fixture_file(&mut db, temp.path(), "src/app.spec.tsx", "export {};\n");
        let nested_test =
            add_fixture_file(&mut db, temp.path(), "src/__tests__/app.ts", "export {};\n");
        let generated =
            add_fixture_file(&mut db, temp.path(), "generated/client.ts", "export {};\n");
        let generated_named =
            add_fixture_file(&mut db, temp.path(), "src/api.generated.ts", "export {};\n");
        let vendor =
            add_fixture_file(&mut db, temp.path(), "node_modules/pkg/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(source_set_for_file(&output, source, SourceSetKind::Source));
        assert!(source_set_for_file(&output, test, SourceSetKind::Test));
        assert!(source_set_for_file(&output, spec, SourceSetKind::Test));
        assert!(source_set_for_file(&output, nested_test, SourceSetKind::Test));
        assert!(source_set_for_file(
            &output,
            generated,
            SourceSetKind::Generated
        ));
        assert!(source_set_for_file(
            &output,
            generated_named,
            SourceSetKind::Generated
        ));
        assert!(source_set_for_file(&output, vendor, SourceSetKind::Vendor));
    }

    #[test]
    fn collect_ts_topology_records_tsconfig_alias_and_reference_evidence() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(temp.path(), "package.json", r#"{"name":"root"}"#);
        write_fixture(
            temp.path(),
            "tsconfig.json",
            r#"{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": { "@/*": ["src/*"] },
    "rootDirs": ["src", "generated"]
  },
  "references": [{ "path": "./packages/ui" }]
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.overlays.iter().any(|overlay| {
            overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                && overlay.label == "tsconfig:paths:@/*"
        }));
        assert!(output.overlays.iter().any(|overlay| {
            overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                && overlay.label == "tsconfig:baseUrl:."
        }));
        assert!(output.overlays.iter().any(|overlay| {
            overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                && overlay.label == "tsconfig:rootDirs:generated"
        }));
        assert!(output.overlays.iter().any(|overlay| {
            overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                && overlay.label == "tsconfig:reference:./packages/ui"
        }));
    }

    fn source_set_for_file(
        output: &crate::module_graph::topology::TopologyOutput,
        file: FileId,
        kind: SourceSetKind,
    ) -> bool {
        output
            .source_sets
            .iter()
            .any(|source_set| source_set.files == vec![file] && source_set.kind == kind)
    }

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
    ) -> FileId {
        let path = write_fixture(root, relative_path, source);
        db.add_file(path, relative_path.to_string(), source.to_string())
    }
}

#[cfg(test)]
mod dependency_topology {
    use super::collect_ts_topology;
    use crate::config::load_config;
    use crate::core::AnalysisDb;
    use crate::module_graph::topology::{
        RequirementKind, ResolvedDependencyKind, TopologyPrecision, TopologyStatus,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn collect_ts_topology_emits_declared_dependency_requirement_kinds() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{
  "name": "root",
  "dependencies": { "react": "^18.0.0", "@acme/workspace": "workspace:*" },
  "devDependencies": { "vitest": "^2.0.0" },
  "peerDependencies": { "typescript": "^5.0.0" },
  "optionalDependencies": { "fsevents": "^2.0.0" },
  "bundleDependencies": ["left-pad"]
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);
        let requirements = output
            .dependency_requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.target_name.as_str(),
                    requirement.version_requirement.as_deref(),
                    requirement.kind,
                    requirement.status,
                )
            })
            .collect::<Vec<_>>();

        assert!(requirements.contains(&(
            "react",
            Some("^18.0.0"),
            RequirementKind::Direct,
            TopologyStatus::Present
        )));
        assert!(requirements.contains(&(
            "@acme/workspace",
            Some("workspace:*"),
            RequirementKind::Workspace,
            TopologyStatus::Present
        )));
        assert!(requirements.contains(&(
            "vitest",
            Some("^2.0.0"),
            RequirementKind::Dev,
            TopologyStatus::Present
        )));
        assert!(requirements.contains(&(
            "typescript",
            Some("^5.0.0"),
            RequirementKind::Peer,
            TopologyStatus::Present
        )));
        assert!(requirements.contains(&(
            "fsevents",
            Some("^2.0.0"),
            RequirementKind::Optional,
            TopologyStatus::Present
        )));
        assert!(requirements.contains(&(
            "left-pad",
            None,
            RequirementKind::Bundled,
            TopologyStatus::Present
        )));
    }

    #[test]
    fn collect_ts_topology_emits_package_lock_selected_versions() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "package-lock.json",
            r#"{
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "root", "version": "1.0.0" },
    "node_modules/react": { "version": "18.2.0" }
  }
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.resolved_version.as_deref() == Some("18.2.0")
                && edge.kind == ResolvedDependencyKind::LockfileSelected
                && edge.precision == TopologyPrecision::ExactLockfile
                && edge.status == TopologyStatus::Resolved
                && edge.stable_key.contains("source=package-lock.json")
                && edge.stable_key.contains("schema=package-lock-v3")
        }));
    }

    #[test]
    fn collect_ts_topology_marks_unsupported_and_missing_lockfile_evidence() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
        write_fixture(temp.path(), "yarn.lock", "# yarn lockfile\n");
        write_fixture(temp.path(), "bun.lock", "# bun lockfile\n");
        write_fixture(temp.path(), "bun.lockb", "binary");
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.stable_key.contains("pnpm-lock-present")
                && edge.status == TopologyStatus::Unsupported
                && edge.precision == TopologyPrecision::Unsupported
        }));
        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.stable_key.contains("yarn-lock-present")
                && edge.status == TopologyStatus::Unsupported
                && edge.precision == TopologyPrecision::Unsupported
        }));
        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.stable_key.contains("bun-lock-present")
                && edge.status == TopologyStatus::Unsupported
                && edge.precision == TopologyPrecision::Unsupported
        }));
        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.stable_key.contains("bun-lockb-present")
                && edge.status == TopologyStatus::Unsupported
                && edge.precision == TopologyPrecision::Unsupported
        }));
    }

    #[test]
    fn collect_ts_topology_marks_missing_lockfile_when_declared_external_deps_have_no_lockfile() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","dependencies":{"react":"^18.0.0"}}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.status == TopologyStatus::MissingLockfile
                && edge.precision == TopologyPrecision::Unknown
        }));
    }

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
    ) {
        let path = write_fixture(root, relative_path, source);
        db.add_file(path, relative_path.to_string(), source.to_string());
    }
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

pub(crate) fn collect_ts_topology(
    loaded: &LoadedConfig,
    db: &AnalysisDb,
    _resolver: Option<&TsResolverContext>,
) -> TopologyOutput {
    let mut output = TopologyOutput::default();
    let ts_files = db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
        .collect::<Vec<_>>();
    let package_manifests = collect_package_manifests(loaded, &ts_files);
    let workspace_roots = package_manifests
        .iter()
        .filter(|(_, manifest)| !manifest.workspaces.is_empty())
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();

    let mut root_ids_by_path = BTreeMap::new();
    for package_path in &workspace_roots {
        let id = WorkspaceRootId(output.workspace_roots.len() as u64);
        output.workspace_roots.push(WorkspaceRootFact {
            id,
            kind: WorkspaceRootKind::JsWorkspace,
            root_path: package_path.clone(),
            manifest_path: Some(package_manifest_path(package_path)),
            language: Some(Language::TypeScript),
            stable_key: format!("js-workspace:{package_path}"),
            producer_id: TS_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        });
        root_ids_by_path.insert(package_path.clone(), id);
    }

    let mut package_ids_by_path = BTreeMap::new();
    for (package_path, manifest) in &package_manifests {
        let id = TopologyPackageId(output.packages.len() as u64);
        let workspace_root = workspace_root_for_package(package_path, &workspace_roots)
            .and_then(|root| root_ids_by_path.get(root).copied());
        output.packages.push(TopologyPackageFact {
            id,
            workspace_root,
            package: None,
            module_node: None,
            kind: TopologyPackageKind::JsPackage,
            name: manifest
                .name
                .clone()
                .unwrap_or_else(|| package_path.clone()),
            version: manifest.version.clone(),
            path: package_path.clone(),
            language: Some(Language::TypeScript),
            stable_key: format!(
                "js-package:{package_path}:{}",
                manifest.name.as_deref().unwrap_or("")
            ),
            producer_id: TS_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        });
        package_ids_by_path.insert(package_path.clone(), id);
    }

    for (package_path, manifest) in &package_manifests {
        emit_package_manager_overlays(&mut output, package_path, manifest);
        emit_lockfile_overlays(&mut output, loaded, package_path);
        if let Some(package_id) = package_ids_by_path.get(package_path).copied() {
            emit_package_requirements(&mut output, package_id, package_path, manifest);
        }
    }
    for (package_path, package_id) in &package_ids_by_path {
        emit_package_lock_edges(&mut output, loaded, package_path, *package_id);
        emit_unsupported_lockfile_edges(&mut output, loaded, package_path, *package_id);
        emit_missing_lockfile_edges(&mut output, loaded, package_path, *package_id);
    }
    emit_pnpm_workspace_overlays(&mut output, loaded);
    emit_tsconfig_overlays(&mut output, loaded, &ts_files);

    for file in ts_files {
        let package_path = nearest_package_root_for_relative_path(loaded, &file.relative_path);
        let package = package_path
            .as_ref()
            .and_then(|path| package_ids_by_path.get(path).copied());
        let root = package_path
            .as_ref()
            .and_then(|path| workspace_root_for_package(path, &workspace_roots))
            .and_then(|path| root_ids_by_path.get(path).copied());
        let kind = classify_ts_source_set(file);
        output.source_sets.push(SourceSetFact {
            id: SourceSetId(output.source_sets.len() as u64),
            package,
            root,
            kind,
            path: file.relative_path.clone(),
            language: Some(file.language),
            files: vec![file.id],
            stable_key: format!(
                "ts-source-set:{}:{}",
                source_set_kind_label(kind),
                file.relative_path
            ),
            producer_id: TS_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        });
    }

    output.normalized()
}

const TS_TOPOLOGY_PROVIDER_ID: &str = "polint.module_graph";

fn collect_package_manifests(
    loaded: &LoadedConfig,
    ts_files: &[&SourceFile],
) -> BTreeMap<String, PackageJsonManifest> {
    let mut package_paths = BTreeSet::new();
    if loaded.root.join("package.json").is_file() {
        package_paths.insert(".".to_string());
    }
    for file in ts_files {
        if let Some(package_path) = nearest_package_root_for_relative_path(loaded, &file.relative_path)
        {
            package_paths.insert(package_path);
        }
    }

    let mut manifests = BTreeMap::new();
    for package_path in package_paths {
        if let Some(manifest) = read_package_manifest(loaded, &package_path) {
            for workspace in &manifest.workspaces {
                for member in expand_workspace_glob(&loaded.root, workspace) {
                    if let Some(member_manifest) = read_package_manifest(loaded, &member) {
                        manifests.insert(member, member_manifest);
                    }
                }
            }
            manifests.insert(package_path, manifest);
        }
    }
    manifests
}

fn read_package_manifest(loaded: &LoadedConfig, package_path: &str) -> Option<PackageJsonManifest> {
    let manifest_path = package_manifest_path(package_path);
    let contents = fs::read_to_string(loaded.root.join(&manifest_path)).ok()?;
    Some(parse_package_json(&manifest_path, &contents))
}

fn package_manifest_path(package_path: &str) -> String {
    if package_path == "." {
        "package.json".to_string()
    } else {
        format!("{package_path}/package.json")
    }
}

fn nearest_package_root_for_relative_path(
    loaded: &LoadedConfig,
    relative_path: &str,
) -> Option<String> {
    let mut path = Path::new(relative_path).parent()?.to_path_buf();
    loop {
        let package_path = normalize_repo_relative(path.to_string_lossy())?;
        let manifest_path = package_manifest_path(&package_path);
        if loaded.root.join(manifest_path).is_file() {
            return Some(package_path);
        }
        if !path.pop() {
            return None;
        }
    }
}

fn expand_workspace_glob(root: &Path, pattern: &str) -> Vec<String> {
    let Some(base) = pattern.strip_suffix("/*") else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(root.join(base)) else {
        return Vec::new();
    };
    let mut members = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_type().ok()?.is_dir().then_some(entry.path()))
        .filter(|path| path.join("package.json").is_file())
        .filter_map(|path| crate::module_graph::paths::normalize_repo_relative_path(root, &path))
        .collect::<Vec<_>>();
    members.sort();
    members
}

fn workspace_root_for_package<'a>(
    package_path: &str,
    workspace_roots: &'a BTreeSet<String>,
) -> Option<&'a String> {
    workspace_roots
        .iter()
        .filter(|root| package_path == root.as_str() || package_path.starts_with(&format!("{root}/")))
        .max_by_key(|root| root.len())
}

fn emit_package_manager_overlays(
    output: &mut TopologyOutput,
    package_path: &str,
    manifest: &PackageJsonManifest,
) {
    if let Some(manager) = &manifest.package_manager {
        push_overlay(
            output,
            package_path,
            format!("packageManager:{manager}"),
            Some(package_manifest_path(package_path)),
            TopologyPrecision::ExactStatic,
            TopologyStatus::Present,
        );
    }
}

fn emit_lockfile_overlays(output: &mut TopologyOutput, loaded: &LoadedConfig, package_path: &str) {
    let lockfiles = [
        ("package-lock.json", None),
        ("pnpm-lock.yaml", Some("pnpm-lock-present")),
        ("yarn.lock", Some("yarn-lock-present")),
        ("bun.lock", Some("bun-lock-present")),
        ("bun.lockb", Some("bun-lockb-present")),
    ];
    for (file_name, unsupported_schema) in lockfiles {
        let relative_path = package_relative_path(package_path, file_name);
        if !loaded.root.join(&relative_path).is_file() {
            continue;
        }
        let schema = if let Some(schema) = unsupported_schema {
            schema.to_string()
        } else {
            fs::read_to_string(loaded.root.join(&relative_path))
                .map(|contents| parse_package_lock(&relative_path, &contents).schema_label.to_string())
                .unwrap_or_else(|_| "package-lock-unknown".to_string())
        };
        push_overlay(
            output,
            package_path,
            format!("lockfile:{file_name}:{schema}"),
            Some(relative_path),
            if unsupported_schema.is_some() {
                TopologyPrecision::Unsupported
            } else {
                TopologyPrecision::ExactLockfile
            },
            if unsupported_schema.is_some() {
                TopologyStatus::Unsupported
            } else {
                TopologyStatus::Present
            },
        );
    }
}

fn emit_package_requirements(
    output: &mut TopologyOutput,
    package_id: TopologyPackageId,
    package_path: &str,
    manifest: &PackageJsonManifest,
) {
    for dependency in &manifest.dependencies {
        let kind = if dependency
            .version_requirement
            .as_deref()
            .is_some_and(|requirement| requirement.starts_with("workspace:"))
        {
            RequirementKind::Workspace
        } else {
            dependency.kind
        };
        output
            .dependency_requirements
            .push(DependencyRequirementFact {
                id: DependencyRequirementId(output.dependency_requirements.len() as u64),
                from_package: Some(package_id),
                target_package: None,
                target_name: dependency.target_name.clone(),
                version_requirement: dependency.version_requirement.clone(),
                kind,
                manifest_path: Some(package_manifest_path(package_path)),
                stable_key: format!(
                    "js-require:{package_path}:{}:{}:{}",
                    dependency.section,
                    dependency.target_name,
                    dependency.version_requirement.as_deref().unwrap_or("")
                ),
                producer_id: TS_TOPOLOGY_PROVIDER_ID,
                precision: dependency.precision,
                status: dependency.status,
            });
    }
}

fn emit_package_lock_edges(
    output: &mut TopologyOutput,
    loaded: &LoadedConfig,
    package_path: &str,
    package_id: TopologyPackageId,
) {
    let relative_path = package_relative_path(package_path, "package-lock.json");
    let Ok(contents) = fs::read_to_string(loaded.root.join(&relative_path)) else {
        return;
    };
    let manifest = parse_package_lock(&relative_path, &contents);
    for package in manifest
        .packages
        .iter()
        .filter(|package| !package.path.is_empty())
    {
        let Some(package_name) = package.name.clone() else {
            continue;
        };
        let Some(resolved_version) = package.version.clone() else {
            continue;
        };
        output
            .resolved_dependency_edges
            .push(ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(output.resolved_dependency_edges.len() as u64),
                requirement: requirement_id_for(output, package_id, &package_name),
                from_package: Some(package_id),
                to_package: None,
                package_name: package_name.clone(),
                resolved_version: Some(resolved_version.clone()),
                kind: ResolvedDependencyKind::LockfileSelected,
                stable_key: format!(
                    "js-lock-selected:{package_path}:{package_name}:{resolved_version}:source=package-lock.json:schema={}",
                    manifest.schema_label
                ),
                producer_id: TS_TOPOLOGY_PROVIDER_ID,
                precision: TopologyPrecision::ExactLockfile,
                status: TopologyStatus::Resolved,
            });
    }
}

fn emit_unsupported_lockfile_edges(
    output: &mut TopologyOutput,
    loaded: &LoadedConfig,
    package_path: &str,
    package_id: TopologyPackageId,
) {
    for (file_name, schema) in [
        ("pnpm-lock.yaml", "pnpm-lock-present"),
        ("yarn.lock", "yarn-lock-present"),
        ("bun.lock", "bun-lock-present"),
        ("bun.lockb", "bun-lockb-present"),
    ] {
        let relative_path = package_relative_path(package_path, file_name);
        if !loaded.root.join(&relative_path).is_file() {
            continue;
        }
        output
            .resolved_dependency_edges
            .push(ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(output.resolved_dependency_edges.len() as u64),
                requirement: None,
                from_package: Some(package_id),
                to_package: None,
                package_name: String::new(),
                resolved_version: None,
                kind: ResolvedDependencyKind::LockfileSelected,
                stable_key: format!(
                    "js-lock-unsupported:{package_path}:{file_name}:source={file_name}:schema={schema}"
                ),
                producer_id: TS_TOPOLOGY_PROVIDER_ID,
                precision: TopologyPrecision::Unsupported,
                status: TopologyStatus::Unsupported,
            });
    }
}

fn emit_missing_lockfile_edges(
    output: &mut TopologyOutput,
    loaded: &LoadedConfig,
    package_path: &str,
    package_id: TopologyPackageId,
) {
    if has_any_js_lockfile(loaded, package_path) {
        return;
    }
    let requirements = output
        .dependency_requirements
        .iter()
        .filter(|requirement| {
            requirement.from_package == Some(package_id)
                && requirement.kind != RequirementKind::Workspace
                && requirement.status == TopologyStatus::Present
        })
        .map(|requirement| {
            (
                requirement.id,
                requirement.target_name.clone(),
                requirement.version_requirement.clone(),
            )
        })
        .collect::<Vec<_>>();
    for (requirement_id, target_name, version_requirement) in requirements {
        output
            .resolved_dependency_edges
            .push(ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(output.resolved_dependency_edges.len() as u64),
                requirement: Some(requirement_id),
                from_package: Some(package_id),
                to_package: None,
                package_name: target_name.clone(),
                resolved_version: version_requirement.clone(),
                kind: ResolvedDependencyKind::LockfileSelected,
                stable_key: format!(
                    "js-lock-missing:{package_path}:{target_name}:{}:source=package-lock.json",
                    version_requirement.as_deref().unwrap_or("")
                ),
                producer_id: TS_TOPOLOGY_PROVIDER_ID,
                precision: TopologyPrecision::Unknown,
                status: TopologyStatus::MissingLockfile,
            });
    }
}

fn has_any_js_lockfile(loaded: &LoadedConfig, package_path: &str) -> bool {
    [
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "bun.lock",
        "bun.lockb",
    ]
    .iter()
    .any(|file_name| {
        loaded
            .root
            .join(package_relative_path(package_path, file_name))
            .is_file()
    })
}

fn requirement_id_for(
    output: &TopologyOutput,
    package_id: TopologyPackageId,
    target_name: &str,
) -> Option<DependencyRequirementId> {
    output
        .dependency_requirements
        .iter()
        .find(|requirement| {
            requirement.from_package == Some(package_id) && requirement.target_name == target_name
        })
        .map(|requirement| requirement.id)
}

fn package_relative_path(package_path: &str, file_name: &str) -> String {
    if package_path == "." {
        file_name.to_string()
    } else {
        format!("{package_path}/{file_name}")
    }
}

fn emit_pnpm_workspace_overlays(output: &mut TopologyOutput, loaded: &LoadedConfig) {
    let relative_path = "pnpm-workspace.yaml";
    let Ok(contents) = fs::read_to_string(loaded.root.join(relative_path)) else {
        return;
    };
    for workspace in parse_pnpm_workspace_packages(&contents) {
        push_overlay(
            output,
            ".",
            format!("pnpm-workspace.yaml:{workspace}"),
            Some(relative_path.to_string()),
            TopologyPrecision::Heuristic,
            TopologyStatus::Present,
        );
    }
}

fn parse_pnpm_workspace_packages(contents: &str) -> Vec<String> {
    let mut in_packages = false;
    let mut packages = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }
        if !in_packages {
            continue;
        }
        let Some(entry) = trimmed.strip_prefix('-') else {
            if !trimmed.is_empty() && !line.starts_with(' ') {
                break;
            }
            continue;
        };
        packages.push(entry.trim().trim_matches(['"', '\'']).to_string());
    }
    packages.sort();
    packages.dedup();
    packages
}

fn emit_tsconfig_overlays(
    output: &mut TopologyOutput,
    loaded: &LoadedConfig,
    ts_files: &[&SourceFile],
) {
    let mut configs = BTreeSet::new();
    for file in ts_files {
        let absolute = if file.path.is_absolute() {
            file.path.clone()
        } else {
            loaded.root.join(&file.relative_path)
        };
        if let Some(config) = nearest_tsconfig_path(&loaded.root, &absolute)
            && let Some(relative) =
                crate::module_graph::paths::normalize_repo_relative_path(&loaded.root, &config)
        {
            configs.insert(relative);
        }
    }
    for config in configs {
        let Some(value) = read_json_with_comments(&loaded.root.join(&config)) else {
            continue;
        };
        let Some(object) = value.as_object() else {
            continue;
        };
        if let Some(options) = object
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object)
        {
            if let Some(base_url) = options.get("baseUrl").and_then(serde_json::Value::as_str) {
                push_overlay(
                    output,
                    ".",
                    format!("tsconfig:baseUrl:{base_url}"),
                    Some(config.clone()),
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                );
            }
            if let Some(paths) = options.get("paths").and_then(serde_json::Value::as_object) {
                for pattern in paths.keys() {
                    push_overlay(
                        output,
                        ".",
                        format!("tsconfig:paths:{pattern}"),
                        Some(config.clone()),
                        TopologyPrecision::ExactStatic,
                        TopologyStatus::Present,
                    );
                }
            }
            if let Some(root_dirs) = options
                .get("rootDirs")
                .and_then(serde_json::Value::as_array)
            {
                for root_dir in root_dirs.iter().filter_map(serde_json::Value::as_str) {
                    push_overlay(
                        output,
                        ".",
                        format!("tsconfig:rootDirs:{root_dir}"),
                        Some(config.clone()),
                        TopologyPrecision::ExactStatic,
                        TopologyStatus::Present,
                    );
                }
            }
        }
        if let Some(references) = object
            .get("references")
            .and_then(serde_json::Value::as_array)
        {
            for reference in references
                .iter()
                .filter_map(serde_json::Value::as_object)
                .filter_map(|reference| reference.get("path").and_then(serde_json::Value::as_str))
            {
                push_overlay(
                    output,
                    ".",
                    format!("tsconfig:reference:{reference}"),
                    Some(config.clone()),
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                );
            }
        }
    }
}

fn read_json_with_comments(path: &Path) -> Option<serde_json::Value> {
    let mut source = fs::read_to_string(path).ok()?;
    if let Some(stripped) = source.strip_prefix('\u{feff}') {
        source = stripped.to_string();
    }
    json_strip_comments::strip(&mut source).ok()?;
    serde_json::from_str(&source).ok()
}

fn classify_ts_source_set(file: &SourceFile) -> SourceSetKind {
    let path = file.relative_path.replace('\\', "/");
    if path.contains("/node_modules/") || path.starts_with("node_modules/") {
        return SourceSetKind::Vendor;
    }
    if path.contains("/generated/")
        || path.starts_with("generated/")
        || path.contains("/gen/")
        || path.starts_with("gen/")
        || path.contains(".generated.")
    {
        return SourceSetKind::Generated;
    }
    if path.contains("/__tests__/")
        || path.starts_with("__tests__/")
        || path.contains("/test/")
        || path.starts_with("test/")
        || path.contains("/tests/")
        || path.starts_with("tests/")
        || path.contains(".test.")
        || path.contains(".spec.")
    {
        return SourceSetKind::Test;
    }
    SourceSetKind::Source
}

fn source_set_kind_label(kind: SourceSetKind) -> &'static str {
    match kind {
        SourceSetKind::Source => "source",
        SourceSetKind::Test => "test",
        SourceSetKind::Generated => "generated",
        SourceSetKind::Vendor => "vendor",
        SourceSetKind::External => "external",
        SourceSetKind::Unknown => "unknown",
    }
}

fn push_overlay(
    output: &mut TopologyOutput,
    package_path: &str,
    label: String,
    path: Option<String>,
    precision: TopologyPrecision,
    status: TopologyStatus,
) {
    output.overlays.push(RepoTopologyOverlayFact {
        id: RepoTopologyOverlayId(output.overlays.len() as u64),
        root: None,
        package: None,
        source_set: None,
        kind: RepoTopologyOverlayKind::SourceOfTruthDirectory,
        stable_key: format!(
            "ts-overlay:{package_path}:{label}:{}",
            path.as_deref().unwrap_or("")
        ),
        label,
        path,
        producer_id: TS_TOPOLOGY_PROVIDER_ID,
        precision,
        status,
    });
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
    use super::{TsResolverContext, collect_ts_topology, resolve_ts_import};
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

    mod topology {
        use super::*;
        use crate::module_graph::topology::{
            RepoTopologyOverlayKind, SourceSetKind, TopologyPackageKind, WorkspaceRootKind,
        };
        use crate::core::FileId;

        #[test]
        fn collect_ts_topology_emits_js_workspace_and_member_packages() {
            let temp = tempfile::tempdir().expect("tempdir");
            write_fixture(
                temp.path(),
                "package.json",
                r#"{"name":"root","workspaces":["packages/*"]}"#,
            );
            write_fixture(
                temp.path(),
                "packages/ui/package.json",
                r#"{"name":"@acme/ui","version":"1.0.0"}"#,
            );
            let mut db = AnalysisDb::new();
            add_fixture_file(
                &mut db,
                temp.path(),
                "packages/ui/src/index.ts",
                "export const ui = true;\n",
            );
            let loaded = load_config(temp.path()).expect("config loads");

            let output = collect_ts_topology(&loaded, &db, None);

            assert!(output.workspace_roots.iter().any(|root| {
                root.kind == WorkspaceRootKind::JsWorkspace
                    && root.root_path == "."
                    && root.manifest_path.as_deref() == Some("package.json")
            }));
            assert!(output.packages.iter().any(|package| {
                package.kind == TopologyPackageKind::JsPackage
                    && package.name == "@acme/ui"
                    && package.path == "packages/ui"
            }));
        }

        #[test]
        fn collect_ts_topology_records_package_manager_and_lockfile_evidence() {
            let temp = tempfile::tempdir().expect("tempdir");
            write_fixture(
                temp.path(),
                "package.json",
                r#"{"name":"root","packageManager":"pnpm@9.0.0"}"#,
            );
            write_fixture(temp.path(), "pnpm-workspace.yaml", "packages:\n  - packages/*\n");
            write_fixture(temp.path(), "package-lock.json", r#"{"lockfileVersion":3}"#);
            write_fixture(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
            write_fixture(temp.path(), "yarn.lock", "# yarn lockfile\n");
            write_fixture(temp.path(), "bun.lock", "# bun lockfile\n");
            write_fixture(temp.path(), "bun.lockb", "binary");
            let mut db = AnalysisDb::new();
            add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
            let loaded = load_config(temp.path()).expect("config loads");

            let output = collect_ts_topology(&loaded, &db, None);
            let labels = output
                .overlays
                .iter()
                .map(|overlay| overlay.label.as_str())
                .collect::<Vec<_>>();

            assert!(labels.contains(&"packageManager:pnpm@9.0.0"));
            assert!(labels.contains(&"pnpm-workspace.yaml:packages/*"));
            assert!(labels.contains(&"lockfile:package-lock.json:package-lock-v3"));
            assert!(labels.contains(&"lockfile:pnpm-lock.yaml:pnpm-lock-present"));
            assert!(labels.contains(&"lockfile:yarn.lock:yarn-lock-present"));
            assert!(labels.contains(&"lockfile:bun.lock:bun-lock-present"));
            assert!(labels.contains(&"lockfile:bun.lockb:bun-lockb-present"));
        }

        #[test]
        fn collect_ts_topology_classifies_ts_source_sets() {
            let temp = tempfile::tempdir().expect("tempdir");
            write_fixture(temp.path(), "package.json", r#"{"name":"root"}"#);
            let mut db = AnalysisDb::new();
            let source = add_fixture_file(&mut db, temp.path(), "src/app.ts", "export {};\n");
            let test = add_fixture_file(&mut db, temp.path(), "src/app.test.ts", "export {};\n");
            let spec = add_fixture_file(&mut db, temp.path(), "src/app.spec.tsx", "export {};\n");
            let nested_test =
                add_fixture_file(&mut db, temp.path(), "src/__tests__/app.ts", "export {};\n");
            let generated =
                add_fixture_file(&mut db, temp.path(), "generated/client.ts", "export {};\n");
            let generated_named =
                add_fixture_file(&mut db, temp.path(), "src/api.generated.ts", "export {};\n");
            let vendor =
                add_fixture_file(&mut db, temp.path(), "node_modules/pkg/index.ts", "export {};\n");
            let loaded = load_config(temp.path()).expect("config loads");

            let output = collect_ts_topology(&loaded, &db, None);

            assert!(source_set_for_file(&output, source, SourceSetKind::Source));
            assert!(source_set_for_file(&output, test, SourceSetKind::Test));
            assert!(source_set_for_file(&output, spec, SourceSetKind::Test));
            assert!(source_set_for_file(&output, nested_test, SourceSetKind::Test));
            assert!(source_set_for_file(
                &output,
                generated,
                SourceSetKind::Generated
            ));
            assert!(source_set_for_file(
                &output,
                generated_named,
                SourceSetKind::Generated
            ));
            assert!(source_set_for_file(&output, vendor, SourceSetKind::Vendor));
        }

        #[test]
        fn collect_ts_topology_records_tsconfig_alias_and_reference_evidence() {
            let temp = tempfile::tempdir().expect("tempdir");
            write_fixture(temp.path(), "package.json", r#"{"name":"root"}"#);
            write_fixture(
                temp.path(),
                "tsconfig.json",
                r#"{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": { "@/*": ["src/*"] },
    "rootDirs": ["src", "generated"]
  },
  "references": [{ "path": "./packages/ui" }]
}"#,
            );
            let mut db = AnalysisDb::new();
            add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
            let loaded = load_config(temp.path()).expect("config loads");

            let output = collect_ts_topology(&loaded, &db, None);

            assert!(output.overlays.iter().any(|overlay| {
                overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                    && overlay.label == "tsconfig:paths:@/*"
            }));
            assert!(output.overlays.iter().any(|overlay| {
                overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                    && overlay.label == "tsconfig:baseUrl:."
            }));
            assert!(output.overlays.iter().any(|overlay| {
                overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                    && overlay.label == "tsconfig:rootDirs:generated"
            }));
            assert!(output.overlays.iter().any(|overlay| {
                overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                    && overlay.label == "tsconfig:reference:./packages/ui"
            }));
        }

        fn source_set_for_file(
            output: &crate::module_graph::topology::TopologyOutput,
            file: FileId,
            kind: SourceSetKind,
        ) -> bool {
            output
                .source_sets
                .iter()
                .any(|source_set| source_set.files == vec![file] && source_set.kind == kind)
        }
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
