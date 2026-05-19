#![expect(
    dead_code,
    reason = "Topology contracts are populated by later Phase 27 collectors."
)]

use crate::core::{FileId, ImportId, Language, ModuleNodeId, PackageId, ResolvedImportId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct WorkspaceRootId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TopologyPackageId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SourceSetId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DependencyRequirementId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ResolvedDependencyEdgeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ImportToPackageId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceRootFact {
    pub(crate) id: WorkspaceRootId,
    pub(crate) kind: WorkspaceRootKind,
    pub(crate) root_path: String,
    pub(crate) manifest_path: Option<String>,
    pub(crate) language: Option<Language>,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceSetFact {
    pub(crate) id: SourceSetId,
    pub(crate) package: Option<TopologyPackageId>,
    pub(crate) root: Option<WorkspaceRootId>,
    pub(crate) kind: SourceSetKind,
    pub(crate) path: String,
    pub(crate) language: Option<Language>,
    pub(crate) files: Vec<FileId>,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyRequirementFact {
    pub(crate) id: DependencyRequirementId,
    pub(crate) from_package: Option<TopologyPackageId>,
    pub(crate) target_package: Option<TopologyPackageId>,
    pub(crate) target_name: String,
    pub(crate) version_requirement: Option<String>,
    pub(crate) kind: RequirementKind,
    pub(crate) manifest_path: Option<String>,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedDependencyEdgeFact {
    pub(crate) id: ResolvedDependencyEdgeId,
    pub(crate) requirement: Option<DependencyRequirementId>,
    pub(crate) from_package: Option<TopologyPackageId>,
    pub(crate) to_package: Option<TopologyPackageId>,
    pub(crate) package_name: String,
    pub(crate) resolved_version: Option<String>,
    pub(crate) kind: ResolvedDependencyKind,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportToPackageFact {
    pub(crate) id: ImportToPackageId,
    pub(crate) import: Option<ImportId>,
    pub(crate) resolved_import: Option<ResolvedImportId>,
    pub(crate) from_file: Option<FileId>,
    pub(crate) from_package: Option<TopologyPackageId>,
    pub(crate) to_package: Option<TopologyPackageId>,
    pub(crate) target_node: Option<ModuleNodeId>,
    pub(crate) context: ImportContextKind,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: ImportToPackageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepoTopologyOverlayFact {
    pub(crate) id: RepoTopologyOverlayId,
    pub(crate) root: Option<WorkspaceRootId>,
    pub(crate) package: Option<TopologyPackageId>,
    pub(crate) source_set: Option<SourceSetId>,
    pub(crate) kind: RepoTopologyOverlayKind,
    pub(crate) label: String,
    pub(crate) path: Option<String>,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TopologyOutput {
    pub(crate) workspace_roots: Vec<WorkspaceRootFact>,
    pub(crate) packages: Vec<TopologyPackageFact>,
    pub(crate) source_sets: Vec<SourceSetFact>,
    pub(crate) dependency_requirements: Vec<DependencyRequirementFact>,
    pub(crate) resolved_dependency_edges: Vec<ResolvedDependencyEdgeFact>,
    pub(crate) import_to_package_edges: Vec<ImportToPackageFact>,
    pub(crate) overlays: Vec<RepoTopologyOverlayFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum WorkspaceRootKind {
    Repository,
    GoModule,
    GoWorkspace,
    PackageWorkspace,
    TsProject,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum TopologyPackageKind {
    Workspace,
    Project,
    Package,
    External,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum SourceSetKind {
    Source,
    Test,
    Generated,
    Vendor,
    External,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RequirementKind {
    Runtime,
    Development,
    Peer,
    Optional,
    Build,
    Test,
    Tool,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ResolvedDependencyKind {
    Lockfile,
    ToolResolved,
    LocalReplacement,
    Workspace,
    External,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ImportContextKind {
    Source,
    Test,
    Generated,
    Vendor,
    External,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RepoTopologyOverlayKind {
    Ownership,
    ArchitectureLayer,
    DeployUnit,
    GeneratedZone,
    TestVisibility,
    ApiBoundary,
    SourceOfTruth,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum TopologyPrecision {
    ExactStatic,
    ExactLockfile,
    Heuristic,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum TopologyStatus {
    Present,
    Resolved,
    Ambiguous,
    Unresolved,
    SetupMissing,
    Generated,
    External,
    Unsupported,
}

impl TopologyOutput {
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
            import: Some(ImportId(0)),
            resolved_import: Some(ResolvedImportId(0)),
            from_file: Some(FileId(0)),
            from_package: Some(TopologyPackageId(0)),
            to_package: None,
            target_node: Some(ModuleNodeId(0)),
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
            kind: RepoTopologyOverlayKind::Ownership,
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
