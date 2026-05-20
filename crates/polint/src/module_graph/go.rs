use crate::config::LoadedConfig;
use crate::core::{
    AnalysisDb, FileId, Language, ModuleEdgeKind, ModuleNodeId, ResolutionPrecision,
    ResolutionStatus, UnresolvedReason,
};
use crate::go::lifecycle::{self, GoAnalysisConfig};
use crate::module_graph::formats::go_mod::parse_go_mod;
use crate::module_graph::formats::go_work::parse_go_work;
use crate::module_graph::model::{
    ModuleGraphBuilder, ModuleNodeDraft, ResolvedImportDraft, ResolverInput,
};
use crate::module_graph::paths::{
    self, TOPOLOGY_LOCKFILE_MAX_BYTES, TOPOLOGY_MANIFEST_MAX_BYTES,
    read_repo_file_to_string_with_limit, repo_file_exists,
};
use crate::module_graph::topology::{
    DependencyRequirementFact, DependencyRequirementId, RequirementKind,
    ResolvedDependencyEdgeFact, ResolvedDependencyEdgeId, ResolvedDependencyKind, SourceSetFact,
    SourceSetId, SourceSetKind, TopologyOutput, TopologyPackageFact, TopologyPackageId,
    TopologyPackageKind, TopologyPrecision, TopologyStatus, WorkspaceRootFact, WorkspaceRootId,
    WorkspaceRootKind,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

const GO_TOPOLOGY_PROVIDER_ID: &str = "polint.module_graph";

#[derive(Debug, Clone)]
pub(crate) struct GoPackageIndex {
    by_import_path: BTreeMap<String, GoPackageMetadata>,
    file_to_import_path: BTreeMap<FileId, String>,
    module_paths: BTreeSet<String>,
    setup_missing_reason: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GoModuleOwnership {
    file_owner_modules: BTreeMap<FileId, ModuleNodeId>,
    package_nodes_by_file: BTreeMap<FileId, ModuleNodeId>,
}

impl GoModuleOwnership {
    #[cfg(test)]
    pub(crate) fn module_node_for_file(&self, file: FileId) -> Option<ModuleNodeId> {
        self.file_owner_modules.get(&file).copied()
    }

    pub(crate) fn package_node_for_file(&self, file: FileId) -> Option<ModuleNodeId> {
        self.package_nodes_by_file.get(&file).copied()
    }

    pub(crate) fn file_owner_modules(&self) -> impl Iterator<Item = (FileId, ModuleNodeId)> + '_ {
        self.file_owner_modules
            .iter()
            .map(|(file, node)| (*file, *node))
    }

    pub(crate) fn package_nodes_by_file(
        &self,
    ) -> impl Iterator<Item = (FileId, ModuleNodeId)> + '_ {
        self.package_nodes_by_file
            .iter()
            .map(|(file, node)| (*file, *node))
    }
}

impl Default for GoPackageIndex {
    fn default() -> Self {
        Self::setup_missing("Go package metadata was not loaded.")
    }
}

impl GoPackageIndex {
    pub(crate) fn load(loaded: &LoadedConfig, db: &AnalysisDb) -> Self {
        let config = match GoAnalysisConfig::from_loaded(loaded, db) {
            Ok(config) => config,
            Err(error) => return Self::setup_missing(error.reason()),
        };
        Self::load_with_runner(loaded.root.as_path(), db, &config, run_go_list)
    }

    fn load_with_runner(
        root: &Path,
        db: &AnalysisDb,
        config: &GoAnalysisConfig,
        run: impl FnOnce(&Path, &GoAnalysisConfig) -> GoCommandOutput,
    ) -> Self {
        if !config.files_without_module_root.is_empty() {
            return Self::setup_missing("some Go files are not under a go.mod module root.");
        }
        let missing_roots = config.missing_module_roots(root);
        if !missing_roots.is_empty() {
            return Self::setup_missing(format!(
                "configured Go module roots are missing go.mod: {}.",
                missing_roots.join(", ")
            ));
        }

        let output = run(root, config);
        if !output.status.success() {
            return Self::setup_missing(go_list_failure_reason(&output));
        }

        Self::from_go_list_stdout(root, db, &output.stdout)
    }

    fn from_go_list_stdout(root: &Path, db: &AnalysisDb, stdout: &[u8]) -> Self {
        let mut packages = Vec::new();
        let stream = serde_json::Deserializer::from_slice(stdout).into_iter::<GoListPackage>();
        for parsed in stream {
            match parsed {
                Ok(package) => packages.push(package),
                Err(error) => {
                    return Self::setup_missing(format!(
                        "go list -json ./... failed to parse output: {error}"
                    ));
                }
            }
        }
        packages.sort_by(|left, right| left.import_path.cmp(&right.import_path));

        let file_ids = db
            .files()
            .iter()
            .filter_map(|file| {
                paths::normalize_repo_relative(&file.relative_path)
                    .map(|relative_path| (relative_path, file.id))
            })
            .collect::<BTreeMap<_, _>>();

        let mut index = Self {
            by_import_path: BTreeMap::new(),
            file_to_import_path: BTreeMap::new(),
            module_paths: BTreeSet::new(),
            setup_missing_reason: None,
        };
        for package in packages {
            if package.import_path.is_empty() {
                continue;
            }
            let metadata = GoPackageMetadata::from_go_list_package(root, &file_ids, package);
            if let Some(module_path) = &metadata.module_path {
                index.module_paths.insert(module_path.clone());
            }
            for file in &metadata.files {
                index
                    .file_to_import_path
                    .insert(*file, metadata.import_path.clone());
            }
            index
                .by_import_path
                .insert(metadata.import_path.clone(), metadata);
        }
        index
    }

    fn setup_missing(reason: impl Into<String>) -> Self {
        Self {
            by_import_path: BTreeMap::new(),
            file_to_import_path: BTreeMap::new(),
            module_paths: BTreeSet::new(),
            setup_missing_reason: Some(reason.into()),
        }
    }

    pub(crate) fn setup_missing_reason(&self) -> Option<&str> {
        self.setup_missing_reason.as_deref()
    }

    pub(crate) fn is_setup_missing(&self) -> bool {
        self.setup_missing_reason.is_some()
    }

    #[cfg(test)]
    pub(crate) fn module_path(&self) -> Option<&String> {
        self.module_paths.iter().next()
    }

    pub(crate) fn package(&self, import_path: &str) -> Option<&GoPackageMetadata> {
        self.by_import_path.get(import_path)
    }

    #[cfg(test)]
    pub(crate) fn import_paths(&self) -> impl Iterator<Item = &str> {
        self.by_import_path.keys().map(String::as_str)
    }

    fn local_packages(&self) -> impl Iterator<Item = &GoPackageMetadata> {
        self.by_import_path
            .values()
            .filter(|package| self.is_local_package(package))
    }

    fn is_local_package(&self, package: &GoPackageMetadata) -> bool {
        !package.standard
            && !package.files.is_empty()
            && package.module_path.as_deref().is_some_and(|module_path| {
                self.module_paths.contains(module_path)
                    && import_is_within_module(&package.import_path, module_path)
            })
    }

    fn import_is_external_dependency(&self, import_path: &str) -> bool {
        if !self.module_paths.is_empty() {
            return !self
                .module_paths
                .iter()
                .any(|module_path| import_is_within_module(import_path, module_path));
        }
        is_go_stdlib_import_path(import_path)
    }

    #[cfg(test)]
    fn load_with_runner_for_test(
        root: &Path,
        db: &AnalysisDb,
        run: impl FnOnce() -> GoCommandOutput,
    ) -> Self {
        let loaded = crate::config::load_config(root).expect("config loads");
        let config = GoAnalysisConfig::from_loaded(&loaded, db).expect("Go lifecycle config loads");
        Self::load_with_runner(root, db, &config, |_, _| run())
    }
}

pub(crate) fn seed_go_module_nodes(
    builder: &mut ModuleGraphBuilder,
    metadata: &GoPackageIndex,
) -> GoModuleOwnership {
    let mut ownership = GoModuleOwnership::default();

    for package in metadata.local_packages() {
        let Some(module_path) = &package.module_path else {
            continue;
        };
        let module = builder.ensure_module_node(module_path.clone());
        let package_node =
            builder.ensure_package_node_with_label(package.import_path(), None, Some(Language::Go));
        builder.link_module_contains(module, package_node);
        for file in package.files() {
            let file_node = builder.ensure_file_node(file);
            builder.link_contains(package_node, file_node);
            ownership.file_owner_modules.insert(file, module);
            ownership.package_nodes_by_file.insert(file, package_node);
        }
    }

    ownership
}

