//! Symbol, reference, definition, and metrics fact rows shared across crates.
//!
//! Owned here so analysis algorithms and frontends can name these rows without
//! depending on the facade `core` module.

use crate::internal_core::{
    DefinitionId, FileId, FunctionId, Language, ModuleNodeId, PackageId, ReferenceId, Span,
    StableKeyId, SymbolId,
};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SymbolNamespace {
    Value,
    Type,
    Namespace,
    Package,
    Module,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DefinitionKind {
    Declaration,
    Definition,
    Import,
    Export,
    Implicit,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
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
    pub stable_key: StableKeyId,
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
    pub stable_key: StableKeyId,
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
    pub stable_key: StableKeyId,
    pub status: SymbolResolutionStatus,
    pub precision: SymbolPrecision,
}

/// Scope identifier used by semantic import / export rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ScopeId(pub u64);

/// Semantic import row identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SemanticImportId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SemanticStatus {
    Resolved,
    Ambiguous,
    Unresolved,
    Cycle,
    Generated,
    Dynamic,
    External,
    SetupMissing,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SemanticImportKind {
    StaticNamed,
    StaticDefault,
    StaticNamespace,
    SideEffect,
    TypeOnly,
    CommonJsRequire,
    DynamicImport,
    GoDefault,
    GoNamed,
    GoDot,
    GoBlank,
    GoImplicit,
    EsNamed,
    EsDefault,
    EsNamespace,
    ReExport,
    CommonJs,
    Dynamic,
    Unknown,
}

/// Setup-aware semantic import binding used by call-target resolution and module linking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticImportFact {
    pub id: SemanticImportId,
    pub language: Language,
    pub file: Option<FileId>,
    pub package: Option<PackageId>,
    pub module: Option<ModuleNodeId>,
    pub scope: Option<ScopeId>,
    pub import_path: String,
    pub local_name: Option<String>,
    pub imported_name: Option<String>,
    pub namespace: SymbolNamespace,
    pub kind: SemanticImportKind,
    pub stable_key: StableKeyId,
    pub status: SemanticStatus,
}
impl FileMetricFact {
    /// Constructs file metrics from their complete fields.
    pub fn new(
        file: FileId,
        language: Language,
        line_count: u32,
        non_empty_line_count: u32,
        byte_count: u32,
        function_count: u32,
    ) -> Self {
        Self {
            file,
            language,
            line_count,
            non_empty_line_count,
            byte_count,
            function_count,
        }
    }
}

impl FunctionMetricFact {
    /// Constructs function metrics from their complete fields.
    pub fn new(
        function: FunctionId,
        file: FileId,
        name: String,
        span: Span,
        language: Language,
        line_count: u32,
        byte_count: u32,
    ) -> Self {
        Self {
            function,
            file,
            name,
            span,
            language,
            line_count,
            byte_count,
        }
    }
}

impl ComplexityMetricFact {
    /// Constructs complexity metrics from their complete fields.
    pub fn new(
        function: FunctionId,
        file: FileId,
        name: String,
        span: Span,
        language: Language,
        cyclomatic_complexity: u32,
    ) -> Self {
        Self {
            function,
            file,
            name,
            span,
            language,
            cyclomatic_complexity,
        }
    }
}

impl SymbolFact {
    /// Constructs a symbol fact from its complete fields.
    #[expect(
        clippy::too_many_arguments,
        reason = "This constructor mirrors the complete non-exhaustive public fact schema."
    )]
    pub fn new(
        id: SymbolId,
        language: Language,
        name: String,
        qualified_name: String,
        kind: SymbolKind,
        namespace: SymbolNamespace,
        file: Option<FileId>,
        package: Option<PackageId>,
        module: Option<ModuleNodeId>,
        owner: Option<SymbolId>,
        primary_span: Option<Span>,
        is_exported: bool,
        stable_key: StableKeyId,
        precision: SymbolPrecision,
    ) -> Self {
        Self {
            id,
            language,
            name,
            qualified_name,
            kind,
            namespace,
            file,
            package,
            module,
            owner,
            primary_span,
            is_exported,
            stable_key,
            precision,
        }
    }
}

impl DefinitionFact {
    /// Constructs a definition fact from its complete fields.
    #[expect(
        clippy::too_many_arguments,
        reason = "This constructor mirrors the complete non-exhaustive public fact schema."
    )]
    pub fn new(
        id: DefinitionId,
        symbol: SymbolId,
        language: Language,
        name: String,
        qualified_name: String,
        kind: DefinitionKind,
        namespace: SymbolNamespace,
        file: Option<FileId>,
        package: Option<PackageId>,
        module: Option<ModuleNodeId>,
        owner: Option<SymbolId>,
        primary_span: Option<Span>,
        is_primary: bool,
        is_exported: bool,
        stable_key: StableKeyId,
        precision: SymbolPrecision,
    ) -> Self {
        Self {
            id,
            symbol,
            language,
            name,
            qualified_name,
            kind,
            namespace,
            file,
            package,
            module,
            owner,
            primary_span,
            is_primary,
            is_exported,
            stable_key,
            precision,
        }
    }
}

impl ReferenceFact {
    /// Constructs a reference fact from its complete fields.
    #[expect(
        clippy::too_many_arguments,
        reason = "This constructor mirrors the complete non-exhaustive public fact schema."
    )]
    pub fn new(
        id: ReferenceId,
        language: Language,
        name: String,
        qualified_name: String,
        kind: ReferenceKind,
        namespace: SymbolNamespace,
        file: Option<FileId>,
        package: Option<PackageId>,
        module: Option<ModuleNodeId>,
        owner: Option<SymbolId>,
        primary_span: Option<Span>,
        target: Option<SymbolId>,
        candidates: Vec<SymbolId>,
        stable_key: StableKeyId,
        status: SymbolResolutionStatus,
        precision: SymbolPrecision,
    ) -> Self {
        Self {
            id,
            language,
            name,
            qualified_name,
            kind,
            namespace,
            file,
            package,
            module,
            owner,
            primary_span,
            target,
            candidates,
            stable_key,
            status,
            precision,
        }
    }
}
