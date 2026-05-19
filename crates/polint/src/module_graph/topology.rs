use crate::core::{FileId, ImportId, Language, ModuleNodeId, PackageId, ResolvedImportId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct WorkspaceRootId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TopologyPackageId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct SourceSetId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DependencyRequirementId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct ResolvedDependencyEdgeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct ImportToPackageId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct RepoTopologyOverlayId(pub(crate) u64);

macro_rules! impl_topology_id_from_u64 {
    ($($id:ty),* $(,)?) => {
        $(
            impl From<u64> for $id {
                fn from(value: u64) -> Self {
                    Self(value)
                }
            }
        )*
    };
}

impl_topology_id_from_u64!(
    WorkspaceRootId,
    TopologyPackageId,
    SourceSetId,
    DependencyRequirementId,
    ResolvedDependencyEdgeId,
    ImportToPackageId,
    RepoTopologyOverlayId,
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WorkspaceRootFact {
    pub(crate) id: WorkspaceRootId,
    pub(crate) kind: WorkspaceRootKind,
    pub(crate) root_path: String,
    pub(crate) manifest_path: Option<String>,
    pub(crate) language: Option<Language>,
    pub(crate) stable_key: String,
    #[serde(skip_deserializing, default = "module_graph_producer_id")]
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TopologyPackageFact {
    pub(crate) id: TopologyPackageId,
    pub(crate) workspace_root: Option<WorkspaceRootId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) module_node: Option<ModuleNodeId>,
    pub(crate) kind: TopologyPackageKind,
    pub(crate) name: String,
    pub(crate) version: Option<String>,
    pub(crate) path: String,
    pub(crate) language: Option<Language>,
    pub(crate) stable_key: String,
    #[serde(skip_deserializing, default = "module_graph_producer_id")]
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SourceSetFact {
    pub(crate) id: SourceSetId,
    pub(crate) package: Option<TopologyPackageId>,
    pub(crate) root: Option<WorkspaceRootId>,
    pub(crate) kind: SourceSetKind,
    pub(crate) path: String,
    pub(crate) language: Option<Language>,
    pub(crate) files: Vec<FileId>,
    pub(crate) stable_key: String,
    #[serde(skip_deserializing, default = "module_graph_producer_id")]
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DependencyRequirementFact {
    pub(crate) id: DependencyRequirementId,
    pub(crate) from_package: Option<TopologyPackageId>,
    pub(crate) target_package: Option<TopologyPackageId>,
    pub(crate) target_name: String,
    pub(crate) version_requirement: Option<String>,
    pub(crate) kind: RequirementKind,
    pub(crate) manifest_path: Option<String>,
    pub(crate) stable_key: String,
    #[serde(skip_deserializing, default = "module_graph_producer_id")]
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ResolvedDependencyEdgeFact {
    pub(crate) id: ResolvedDependencyEdgeId,
    pub(crate) requirement: Option<DependencyRequirementId>,
    pub(crate) from_package: Option<TopologyPackageId>,
    pub(crate) to_package: Option<TopologyPackageId>,
    pub(crate) package_name: String,
    pub(crate) resolved_version: Option<String>,
    pub(crate) kind: ResolvedDependencyKind,
    pub(crate) stable_key: String,
    #[serde(skip_deserializing, default = "module_graph_producer_id")]
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ImportToPackageFact {
    pub(crate) id: ImportToPackageId,
    pub(crate) syntax_import: Option<ImportId>,
    pub(crate) resolved_import: Option<ResolvedImportId>,
    pub(crate) semantic_import_stable_key: Option<String>,
    pub(crate) from_file: Option<FileId>,
    pub(crate) from_package: Option<TopologyPackageId>,
    pub(crate) to_package: Option<TopologyPackageId>,
    pub(crate) target_node: Option<ModuleNodeId>,
    pub(crate) from_package_stable_key: Option<String>,
    pub(crate) to_package_stable_key: Option<String>,
    pub(crate) source_set_stable_key: Option<String>,
    pub(crate) import_path: String,
    pub(crate) context: ImportContextKind,
    pub(crate) stable_key: String,
    #[serde(skip_deserializing, default = "module_topology_producer_id")]
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: ImportToPackageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RepoTopologyOverlayFact {
    pub(crate) id: RepoTopologyOverlayId,
    pub(crate) root: Option<WorkspaceRootId>,
    pub(crate) package: Option<TopologyPackageId>,
    pub(crate) source_set: Option<SourceSetId>,
    pub(crate) kind: RepoTopologyOverlayKind,
    pub(crate) label: String,
    pub(crate) path: Option<String>,
    pub(crate) stable_key: String,
    #[serde(skip_deserializing, default = "module_graph_producer_id")]
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TopologyOutput {
    pub(crate) workspace_roots: Vec<WorkspaceRootFact>,
    pub(crate) packages: Vec<TopologyPackageFact>,
    pub(crate) source_sets: Vec<SourceSetFact>,
    pub(crate) dependency_requirements: Vec<DependencyRequirementFact>,
    pub(crate) resolved_dependency_edges: Vec<ResolvedDependencyEdgeFact>,
    pub(crate) import_to_package_edges: Vec<ImportToPackageFact>,
    pub(crate) overlays: Vec<RepoTopologyOverlayFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum WorkspaceRootKind {
    Repository,
    GoModule,
    GoWorkspace,
    JsWorkspace,
    PackageWorkspace,
    TsProject,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TopologyPackageKind {
    Workspace,
    JsPackage,
    Project,
    Package,
    External,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum SourceSetKind {
    Source,
    Test,
    Generated,
    Vendor,
    External,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum RequirementKind {
    Direct,
    Runtime,
    Dev,
    Development,
    Peer,
    Optional,
    Bundled,
    Workspace,
    Build,
    Test,
    Tool,
    Replace,
    Exclude,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum ResolvedDependencyKind {
    Lockfile,
    LockfileSelected,
    ChecksumEvidence,
    ToolResolved,
    LocalReplacement,
    Workspace,
    External,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum ImportToPackageStatus {
    Resolved,
    External,
    Unresolved,
    SetupMissing,
    Unsupported,
    Dynamic,
    Ambiguous,
    Undeclared,
    OutsideWorkspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum ImportContextKind {
    Source,
    Test,
    Generated,
    Vendor,
    External,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum RepoTopologyOverlayKind {
    OwnershipZone,
    ArchitectureLayer,
    DeployUnit,
    GeneratedZone,
    TestOnlyVisibility,
    InternalPublicApiBoundary,
    SourceOfTruthDirectory,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TopologyPrecision {
    ExactStatic,
    ExactLockfile,
    Heuristic,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TopologyStatus {
    Present,
    Resolved,
    Ambiguous,
    Unresolved,
    Unknown,
    SetupMissing,
    MissingLockfile,
    Generated,
    External,
    Unsupported,
}

impl TopologyOutput {
    pub(crate) fn merge(&mut self, mut other: TopologyOutput) {
        let root_offset = self.workspace_roots.len() as u64;
        let package_offset = self.packages.len() as u64;
        let source_set_offset = self.source_sets.len() as u64;
        let requirement_offset = self.dependency_requirements.len() as u64;
        let resolved_dependency_offset = self.resolved_dependency_edges.len() as u64;
        let import_to_package_offset = self.import_to_package_edges.len() as u64;
        let overlay_offset = self.overlays.len() as u64;

        offset_output_ids(
            &mut other,
            root_offset,
            package_offset,
            source_set_offset,
            requirement_offset,
            resolved_dependency_offset,
            import_to_package_offset,
            overlay_offset,
        );
        self.workspace_roots.extend(other.workspace_roots);
        self.packages.extend(other.packages);
        self.source_sets.extend(other.source_sets);
        self.dependency_requirements
            .extend(other.dependency_requirements);
        self.resolved_dependency_edges
            .extend(other.resolved_dependency_edges);
        self.import_to_package_edges
            .extend(other.import_to_package_edges);
        self.overlays.extend(other.overlays);
    }

    pub(crate) fn normalized(mut self) -> Self {
        let root_ids = normalize_rows(
            &mut self.workspace_roots,
            |row| row.id,
            |row| &row.stable_key,
            |row, id| row.id = WorkspaceRootId(id),
        );
        let package_ids = normalize_rows(
            &mut self.packages,
            |row| row.id,
            |row| &row.stable_key,
            |row, id| row.id = TopologyPackageId(id),
        );
        let source_set_ids = normalize_rows(
            &mut self.source_sets,
            |row| row.id,
            |row| &row.stable_key,
            |row, id| row.id = SourceSetId(id),
        );
        let requirement_ids = normalize_rows(
            &mut self.dependency_requirements,
            |row| row.id,
            |row| &row.stable_key,
            |row, id| row.id = DependencyRequirementId(id),
        );
        normalize_rows(
            &mut self.resolved_dependency_edges,
            |row| row.id,
            |row| &row.stable_key,
            |row, id| row.id = ResolvedDependencyEdgeId(id),
        );
        normalize_rows(
            &mut self.import_to_package_edges,
            |row| row.id,
            |row| &row.stable_key,
            |row, id| row.id = ImportToPackageId(id),
        );
        normalize_rows(
            &mut self.overlays,
            |row| row.id,
            |row| &row.stable_key,
            |row, id| row.id = RepoTopologyOverlayId(id),
        );
        for package in &mut self.packages {
            remap_option(&mut package.workspace_root, &root_ids);
        }
        for source_set in &mut self.source_sets {
            remap_option(&mut source_set.package, &package_ids);
            remap_option(&mut source_set.root, &root_ids);
        }
        for requirement in &mut self.dependency_requirements {
            remap_option(&mut requirement.from_package, &package_ids);
            remap_option(&mut requirement.target_package, &package_ids);
        }
        for edge in &mut self.resolved_dependency_edges {
            remap_option(&mut edge.requirement, &requirement_ids);
            remap_option(&mut edge.from_package, &package_ids);
            remap_option(&mut edge.to_package, &package_ids);
        }
        for edge in &mut self.import_to_package_edges {
            remap_option(&mut edge.from_package, &package_ids);
            remap_option(&mut edge.to_package, &package_ids);
        }
        for overlay in &mut self.overlays {
            remap_option(&mut overlay.root, &root_ids);
            remap_option(&mut overlay.package, &package_ids);
            remap_option(&mut overlay.source_set, &source_set_ids);
        }
        self
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each topology ID family has an independent run-local namespace that must be offset separately"
)]
fn offset_output_ids(
    output: &mut TopologyOutput,
    root_offset: u64,
    package_offset: u64,
    source_set_offset: u64,
    requirement_offset: u64,
    resolved_dependency_offset: u64,
    import_to_package_offset: u64,
    overlay_offset: u64,
) {
    for root in &mut output.workspace_roots {
        root.id.0 += root_offset;
    }
    for package in &mut output.packages {
        package.id.0 += package_offset;
        offset_option_id(&mut package.workspace_root, root_offset);
    }
    for source_set in &mut output.source_sets {
        source_set.id.0 += source_set_offset;
        offset_option_id(&mut source_set.package, package_offset);
        offset_option_id(&mut source_set.root, root_offset);
    }
    for requirement in &mut output.dependency_requirements {
        requirement.id.0 += requirement_offset;
        offset_option_id(&mut requirement.from_package, package_offset);
        offset_option_id(&mut requirement.target_package, package_offset);
    }
    for edge in &mut output.resolved_dependency_edges {
        edge.id.0 += resolved_dependency_offset;
        offset_option_id(&mut edge.requirement, requirement_offset);
        offset_option_id(&mut edge.from_package, package_offset);
        offset_option_id(&mut edge.to_package, package_offset);
    }
    for edge in &mut output.import_to_package_edges {
        edge.id.0 += import_to_package_offset;
        offset_option_id(&mut edge.from_package, package_offset);
        offset_option_id(&mut edge.to_package, package_offset);
    }
    for overlay in &mut output.overlays {
        overlay.id.0 += overlay_offset;
        offset_option_id(&mut overlay.root, root_offset);
        offset_option_id(&mut overlay.package, package_offset);
        offset_option_id(&mut overlay.source_set, source_set_offset);
    }
}

trait OffsetTopologyId {
    fn offset(&mut self, offset: u64);
}

macro_rules! impl_offset_topology_id {
    ($($id:ty),* $(,)?) => {
        $(
            impl OffsetTopologyId for $id {
                fn offset(&mut self, offset: u64) {
                    self.0 += offset;
                }
            }
        )*
    };
}

impl_offset_topology_id!(
    WorkspaceRootId,
    TopologyPackageId,
    SourceSetId,
    DependencyRequirementId,
);

fn offset_option_id<Id: OffsetTopologyId>(value: &mut Option<Id>, offset: u64) {
    if let Some(id) = value {
        id.offset(offset);
    }
}

fn normalize_rows<T, Id>(
    rows: &mut [T],
    id: impl Fn(&T) -> Id,
    stable_key: impl Fn(&T) -> &str,
    mut assign_id: impl FnMut(&mut T, u64),
) -> BTreeMap<Id, Id>
where
    Id: Copy + Ord + From<u64>,
{
    rows.sort_by(|left, right| stable_key(left).cmp(stable_key(right)));
    let mut ids = BTreeMap::new();
    for (index, row) in rows.iter_mut().enumerate() {
        let old_id = id(row);
        let new_id = Id::from(index as u64);
        assign_id(row, index as u64);
        ids.insert(old_id, new_id);
    }
    ids
}

fn remap_option<Id: Copy + Ord>(value: &mut Option<Id>, ids: &BTreeMap<Id, Id>) {
    if let Some(id) = value
        && let Some(remapped) = ids.get(id)
    {
        *value = Some(*remapped);
    }
}

fn module_graph_producer_id() -> &'static str {
    "polint.module_graph"
}

fn module_topology_producer_id() -> &'static str {
    "polint.module_topology"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(key: &str, id: u64) -> WorkspaceRootFact {
        WorkspaceRootFact {
            id: WorkspaceRootId(id),
            kind: WorkspaceRootKind::Repository,
            root_path: ".".to_string(),
            manifest_path: None,
            language: None,
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        }
    }

    fn package(key: &str, id: u64) -> TopologyPackageFact {
        TopologyPackageFact {
            id: TopologyPackageId(id),
            workspace_root: Some(WorkspaceRootId(0)),
            package: Some(PackageId(0)),
            module_node: Some(ModuleNodeId(0)),
            kind: TopologyPackageKind::Workspace,
            name: key.to_string(),
            version: None,
            path: ".".to_string(),
            language: Some(Language::TypeScript),
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        }
    }

    fn source_set(key: &str, id: u64) -> SourceSetFact {
        SourceSetFact {
            id: SourceSetId(id),
            package: Some(TopologyPackageId(0)),
            root: Some(WorkspaceRootId(0)),
            kind: SourceSetKind::Source,
            path: "src".to_string(),
            language: Some(Language::TypeScript),
            files: vec![FileId(0)],
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        }
    }

    fn requirement(key: &str, id: u64) -> DependencyRequirementFact {
        DependencyRequirementFact {
            id: DependencyRequirementId(id),
            from_package: Some(TopologyPackageId(0)),
            target_package: None,
            target_name: key.to_string(),
            version_requirement: Some("^1.0.0".to_string()),
            kind: RequirementKind::Runtime,
            manifest_path: Some("package.json".to_string()),
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        }
    }

    fn resolved_edge(key: &str, id: u64) -> ResolvedDependencyEdgeFact {
        ResolvedDependencyEdgeFact {
            id: ResolvedDependencyEdgeId(id),
            requirement: Some(DependencyRequirementId(0)),
            from_package: Some(TopologyPackageId(0)),
            to_package: None,
            package_name: key.to_string(),
            resolved_version: Some("1.0.0".to_string()),
            kind: ResolvedDependencyKind::Lockfile,
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactLockfile,
            status: TopologyStatus::Resolved,
        }
    }

    fn import_edge(key: &str, id: u64) -> ImportToPackageFact {
        ImportToPackageFact {
            id: ImportToPackageId(id),
            syntax_import: Some(ImportId(0)),
            resolved_import: Some(ResolvedImportId(0)),
            semantic_import_stable_key: None,
            from_file: Some(FileId(0)),
            from_package: Some(TopologyPackageId(0)),
            to_package: None,
            target_node: Some(ModuleNodeId(0)),
            from_package_stable_key: None,
            to_package_stable_key: None,
            source_set_stable_key: None,
            import_path: "example".to_string(),
            context: ImportContextKind::Source,
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: ImportToPackageStatus::Resolved,
        }
    }

    fn overlay(key: &str, id: u64) -> RepoTopologyOverlayFact {
        RepoTopologyOverlayFact {
            id: RepoTopologyOverlayId(id),
            root: Some(WorkspaceRootId(0)),
            package: Some(TopologyPackageId(0)),
            source_set: Some(SourceSetId(0)),
            kind: RepoTopologyOverlayKind::OwnershipZone,
            label: key.to_string(),
            path: Some("src".to_string()),
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::Heuristic,
            status: TopologyStatus::Present,
        }
    }

    #[test]
    fn normalized_sorts_every_family_by_stable_key_and_reassigns_ids() {
        let output = TopologyOutput {
            workspace_roots: vec![root("root:z", 99), root("root:a", 42)],
            packages: vec![package("package:z", 99), package("package:a", 42)],
            source_sets: vec![
                source_set("source-set:z", 99),
                source_set("source-set:a", 42),
            ],
            dependency_requirements: vec![
                requirement("requirement:z", 99),
                requirement("requirement:a", 42),
            ],
            resolved_dependency_edges: vec![
                resolved_edge("resolved:z", 99),
                resolved_edge("resolved:a", 42),
            ],
            import_to_package_edges: vec![import_edge("import:z", 99), import_edge("import:a", 42)],
            overlays: vec![overlay("overlay:z", 99), overlay("overlay:a", 42)],
        }
        .normalized();

        assert_eq!(output.workspace_roots[0].id, WorkspaceRootId(0));
        assert_eq!(output.workspace_roots[0].stable_key, "root:a");
        assert_eq!(output.packages[0].id, TopologyPackageId(0));
        assert_eq!(output.packages[0].stable_key, "package:a");
        assert_eq!(output.source_sets[0].id, SourceSetId(0));
        assert_eq!(output.source_sets[0].stable_key, "source-set:a");
        assert_eq!(
            output.dependency_requirements[0].id,
            DependencyRequirementId(0)
        );
        assert_eq!(
            output.dependency_requirements[0].stable_key,
            "requirement:a"
        );
        assert_eq!(
            output.resolved_dependency_edges[0].id,
            ResolvedDependencyEdgeId(0)
        );
        assert_eq!(output.resolved_dependency_edges[0].stable_key, "resolved:a");
        assert_eq!(output.import_to_package_edges[0].id, ImportToPackageId(0));
        assert_eq!(output.import_to_package_edges[0].stable_key, "import:a");
        assert_eq!(output.overlays[0].id, RepoTopologyOverlayId(0));
        assert_eq!(output.overlays[0].stable_key, "overlay:a");
    }

    #[test]
    fn merge_offsets_colliding_ids_before_final_normalization() {
        let mut left = TopologyOutput {
            workspace_roots: vec![root("root:b", 0)],
            packages: vec![package("package:b", 0)],
            source_sets: vec![source_set("source-set:b", 0)],
            dependency_requirements: vec![requirement("requirement:b", 0)],
            resolved_dependency_edges: vec![resolved_edge("resolved:b", 0)],
            import_to_package_edges: vec![import_edge("import:b", 0)],
            overlays: vec![overlay("overlay:b", 0)],
        };
        let right = TopologyOutput {
            workspace_roots: vec![root("root:a", 0)],
            packages: vec![package("package:a", 0)],
            source_sets: vec![source_set("source-set:a", 0)],
            dependency_requirements: vec![requirement("requirement:a", 0)],
            resolved_dependency_edges: vec![resolved_edge("resolved:a", 0)],
            import_to_package_edges: vec![import_edge("import:a", 0)],
            overlays: vec![overlay("overlay:a", 0)],
        };

        left.merge(right);
        let output = left.normalized();

        let source_set_a = output
            .source_sets
            .iter()
            .find(|row| row.stable_key == "source-set:a")
            .expect("right source set survives merge");
        assert_eq!(
            stable_key_for_root(&output, source_set_a.root),
            Some("root:a")
        );
        assert_eq!(
            stable_key_for_package(&output, source_set_a.package),
            Some("package:a")
        );

        let requirement_a = output
            .dependency_requirements
            .iter()
            .find(|row| row.stable_key == "requirement:a")
            .expect("right requirement survives merge");
        assert_eq!(
            stable_key_for_package(&output, requirement_a.from_package),
            Some("package:a")
        );

        let resolved_a = output
            .resolved_dependency_edges
            .iter()
            .find(|row| row.stable_key == "resolved:a")
            .expect("right resolved edge survives merge");
        assert_eq!(
            stable_key_for_requirement(&output, resolved_a.requirement),
            Some("requirement:a")
        );
        assert_eq!(
            stable_key_for_package(&output, resolved_a.from_package),
            Some("package:a")
        );

        let overlay_a = output
            .overlays
            .iter()
            .find(|row| row.stable_key == "overlay:a")
            .expect("right overlay survives merge");
        assert_eq!(stable_key_for_root(&output, overlay_a.root), Some("root:a"));
        assert_eq!(
            stable_key_for_package(&output, overlay_a.package),
            Some("package:a")
        );
        assert_eq!(
            stable_key_for_source_set(&output, overlay_a.source_set),
            Some("source-set:a")
        );
    }

    fn stable_key_for_root(output: &TopologyOutput, id: Option<WorkspaceRootId>) -> Option<&str> {
        output
            .workspace_roots
            .iter()
            .find(|row| Some(row.id) == id)
            .map(|row| row.stable_key.as_str())
    }

    fn stable_key_for_package(
        output: &TopologyOutput,
        id: Option<TopologyPackageId>,
    ) -> Option<&str> {
        output
            .packages
            .iter()
            .find(|row| Some(row.id) == id)
            .map(|row| row.stable_key.as_str())
    }

    fn stable_key_for_source_set(output: &TopologyOutput, id: Option<SourceSetId>) -> Option<&str> {
        output
            .source_sets
            .iter()
            .find(|row| Some(row.id) == id)
            .map(|row| row.stable_key.as_str())
    }

    fn stable_key_for_requirement(
        output: &TopologyOutput,
        id: Option<DependencyRequirementId>,
    ) -> Option<&str> {
        output
            .dependency_requirements
            .iter()
            .find(|row| Some(row.id) == id)
            .map(|row| row.stable_key.as_str())
    }

    #[test]
    fn import_to_package_status_contains_required_uncertainty_states() {
        let statuses = [
            ImportToPackageStatus::Resolved,
            ImportToPackageStatus::External,
            ImportToPackageStatus::Unresolved,
            ImportToPackageStatus::SetupMissing,
            ImportToPackageStatus::Unsupported,
            ImportToPackageStatus::Dynamic,
            ImportToPackageStatus::Ambiguous,
            ImportToPackageStatus::Undeclared,
            ImportToPackageStatus::OutsideWorkspace,
        ];

        assert_eq!(statuses.len(), 9);
    }

    #[test]
    fn source_set_kind_contains_required_contexts() {
        let kinds = [
            SourceSetKind::Source,
            SourceSetKind::Test,
            SourceSetKind::Generated,
            SourceSetKind::Vendor,
            SourceSetKind::External,
            SourceSetKind::Unknown,
        ];

        assert_eq!(kinds.len(), 6);
    }
}