pub(crate) fn collect_go_topology(
    loaded: &LoadedConfig,
    db: &AnalysisDb,
    metadata: &GoPackageIndex,
) -> TopologyOutput {
    let config = match GoAnalysisConfig::from_loaded(loaded, db) {
        Ok(config) => config,
        Err(_) => {
            return TopologyOutput {
                workspace_roots: vec![repository_root()],
                source_sets: go_files_setup_missing(db, None),
                ..TopologyOutput::default()
            }
            .normalized();
        }
    };

    let mut output = TopologyOutput::default();
    output.workspace_roots.push(repository_root());

    let mut root_ids_by_path = BTreeMap::new();
    for module_root in &config.module_roots {
        let id = WorkspaceRootId(output.workspace_roots.len() as u64);
        let go_mod_path = module_root_manifest_path(module_root, "go.mod");
        let present = repo_file_exists(&loaded.root, &go_mod_path);
        output.workspace_roots.push(WorkspaceRootFact {
            id,
            kind: WorkspaceRootKind::GoModule,
            root_path: module_root.clone(),
            manifest_path: present.then_some(go_mod_path),
            language: Some(Language::Go),
            stable_key: format!("go-root:{module_root}"),
            producer_id: GO_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::ExactStatic,
            status: if present {
                TopologyStatus::Present
            } else {
                TopologyStatus::SetupMissing
            },
        });
        root_ids_by_path.insert(module_root.clone(), id);
    }

    let go_work = loaded.root.join("go.work");
    if repo_file_exists(&loaded.root, "go.work")
        && lifecycle::go_work_covers_module_roots(&loaded.root, &go_work, &config.module_roots)
    {
        output.workspace_roots.push(WorkspaceRootFact {
            id: WorkspaceRootId(output.workspace_roots.len() as u64),
            kind: WorkspaceRootKind::GoWorkspace,
            root_path: ".".to_string(),
            manifest_path: Some("go.work".to_string()),
            language: Some(Language::Go),
            stable_key: "go-work:.".to_string(),
            producer_id: GO_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        });
        if let Ok(contents) = read_repo_file_to_string_with_limit(
            &loaded.root,
            "go.work",
            TOPOLOGY_MANIFEST_MAX_BYTES,
        ) {
            let _ = parse_go_work("go.work", &contents);
        }
    }

    let mut package_ids_by_import_path = BTreeMap::new();
    let mut go_module_paths = Vec::new();
    let mut module_requirements = BTreeMap::new();
    let mut module_replacements = BTreeMap::new();
    let mut module_package_ids = BTreeMap::new();
    for module_root in &config.module_roots {
        let go_mod_path = module_root_manifest_path(module_root, "go.mod");
        let Ok(contents) = read_repo_file_to_string_with_limit(
            &loaded.root,
            &go_mod_path,
            TOPOLOGY_MANIFEST_MAX_BYTES,
        ) else {
            continue;
        };
        let manifest = parse_go_mod(&go_mod_path, &contents);
        let Some(module_path) = manifest.module_path.clone() else {
            continue;
        };
        let id = TopologyPackageId(output.packages.len() as u64);
        output.packages.push(TopologyPackageFact {
            id,
            workspace_root: root_ids_by_path.get(module_root).copied(),
            package: None,
            module_node: None,
            kind: TopologyPackageKind::Workspace,
            name: module_path.clone(),
            version: None,
            path: module_root.clone(),
            language: Some(Language::Go),
            stable_key: format!("go-module:{module_root}:{module_path}"),
            producer_id: GO_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        });
        package_ids_by_import_path.insert(module_path.clone(), id);
        go_module_paths.push(module_path);
        module_package_ids.insert(module_root.clone(), id);
        emit_go_mod_requirements(
            &mut output,
            id,
            &go_mod_path,
            &manifest.requirements,
            &manifest.replacements,
            &manifest.excludes,
        );
        module_requirements.insert(module_root.clone(), manifest.requirements);
        module_replacements.insert(module_root.clone(), manifest.replacements);
    }

    for package in metadata.local_packages() {
        let id = TopologyPackageId(output.packages.len() as u64);
        let path = go_package_path(db, package);
        output.packages.push(TopologyPackageFact {
            id,
            workspace_root: package
                .module_path
                .as_deref()
                .and_then(|module_path| module_root_for_module_path(module_path, &config, &output))
                .and_then(|module_root| root_ids_by_path.get(module_root).copied()),
            package: None,
            module_node: None,
            kind: TopologyPackageKind::Package,
            name: package.import_path.clone(),
            version: package.module_version.clone(),
            path,
            language: Some(Language::Go),
            stable_key: format!("go-package:{}", package.import_path),
            producer_id: GO_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        });
        package_ids_by_import_path.insert(package.import_path.clone(), id);
    }

    let files_without_roots = config
        .files_without_module_root
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for file in lifecycle::go_files(db) {
        let (root, status) = if files_without_roots.contains(&file.relative_path) {
            (None, TopologyStatus::SetupMissing)
        } else {
            (
                module_root_for_file(&file.relative_path, &config.module_roots)
                    .and_then(|root| root_ids_by_path.get(root).copied()),
                TopologyStatus::Present,
            )
        };
        let package = metadata
            .file_to_import_path
            .get(&file.id)
            .and_then(|import_path| package_ids_by_import_path.get(import_path))
            .copied();
        let kind = classify_go_source_set(file);
        output.source_sets.push(SourceSetFact {
            id: SourceSetId(output.source_sets.len() as u64),
            package,
            root,
            kind,
            path: file.relative_path.clone(),
            language: Some(Language::Go),
            files: vec![file.id],
            stable_key: format!(
                "go-source-set:{}:{}",
                source_set_kind_label(kind),
                file.relative_path
            ),
            producer_id: GO_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::ExactStatic,
            status,
        });
    }

    emit_go_sum_edges(
        &mut output,
        &loaded.root,
        &config.module_roots,
        &module_requirements,
        &module_replacements,
        &module_package_ids,
        &go_module_paths,
    );

    output.normalized()
}

fn repository_root() -> WorkspaceRootFact {
    WorkspaceRootFact {
        id: WorkspaceRootId(0),
        kind: WorkspaceRootKind::Repository,
        root_path: ".".to_string(),
        manifest_path: None,
        language: None,
        stable_key: "repository:.".to_string(),
        producer_id: GO_TOPOLOGY_PROVIDER_ID,
        precision: TopologyPrecision::ExactStatic,
        status: TopologyStatus::Present,
    }
}

fn go_files_setup_missing(db: &AnalysisDb, root: Option<WorkspaceRootId>) -> Vec<SourceSetFact> {
    lifecycle::go_files(db)
        .into_iter()
        .enumerate()
        .map(|(index, file)| SourceSetFact {
            id: SourceSetId(index as u64),
            package: None,
            root,
            kind: classify_go_source_set(file),
            path: file.relative_path.clone(),
            language: Some(Language::Go),
            files: vec![file.id],
            stable_key: format!(
                "go-source-set:{}:{}",
                source_set_kind_label(classify_go_source_set(file)),
                file.relative_path
            ),
            producer_id: GO_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::Unknown,
            status: TopologyStatus::SetupMissing,
        })
        .collect()
}

