use super::StableKeyId;
use super::ids::{
    DefinitionId, FileId, FunctionId, ModuleNodeId, PackageId, ReferenceId, SymbolId,
};
use super::lang::Language;
use super::span::Span;
use serde::{Deserialize, Serialize};

pub use polint_analysis_api::{
    BranchObligation, CoverageFact, FunctionFact, ImportFact, JsxAttributeFact, ModuleEdge,
    ModuleEdgeKind, ModuleNode, ModuleNodeKind, PackageFact, ResolutionPrecision, ResolutionStatus,
    ResolvedImportFact, SourceFile, StringLiteralFact, TestFact, TsClassFact, TsComponentFact,
    UnresolvedReason,
};
#[cfg(test)]
pub(crate) use polint_analysis_api::{CachedFileAnalysis, TS_JS_MODULE_FUNCTION_NAME};
pub(crate) use polint_analysis_api::{CachedFileFacts, is_synthetic_ts_js_module_function};

/// Source-file size and aggregate function metrics derived from parsed facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FileMetricFact {
    pub file: FileId,
    pub language: Language,
    pub line_count: u32,
    pub non_empty_line_count: u32,
    pub byte_count: u32,
    pub function_count: u32,
}

/// Function-size metrics derived once per function and shared by requesting rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FunctionMetricFact {
    pub function: FunctionId,
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub language: Language,
    pub line_count: u32,
    pub byte_count: u32,
}

/// Function complexity metrics derived once per function and shared by requesting rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ComplexityMetricFact {
    pub function: FunctionId,
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub language: Language,
    pub cyclomatic_complexity: u32,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Package,
    Module,
    File,
    Function,
    Method,
    Class,
    Interface,
    TypeAlias,
    Enum,
    EnumMember,
    Variable,
    Constant,
    Parameter,
    Field,
    Property,
    Namespace,
    Import,
    Export,
    Unknown,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolNamespace {
    Value,
    Type,
    Namespace,
    Package,
    Module,
    Unknown,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefinitionKind {
    Declaration,
    Definition,
    Import,
    Export,
    Implicit,
    Unknown,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceKind {
    Read,
    Write,
    ReadWrite,
    Call,
    TypeUse,
    Import,
    Export,
    MemberAccess,
    Assignment,
    DeclarationUse,
    Unknown,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolPrecision {
    ExactSemantic,
    ExactLocal,
    ModuleLinked,
    Heuristic,
    Unresolved,
    Ambiguous,
    SetupMissing,
    Unsupported,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolResolutionStatus {
    Resolved,
    Unresolved,
    Ambiguous,
    SetupMissing,
    Unsupported,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SymbolFact {
    pub id: SymbolId,
    pub language: Language,
    pub name: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub namespace: SymbolNamespace,
    pub file: Option<FileId>,
    pub package: Option<PackageId>,
    pub module: Option<ModuleNodeId>,
    pub owner: Option<SymbolId>,
    pub primary_span: Option<Span>,
    pub is_exported: bool,
    pub(crate) stable_key: StableKeyId,
    pub precision: SymbolPrecision,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DefinitionFact {
    pub id: DefinitionId,
    pub symbol: SymbolId,
    pub language: Language,
    pub name: String,
    pub qualified_name: String,
    pub kind: DefinitionKind,
    pub namespace: SymbolNamespace,
    pub file: Option<FileId>,
    pub package: Option<PackageId>,
    pub module: Option<ModuleNodeId>,
    pub owner: Option<SymbolId>,
    pub primary_span: Option<Span>,
    pub is_primary: bool,
    pub is_exported: bool,
    pub(crate) stable_key: StableKeyId,
    pub precision: SymbolPrecision,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ReferenceFact {
    pub id: ReferenceId,
    pub language: Language,
    pub name: String,
    pub qualified_name: String,
    pub kind: ReferenceKind,
    pub namespace: SymbolNamespace,
    pub file: Option<FileId>,
    pub package: Option<PackageId>,
    pub module: Option<ModuleNodeId>,
    pub owner: Option<SymbolId>,
    pub primary_span: Option<Span>,
    pub target: Option<SymbolId>,
    pub candidates: Vec<SymbolId>,
    pub(crate) stable_key: StableKeyId,
    pub status: SymbolResolutionStatus,
    pub precision: SymbolPrecision,
}
