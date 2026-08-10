//! Module-graph fact rows shared by frontends and analyses.
//!
//! Owned here so language crates (for example `polint-ts` direct-binding) can name
//! resolved-import / module-node rows without depending on the facade.

use polint_core::{
    FileId, ImportId, Language, ModuleEdgeId, ModuleNodeId, PackageId, ResolvedImportId,
};
use serde::{Deserialize, Serialize};

/// File, package, module, or external target participating in the module graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleNode {
    pub id: ModuleNodeId,
    pub kind: ModuleNodeKind,
    pub label: String,
    pub file: Option<FileId>,
    pub package: Option<PackageId>,
    pub language: Option<Language>,
}

/// Relationship edge between two module graph nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleEdge {
    pub id: ModuleEdgeId,
    pub from: ModuleNodeId,
    pub to: ModuleNodeId,
    pub import: Option<ImportId>,
    pub resolved_import: Option<ResolvedImportId>,
    pub kind: ModuleEdgeKind,
    pub status: ResolutionStatus,
}

/// Setup-aware resolution result for one syntactic import fact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedImportFact {
    pub id: ResolvedImportId,
    pub import: ImportId,
    pub from_file: FileId,
    pub target_node: Option<ModuleNodeId>,
    pub status: ResolutionStatus,
    pub precision: ResolutionPrecision,
    pub reason: Option<UnresolvedReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleNodeKind {
    File,
    Package,
    Module,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleEdgeKind {
    Contains,
    Imports,
    DependsOn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionStatus {
    Resolved,
    External,
    Unresolved,
    SetupMissing,
    Dynamic,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionPrecision {
    ExactFile,
    Package,
    ExternalPackage,
    Heuristic,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnresolvedReason {
    NotFound,
    SetupMissing,
    DynamicExpression,
    UnsupportedLanguage,
    UnsupportedImport,
    ResolverError,
    OutsideWorkspace,
}