fn emit_go_mod_requirements(
    output: &mut TopologyOutput,
    from_package: TopologyPackageId,
    manifest_path: &str,
    requirements: &[crate::module_graph::formats::go_mod::GoRequirementDirective],
    replacements: &[crate::module_graph::formats::go_mod::GoReplacementDirective],
    excludes: &[crate::module_graph::formats::go_mod::GoRequirementDirective],
) {
    for requirement in requirements {
        output
            .dependency_requirements
            .push(DependencyRequirementFact {
                id: DependencyRequirementId(output.dependency_requirements.len() as u64),
                from_package: Some(from_package),
                target_package: None,
                target_name: requirement.target_name.clone(),
                version_requirement: requirement.version_requirement.clone(),
                kind: RequirementKind::Direct,
                manifest_path: Some(manifest_path.to_string()),
                stable_key: format!(
                    "go-require:{manifest_path}:{}:{}",
                    requirement.target_name,
                    requirement.version_requirement.as_deref().unwrap_or("")
                ),
                producer_id: GO_TOPOLOGY_PROVIDER_ID,
                precision: requirement.precision,
                status: requirement.status,
            });
    }
    for replacement in replacements {
        output
            .dependency_requirements
            .push(DependencyRequirementFact {
                id: DependencyRequirementId(output.dependency_requirements.len() as u64),
                from_package: Some(from_package),
                target_package: None,
                target_name: replacement.target_name.clone(),
                version_requirement: replacement.version_requirement.clone(),
                kind: RequirementKind::Replace,
                manifest_path: Some(manifest_path.to_string()),
                stable_key: format!(
                    "go-replace:{manifest_path}:{}:{}=>{}:{}",
                    replacement.target_name,
                    replacement.version_requirement.as_deref().unwrap_or(""),
                    replacement.replacement_target,
                    replacement.replacement_version.as_deref().unwrap_or("")
                ),
                producer_id: GO_TOPOLOGY_PROVIDER_ID,
                precision: replacement.precision,
                status: replacement.status,
            });
    }
    for exclude in excludes {
        output
            .dependency_requirements
            .push(DependencyRequirementFact {
                id: DependencyRequirementId(output.dependency_requirements.len() as u64),
                from_package: Some(from_package),
                target_package: None,
                target_name: exclude.target_name.clone(),
                version_requirement: exclude.version_requirement.clone(),
                kind: RequirementKind::Exclude,
                manifest_path: Some(manifest_path.to_string()),
                stable_key: format!(
                    "go-exclude:{manifest_path}:{}:{}",
                    exclude.target_name,
                    exclude.version_requirement.as_deref().unwrap_or("")
                ),
                producer_id: GO_TOPOLOGY_PROVIDER_ID,
                precision: exclude.precision,
                status: exclude.status,
            });
    }
}

fn emit_go_sum_edges(
    output: &mut TopologyOutput,
    root: &Path,
    module_roots: &[String],
    module_requirements: &BTreeMap<
        String,
        Vec<crate::module_graph::formats::go_mod::GoRequirementDirective>,
    >,
    module_replacements: &BTreeMap<
        String,
        Vec<crate::module_graph::formats::go_mod::GoReplacementDirective>,
    >,
    module_package_ids: &BTreeMap<String, TopologyPackageId>,
    go_module_paths: &[String],
) {
    for module_root in module_roots {
        let go_sum_path = module_root_manifest_path(module_root, "go.sum");
        let source_label = "go.sum";
        let module_package = module_package_ids.get(module_root).copied();
        let requirements = module_requirements
            .get(module_root)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let replacements = module_replacements
            .get(module_root)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if let Ok(contents) =
            read_repo_file_to_string_with_limit(root, &go_sum_path, TOPOLOGY_LOCKFILE_MAX_BYTES)
        {
            for (line_index, line) in contents.lines().enumerate() {
                if let Some((package_name, resolved_version)) = parse_go_sum_line(line) {
                    output.resolved_dependency_edges.push(ResolvedDependencyEdgeFact {
                        id: ResolvedDependencyEdgeId(output.resolved_dependency_edges.len() as u64),
                        requirement: requirement_id_for(
                            output,
                            module_package,
                            &package_name,
                            Some(&resolved_version),
                        ),
                        from_package: module_package,
                        to_package: None,
                        package_name: package_name.clone(),
                        resolved_version: Some(resolved_version.clone()),
                        kind: ResolvedDependencyKind::ChecksumEvidence,
                        stable_key: format!(
                            "go-sum-line:{go_sum_path}:{line_index}:{package_name}:{resolved_version}:source_label={source_label}"
                        ),
                        producer_id: GO_TOPOLOGY_PROVIDER_ID,
                        precision: TopologyPrecision::ExactLockfile,
                        status: TopologyStatus::Resolved,
                    });
                }
            }
            continue;
        }

        for requirement in requirements.iter().filter(|requirement| {
            is_external_requirement(&requirement.target_name, go_module_paths)
                && !requirement_has_local_replacement(requirement, replacements)
        }) {
            let resolved_version = requirement.version_requirement.clone();
            output
                .resolved_dependency_edges
                .push(ResolvedDependencyEdgeFact {
                    id: ResolvedDependencyEdgeId(output.resolved_dependency_edges.len() as u64),
                    requirement: requirement_id_for(
                        output,
                        module_package,
                        &requirement.target_name,
                        resolved_version.as_deref(),
                    ),
                    from_package: module_package,
                    to_package: None,
                    package_name: requirement.target_name.clone(),
                    resolved_version,
                    kind: ResolvedDependencyKind::ChecksumEvidence,
                    stable_key: format!(
                        "go.sum:absent:{go_sum_path}:{}:{}:source_label={source_label}",
                        requirement.target_name,
                        requirement.version_requirement.as_deref().unwrap_or("")
                    ),
                    producer_id: GO_TOPOLOGY_PROVIDER_ID,
                    precision: TopologyPrecision::Unknown,
                    status: TopologyStatus::MissingLockfile,
                });
        }
    }
}

fn requirement_has_local_replacement(
    requirement: &crate::module_graph::formats::go_mod::GoRequirementDirective,
    replacements: &[crate::module_graph::formats::go_mod::GoReplacementDirective],
) -> bool {
    replacements.iter().any(|replacement| {
        replacement.target_name == requirement.target_name
            && (replacement.version_requirement.is_none()
                || replacement.version_requirement == requirement.version_requirement)
            && replacement.replacement_version.is_none()
            && is_local_go_replacement_target(&replacement.replacement_target)
    })
}

fn is_local_go_replacement_target(target: &str) -> bool {
    matches!(target, "." | "..")
        || target.starts_with("./")
        || target.starts_with("../")
        || Path::new(target).is_absolute()
}

fn parse_go_sum_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.split_whitespace();
    let package_name = parts.next()?.to_string();
    let version = parts.next()?.trim_end_matches("/go.mod").to_string();
    let _checksum = parts.next()?;
    Some((package_name, version))
}

fn requirement_id_for(
    output: &TopologyOutput,
    from_package: Option<TopologyPackageId>,
    target_name: &str,
    version_requirement: Option<&str>,
) -> Option<DependencyRequirementId> {
    output
        .dependency_requirements
        .iter()
        .find(|requirement| {
            requirement.from_package == from_package
                && requirement.target_name == target_name
                && requirement.version_requirement.as_deref() == version_requirement
        })
        .map(|requirement| requirement.id)
}

fn is_external_requirement(target_name: &str, go_module_paths: &[String]) -> bool {
    !go_module_paths
        .iter()
        .any(|module_path| import_is_within_module(target_name, module_path))
}

fn module_root_manifest_path(module_root: &str, manifest_name: &str) -> String {
    if module_root == "." {
        manifest_name.to_string()
    } else {
        format!("{module_root}/{manifest_name}")
    }
}

fn module_root_for_file<'a>(relative_path: &str, module_roots: &'a [String]) -> Option<&'a str> {
    module_roots
        .iter()
        .filter(|module_root| file_is_under_module_root(relative_path, module_root))
        .max_by_key(|module_root| module_root.len())
        .map(String::as_str)
}

fn file_is_under_module_root(relative_path: &str, module_root: &str) -> bool {
    module_root == "."
        || relative_path == module_root
        || relative_path
            .strip_prefix(module_root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn module_root_for_module_path<'a>(
    module_path: &str,
    config: &'a GoAnalysisConfig,
    output: &TopologyOutput,
) -> Option<&'a str> {
    output
        .packages
        .iter()
        .find(|package| {
            package.kind == TopologyPackageKind::Workspace && package.name == module_path
        })
        .and_then(|package| {
            config
                .module_roots
                .iter()
                .find(|module_root| package.path == module_root.as_str())
        })
        .map(String::as_str)
}

fn go_package_path(db: &AnalysisDb, package: &GoPackageMetadata) -> String {
    package
        .files
        .iter()
        .filter_map(|file| db.files().iter().find(|source| source.id == *file))
        .filter_map(|file| parent_path(&file.relative_path))
        .next()
        .unwrap_or_else(|| ".".to_string())
}

fn parent_path(relative_path: &str) -> Option<String> {
    relative_path
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .or_else(|| Some(".".to_string()))
}

