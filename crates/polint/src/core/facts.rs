use super::StableKeyId;
use super::ids::{
    BranchId, DefinitionId, FileId, FunctionId, ImportId, ModuleEdgeId, ModuleNodeId, PackageId,
    ReferenceId, ResolvedImportId, SymbolId,
};
use super::lang::Language;
use super::span::Span;
use crate::diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SourceFile {
    pub id: FileId,
    pub path: PathBuf,
    pub relative_path: String,
    pub language: Language,
    pub source: Arc<str>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FunctionFact {
    pub id: FunctionId,
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub language: Language,
    pub is_test: bool,
    pub is_exported: bool,
    pub cyclomatic_complexity: u32,
    pub calls: Vec<String>,
}

pub(crate) const TS_JS_MODULE_FUNCTION_NAME: &str = "<polint:module>";

pub(crate) fn is_synthetic_ts_js_module_function(function: &FunctionFact) -> bool {
    function.language.is_ts_family() && function.name == TS_JS_MODULE_FUNCTION_NAME
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PackageFact {
    pub id: PackageId,
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImportFact {
    pub id: ImportId,
    pub file: FileId,
    pub package: Option<String>,
    pub path: String,
    pub span: Span,
    pub language: Language,
}

/// File, package, module, or external target participating in the module graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
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
#[non_exhaustive]
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
#[non_exhaustive]
pub struct ResolvedImportFact {
    pub id: ResolvedImportId,
    pub import: ImportId,
    pub from_file: FileId,
    pub target_node: Option<ModuleNodeId>,
    pub status: ResolutionStatus,
    pub precision: ResolutionPrecision,
    pub reason: Option<UnresolvedReason>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleNodeKind {
    File,
    Package,
    Module,
    External,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleEdgeKind {
    Contains,
    Imports,
    DependsOn,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionStatus {
    Resolved,
    External,
    Unresolved,
    SetupMissing,
    Dynamic,
    Unsupported,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionPrecision {
    ExactFile,
    Package,
    ExternalPackage,
    Heuristic,
    None,
}

#[non_exhaustive]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BranchObligation {
    pub id: BranchId,
    pub function: Option<FunctionId>,
    pub file: FileId,
    pub decision_span: Span,
    pub condition_text: String,
    pub edge_label: String,
    pub is_error_path: bool,
    pub stable_fingerprint: String,
}

/// Facts harvested from Go test functions (`TestXxx`, benchmarks, fuzz) in `_test.go` files.
///
/// See the polint repository's `docs/facts/go-tests.md` for field semantics and harvester limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TestFact {
    pub file: FileId,
    pub function: Option<FunctionId>,
    pub name: String,
    pub span: Span,
    pub evidence_terms: Vec<String>,
    pub assertion_count: u32,
    pub subtest_count: u32,
    /// Literal string names from direct `t.Run("name", ...)` calls (first argument only).
    pub subtest_names: Vec<String>,
    pub table_rows: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CoverageFact {
    pub branch: BranchId,
    pub covered: Option<bool>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TsComponentFact {
    pub file: FileId,
    pub function: Option<FunctionId>,
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TsClassFact {
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub is_exported: bool,
    pub is_component_like: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StringLiteralFact {
    pub file: FileId,
    pub value: String,
    pub span: Span,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct JsxAttributeFact {
    pub file: FileId,
    pub name: String,
    pub value: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedFileAnalysis {
    pub(crate) schema: String,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) facts: CachedFileFacts,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CachedFileFacts {
    pub packages: Vec<PackageFact>,
    pub functions: Vec<FunctionFact>,
    pub imports: Vec<ImportFact>,
    pub branches: Vec<BranchObligation>,
    pub tests: Vec<TestFact>,
    pub coverage: Vec<CoverageFact>,
    pub ts_components: Vec<TsComponentFact>,
    pub ts_classes: Vec<TsClassFact>,
    pub string_literals: Vec<StringLiteralFact>,
    pub jsx_attributes: Vec<JsxAttributeFact>,
}
