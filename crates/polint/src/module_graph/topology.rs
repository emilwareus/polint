use crate::core::{FileId, ImportId, Language, ModuleNodeId, PackageId, ResolvedImportId};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceRootFact {
    pub(crate) id: WorkspaceRootId,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopologyPackageFact {
    pub(crate) id: TopologyPackageId,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceSetFact {
    pub(crate) id: SourceSetId,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyRequirementFact {
    pub(crate) id: DependencyRequirementId,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedDependencyEdgeFact {
    pub(crate) id: ResolvedDependencyEdgeId,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportToPackageFact {
    pub(crate) id: ImportToPackageId,
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: ImportToPackageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepoTopologyOverlayFact {
    pub(crate) id: RepoTopologyOverlayId,
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
    pub(crate) fn normalized(self) -> Self {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(key: &str, id: u64) -> WorkspaceRootFact {
        WorkspaceRootFact {
            id: WorkspaceRootId(id),
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        }
    }

    fn package(key: &str, id: u64) -> TopologyPackageFact {
        TopologyPackageFact {
            id: TopologyPackageId(id),
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        }
    }

    fn source_set(key: &str, id: u64) -> SourceSetFact {
        SourceSetFact {
            id: SourceSetId(id),
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        }
    }

    fn requirement(key: &str, id: u64) -> DependencyRequirementFact {
        DependencyRequirementFact {
            id: DependencyRequirementId(id),
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        }
    }

    fn resolved_edge(key: &str, id: u64) -> ResolvedDependencyEdgeFact {
        ResolvedDependencyEdgeFact {
            id: ResolvedDependencyEdgeId(id),
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactLockfile,
            status: TopologyStatus::Resolved,
        }
    }

    fn import_edge(key: &str, id: u64) -> ImportToPackageFact {
        ImportToPackageFact {
            id: ImportToPackageId(id),
            stable_key: key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: ImportToPackageStatus::Resolved,
        }
    }

    fn overlay(key: &str, id: u64) -> RepoTopologyOverlayFact {
        RepoTopologyOverlayFact {
            id: RepoTopologyOverlayId(id),
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
            source_sets: vec![source_set("source-set:z", 99), source_set("source-set:a", 42)],
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