fn classify_go_source_set(file: &crate::core::SourceFile) -> SourceSetKind {
    if path_contains_vendor(&file.relative_path) {
        return SourceSetKind::Vendor;
    }
    if file.relative_path.ends_with("_test.go") {
        return SourceSetKind::Test;
    }
    if file.relative_path.ends_with(".pb.go") || first_comment_is_generated(&file.source) {
        return SourceSetKind::Generated;
    }
    SourceSetKind::Source
}

fn path_contains_vendor(relative_path: &str) -> bool {
    relative_path == "vendor"
        || relative_path.starts_with("vendor/")
        || relative_path.contains("/vendor/")
}

fn first_comment_is_generated(source: &str) -> bool {
    source
        .lines()
        .map(str::trim_start)
        .find(|line| !line.is_empty())
        .is_some_and(|line| {
            line.starts_with("// Code generated") || line.starts_with("/* Code generated")
        })
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

#[derive(Debug, Clone)]
pub(crate) struct GoPackageMetadata {
    import_path: String,
    #[allow(dead_code)]
    name: Option<String>,
    files: Vec<FileId>,
    standard: bool,
    module_path: Option<String>,
    #[allow(dead_code)]
    module_version: Option<String>,
}

impl GoPackageMetadata {
    fn from_go_list_package(
        root: &Path,
        file_ids: &BTreeMap<String, FileId>,
        package: GoListPackage,
    ) -> Self {
        let files = mapped_package_files(root, file_ids, &package)
            .into_iter()
            .collect();
        let module_path = package
            .module
            .as_ref()
            .and_then(|module| module.path.clone());
        let module_version = package
            .module
            .as_ref()
            .and_then(|module| module.version.clone());
        Self {
            import_path: package.import_path,
            name: package.name,
            files,
            standard: package.standard,
            module_path,
            module_version,
        }
    }

    pub(crate) fn import_path(&self) -> &str {
        &self.import_path
    }

    pub(crate) fn files(&self) -> impl Iterator<Item = FileId> + '_ {
        self.files.iter().copied()
    }
}

#[derive(Debug, Deserialize)]
struct GoListPackage {
    #[serde(rename = "ImportPath", default)]
    import_path: String,
    #[serde(rename = "Name", default)]
    name: Option<String>,
    #[serde(rename = "Dir", default)]
    dir: Option<PathBuf>,
    #[serde(rename = "GoFiles", default)]
    go_files: Vec<PathBuf>,
    #[serde(rename = "TestGoFiles", default)]
    test_go_files: Vec<PathBuf>,
    #[serde(rename = "CompiledGoFiles", default)]
    compiled_go_files: Vec<PathBuf>,
    #[serde(rename = "Standard", default)]
    standard: bool,
    #[serde(rename = "Module", default)]
    module: Option<GoListModule>,
}

#[derive(Debug, Deserialize)]
struct GoListModule {
    #[serde(rename = "Path", default)]
    path: Option<String>,
    #[serde(rename = "Version", default)]
    version: Option<String>,
}

pub(crate) struct GoCommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl From<std::process::Output> for GoCommandOutput {
    fn from(output: std::process::Output) -> Self {
        Self {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }
}

fn run_go_list(root: &Path, config: &GoAnalysisConfig) -> GoCommandOutput {
    let (mut command, _workspace) = match lifecycle::command_with_go_env(root, &config.module_roots)
    {
        Ok(command) => command,
        Err(error) => {
            return GoCommandOutput {
                status: failed_exit_status(),
                stdout: Vec::new(),
                stderr: error.reason().as_bytes().to_vec(),
            };
        }
    };
    command.args(["list", "-mod=readonly", "-json"]);
    if !config.build_tags.is_empty() {
        command.arg(format!("-tags={}", config.build_tags.join(",")));
    }
    command
        .args(config.rooted_package_patterns())
        .output()
        .map(Into::into)
        .unwrap_or_else(|error| GoCommandOutput {
            status: failed_exit_status(),
            stdout: Vec::new(),
            stderr: error.to_string().into_bytes(),
        })
}

fn mapped_package_files(
    root: &Path,
    file_ids: &BTreeMap<String, FileId>,
    package: &GoListPackage,
) -> BTreeSet<FileId> {
    let Some(dir) = &package.dir else {
        return BTreeSet::new();
    };
    package
        .go_files
        .iter()
        .chain(package.test_go_files.iter())
        .chain(package.compiled_go_files.iter())
        .filter_map(|entry| map_package_file(root, dir, entry, file_ids))
        .collect()
}

fn map_package_file(
    root: &Path,
    dir: &Path,
    entry: &Path,
    file_ids: &BTreeMap<String, FileId>,
) -> Option<FileId> {
    let absolute_dir = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        root.join(dir)
    };
    let absolute_path = if entry.is_absolute() {
        entry.to_path_buf()
    } else {
        absolute_dir.join(entry)
    };
    let relative_path = paths::normalize_repo_relative_path(root, &absolute_path)?;
    file_ids.get(&relative_path).copied()
}

fn go_list_failure_reason(output: &GoCommandOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        format!("go list -json ./... failed: {}", output.status)
    } else {
        format!("go list -json ./... failed: {stderr}")
    }
}

fn failed_exit_status() -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(1 << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(1)
    }
}

pub(crate) fn resolve_go_import(
    input: ResolverInput<'_>,
    metadata: &GoPackageIndex,
) -> ResolvedImportDraft {
    let _ = (input.root, input.db.files().len(), input.owner_module);
    if input.import.language != Language::Go {
        return ResolvedImportDraft::unsupported_language();
    }
    if metadata.is_setup_missing() {
        return ResolvedImportDraft::setup_missing();
    }

    if let Some(package) = metadata.package(&input.import.path) {
        if metadata.is_local_package(package) {
            return ResolvedImportDraft {
                target: Some(ModuleNodeDraft::package(
                    package.import_path(),
                    None,
                    Some(Language::Go),
                )),
                status: ResolutionStatus::Resolved,
                precision: ResolutionPrecision::Package,
                reason: None,
                edge_kind: Some(ModuleEdgeKind::DependsOn),
            };
        }
        return external_draft(input.import.path.clone());
    }

    if metadata.import_is_external_dependency(&input.import.path) {
        return external_draft(input.import.path.clone());
    }

    ResolvedImportDraft::unresolved(UnresolvedReason::NotFound)
}

fn external_draft(label: String) -> ResolvedImportDraft {
    ResolvedImportDraft {
        target: Some(ModuleNodeDraft::external(label, Some(Language::Go))),
        status: ResolutionStatus::External,
        precision: ResolutionPrecision::ExternalPackage,
        reason: None,
        edge_kind: Some(ModuleEdgeKind::DependsOn),
    }
}

