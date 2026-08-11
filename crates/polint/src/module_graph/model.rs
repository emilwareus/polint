use crate::core::{
    AnalysisDb, CapabilitySupport, ImportFact, ModuleEdge, ModuleNode, ResolvedImportFact,
};
use crate::diagnostics::Diagnostic;
use crate::module_graph::topology::{
    DependencyRequirementFact, ImportToPackageFact, RepoTopologyOverlayFact,
    ResolvedDependencyEdgeFact, SourceSetFact, TopologyPackageFact, WorkspaceRootFact,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(crate) const MODULE_GRAPH_LAYER_SCHEMA: &str = "module-graph-facts-v3";
pub(crate) const MODULE_TOPOLOGY_LAYER_SCHEMA: &str = "module-topology-facts-v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ModuleGraphLayerPayload {
    pub(crate) schema: String,
    pub(crate) stable_key_texts: Vec<String>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: Vec<CapabilitySupport>,
    pub(crate) resolved_imports: Vec<ResolvedImportFact>,
    pub(crate) nodes: Vec<ModuleNode>,
    pub(crate) edges: Vec<ModuleEdge>,
    pub(crate) workspace_roots: Vec<WorkspaceRootFact>,
    pub(crate) topology_packages: Vec<TopologyPackageFact>,
    pub(crate) source_sets: Vec<SourceSetFact>,
    pub(crate) dependency_requirements: Vec<DependencyRequirementFact>,
    pub(crate) resolved_dependency_edges: Vec<ResolvedDependencyEdgeFact>,
    pub(crate) repo_topology_overlays: Vec<RepoTopologyOverlayFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ModuleTopologyLayerPayload {
    pub(crate) schema: String,
    pub(crate) stable_key_texts: Vec<String>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: Vec<CapabilitySupport>,
    pub(crate) import_to_package_edges: Vec<ImportToPackageFact>,
}

/// Resolver input remains facade-owned until the concrete adapter split because
/// its optional TS resolver context is language-specific.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolverInput<'a> {
    pub(crate) root: &'a Path,
    pub(crate) db: &'a AnalysisDb,
    pub(crate) import: &'a ImportFact,
    pub(crate) ts_resolver: Option<&'a crate::module_graph::ts::TsResolverContext>,
    pub(crate) owner_module: Option<crate::core::ModuleNodeId>,
    pub(crate) owner_package: Option<crate::core::ModuleNodeId>,
}

// Neutral builder and draft contracts are owned by polint-analysis.
pub(crate) use polint_analysis::module_graph::model::{
    ModuleGraphBuilder, ModuleNodeDraft, ResolvedImportDraft, sort_packages,
};