fn import_is_within_module(import_path: &str, module_path: &str) -> bool {
    import_path == module_path
        || import_path
            .strip_prefix(module_path)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn is_go_stdlib_import_path(import_path: &str) -> bool {
    import_path
        .split('/')
        .next()
        .is_some_and(|first| !first.contains('.') && !first.is_empty())
}

#[cfg(test)]
mod topology_monorepo {
    use super::*;
    use crate::config::load_config;
    use crate::module_graph::topology::{SourceSetKind, TopologyStatus, WorkspaceRootKind};
    use std::path::Path;

    #[test]
    fn topology_monorepo_infers_go_module_roots_without_repo_go_mod() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("services/api")).expect("mkdir api");
        std::fs::create_dir_all(temp.path().join("libs/common")).expect("mkdir common");
        std::fs::write(
            temp.path().join("services/api/go.mod"),
            "module example.com/api\n\ngo 1.24\n",
        )
        .expect("write api go.mod");
        std::fs::write(
            temp.path().join("libs/common/go.mod"),
            "module example.com/common\n\ngo 1.24\n",
        )
        .expect("write common go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(
            &mut db,
            temp.path(),
            "services/api/main.go",
            "package api\n",
        );
        add_go_file(
            &mut db,
            temp.path(),
            "libs/common/common.go",
            "package common\n",
        );
        let metadata = go_index_with_module_path(
            temp.path(),
            &db,
            "example.com/api",
            &[("example.com/api", "services/api", &["main.go"], false)],
        );
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_go_topology(&loaded, &db, &metadata);
        let module_roots = output
            .workspace_roots
            .iter()
            .filter(|root| root.kind == WorkspaceRootKind::GoModule)
            .map(|root| (root.root_path.as_str(), root.status))
            .collect::<Vec<_>>();

        assert_eq!(
            module_roots,
            vec![
                ("libs/common", TopologyStatus::Present),
                ("services/api", TopologyStatus::Present),
            ]
        );
    }

    #[test]
    fn topology_monorepo_configured_module_roots_win_over_nearest_discovery() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("services/api")).expect("mkdir api");
        std::fs::create_dir_all(temp.path().join("libs/common")).expect("mkdir common");
        std::fs::write(
            temp.path().join(".polint.toml"),
            "[languages.go]\nmodule_roots = [\"services/api\"]\n",
        )
        .expect("write config");
        std::fs::write(
            temp.path().join("services/api/go.mod"),
            "module example.com/api\n\ngo 1.24\n",
        )
        .expect("write api go.mod");
        std::fs::write(
            temp.path().join("libs/common/go.mod"),
            "module example.com/common\n\ngo 1.24\n",
        )
        .expect("write common go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(
            &mut db,
            temp.path(),
            "services/api/main.go",
            "package api\n",
        );
        let outside = add_go_file(
            &mut db,
            temp.path(),
            "libs/common/common.go",
            "package common\n",
        );
        let metadata = GoPackageIndex::default();
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_go_topology(&loaded, &db, &metadata);
        let module_roots = output
            .workspace_roots
            .iter()
            .filter(|root| root.kind == WorkspaceRootKind::GoModule)
            .map(|root| root.root_path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(module_roots, vec!["services/api"]);
        assert!(output.source_sets.iter().any(|source_set| {
            source_set.files == vec![outside] && source_set.status == TopologyStatus::SetupMissing
        }));
    }

    #[test]
    fn topology_source_sets_classify_tests_vendor_and_generated_go_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24\n",
        )
        .expect("write go.mod");
        let mut db = AnalysisDb::new();
        let source = add_go_file(&mut db, temp.path(), "main.go", "package app\n");
        let test = add_go_file(&mut db, temp.path(), "main_test.go", "package app\n");
        let proto = add_go_file(&mut db, temp.path(), "api.pb.go", "package app\n");
        let generated = add_go_file(
            &mut db,
            temp.path(),
            "generated.go",
            "// Code generated by tool. DO NOT EDIT.\npackage app\n",
        );
        let vendor = add_go_file(
            &mut db,
            temp.path(),
            "vendor/github.com/acme/lib/lib.go",
            "package lib\n",
        );
        let metadata = go_index(
            temp.path(),
            &db,
            &[(
                "example.com/app",
                ".",
                &["main.go", "main_test.go", "api.pb.go", "generated.go"],
                false,
            )],
        );
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_go_topology(&loaded, &db, &metadata);

        assert!(source_set_for_file(&output, source, SourceSetKind::Source));
        assert!(source_set_for_file(&output, test, SourceSetKind::Test));
        assert!(source_set_for_file(
            &output,
            proto,
            SourceSetKind::Generated
        ));
        assert!(source_set_for_file(
            &output,
            generated,
            SourceSetKind::Generated
        ));
        assert!(source_set_for_file(&output, vendor, SourceSetKind::Vendor));
    }

    fn add_go_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("test fixture has parent"))
            .expect("create parent dirs");
        std::fs::write(&path, source).expect("write fixture");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn go_index(
        root: &Path,
        db: &AnalysisDb,
        packages: &[(&str, &str, &[&str], bool)],
    ) -> GoPackageIndex {
        go_index_with_module_path(root, db, "example.com/app", packages)
    }

    fn go_index_with_module_path(
        root: &Path,
        db: &AnalysisDb,
        module_path: &str,
        packages: &[(&str, &str, &[&str], bool)],
    ) -> GoPackageIndex {
        let stdout = packages
            .iter()
            .map(|(import_path, dir, files, standard)| {
                let dir = root.join(dir);
                let files = files
                    .iter()
                    .map(|file| serde_json::to_string(file).expect("serialize file"))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    r#"{{"ImportPath":{},"Name":"pkg","Dir":{},"GoFiles":[{}],"Standard":{},"Module":{{"Path":{}}}}}"#,
                    serde_json::to_string(import_path).expect("serialize import path"),
                    serde_json::to_string(&dir.to_string_lossy()).expect("serialize dir"),
                    files,
                    standard,
                    serde_json::to_string(module_path).expect("serialize module path")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        GoPackageIndex::load_with_runner_for_test(root, db, || GoCommandOutput {
            status: successful_status(),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        })
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

    fn successful_status() -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
    }
}

#[cfg(test)]
mod dependency_topology {
    use super::*;
    use crate::config::load_config;
    use crate::module_graph::topology::{
        RequirementKind, ResolvedDependencyKind, TopologyPrecision, TopologyStatus,
    };
    use std::path::Path;

    #[test]
    fn dependency_topology_emits_direct_requirements_from_go_mod() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24\n\nrequire github.com/acme/lib v1.2.3\n",
        )
        .expect("write go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package app\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_go_topology(&loaded, &db, &GoPackageIndex::default());

        assert!(output.dependency_requirements.iter().any(|requirement| {
            requirement.target_name == "github.com/acme/lib"
                && requirement.version_requirement.as_deref() == Some("v1.2.3")
                && requirement.kind == RequirementKind::Direct
                && requirement.manifest_path.as_deref() == Some("go.mod")
                && requirement.precision == TopologyPrecision::ExactStatic
        }));
    }

    #[test]
    fn dependency_topology_emits_go_sum_checksum_evidence() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24\n\nrequire github.com/acme/lib v1.2.3\n",
        )
        .expect("write go.mod");
        std::fs::write(
            temp.path().join("go.sum"),
            "github.com/acme/lib v1.2.3 h1:abc\ngithub.com/acme/lib v1.2.3/go.mod h1:def\n",
        )
        .expect("write go.sum");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package app\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_go_topology(&loaded, &db, &GoPackageIndex::default());

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "github.com/acme/lib"
                && edge.resolved_version.as_deref() == Some("v1.2.3")
                && edge.kind == ResolvedDependencyKind::ChecksumEvidence
                && edge.precision == TopologyPrecision::ExactLockfile
                && edge.status == TopologyStatus::Resolved
                && edge.stable_key.contains("go-sum-line")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn dependency_topology_does_not_read_go_sum_symlink_escape() {
        let repo = tempfile::tempdir().expect("repo tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        std::fs::write(
            repo.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24\n\nrequire github.com/acme/lib v1.2.3\n",
        )
        .expect("write go.mod");
        std::fs::write(
            outside.path().join("go.sum"),
            "github.com/acme/lib v1.2.3 h1:abc\n",
        )
        .expect("write outside go.sum");
        std::os::unix::fs::symlink(outside.path().join("go.sum"), repo.path().join("go.sum"))
            .expect("create symlink");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, repo.path(), "main.go", "package app\n");
        let loaded = load_config(repo.path()).expect("config loads");

        let output = collect_go_topology(&loaded, &db, &GoPackageIndex::default());

        assert!(!output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "github.com/acme/lib"
                && edge.kind == ResolvedDependencyKind::ChecksumEvidence
                && edge.status == TopologyStatus::Resolved
        }));
        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "github.com/acme/lib"
                && edge.kind == ResolvedDependencyKind::ChecksumEvidence
                && edge.status == TopologyStatus::MissingLockfile
        }));
    }

    #[test]
    fn dependency_topology_scopes_go_sum_edges_to_module_package() {
        let temp = tempfile::tempdir().expect("tempdir");
        for module_root in ["services/api", "services/worker"] {
            std::fs::create_dir_all(temp.path().join(module_root)).expect("mkdir module");
            std::fs::write(
                temp.path().join(module_root).join("go.mod"),
                format!(
                    "module example.com/{}\n\ngo 1.24\n\nrequire github.com/acme/lib v1.2.3\n",
                    module_root
                        .rsplit('/')
                        .next()
                        .expect("module root has name")
                ),
            )
            .expect("write go.mod");
            std::fs::write(
                temp.path().join(module_root).join("go.sum"),
                "github.com/acme/lib v1.2.3 h1:abc\n",
            )
            .expect("write go.sum");
        }
        let mut db = AnalysisDb::new();
        add_go_file(
            &mut db,
            temp.path(),
            "services/api/main.go",
            "package api\n",
        );
        add_go_file(
            &mut db,
            temp.path(),
            "services/worker/main.go",
            "package worker\n",
        );
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_go_topology(&loaded, &db, &GoPackageIndex::default());
        let api_package = output
            .packages
            .iter()
            .find(|package| package.path == "services/api")
            .expect("api module package exists");
        let worker_package = output
            .packages
            .iter()
            .find(|package| package.path == "services/worker")
            .expect("worker module package exists");
        let api_requirement = output
            .dependency_requirements
            .iter()
            .find(|requirement| {
                requirement.from_package == Some(api_package.id)
                    && requirement.target_name == "github.com/acme/lib"
                    && requirement.version_requirement.as_deref() == Some("v1.2.3")
            })
            .expect("api requirement exists");
        let worker_requirement = output
            .dependency_requirements
            .iter()
            .find(|requirement| {
                requirement.from_package == Some(worker_package.id)
                    && requirement.target_name == "github.com/acme/lib"
                    && requirement.version_requirement.as_deref() == Some("v1.2.3")
            })
            .expect("worker requirement exists");

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.from_package == Some(api_package.id)
                && edge.requirement == Some(api_requirement.id)
                && edge.package_name == "github.com/acme/lib"
                && edge.stable_key.contains("services/api/go.sum")
        }));
        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.from_package == Some(worker_package.id)
                && edge.requirement == Some(worker_requirement.id)
                && edge.package_name == "github.com/acme/lib"
                && edge.stable_key.contains("services/worker/go.sum")
        }));
    }

    #[test]
    fn dependency_topology_marks_external_requirements_missing_lockfile_when_go_sum_absent() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24\n\nrequire github.com/acme/lib v1.2.3\n",
        )
        .expect("write go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package app\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_go_topology(&loaded, &db, &GoPackageIndex::default());

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "github.com/acme/lib"
                && edge.resolved_version.as_deref() == Some("v1.2.3")
                && edge.status == TopologyStatus::MissingLockfile
                && edge.precision == TopologyPrecision::Unknown
                && edge.stable_key.contains("go.sum:absent")
        }));
    }

    #[test]
    fn dependency_topology_does_not_mark_local_replacements_missing_go_sum() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24\n\nrequire github.com/acme/lib v1.2.3\n\nreplace github.com/acme/lib => ../lib\n",
        )
        .expect("write go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package app\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_go_topology(&loaded, &db, &GoPackageIndex::default());

        assert!(!output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "github.com/acme/lib"
                && edge.kind == ResolvedDependencyKind::ChecksumEvidence
                && edge.status == TopologyStatus::MissingLockfile
                && edge.stable_key.contains("go.sum:absent")
        }));
    }

    fn add_go_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("test fixture has parent"))
            .expect("create parent dirs");
        std::fs::write(&path, source).expect("write fixture");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{GoCommandOutput, GoPackageIndex, resolve_go_import, seed_go_module_nodes};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, Capabilities, CapabilitySupportStatus, FileId, ImportFact, ImportId, Language,
        ModuleEdgeKind, ModuleNodeId, ModuleNodeKind, ResolutionPrecision, ResolutionStatus,
        ResolvedImportId, Rule, RuleMeta, RuleOptions, Span, UnresolvedReason,
        run_rules_with_capability_support,
    };
    use crate::diagnostics::{Diagnostic, Severity, TextRange};
    use crate::module_graph::derive_requested_module_graph;
    use crate::module_graph::model::{ModuleGraphBuilder, ResolverInput};
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::process::ExitStatus;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn module_graph_resolver_contracts_go_without_metadata_is_setup_missing() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("main.go"),
            "main.go".to_string(),
            "package main\nimport \"example.com/project/pkg\"\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "example.com/project/pkg".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::Go,
        });
        let import = &db.imports()[0];

        let draft = resolve_go_import(
            ResolverInput {
                root: Path::new("."),
                db: &db,
                import,
                ts_resolver: None,
                owner_module: None,
                owner_package: None,
            },
            &GoPackageIndex::default(),
        );

        assert_eq!(draft.status, ResolutionStatus::SetupMissing);
        assert_eq!(draft.reason, Some(UnresolvedReason::SetupMissing));
    }

    #[test]
    fn module_graph_go_metadata_missing_module_root_is_setup_missing_without_running_go() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package main\n");
        let command_ran = Cell::new(false);

        let index = GoPackageIndex::load_with_runner_for_test(temp.path(), &db, || {
            command_ran.set(true);
            successful_go_output(b"{}")
        });

        assert!(!command_ran.get());
        assert_eq!(
            index.setup_missing_reason(),
            Some("some Go files are not under a go.mod module root.")
        );
    }

    #[test]
    fn module_graph_go_metadata_nonzero_go_list_is_setup_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n")
            .expect("write go.mod");
        let db = AnalysisDb::new();

        let index = GoPackageIndex::load_with_runner_for_test(temp.path(), &db, || {
            failed_go_output(b"package load failed")
        });

        assert_eq!(
            index.setup_missing_reason(),
            Some("go list -json ./... failed: package load failed")
        );
    }

    #[test]
    fn module_graph_go_metadata_parses_json_stream_sorted_by_import_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n")
            .expect("write go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "internal/z/z.go", "package z\n");
        add_go_file(&mut db, temp.path(), "internal/a/a.go", "package a\n");
        let dir_z = temp.path().join("internal/z");
        let dir_a = temp.path().join("internal/a");
        let stdout = format!(
            r#"{{"ImportPath":"example.com/app/internal/z","Name":"z","Dir":{},"GoFiles":["z.go"],"Standard":false,"Module":{{"Path":"example.com/app"}}}}
{{"ImportPath":"example.com/app/internal/a","Name":"a","Dir":{},"GoFiles":["a.go"],"Standard":false,"Module":{{"Path":"example.com/app"}}}}
"#,
            serde_json::to_string(&dir_z.to_string_lossy()).expect("serialize dir"),
            serde_json::to_string(&dir_a.to_string_lossy()).expect("serialize dir"),
        );

        let index = GoPackageIndex::load_with_runner_for_test(temp.path(), &db, || {
            successful_go_output(stdout.as_bytes())
        });

        assert_eq!(
            index.import_paths().collect::<Vec<_>>(),
            vec!["example.com/app/internal/a", "example.com/app/internal/z"]
        );
        assert_eq!(
            index.module_path().map(String::as_str),
            Some("example.com/app")
        );
    }

    #[test]
    fn module_graph_go_metadata_maps_go_files_to_analysis_file_ids() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n")
            .expect("write go.mod");
        let mut db = AnalysisDb::new();
        let source = add_go_file(
            &mut db,
            temp.path(),
            "internal/worker/worker.go",
            "package worker\n",
        );
        let test = add_go_file(
            &mut db,
            temp.path(),
            "internal/worker/worker_test.go",
            "package worker\n",
        );
        let ignored = temp.path().join("internal/worker/generated.go");
        let dir = temp.path().join("internal/worker");
        let stdout = format!(
            r#"{{"ImportPath":"example.com/app/internal/worker","Name":"worker","Dir":{},"GoFiles":["worker.go"],"TestGoFiles":["worker_test.go"],"CompiledGoFiles":["generated.go"],"Standard":false,"Module":{{"Path":"example.com/app"}}}}
"#,
            serde_json::to_string(&dir.to_string_lossy()).expect("serialize dir"),
        );
        std::fs::write(ignored, "package worker\n").expect("write ignored fixture");

        let index = GoPackageIndex::load_with_runner_for_test(temp.path(), &db, || {
            successful_go_output(stdout.as_bytes())
        });
        let package = index
            .package("example.com/app/internal/worker")
            .expect("package metadata exists");

        assert_eq!(package.files().collect::<Vec<_>>(), vec![source, test]);
    }

    #[test]
    fn module_graph_go_metadata_loads_monorepo_submodule_without_repo_go_mod() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("services/app")).expect("mkdir service");
        std::fs::write(
            temp.path().join("services/app/go.mod"),
            "module example.com/app\n\ngo 1.24\n",
        )
        .expect("write go.mod");
        let mut db = AnalysisDb::new();
        let main = add_go_file(
            &mut db,
            temp.path(),
            "services/app/main.go",
            "package app\n",
        );
        let dir = temp.path().join("services/app");
        let stdout = format!(
            r#"{{"ImportPath":"example.com/app","Name":"app","Dir":{},"GoFiles":["main.go"],"Standard":false,"Module":{{"Path":"example.com/app"}}}}
"#,
            serde_json::to_string(&dir.to_string_lossy()).expect("serialize dir"),
        );
        let command_ran = Cell::new(false);

        let index = GoPackageIndex::load_with_runner_for_test(temp.path(), &db, || {
            command_ran.set(true);
            successful_go_output(stdout.as_bytes())
        });

        assert!(command_ran.get());
        assert_eq!(index.setup_missing_reason(), None);
        let package = index.package("example.com/app").expect("package exists");
        assert_eq!(package.files().collect::<Vec<_>>(), vec![main]);
    }

    #[test]
    fn module_graph_go_resolution_resolves_local_import_to_package_node_and_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n")
            .expect("write go.mod");
        let mut db = AnalysisDb::new();
        let main = add_go_file(&mut db, temp.path(), "cmd/app/main.go", "package main\n");
        let worker = add_go_file(
            &mut db,
            temp.path(),
            "internal/worker/worker.go",
            "package worker\n",
        );
        push_go_import(&mut db, main, "example.com/app/internal/worker");
        let index = go_index(
            temp.path(),
            &db,
            &[
                ("example.com/app/cmd/app", "cmd/app", &["main.go"], false),
                (
                    "example.com/app/internal/worker",
                    "internal/worker",
                    &["worker.go"],
                    false,
                ),
            ],
        );
        let mut builder = ModuleGraphBuilder::new(&db);
        let ownership = seed_go_module_nodes(&mut builder, &index);
        let owner = ownership
            .package_node_for_file(main)
            .expect("main package owner exists");

        let draft = resolve_go_import(
            ResolverInput {
                root: temp.path(),
                db: &db,
                import: &db.imports()[0],
                ts_resolver: None,
                owner_module: ownership.module_node_for_file(main),
                owner_package: Some(owner),
            },
            &index,
        );
        let fact = builder.apply_resolved_import_draft_with_id(
            &db.imports()[0],
            owner,
            draft,
            ResolvedImportId(0),
        );
        let output = builder.finish();
        let module = node_by_label(&output.nodes, "example.com/app");
        let worker_package = node_by_label(&output.nodes, "example.com/app/internal/worker");
        let worker_file = file_node(&output.nodes, worker);

        assert_eq!(fact.status, ResolutionStatus::Resolved);
        assert_eq!(fact.precision, ResolutionPrecision::Package);
        assert_eq!(fact.target_node, Some(worker_package));
        assert!(output.edges.iter().any(|edge| {
            edge.kind == ModuleEdgeKind::Contains
                && edge.from == module
                && edge.to == worker_package
        }));
        assert!(output.edges.iter().any(|edge| {
            edge.kind == ModuleEdgeKind::Contains
                && edge.from == worker_package
                && edge.to == worker_file
        }));
        assert!(output.edges.iter().any(|edge| {
            edge.kind == ModuleEdgeKind::DependsOn
                && edge.from == owner
                && edge.to == worker_package
                && edge.status == ResolutionStatus::Resolved
        }));
    }

    #[test]
    fn module_graph_go_resolution_classifies_stdlib_import_as_external() {
        let (db, index) = db_and_index_for_import("fmt");

        let draft = resolve_go_import(
            ResolverInput {
                root: Path::new("."),
                db: &db,
                import: &db.imports()[0],
                ts_resolver: None,
                owner_module: None,
                owner_package: None,
            },
            &index,
        );

        assert_eq!(draft.status, ResolutionStatus::External);
        assert_eq!(draft.precision, ResolutionPrecision::ExternalPackage);
    }

    #[test]
    fn module_graph_go_resolution_classifies_dependency_import_as_external() {
        let (db, index) = db_and_index_for_import("github.com/acme/lib");

        let draft = resolve_go_import(
            ResolverInput {
                root: Path::new("."),
                db: &db,
                import: &db.imports()[0],
                ts_resolver: None,
                owner_module: None,
                owner_package: None,
            },
            &index,
        );

        assert_eq!(draft.status, ResolutionStatus::External);
        assert_eq!(draft.precision, ResolutionPrecision::ExternalPackage);
    }

    #[test]
    fn module_graph_go_resolution_keeps_unknown_local_import_unresolved_not_found() {
        let (db, index) = db_and_index_for_import("example.com/app/internal/missing");

        let draft = resolve_go_import(
            ResolverInput {
                root: Path::new("."),
                db: &db,
                import: &db.imports()[0],
                ts_resolver: None,
                owner_module: None,
                owner_package: None,
            },
            &index,
        );

        assert_eq!(draft.status, ResolutionStatus::Unresolved);
        assert_eq!(draft.precision, ResolutionPrecision::None);
        assert_eq!(draft.reason, Some(UnresolvedReason::NotFound));
    }

    #[test]
    fn module_graph_go_resolution_keeps_dotless_module_missing_local_import_unresolved_not_found() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module mycorp/app\n").expect("write go.mod");
        let mut db = AnalysisDb::new();
        let main = add_go_file(&mut db, temp.path(), "cmd/app/main.go", "package main\n");
        push_go_import(&mut db, main, "mycorp/app/internal/missing");
        let index = go_index_with_module_path(
            temp.path(),
            &db,
            "mycorp/app",
            &[("mycorp/app/cmd/app", "cmd/app", &["main.go"], false)],
        );

        let draft = resolve_go_import(
            ResolverInput {
                root: Path::new("."),
                db: &db,
                import: &db.imports()[0],
                ts_resolver: None,
                owner_module: None,
                owner_package: None,
            },
            &index,
        );

        assert_eq!(draft.status, ResolutionStatus::Unresolved);
        assert_eq!(draft.precision, ResolutionPrecision::None);
        assert_eq!(draft.reason, Some(UnresolvedReason::NotFound));
    }

    #[test]
    fn module_graph_go_setup_missing_blocks_requesting_rule_execution() {
        let temp = tempfile::tempdir().expect("tempdir");
        let loaded = load_config(temp.path()).expect("default config loads");
        let mut db = AnalysisDb::new();
        let file = add_go_file(&mut db, temp.path(), "main.go", "package main\n");
        push_go_import(&mut db, file, "example.com/app/internal/worker");
        let plan = AnalysisPlan::from_capability_names_for_test(&["resolved_imports"]);

        let derivation = derive_requested_module_graph(&mut db, &loaded, &plan);

        assert_eq!(derivation.diagnostics.len(), 1);
        let diagnostic = &derivation.diagnostics[0];
        assert_eq!(diagnostic.rule_id, "polint/capability");
        assert!(diagnostic.evidence.iter().any(|evidence| {
            evidence.label == "capability" && evidence.value == "resolved_imports"
        }));
        assert!(
            diagnostic.evidence.iter().any(|evidence| {
                evidence.label == "status" && evidence.value == "setup_missing"
            })
        );
        assert_eq!(
            derivation.capability_support[0].status,
            CapabilitySupportStatus::SetupMissing
        );
        assert_eq!(
            derivation.capability_support[0].reason.as_deref(),
            Some("some Go files are not under a go.mod module root.")
        );
        assert_eq!(db.resolved_imports().len(), 1);
        let fact = &db.resolved_imports()[0];
        assert_eq!(fact.status, ResolutionStatus::SetupMissing);
        assert_eq!(fact.precision, ResolutionPrecision::None);
        assert_eq!(fact.reason, Some(UnresolvedReason::SetupMissing));
        assert_eq!(fact.target_node, None);

        let ran = Arc::new(AtomicBool::new(false));
        let ran_for_rule = Arc::clone(&ran);
        let rule = Rule::from_parts(
            || RuleMeta {
                id: "test/requested-capabilities".to_string(),
                description: "Resolved import requester".to_string(),
                severity: Severity::Warn,
            },
            || Capabilities::new().resolved_imports(),
            move |_db, ctx| {
                ran_for_rule.store(true, Ordering::SeqCst);
                ctx.report(Diagnostic::warning(
                    ctx.rule_id(),
                    "<workspace>",
                    TextRange::point(1, 1),
                    "rule should not run",
                ));
                Ok(())
            },
        );
        let capability_support = derivation.support_view(plan.support_view());
        let rule_diagnostics = run_rules_with_capability_support(
            &db,
            &[rule],
            &BTreeMap::<String, RuleOptions>::new(),
            None,
            false,
            &capability_support,
        );

        assert!(!ran.load(Ordering::SeqCst));
        assert!(rule_diagnostics.is_empty());
    }

    mod topology {
        use super::*;
        use crate::module_graph::go::collect_go_topology;
        use crate::module_graph::topology::{SourceSetKind, TopologyStatus, WorkspaceRootKind};

        #[test]
        fn topology_monorepo_infers_go_module_roots_without_repo_go_mod() {
            let temp = tempfile::tempdir().expect("tempdir");
            std::fs::create_dir_all(temp.path().join("services/api")).expect("mkdir api");
            std::fs::create_dir_all(temp.path().join("libs/common")).expect("mkdir common");
            std::fs::write(
                temp.path().join("services/api/go.mod"),
                "module example.com/api\n\ngo 1.24\n",
            )
            .expect("write api go.mod");
            std::fs::write(
                temp.path().join("libs/common/go.mod"),
                "module example.com/common\n\ngo 1.24\n",
            )
            .expect("write common go.mod");
            let mut db = AnalysisDb::new();
            add_go_file(
                &mut db,
                temp.path(),
                "services/api/main.go",
                "package api\n",
            );
            add_go_file(
                &mut db,
                temp.path(),
                "libs/common/common.go",
                "package common\n",
            );
            let metadata = go_index_with_module_path(
                temp.path(),
                &db,
                "example.com/api",
                &[("example.com/api", "services/api", &["main.go"], false)],
            );
            let loaded = load_config(temp.path()).expect("config loads");

            let output = collect_go_topology(&loaded, &db, &metadata);
            let module_roots = output
                .workspace_roots
                .iter()
                .filter(|root| root.kind == WorkspaceRootKind::GoModule)
                .map(|root| (root.root_path.as_str(), root.status))
                .collect::<Vec<_>>();

            assert_eq!(
                module_roots,
                vec![
                    ("libs/common", TopologyStatus::Present),
                    ("services/api", TopologyStatus::Present),
                ]
            );
        }

        #[test]
        fn topology_monorepo_configured_module_roots_win_over_nearest_discovery() {
            let temp = tempfile::tempdir().expect("tempdir");
            std::fs::create_dir_all(temp.path().join("services/api")).expect("mkdir api");
            std::fs::create_dir_all(temp.path().join("libs/common")).expect("mkdir common");
            std::fs::write(
                temp.path().join(".polint.toml"),
                "[languages.go]\nmodule_roots = [\"services/api\"]\n",
            )
            .expect("write config");
            std::fs::write(
                temp.path().join("services/api/go.mod"),
                "module example.com/api\n\ngo 1.24\n",
            )
            .expect("write api go.mod");
            std::fs::write(
                temp.path().join("libs/common/go.mod"),
                "module example.com/common\n\ngo 1.24\n",
            )
            .expect("write common go.mod");
            let mut db = AnalysisDb::new();
            add_go_file(
                &mut db,
                temp.path(),
                "services/api/main.go",
                "package api\n",
            );
            let outside = add_go_file(
                &mut db,
                temp.path(),
                "libs/common/common.go",
                "package common\n",
            );
            let metadata = GoPackageIndex::default();
            let loaded = load_config(temp.path()).expect("config loads");

            let output = collect_go_topology(&loaded, &db, &metadata);
            let module_roots = output
                .workspace_roots
                .iter()
                .filter(|root| root.kind == WorkspaceRootKind::GoModule)
                .map(|root| root.root_path.as_str())
                .collect::<Vec<_>>();

            assert_eq!(module_roots, vec!["services/api"]);
            assert!(output.source_sets.iter().any(|source_set| {
                source_set.files == vec![outside]
                    && source_set.status == TopologyStatus::SetupMissing
            }));
        }

        #[test]
        fn topology_source_sets_classify_tests_vendor_and_generated_go_files() {
            let temp = tempfile::tempdir().expect("tempdir");
            std::fs::write(
                temp.path().join("go.mod"),
                "module example.com/app\n\ngo 1.24\n",
            )
            .expect("write go.mod");
            let mut db = AnalysisDb::new();
            let source = add_go_file(&mut db, temp.path(), "main.go", "package app\n");
            let test = add_go_file(&mut db, temp.path(), "main_test.go", "package app\n");
            let proto = add_go_file(&mut db, temp.path(), "api.pb.go", "package app\n");
            let generated = add_go_file(
                &mut db,
                temp.path(),
                "generated.go",
                "// Code generated by tool. DO NOT EDIT.\npackage app\n",
            );
            let vendor = add_go_file(
                &mut db,
                temp.path(),
                "vendor/github.com/acme/lib/lib.go",
                "package lib\n",
            );
            let metadata = go_index(
                temp.path(),
                &db,
                &[(
                    "example.com/app",
                    ".",
                    &["main.go", "main_test.go", "api.pb.go", "generated.go"],
                    false,
                )],
            );
            let loaded = load_config(temp.path()).expect("config loads");

            let output = collect_go_topology(&loaded, &db, &metadata);

            assert!(source_set_for_file(&output, source, SourceSetKind::Source));
            assert!(source_set_for_file(&output, test, SourceSetKind::Test));
            assert!(source_set_for_file(
                &output,
                proto,
                SourceSetKind::Generated
            ));
            assert!(source_set_for_file(
                &output,
                generated,
                SourceSetKind::Generated
            ));
            assert!(source_set_for_file(&output, vendor, SourceSetKind::Vendor));
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

    fn add_go_file(
        db: &mut AnalysisDb,
        root: &Path,
        relative_path: &str,
        source: &str,
    ) -> crate::core::FileId {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("test fixture has parent"))
            .expect("create parent dirs");
        std::fs::write(&path, source).expect("write fixture");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn push_go_import(db: &mut AnalysisDb, file: FileId, path: &str) -> ImportId {
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: path.to_string(),
            span: Span::point(file, 1, 1),
            language: Language::Go,
        })
    }

    fn db_and_index_for_import(path: &str) -> (AnalysisDb, GoPackageIndex) {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n")
            .expect("write go.mod");
        let mut db = AnalysisDb::new();
        let main = add_go_file(&mut db, temp.path(), "cmd/app/main.go", "package main\n");
        push_go_import(&mut db, main, path);
        let index = go_index(
            temp.path(),
            &db,
            &[("example.com/app/cmd/app", "cmd/app", &["main.go"], false)],
        );
        (db, index)
    }

    fn go_index(
        root: &Path,
        db: &AnalysisDb,
        packages: &[(&str, &str, &[&str], bool)],
    ) -> GoPackageIndex {
        go_index_with_module_path(root, db, "example.com/app", packages)
    }

    fn go_index_with_module_path(
        root: &Path,
        db: &AnalysisDb,
        module_path: &str,
        packages: &[(&str, &str, &[&str], bool)],
    ) -> GoPackageIndex {
        let stdout = packages
            .iter()
            .map(|(import_path, dir, files, standard)| {
                let dir = root.join(dir);
                let files = files
                    .iter()
                    .map(|file| serde_json::to_string(file).expect("serialize file"))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    r#"{{"ImportPath":{},"Name":"pkg","Dir":{},"GoFiles":[{}],"Standard":{},"Module":{{"Path":{}}}}}"#,
                    serde_json::to_string(import_path).expect("serialize import path"),
                    serde_json::to_string(&dir.to_string_lossy()).expect("serialize dir"),
                    files,
                    standard,
                    serde_json::to_string(module_path).expect("serialize module path")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        GoPackageIndex::load_with_runner_for_test(root, db, || {
            successful_go_output(stdout.as_bytes())
        })
    }

    fn node_by_label(nodes: &[crate::core::ModuleNode], label: &str) -> ModuleNodeId {
        nodes
            .iter()
            .find(|node| node.label == label)
            .map(|node| {
                assert!(matches!(
                    node.kind,
                    ModuleNodeKind::Module | ModuleNodeKind::Package
                ));
                node.id
            })
            .expect("node exists")
    }

    fn file_node(nodes: &[crate::core::ModuleNode], file: FileId) -> ModuleNodeId {
        nodes
            .iter()
            .find(|node| node.kind == ModuleNodeKind::File && node.file == Some(file))
            .map(|node| node.id)
            .expect("file node exists")
    }

    fn successful_go_output(stdout: &[u8]) -> GoCommandOutput {
        GoCommandOutput {
            status: successful_status(),
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
        }
    }

    fn failed_go_output(stderr: &[u8]) -> GoCommandOutput {
        GoCommandOutput {
            status: failed_status(),
            stdout: Vec::new(),
            stderr: stderr.to_vec(),
        }
    }

    fn successful_status() -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
    }

    fn failed_status() -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(1 << 8)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(1)
        }
    }
}
