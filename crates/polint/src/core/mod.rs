use crate::diagnostics::{
    Diagnostic, Severity, TextRange as DiagnosticRange, dedupe_diagnostics, fingerprint,
};
use crate::rule_error::RuleResult;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Arbitrary per-rule configuration value from `.polint.toml`.
///
/// Rules read these through [`RuleOptions::settings`] when the built-in shortcut
/// fields (`max`, `deny`, `forbidden_imports`, etc.) are not expressive enough.
pub type RuleConfigValue = toml::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FileId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FunctionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PackageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BranchId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ImportId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResolvedImportId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ModuleNodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ModuleEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DefinitionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReferenceId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RuleId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Go,
    TypeScript,
    Tsx,
    JavaScript,
    Jsx,
    Unknown,
}

impl Language {
    pub fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
        {
            "go" => Self::Go,
            "ts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "js" => Self::JavaScript,
            "jsx" => Self::Jsx,
            _ => Self::Unknown,
        }
    }

    pub fn is_ts_family(self) -> bool {
        matches!(
            self,
            Self::TypeScript | Self::Tsx | Self::JavaScript | Self::Jsx
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub file: FileId,
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Span {
    pub fn point(file: FileId, line: u32, col: u32) -> Self {
        Self {
            file,
            start_byte: 0,
            end_byte: 0,
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col,
        }
    }

    pub fn diagnostic_range(&self) -> DiagnosticRange {
        DiagnosticRange {
            start_line: self.start_line,
            start_col: self.start_col,
            end_line: self.end_line,
            end_col: self.end_col,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: FileId,
    pub path: PathBuf,
    pub relative_path: String,
    pub language: Language,
    pub source: Arc<str>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Source-file size and aggregate function metrics derived from parsed facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct ComplexityMetricFact {
    pub function: FunctionId,
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub language: Language,
    pub cyclomatic_complexity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageFact {
    pub id: PackageId,
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub stable_key: String,
    pub precision: SymbolPrecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub stable_key: String,
    pub precision: SymbolPrecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub stable_key: String,
    pub status: SymbolResolutionStatus,
    pub precision: SymbolPrecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct CoverageFact {
    pub branch: BranchId,
    pub covered: Option<bool>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsComponentFact {
    pub file: FileId,
    pub function: Option<FunctionId>,
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsClassFact {
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub is_exported: bool,
    pub is_component_like: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringLiteralFact {
    pub file: FileId,
    pub value: String,
    pub span: Span,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Default, Clone)]
pub struct AnalysisDb {
    files: Vec<SourceFile>,
    packages: Vec<PackageFact>,
    functions: Vec<FunctionFact>,
    imports: Vec<ImportFact>,
    resolved_imports: Vec<ResolvedImportFact>,
    module_nodes: Vec<ModuleNode>,
    module_edges: Vec<ModuleEdge>,
    symbols: Vec<SymbolFact>,
    definitions: Vec<DefinitionFact>,
    references: Vec<ReferenceFact>,
    symbols_by_id: BTreeMap<SymbolId, usize>,
    definitions_by_symbol: BTreeMap<SymbolId, Vec<usize>>,
    references_by_target: BTreeMap<SymbolId, Vec<usize>>,
    symbols_by_file: BTreeMap<FileId, Vec<usize>>,
    references_by_file: BTreeMap<FileId, Vec<usize>>,
    symbols_by_name: BTreeMap<String, Vec<usize>>,
    branches: Vec<BranchObligation>,
    tests: Vec<TestFact>,
    coverage: Vec<CoverageFact>,
    file_metrics: Vec<FileMetricFact>,
    function_metrics: Vec<FunctionMetricFact>,
    complexity_metrics: Vec<ComplexityMetricFact>,
    ts_components: Vec<TsComponentFact>,
    ts_classes: Vec<TsClassFact>,
    string_literals: Vec<StringLiteralFact>,
    jsx_attributes: Vec<JsxAttributeFact>,
    path_contexts: Option<crate::path_context::PathContextIndex>,
}

impl AnalysisDb {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self, path: PathBuf, relative_path: String, source: String) -> FileId {
        let language = Language::from_path(&path);
        let content_hash = fingerprint(&[&source]);
        self.push_source_file(
            path,
            relative_path,
            language,
            Arc::from(source),
            content_hash,
        )
    }

    pub fn add_source_file(
        &mut self,
        path: PathBuf,
        relative_path: String,
        language: Language,
        source: Arc<str>,
        content_hash: String,
    ) -> FileId {
        self.push_source_file(path, relative_path, language, source, content_hash)
    }

    fn push_source_file(
        &mut self,
        path: PathBuf,
        relative_path: String,
        language: Language,
        source: Arc<str>,
        content_hash: String,
    ) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile {
            id,
            path,
            relative_path,
            language,
            source,
            content_hash,
        });
        id
    }

    pub fn push_package(&mut self, mut fact: PackageFact) -> PackageId {
        let id = PackageId(self.packages.len() as u64);
        fact.id = id;
        self.packages.push(fact);
        id
    }

    pub fn push_function(&mut self, mut fact: FunctionFact) -> FunctionId {
        let id = FunctionId(self.functions.len() as u64);
        fact.id = id;
        self.functions.push(fact);
        id
    }

    pub fn push_import(&mut self, mut fact: ImportFact) -> ImportId {
        let id = ImportId(self.imports.len() as u64);
        fact.id = id;
        self.imports.push(fact);
        id
    }

    pub fn push_branch(&mut self, mut fact: BranchObligation) -> BranchId {
        let id = BranchId(self.branches.len() as u64);
        fact.id = id;
        self.branches.push(fact);
        id
    }

    pub fn push_test(&mut self, fact: TestFact) {
        self.tests.push(fact);
    }

    pub fn push_coverage(&mut self, fact: CoverageFact) {
        self.coverage.push(fact);
    }

    pub(crate) fn replace_metric_facts(
        &mut self,
        file_metrics: Vec<FileMetricFact>,
        function_metrics: Vec<FunctionMetricFact>,
        complexity_metrics: Vec<ComplexityMetricFact>,
    ) {
        self.file_metrics = file_metrics;
        self.function_metrics = function_metrics;
        self.complexity_metrics = complexity_metrics;
    }

    pub(crate) fn replace_module_graph_facts(
        &mut self,
        mut resolved_imports: Vec<ResolvedImportFact>,
        mut module_nodes: Vec<ModuleNode>,
        mut module_edges: Vec<ModuleEdge>,
    ) {
        for (index, fact) in resolved_imports.iter_mut().enumerate() {
            fact.id = ResolvedImportId(index as u64);
        }
        for (index, node) in module_nodes.iter_mut().enumerate() {
            node.id = ModuleNodeId(index as u64);
        }
        for (index, edge) in module_edges.iter_mut().enumerate() {
            edge.id = ModuleEdgeId(index as u64);
        }

        self.resolved_imports = resolved_imports;
        self.module_nodes = module_nodes;
        self.module_edges = module_edges;
    }

    pub(crate) fn replace_symbol_graph_facts(
        &mut self,
        symbols: Vec<SymbolFact>,
        definitions: Vec<DefinitionFact>,
        references: Vec<ReferenceFact>,
    ) {
        self.symbols = symbols;
        self.definitions = definitions;
        self.references = references;
        self.rebuild_symbol_graph_indexes();
    }

    fn rebuild_symbol_graph_indexes(&mut self) {
        self.symbols_by_id.clear();
        self.definitions_by_symbol.clear();
        self.references_by_target.clear();
        self.symbols_by_file.clear();
        self.references_by_file.clear();
        self.symbols_by_name.clear();

        for (index, symbol) in self.symbols.iter().enumerate() {
            self.symbols_by_id.insert(symbol.id, index);
            if let Some(file) = symbol.file {
                self.symbols_by_file.entry(file).or_default().push(index);
            }
            self.symbols_by_name
                .entry(symbol.name.clone())
                .or_default()
                .push(index);
        }

        for (index, definition) in self.definitions.iter().enumerate() {
            self.definitions_by_symbol
                .entry(definition.symbol)
                .or_default()
                .push(index);
        }

        for (index, reference) in self.references.iter().enumerate() {
            if let Some(target) = reference.target {
                self.references_by_target
                    .entry(target)
                    .or_default()
                    .push(index);
            }
            if let Some(file) = reference.file {
                self.references_by_file.entry(file).or_default().push(index);
            }
        }

        let symbols = &self.symbols;
        for indexes in self.symbols_by_file.values_mut() {
            indexes.sort_by_key(|index| symbols[*index].id);
        }
        for indexes in self.symbols_by_name.values_mut() {
            indexes.sort_by_key(|index| symbols[*index].id);
        }

        let definitions = &self.definitions;
        for indexes in self.definitions_by_symbol.values_mut() {
            indexes.sort_by_key(|index| definitions[*index].id);
        }

        let references = &self.references;
        for indexes in self.references_by_target.values_mut() {
            indexes.sort_by_key(|index| references[*index].id);
        }
        for indexes in self.references_by_file.values_mut() {
            indexes.sort_by_key(|index| references[*index].id);
        }
    }

    pub fn push_ts_component(&mut self, fact: TsComponentFact) {
        self.ts_components.push(fact);
    }

    pub fn push_ts_class(&mut self, fact: TsClassFact) {
        self.ts_classes.push(fact);
    }

    pub fn push_string_literal(&mut self, fact: StringLiteralFact) {
        self.string_literals.push(fact);
    }

    pub fn push_jsx_attribute(&mut self, fact: JsxAttributeFact) {
        self.jsx_attributes.push(fact);
    }

    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    pub(crate) fn set_path_contexts(&mut self, index: crate::path_context::PathContextIndex) {
        self.path_contexts = Some(index);
    }

    /// Repo-relative paths paired with `relative_path` (see `.polint.toml` `[path_contexts]`).
    pub fn path_context_related(&self, pair_name: &str, relative_path: &str) -> Vec<String> {
        self.path_contexts
            .as_ref()
            .map(|ix| ix.related_paths(pair_name, relative_path))
            .unwrap_or_default()
    }

    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }

    /// Relative paths as in diagnostics (`SourceFile.relative_path`) → full source text.
    pub fn sources_by_relative_path(&self) -> BTreeMap<String, Arc<str>> {
        self.files
            .iter()
            .map(|file| (file.relative_path.clone(), Arc::clone(&file.source)))
            .collect()
    }

    pub fn packages(&self) -> &[PackageFact] {
        &self.packages
    }

    pub fn functions(&self) -> &[FunctionFact] {
        &self.functions
    }

    pub fn imports(&self) -> &[ImportFact] {
        &self.imports
    }

    pub fn resolved_imports(&self) -> &[ResolvedImportFact] {
        &self.resolved_imports
    }

    pub fn module_nodes(&self) -> &[ModuleNode] {
        &self.module_nodes
    }

    pub fn module_edges(&self) -> &[ModuleEdge] {
        &self.module_edges
    }

    pub fn symbols(&self) -> &[SymbolFact] {
        &self.symbols
    }

    pub fn definitions(&self) -> &[DefinitionFact] {
        &self.definitions
    }

    pub fn references(&self) -> &[ReferenceFact] {
        &self.references
    }

    pub(crate) fn symbol_by_id(&self, id: SymbolId) -> Option<&SymbolFact> {
        self.symbols_by_id
            .get(&id)
            .and_then(|index| self.symbols.get(*index))
    }

    pub(crate) fn symbols_for_file(
        &self,
        file: FileId,
    ) -> impl Iterator<Item = &SymbolFact> + '_ {
        self.symbols_by_file
            .get(&file)
            .into_iter()
            .flat_map(|indexes| indexes.iter().filter_map(|index| self.symbols.get(*index)))
    }

    pub(crate) fn symbols_by_name(&self, name: &str) -> impl Iterator<Item = &SymbolFact> + '_ {
        self.symbols_by_name
            .get(name)
            .into_iter()
            .flat_map(|indexes| indexes.iter().filter_map(|index| self.symbols.get(*index)))
    }

    pub(crate) fn definition_for_symbol(&self, symbol: SymbolId) -> Option<&DefinitionFact> {
        let mut definitions = self.definitions_for_symbol(symbol);
        let first = definitions.next();
        first
            .filter(|definition| definition.is_primary)
            .or_else(|| definitions.find(|definition| definition.is_primary))
            .or(first)
    }

    pub(crate) fn definitions_for_symbol(
        &self,
        symbol: SymbolId,
    ) -> impl Iterator<Item = &DefinitionFact> + '_ {
        self.definitions_by_symbol
            .get(&symbol)
            .into_iter()
            .flat_map(|indexes| {
                indexes
                    .iter()
                    .filter_map(|index| self.definitions.get(*index))
            })
    }

    pub(crate) fn references_to_symbol(
        &self,
        symbol: SymbolId,
    ) -> impl Iterator<Item = &ReferenceFact> + '_ {
        self.references_by_target
            .get(&symbol)
            .into_iter()
            .flat_map(|indexes| {
                indexes.iter().filter_map(|index| self.references.get(*index))
            })
    }

    pub(crate) fn references_for_file(
        &self,
        file: FileId,
    ) -> impl Iterator<Item = &ReferenceFact> + '_ {
        self.references_by_file
            .get(&file)
            .into_iter()
            .flat_map(|indexes| {
                indexes.iter().filter_map(|index| self.references.get(*index))
            })
    }

    pub fn branches(&self) -> &[BranchObligation] {
        &self.branches
    }

    pub fn tests(&self) -> &[TestFact] {
        &self.tests
    }

    pub fn coverage(&self) -> &[CoverageFact] {
        &self.coverage
    }

    pub fn file_metrics(&self) -> &[FileMetricFact] {
        &self.file_metrics
    }

    pub fn function_metrics(&self) -> &[FunctionMetricFact] {
        &self.function_metrics
    }

    pub fn complexity_metrics(&self) -> &[ComplexityMetricFact] {
        &self.complexity_metrics
    }

    pub fn ts_components(&self) -> &[TsComponentFact] {
        &self.ts_components
    }

    pub fn ts_classes(&self) -> &[TsClassFact] {
        &self.ts_classes
    }

    pub fn string_literals(&self) -> &[StringLiteralFact] {
        &self.string_literals
    }

    pub fn jsx_attributes(&self) -> &[JsxAttributeFact] {
        &self.jsx_attributes
    }

    pub fn path_for(&self, file: FileId) -> String {
        self.file(file)
            .map(|file| file.relative_path.clone())
            .unwrap_or_else(|| "<unknown>".to_string())
    }

    pub fn facts_for_file(&self, file: FileId) -> CachedFileFacts {
        let branch_ids = self
            .branches
            .iter()
            .filter(|branch| branch.file == file)
            .map(|branch| branch.id)
            .collect::<BTreeSet<_>>();
        CachedFileFacts {
            packages: self
                .packages
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            functions: self
                .functions
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            imports: self
                .imports
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            branches: self
                .branches
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            tests: self
                .tests
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            coverage: self
                .coverage
                .iter()
                .filter(|fact| branch_ids.contains(&fact.branch))
                .cloned()
                .collect(),
            ts_components: self
                .ts_components
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            ts_classes: self
                .ts_classes
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            string_literals: self
                .string_literals
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            jsx_attributes: self
                .jsx_attributes
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
        }
    }

    pub fn restore_file_facts(&mut self, file: FileId, facts: CachedFileFacts) {
        let mut function_ids = BTreeMap::new();
        let mut branch_ids = BTreeMap::new();

        for mut package in facts.packages {
            package.file = file;
            package.span.file = file;
            self.push_package(package);
        }

        for mut function in facts.functions {
            let cached_id = function.id;
            function.file = file;
            function.span.file = file;
            let restored_id = self.push_function(function);
            function_ids.insert(cached_id, restored_id);
        }

        for mut import in facts.imports {
            import.file = file;
            import.span.file = file;
            self.push_import(import);
        }

        for mut branch in facts.branches {
            let cached_id = branch.id;
            branch.file = file;
            branch.function = branch
                .function
                .and_then(|function| function_ids.get(&function).copied());
            branch.decision_span.file = file;
            let restored_id = self.push_branch(branch);
            branch_ids.insert(cached_id, restored_id);
        }

        for mut test in facts.tests {
            test.file = file;
            test.function = test
                .function
                .and_then(|function| function_ids.get(&function).copied());
            test.span.file = file;
            self.push_test(test);
        }

        for mut coverage in facts.coverage {
            if let Some(branch) = branch_ids.get(&coverage.branch).copied() {
                coverage.branch = branch;
                self.push_coverage(coverage);
            }
        }

        for mut component in facts.ts_components {
            component.file = file;
            component.function = component
                .function
                .and_then(|function| function_ids.get(&function).copied());
            component.span.file = file;
            self.push_ts_component(component);
        }

        for mut class in facts.ts_classes {
            class.file = file;
            class.span.file = file;
            self.push_ts_class(class);
        }

        for mut literal in facts.string_literals {
            literal.file = file;
            literal.span.file = file;
            self.push_string_literal(literal);
        }

        for mut attribute in facts.jsx_attributes {
            attribute.file = file;
            attribute.span.file = file;
            self.push_jsx_attribute(attribute);
        }
    }
}

/// Static metadata for a rule as shown in diagnostics, config, and registries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMeta {
    pub id: String,
    pub description: String,
    pub severity: Severity,
}

/// Fact families a rule wants the host to provide.
///
/// Capabilities are declarative: they describe which analysis facts a rule
/// consumes without changing the `Rule` trait. The current host may harvest a
/// superset of facts for a language; capability names are still the public
/// contract a rule should declare and docs should not imply unavailable facts
/// are produced.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Capabilities {
    /// Needs source files and syntax-derived facts. Currently descriptive; adapters still run their standard fact harvesters.
    pub syntax: bool,
    /// Needs syntactic import facts.
    pub imports: bool,
    /// Needs setup-aware resolved import facts.
    pub resolved_imports: bool,
    /// Needs file/package/module relationship graph facts.
    pub module_graph: bool,
    /// Needs symbol identities and definition facts.
    pub symbols: bool,
    /// Needs symbol reference facts; this also requires symbol identities.
    pub references: bool,
    /// Reserved for future control-flow graph facts. Branch obligations are available through [`Capabilities::branch_obligations`].
    pub cfg: bool,
    /// Reserved for future call graph facts. Direct syntactic calls are available on [`FunctionFact::calls`].
    pub call_graph: bool,
    /// Reserved for future dataflow facts built on CFG, symbols, and call graph support.
    pub dataflow: bool,
    /// Needs Go test facts harvested from `_test.go` files.
    pub go_tests: bool,
    /// Needs syntax-level branch obligation facts.
    pub branch_obligations: bool,
    /// Reserved for future external coverage imports.
    pub coverage_facts: bool,
    /// Needs aggregate-like Go test metrics currently stored on [`TestFact`].
    pub test_suite_metrics: bool,
    /// Needs derived source-file size and aggregate function metrics.
    pub file_metrics: bool,
    /// Needs derived per-function size metrics.
    pub function_metrics: bool,
    /// Needs derived per-function complexity metrics.
    pub complexity_metrics: bool,
    /// Needs TypeScript/JavaScript component-like function facts.
    pub ts_components: bool,
    /// Needs TypeScript/JavaScript class facts.
    pub ts_classes: bool,
    /// Needs string and regex literal facts.
    pub string_literals: bool,
    /// Needs JSX attribute facts.
    pub jsx_attributes: bool,
}

impl Capabilities {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn syntax(mut self) -> Self {
        self.syntax = true;
        self
    }

    pub fn imports(mut self) -> Self {
        self.imports = true;
        self
    }

    pub fn resolved_imports(mut self) -> Self {
        self.resolved_imports = true;
        self
    }

    pub fn module_graph(mut self) -> Self {
        self.module_graph = true;
        self
    }

    pub fn symbols(mut self) -> Self {
        self.symbols = true;
        self
    }

    pub fn references(mut self) -> Self {
        self.references = true;
        self.symbols = true;
        self
    }

    pub fn cfg(mut self) -> Self {
        self.cfg = true;
        self
    }

    pub fn call_graph(mut self) -> Self {
        self.call_graph = true;
        self
    }

    pub fn dataflow(mut self) -> Self {
        self.dataflow = true;
        self
    }

    pub fn go_tests(mut self) -> Self {
        self.go_tests = true;
        self
    }

    pub fn branch_obligations(mut self) -> Self {
        self.branch_obligations = true;
        self
    }

    pub fn coverage_facts(mut self) -> Self {
        self.coverage_facts = true;
        self
    }

    pub fn test_suite_metrics(mut self) -> Self {
        self.test_suite_metrics = true;
        self
    }

    pub fn file_metrics(mut self) -> Self {
        self.file_metrics = true;
        self
    }

    pub fn function_metrics(mut self) -> Self {
        self.function_metrics = true;
        self
    }

    pub fn complexity_metrics(mut self) -> Self {
        self.complexity_metrics = true;
        self
    }

    pub fn ts_components(mut self) -> Self {
        self.ts_components = true;
        self
    }

    pub fn ts_classes(mut self) -> Self {
        self.ts_classes = true;
        self
    }

    pub fn string_literals(mut self) -> Self {
        self.string_literals = true;
        self
    }

    pub fn jsx_attributes(mut self) -> Self {
        self.jsx_attributes = true;
        self
    }

    pub(crate) fn requested_names(self) -> impl Iterator<Item = &'static str> {
        [
            ("syntax", self.syntax),
            ("imports", self.imports),
            ("resolved_imports", self.resolved_imports),
            ("module_graph", self.module_graph),
            ("symbols", self.symbols),
            ("references", self.references),
            ("cfg", self.cfg),
            ("call_graph", self.call_graph),
            ("dataflow", self.dataflow),
            ("go_tests", self.go_tests),
            ("branch_obligations", self.branch_obligations),
            ("coverage_facts", self.coverage_facts),
            ("test_suite_metrics", self.test_suite_metrics),
            ("file_metrics", self.file_metrics),
            ("function_metrics", self.function_metrics),
            ("complexity_metrics", self.complexity_metrics),
            ("ts_components", self.ts_components),
            ("ts_classes", self.ts_classes),
            ("string_literals", self.string_literals),
            ("jsx_attributes", self.jsx_attributes),
        ]
        .into_iter()
        .filter_map(|(name, requested)| requested.then_some(name))
    }
}

/// Support state for a requested capability in the resolved analysis plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilitySupportStatus {
    /// The host can provide this capability for the current plan.
    Supported,
    /// The capability is known but not implemented or not requestable yet.
    Unsupported,
    /// The capability is implemented but required local setup is missing.
    SetupMissing,
}

/// Read-only support information for one capability row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySupport {
    /// Stable capability name, such as `imports` or `cfg`.
    pub capability: String,
    /// Language this support row applies to, if language-specific.
    pub language: Option<Language>,
    /// Current support status for the capability row.
    pub status: CapabilitySupportStatus,
    /// Rule IDs that requested the capability.
    pub rules: Vec<String>,
    /// Deterministic explanation for unsupported or setup-missing rows.
    pub reason: Option<String>,
    /// Actionable remediation hint, when available.
    pub hint: Option<String>,
    /// Repository docs path for more context, when available.
    pub docs_path: Option<String>,
}

/// Read-only capability support rows exposed to rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySupportView {
    entries: Vec<CapabilitySupport>,
}

impl CapabilitySupportView {
    /// Returns an empty support view for compatibility paths that do not build a plan.
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Creates a support view from deterministic support rows.
    pub fn new(entries: Vec<CapabilitySupport>) -> Self {
        Self { entries }
    }

    /// Returns support rows in deterministic plan order.
    pub fn entries(&self) -> &[CapabilitySupport] {
        &self.entries
    }

    /// Returns the first status for a capability name, if present.
    pub fn status_for(&self, capability: &str) -> Option<CapabilitySupportStatus> {
        self.entries
            .iter()
            .find(|entry| entry.capability == capability)
            .map(|entry| entry.status.clone())
    }
}

type RuleMetaFn = dyn Fn() -> RuleMeta + Send + Sync;
type RuleCapabilitiesFn = dyn Fn() -> Capabilities + Send + Sync;
type RuleRunFn = dyn Fn(&AnalysisDb, &mut RuleCtx<'_>) -> RuleResult + Send + Sync;

/// Opaque static-analysis rule registered with the runner.
///
/// Rule authors create this through `#[polint::rule]`. The value is intentionally
/// not a trait: repo-local rules must be written in the analyzable macro shape so
/// capabilities come from typed fact-view parameters.
#[derive(Clone)]
pub struct Rule {
    meta: Arc<RuleMetaFn>,
    capabilities: Arc<RuleCapabilitiesFn>,
    run: Arc<RuleRunFn>,
}

impl Rule {
    pub(crate) fn from_parts<M, C, R>(meta: M, capabilities: C, run: R) -> Self
    where
        M: Fn() -> RuleMeta + Send + Sync + 'static,
        C: Fn() -> Capabilities + Send + Sync + 'static,
        R: Fn(&AnalysisDb, &mut RuleCtx<'_>) -> RuleResult + Send + Sync + 'static,
    {
        Self {
            meta: Arc::new(meta),
            capabilities: Arc::new(capabilities),
            run: Arc::new(run),
        }
    }

    pub(crate) fn meta(&self) -> RuleMeta {
        (self.meta)()
    }

    pub(crate) fn capabilities(&self) -> Capabilities {
        (self.capabilities)()
    }

    pub(crate) fn run(&self, db: &AnalysisDb, ctx: &mut RuleCtx<'_>) -> RuleResult {
        (self.run)(db, ctx)
    }
}

/// Resolved per-rule options from configuration.
///
/// Built-in and repo-local rules can read these values through
/// [`RuleCtx::options`] to apply severity overrides, file filters, thresholds,
/// denied values, or import-boundary settings.
#[derive(Debug, Clone, Default)]
pub struct RuleOptions {
    /// Severity override from config, if any.
    pub severity: Option<Severity>,
    /// File include globs for this rule.
    pub files: Vec<String>,
    /// File globs to skip for this rule.
    pub allow_files: Vec<String>,
    /// Rule-defined allow list. Helpers treat this as exact file paths only when documented.
    pub allow: Vec<String>,
    /// Common numeric threshold used by example complexity/test-size rules.
    pub max: Option<u32>,
    /// Common deny list used by literal/pattern rules.
    pub deny: Vec<String>,
    /// Common import-boundary map: source glob -> forbidden import patterns.
    pub forbidden_imports: BTreeMap<String, Vec<String>>,
    /// Arbitrary extra fields from this rule's `[[rules.config]]` table.
    pub settings: BTreeMap<String, RuleConfigValue>,
}

/// Borrowed execution context passed to a single rule run.
///
/// The context owns reporting, options, path lookup, and support metadata.
/// Analysis facts are exposed through typed SDK fact views requested in a
/// `#[polint::rule]` function signature.
pub struct RuleCtx<'a> {
    db: &'a AnalysisDb,
    diagnostics: Vec<Diagnostic>,
    rule: RuleMeta,
    options: RuleOptions,
    capability_support: CapabilitySupportView,
}

impl<'a> RuleCtx<'a> {
    /// Creates a rule context for one rule execution.
    #[cfg(test)]
    pub(crate) fn new(db: &'a AnalysisDb, rule: RuleMeta, options: RuleOptions) -> Self {
        Self::with_capability_support(db, rule, options, CapabilitySupportView::empty())
    }

    pub(crate) fn with_capability_support(
        db: &'a AnalysisDb,
        rule: RuleMeta,
        options: RuleOptions,
        capability_support: CapabilitySupportView,
    ) -> Self {
        Self {
            db,
            diagnostics: Vec::new(),
            rule,
            options,
            capability_support,
        }
    }

    /// Returns resolved options for the current rule.
    pub fn options(&self) -> &RuleOptions {
        &self.options
    }

    /// Returns the stable ID of the current rule.
    pub fn rule_id(&self) -> &str {
        &self.rule.id
    }

    /// Returns read-only capability support information for the resolved plan.
    pub fn capability_support(&self) -> &CapabilitySupportView {
        &self.capability_support
    }

    /// Paths paired with `file`'s relative path under the named `[path_contexts]` rule.
    pub fn path_context_related(&self, pair_name: &str, file: FileId) -> Vec<String> {
        let Some(source) = self.db.file(file) else {
            return Vec::new();
        };
        self.db
            .path_context_related(pair_name, &source.relative_path)
    }

    /// Returns a display path for a file ID, or `<unknown>` if missing.
    pub fn file_path(&self, file: FileId) -> String {
        self.db.path_for(file)
    }

    /// Adds a diagnostic for the current rule, applying severity overrides.
    pub fn report(&mut self, mut diagnostic: Diagnostic) {
        if let Some(severity) = self.options.severity {
            diagnostic.severity = severity;
        }
        self.diagnostics.push(diagnostic);
    }

    /// Reports an error diagnostic at a core span.
    pub fn error(&mut self, span: &Span, message: impl Into<String>) {
        let file = self.file_path(span.file);
        self.report(Diagnostic::error(
            self.rule.id.clone(),
            file,
            span.diagnostic_range(),
            message,
        ));
    }

    /// Reports a warning diagnostic at a core span.
    pub fn warn(&mut self, span: &Span, message: impl Into<String>) {
        let file = self.file_path(span.file);
        self.report(Diagnostic::warning(
            self.rule.id.clone(),
            file,
            span.diagnostic_range(),
            message,
        ));
    }

    /// Consumes the context and returns buffered diagnostics.
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// Test-only registry that exposes the [`Capabilities`]/[`Rule`] shape independent of `run_rules`.
/// Production runs use [`run_rules`] directly with `&[Rule]`.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct RuleRegistry {
    rules: Vec<Rule>,
}

#[cfg(test)]
impl RuleRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub(crate) fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

// Intentionally `pub` (not `pub(crate)`): `polint::_bench::core` glob-re-exports this for
// `polint-bench`, an external crate. `unreachable_pub` does not follow that path.
#[allow(unreachable_pub)]
pub fn run_rules(
    db: &AnalysisDb,
    rules: &[Rule],
    options: &BTreeMap<String, RuleOptions>,
    enabled: Option<&BTreeSet<String>>,
    parallel: bool,
) -> Vec<Diagnostic> {
    let capability_support = CapabilitySupportView::empty();
    run_rules_with_capability_support(db, rules, options, enabled, parallel, &capability_support)
}

pub(crate) fn run_rules_with_capability_support(
    db: &AnalysisDb,
    rules: &[Rule],
    options: &BTreeMap<String, RuleOptions>,
    enabled: Option<&BTreeSet<String>>,
    parallel: bool,
    capability_support: &CapabilitySupportView,
) -> Vec<Diagnostic> {
    let run_one = |rule: &Rule| {
        let meta = match catch_unwind(AssertUnwindSafe(|| rule.meta())) {
            Ok(meta) => meta,
            Err(_) => {
                return vec![internal_rule_error_for_id(
                    db,
                    "unknown",
                    "rule metadata panicked".to_string(),
                )];
            }
        };
        if let Some(enabled) = enabled
            && !enabled
                .iter()
                .any(|pattern| rule_id_matches(pattern, &meta.id))
        {
            return Vec::new();
        }
        if has_blocking_capability(&meta.id, capability_support) {
            return Vec::new();
        }
        let rule_options = options.get(&meta.id).cloned().unwrap_or_default();
        let mut ctx = RuleCtx::with_capability_support(
            db,
            meta.clone(),
            rule_options,
            capability_support.clone(),
        );
        let result = catch_unwind(AssertUnwindSafe(|| rule.run(db, &mut ctx)));
        match result {
            Ok(Ok(())) => ctx.into_diagnostics(),
            Ok(Err(error)) => vec![internal_rule_error(db, &meta, error.to_string())],
            Err(_) => vec![internal_rule_error(db, &meta, "rule panicked".to_string())],
        }
    };

    let diagnostics = if parallel {
        rules
            .par_iter()
            .map(run_one)
            .collect::<Vec<_>>()
            .into_iter()
            .flatten()
            .collect()
    } else {
        rules.iter().flat_map(run_one).collect()
    };

    dedupe_diagnostics(diagnostics)
}

fn has_blocking_capability(rule_id: &str, support: &CapabilitySupportView) -> bool {
    support.entries.iter().any(|entry| {
        entry.rules.iter().any(|entry_rule| entry_rule == rule_id)
            && entry.status != CapabilitySupportStatus::Supported
    })
}

fn internal_rule_error(db: &AnalysisDb, meta: &RuleMeta, message: String) -> Diagnostic {
    internal_rule_error_for_id(db, &meta.id, message)
}

fn internal_rule_error_for_id(db: &AnalysisDb, rule_id: &str, message: String) -> Diagnostic {
    let (file, range) = db
        .files()
        .first()
        .map(|file| (file.relative_path.clone(), DiagnosticRange::point(1, 1)))
        .unwrap_or_else(|| ("<workspace>".to_string(), DiagnosticRange::point(1, 1)));

    Diagnostic::error(
        format!("internal/{rule_id}"),
        file,
        range,
        format!("Rule `{rule_id}` failed: {message}"),
    )
}

pub(crate) fn rule_id_matches(pattern: &str, rule_id: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return rule_id.starts_with(&format!("{prefix}/"));
    }
    pattern == rule_id
}

pub(crate) fn span_from_byte_range(
    file: FileId,
    source: &str,
    start_byte: usize,
    end_byte: usize,
) -> Span {
    let start_byte = start_byte.min(source.len());
    let end_byte = end_byte.min(source.len()).max(start_byte);
    let (start_line, start_col) = line_col(source, start_byte);
    let (end_line, end_col) = line_col(source, end_byte);
    Span {
        file,
        start_byte: start_byte as u32,
        end_byte: end_byte as u32,
        start_line,
        start_col,
        end_line,
        end_col,
    }
}

pub(crate) fn line_col(source: &str, byte_offset: usize) -> (u32, u32) {
    let mut line = 1_u32;
    let mut col = 1_u32;
    let limit = byte_offset.min(source.len());
    for (idx, ch) in source.char_indices() {
        if idx >= limit {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::facts::{
        BranchObligations, FactView, Functions, GoTests, Imports, JsxAttributes, Packages,
        SourceFiles, StringLiterals, TsClasses, TsComponents,
    };
    use anyhow::anyhow;
    use proptest::prelude::*;
    use std::thread;
    use std::time::Duration;

    #[derive(Clone, Copy)]
    enum TestRuleBehavior {
        Report,
        Error,
        Panic,
        MetaPanic,
    }

    #[derive(Clone, Copy)]
    struct TestRule {
        id: &'static str,
        capabilities: Capabilities,
        severity: Severity,
        message: &'static str,
        fingerprint: &'static str,
        delay: Duration,
        behavior: TestRuleBehavior,
    }

    impl TestRule {
        fn report(id: &'static str, severity: Severity, fingerprint: &'static str) -> Self {
            Self {
                id,
                capabilities: Capabilities::new().syntax(),
                severity,
                message: "test diagnostic",
                fingerprint,
                delay: Duration::ZERO,
                behavior: TestRuleBehavior::Report,
            }
        }

        fn with_capabilities(mut self, capabilities: Capabilities) -> Self {
            self.capabilities = capabilities;
            self
        }

        fn with_message(mut self, message: &'static str) -> Self {
            self.message = message;
            self
        }

        fn with_delay(mut self, delay: Duration) -> Self {
            self.delay = delay;
            self
        }

        fn error(id: &'static str) -> Self {
            Self {
                id,
                capabilities: Capabilities::new(),
                severity: Severity::Error,
                message: "rule returned an error",
                fingerprint: "error",
                delay: Duration::ZERO,
                behavior: TestRuleBehavior::Error,
            }
        }

        fn panic(id: &'static str) -> Self {
            Self {
                id,
                capabilities: Capabilities::new(),
                severity: Severity::Error,
                message: "rule panicked",
                fingerprint: "panic",
                delay: Duration::ZERO,
                behavior: TestRuleBehavior::Panic,
            }
        }

        fn meta_panic() -> Self {
            Self {
                id: "examples/meta-panic",
                capabilities: Capabilities::new(),
                severity: Severity::Error,
                message: "metadata panicked",
                fingerprint: "meta-panic",
                delay: Duration::ZERO,
                behavior: TestRuleBehavior::MetaPanic,
            }
        }

        fn into_rule(self) -> Rule {
            let meta_rule = self;
            let capabilities_rule = self;
            let run_rule = self;
            Rule::from_parts(
                move || meta_rule.meta(),
                move || capabilities_rule.capabilities,
                move |_db, ctx| run_rule.run(ctx),
            )
        }

        fn meta(self) -> RuleMeta {
            if matches!(self.behavior, TestRuleBehavior::MetaPanic) {
                panic!("intentional metadata panic");
            }

            RuleMeta {
                id: self.id.to_string(),
                description: format!("Test rule {}", self.id),
                severity: self.severity,
            }
        }

        fn run(self, ctx: &mut RuleCtx<'_>) -> RuleResult {
            if !self.delay.is_zero() {
                thread::sleep(self.delay);
            }

            match self.behavior {
                TestRuleBehavior::Report => {
                    ctx.report(
                        Diagnostic::new(
                            self.id,
                            self.severity,
                            "src/main.go",
                            DiagnosticRange::point(1, 1),
                            self.message,
                        )
                        .with_fingerprint(self.fingerprint),
                    );
                    Ok(())
                }
                TestRuleBehavior::Error => Err(anyhow!("intentional rule error").into()),
                TestRuleBehavior::Panic => panic!("intentional rule panic"),
                TestRuleBehavior::MetaPanic => panic!("intentional metadata panic"),
            }
        }
    }

    fn test_span(file: FileId, line: u32) -> Span {
        Span {
            file,
            start_byte: 0,
            end_byte: 1,
            start_line: line,
            start_col: 1,
            end_line: line,
            end_col: 2,
        }
    }

    #[test]
    fn capability_support_view_reports_status_for_capability() {
        let view = CapabilitySupportView::new(vec![CapabilitySupport {
            capability: "imports".to_string(),
            language: Some(Language::Go),
            status: CapabilitySupportStatus::Supported,
            rules: vec!["examples/imports".to_string()],
            reason: None,
            hint: None,
            docs_path: None,
        }]);

        assert_eq!(
            view.status_for("imports"),
            Some(CapabilitySupportStatus::Supported)
        );
        assert!(view.status_for("cfg").is_none());
        assert_eq!(view.entries().len(), 1);
    }

    #[test]
    fn capability_support_defaults_empty_for_rule_ctx_constructor() {
        let db = AnalysisDb::new();
        let ctx = RuleCtx::new(
            &db,
            RuleMeta {
                id: "examples/support".to_string(),
                description: "Support view constructor test".to_string(),
                severity: Severity::Warn,
            },
            RuleOptions::default(),
        );

        assert!(ctx.capability_support().entries().is_empty());
    }

    #[test]
    fn capability_support_runner_supplies_view_to_rules() {
        let rule = Rule::from_parts(
            || RuleMeta {
                id: "examples/support-probe".to_string(),
                description: "Support probe".to_string(),
                severity: Severity::Warn,
            },
            || Capabilities::new().imports(),
            |_db, ctx| {
                if ctx.capability_support().status_for("imports")
                    == Some(CapabilitySupportStatus::Supported)
                {
                    ctx.report(Diagnostic::warning(
                        ctx.rule_id(),
                        "<workspace>",
                        DiagnosticRange::point(1, 1),
                        "imports are supported",
                    ));
                }
                Ok(())
            },
        );

        let db = AnalysisDb::new();
        let rules = vec![rule];
        let support_view = CapabilitySupportView::new(vec![CapabilitySupport {
            capability: "imports".to_string(),
            language: Some(Language::Go),
            status: CapabilitySupportStatus::Supported,
            rules: vec!["examples/support-probe".to_string()],
            reason: None,
            hint: None,
            docs_path: None,
        }]);

        let diagnostics = run_rules_with_capability_support(
            &db,
            &rules,
            &BTreeMap::new(),
            None,
            false,
            &support_view,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "imports are supported");
    }

    #[test]
    fn run_rules_skips_rules_with_blocking_capabilities() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::report("examples/needs-cfg", Severity::Warn, "cfg")
                .with_capabilities(Capabilities::new().cfg())
                .into_rule(),
            TestRule::panic("examples/needs-dataflow")
                .with_capabilities(Capabilities::new().dataflow())
                .into_rule(),
            TestRule::report("examples/imports", Severity::Warn, "imports")
                .with_capabilities(Capabilities::new().imports())
                .into_rule(),
        ];
        let support_view = CapabilitySupportView::new(vec![
            CapabilitySupport {
                capability: "cfg".to_string(),
                language: Some(Language::Go),
                status: CapabilitySupportStatus::Unsupported,
                rules: vec!["examples/needs-cfg".to_string()],
                reason: None,
                hint: None,
                docs_path: None,
            },
            CapabilitySupport {
                capability: "dataflow".to_string(),
                language: Some(Language::Go),
                status: CapabilitySupportStatus::SetupMissing,
                rules: vec!["examples/needs-dataflow".to_string()],
                reason: None,
                hint: None,
                docs_path: None,
            },
            CapabilitySupport {
                capability: "imports".to_string(),
                language: Some(Language::Go),
                status: CapabilitySupportStatus::Supported,
                rules: vec!["examples/imports".to_string()],
                reason: None,
                hint: None,
                docs_path: None,
            },
        ]);

        let diagnostics = run_rules_with_capability_support(
            &db,
            &rules,
            &BTreeMap::new(),
            None,
            false,
            &support_view,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "examples/imports");
    }

    #[test]
    fn cached_file_facts_round_trip_remaps_ids() {
        let mut source_db = AnalysisDb::new();
        let source_file = source_db.add_file(
            PathBuf::from("src/payment.go"),
            "src/payment.go".to_string(),
            "package payments\nfunc Authorize() {}".to_string(),
        );
        let function = source_db.push_function(FunctionFact {
            id: FunctionId(999),
            file: source_file,
            name: "Authorize".to_string(),
            span: test_span(source_file, 2),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 2,
            calls: vec!["audit".to_string()],
        });
        let branch = source_db.push_branch(BranchObligation {
            id: BranchId(999),
            function: Some(function),
            file: source_file,
            decision_span: test_span(source_file, 3),
            condition_text: "err != nil".to_string(),
            edge_label: "true".to_string(),
            is_error_path: true,
            stable_fingerprint: "branch".to_string(),
        });
        source_db.push_coverage(CoverageFact {
            branch,
            covered: Some(false),
            source: "static".to_string(),
        });
        source_db.push_test(TestFact {
            file: source_file,
            function: Some(function),
            name: "TestAuthorize".to_string(),
            span: test_span(source_file, 5),
            evidence_terms: vec!["Authorize".to_string()],
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });

        let cached = source_db.facts_for_file(source_file);

        let mut restored_db = AnalysisDb::new();
        let target_file = restored_db.add_file(
            PathBuf::from("src/payment.go"),
            "src/payment.go".to_string(),
            "package payments\nfunc Authorize() {}".to_string(),
        );
        let existing_function = restored_db.push_function(FunctionFact {
            id: FunctionId(999),
            file: target_file,
            name: "Existing".to_string(),
            span: test_span(target_file, 1),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        restored_db.push_branch(BranchObligation {
            id: BranchId(999),
            function: Some(existing_function),
            file: target_file,
            decision_span: test_span(target_file, 1),
            condition_text: "existing".to_string(),
            edge_label: "true".to_string(),
            is_error_path: false,
            stable_fingerprint: "existing".to_string(),
        });

        restored_db.restore_file_facts(target_file, cached);

        let restored_function = restored_db
            .functions()
            .iter()
            .find(|fact| fact.name == "Authorize")
            .unwrap();
        let restored_branch = restored_db
            .branches()
            .iter()
            .find(|fact| fact.stable_fingerprint == "branch")
            .unwrap();
        assert_ne!(restored_function.id, function);
        assert_eq!(restored_branch.function, Some(restored_function.id));
        assert_eq!(
            restored_db.coverage().last().unwrap().branch,
            restored_branch.id
        );
        assert_eq!(
            restored_db.tests().last().unwrap().function,
            Some(restored_function.id)
        );
    }

    #[test]
    fn cached_file_analysis_does_not_include_source_text() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/secret.go"),
            "src/secret.go".to_string(),
            "package main\nconst token = \"super-secret-full-source\"".to_string(),
        );
        db.push_package(PackageFact {
            id: PackageId(999),
            file,
            name: "main".to_string(),
            span: test_span(file, 1),
            language: Language::Go,
        });

        let cached = CachedFileAnalysis {
            schema: "go-facts-v1".to_string(),
            diagnostics: Vec::new(),
            facts: db.facts_for_file(file),
        };
        let serialized = format!("{cached:?}");

        assert!(!serialized.contains("super-secret-full-source"));
        assert!(!serialized.contains("source"));
        assert!(!serialized.contains("ast"));
        assert!(!serialized.contains("tree"));
    }

    #[test]
    fn analysis_db_exposes_ts_class_facts() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/component.tsx"),
            "src/component.tsx".to_string(),
            "export class Button {}".to_string(),
        );
        let first_span = test_span(file, 1);
        let second_span = test_span(file, 5);

        db.push_ts_class(TsClassFact {
            file,
            name: "Button".to_string(),
            span: first_span.clone(),
            is_exported: true,
            is_component_like: true,
        });
        db.push_ts_class(TsClassFact {
            file,
            name: "Store".to_string(),
            span: second_span.clone(),
            is_exported: false,
            is_component_like: false,
        });

        let classes = db.ts_classes();
        assert_eq!(classes.len(), 2);
        assert_eq!(classes[0].file, file);
        assert_eq!(classes[0].name, "Button");
        assert_eq!(classes[0].span, first_span);
        assert!(classes[0].is_exported);
        assert!(classes[0].is_component_like);
        assert_eq!(classes[1].file, file);
        assert_eq!(classes[1].name, "Store");
        assert_eq!(classes[1].span, second_span);
        assert!(!classes[1].is_exported);
        assert!(!classes[1].is_component_like);
    }

    #[test]
    fn fact_view_exposes_ts_classes() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/dialog.tsx"),
            "src/dialog.tsx".to_string(),
            "class Dialog {}".to_string(),
        );
        let span = test_span(file, 1);
        db.push_ts_class(TsClassFact {
            file,
            name: "Dialog".to_string(),
            span,
            is_exported: false,
            is_component_like: true,
        });

        let classes = TsClasses::build(&db);

        assert_eq!(classes.all().len(), 1);
        assert_eq!(classes.all()[0].name, db.ts_classes()[0].name);
        assert_eq!(classes.all()[0].span, db.ts_classes()[0].span);
    }

    #[test]
    fn rule_ctx_exposes_sdk_query_helpers() {
        let mut db = AnalysisDb::new();
        let go_file = db.add_file(
            PathBuf::from("src/payment.go"),
            "src/payment.go".to_string(),
            "package payment\n".to_string(),
        );
        let ts_file = db.add_file(
            PathBuf::from("src/component.tsx"),
            "src/component.tsx".to_string(),
            "export function Button() { return <button aria-label=\"Pay\">Pay</button>; }"
                .to_string(),
        );
        let go_span = test_span(go_file, 1);
        let ts_span = test_span(ts_file, 1);

        db.push_package(PackageFact {
            id: PackageId(99),
            file: go_file,
            name: "payment".to_string(),
            span: go_span.clone(),
            language: Language::Go,
        });
        let go_function = db.push_function(FunctionFact {
            id: FunctionId(99),
            file: go_file,
            name: "Charge".to_string(),
            span: go_span.clone(),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 3,
            calls: vec!["authorize".to_string()],
        });
        let ts_function = db.push_function(FunctionFact {
            id: FunctionId(99),
            file: ts_file,
            name: "Button".to_string(),
            span: ts_span.clone(),
            language: Language::Tsx,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: vec!["render".to_string()],
        });
        db.push_import(ImportFact {
            id: ImportId(99),
            file: go_file,
            package: None,
            path: "context".to_string(),
            span: go_span.clone(),
            language: Language::Go,
        });
        db.push_branch(BranchObligation {
            id: BranchId(99),
            function: Some(go_function),
            file: go_file,
            decision_span: go_span.clone(),
            condition_text: "err != nil".to_string(),
            edge_label: "true".to_string(),
            is_error_path: true,
            stable_fingerprint: "branch".to_string(),
        });
        db.push_test(TestFact {
            file: go_file,
            function: Some(go_function),
            name: "TestCharge".to_string(),
            span: go_span,
            evidence_terms: vec!["err".to_string()],
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_ts_component(TsComponentFact {
            file: ts_file,
            function: Some(ts_function),
            name: "Button".to_string(),
            span: ts_span.clone(),
        });
        db.push_ts_class(TsClassFact {
            file: ts_file,
            name: "Dialog".to_string(),
            span: ts_span.clone(),
            is_exported: true,
            is_component_like: true,
        });
        db.push_string_literal(StringLiteralFact {
            file: ts_file,
            value: "Pay".to_string(),
            span: ts_span.clone(),
            language: Language::Tsx,
        });
        db.push_jsx_attribute(JsxAttributeFact {
            file: ts_file,
            name: "aria-label".to_string(),
            value: Some("Pay".to_string()),
            span: ts_span,
        });

        let packages = Packages::build(&db);
        let files = SourceFiles::build(&db);
        let functions = Functions::build(&db);
        let imports = Imports::build(&db);
        let branches = BranchObligations::build(&db);
        let tests = GoTests::build(&db);
        let components = TsComponents::build(&db);
        let classes = TsClasses::build(&db);
        let literals = StringLiterals::build(&db);
        let jsx = JsxAttributes::build(&db);

        assert_eq!(packages.all()[0].name, "payment");
        assert_eq!(branches.all()[0].condition_text, "err != nil");
        assert_eq!(files.get(go_file).unwrap().relative_path, "src/payment.go");
        assert_eq!(
            functions
                .for_file(go_file)
                .map(|function| function.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Charge"]
        );
        assert_eq!(
            imports
                .for_file(go_file)
                .map(|import| import.path.as_str())
                .collect::<Vec<_>>(),
            vec!["context"]
        );
        assert_eq!(branches.for_file(go_file).count(), 1);
        assert_eq!(
            tests
                .for_file(go_file)
                .map(|test| test.name.as_str())
                .collect::<Vec<_>>(),
            vec!["TestCharge"]
        );
        assert_eq!(
            components
                .for_file(ts_file)
                .map(|component| component.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Button"]
        );
        assert_eq!(
            classes
                .for_file(ts_file)
                .map(|class| class.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Dialog"]
        );
        assert_eq!(
            literals
                .for_file(ts_file)
                .map(|literal| literal.value.as_str())
                .collect::<Vec<_>>(),
            vec!["Pay"]
        );
        assert_eq!(
            jsx.for_file(ts_file)
                .map(|attribute| attribute.name.as_str())
                .collect::<Vec<_>>(),
            vec!["aria-label"]
        );
    }

    #[test]
    fn rule_ctx_import_edges_preserve_analysis_order() {
        let mut db = AnalysisDb::new();
        let first_file = db.add_file(
            PathBuf::from("src/first.go"),
            "src/first.go".to_string(),
            "package first\n".to_string(),
        );
        let second_file = db.add_file(
            PathBuf::from("src/second.go"),
            "src/second.go".to_string(),
            "package second\n".to_string(),
        );

        db.push_import(ImportFact {
            id: ImportId(99),
            file: second_file,
            package: None,
            path: "fmt".to_string(),
            span: test_span(second_file, 1),
            language: Language::Go,
        });
        db.push_import(ImportFact {
            id: ImportId(99),
            file: first_file,
            package: None,
            path: "strings".to_string(),
            span: test_span(first_file, 1),
            language: Language::Go,
        });

        let imports = Imports::build(&db);

        assert_eq!(
            imports
                .edges()
                .map(|(file, import)| (file.relative_path.as_str(), import.path.as_str()))
                .collect::<Vec<_>>(),
            vec![("src/second.go", "fmt"), ("src/first.go", "strings")]
        );
    }

    #[test]
    fn rule_ctx_go_tests_for_related_file_matches_companion_tests() {
        let mut db = AnalysisDb::new();
        let production_file = db.add_file(
            PathBuf::from("src/payments/payment.go"),
            "src/payments/payment.go".to_string(),
            "package payments\n".to_string(),
        );
        let companion_file = db.add_file(
            PathBuf::from("src/payments/payment_test.go"),
            "src/payments/payment_test.go".to_string(),
            "package payments\n".to_string(),
        );
        let unrelated_file = db.add_file(
            PathBuf::from("src/users/payment_test.go"),
            "src/users/payment_test.go".to_string(),
            "package users\n".to_string(),
        );

        db.push_test(TestFact {
            file: production_file,
            function: None,
            name: "TestInline".to_string(),
            span: test_span(production_file, 1),
            evidence_terms: Vec::new(),
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_test(TestFact {
            file: companion_file,
            function: None,
            name: "TestPayment".to_string(),
            span: test_span(companion_file, 1),
            evidence_terms: Vec::new(),
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_test(TestFact {
            file: unrelated_file,
            function: None,
            name: "TestUserPayment".to_string(),
            span: test_span(unrelated_file, 1),
            evidence_terms: Vec::new(),
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });

        let tests = GoTests::build(&db);

        assert_eq!(
            tests
                .related_for_file(production_file)
                .iter()
                .map(|test| test.name.as_str())
                .collect::<Vec<_>>(),
            vec!["TestInline", "TestPayment"]
        );
        assert_eq!(
            tests
                .related_for_file(companion_file)
                .iter()
                .map(|test| test.name.as_str())
                .collect::<Vec<_>>(),
            vec!["TestPayment"]
        );
    }

    #[test]
    fn capabilities_expose_ts_classes() {
        assert!(!Capabilities::new().ts_classes);
        let capabilities = Capabilities::new().ts_classes();
        assert!(capabilities.ts_classes);
    }

    fn diagnostic_range(
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> DiagnosticRange {
        DiagnosticRange {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    #[test]
    fn line_col_counts_utf8_boundaries() {
        assert_eq!(line_col("a\nbc", 3), (2, 2));
    }

    #[test]
    fn registry_exposes_capability_declarations() {
        let mut registry = RuleRegistry::new();
        registry.register(
            TestRule::report("examples/capabilities", Severity::Warn, "capabilities")
                .with_capabilities(Capabilities::new().imports().coverage_facts())
                .into_rule(),
        );

        let capabilities = registry.rules()[0].capabilities();
        assert!(capabilities.imports);
        assert!(capabilities.coverage_facts);
        assert!(!capabilities.dataflow);
        assert!(!capabilities.jsx_attributes);
    }

    #[test]
    fn run_rules_filters_enabled_patterns_and_applies_severity_override() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::report("examples/allowed", Severity::Warn, "allowed").into_rule(),
            TestRule::report("custom/blocked", Severity::Error, "blocked").into_rule(),
        ];
        let mut options = BTreeMap::new();
        options.insert(
            "examples/allowed".to_string(),
            RuleOptions {
                severity: Some(Severity::Error),
                ..RuleOptions::default()
            },
        );
        let enabled = BTreeSet::from(["examples/*".to_string()]);

        let diagnostics = run_rules(&db, &rules, &options, Some(&enabled), false);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "examples/allowed");
        assert_eq!(diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn run_rules_none_selection_runs_all_and_empty_selection_runs_none() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::report("examples/one", Severity::Warn, "one").into_rule(),
            TestRule::report("examples/two", Severity::Warn, "two").into_rule(),
        ];

        let all = run_rules(&db, &rules, &BTreeMap::new(), None, false);
        assert_eq!(all.len(), 2);

        let empty = BTreeSet::new();
        let none = run_rules(&db, &rules, &BTreeMap::new(), Some(&empty), false);
        assert!(none.is_empty());
    }

    #[test]
    fn run_rules_contains_rule_errors_and_panics() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::error("examples/error").into_rule(),
            TestRule::panic("examples/panic").into_rule(),
        ];

        let diagnostics = run_rules(&db, &rules, &BTreeMap::new(), None, false);

        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.rule_id.as_str())
                .collect::<Vec<_>>(),
            vec!["internal/examples/error", "internal/examples/panic"]
        );
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.file == "<workspace>")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("intentional rule error"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("rule panicked"))
        );
    }

    #[test]
    fn run_rules_contains_meta_panics() {
        let db = AnalysisDb::new();
        let rules = vec![TestRule::meta_panic().into_rule()];

        let diagnostics = run_rules(&db, &rules, &BTreeMap::new(), None, false);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "internal/unknown");
        assert_eq!(diagnostics[0].file, "<workspace>");
        assert!(diagnostics[0].message.contains("rule metadata panicked"));
    }

    #[test]
    fn run_rules_parallel_matches_sequential() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::report("examples/duplicate", Severity::Warn, "duplicate")
                .with_message("same diagnostic")
                .with_delay(Duration::from_millis(50))
                .into_rule(),
            TestRule::report("examples/duplicate", Severity::Error, "duplicate")
                .with_message("same diagnostic")
                .into_rule(),
        ];

        let sequential = run_rules(&db, &rules, &BTreeMap::new(), None, false);
        let parallel = run_rules(&db, &rules, &BTreeMap::new(), None, true);

        assert_eq!(parallel, sequential);
    }

    #[test]
    fn run_rules_dedupes_duplicate_fingerprints() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::report("examples/duplicate-a", Severity::Warn, "same-fingerprint")
                .into_rule(),
            TestRule::report("examples/duplicate-b", Severity::Error, "same-fingerprint")
                .into_rule(),
        ];

        let diagnostics = run_rules(&db, &rules, &BTreeMap::new(), None, false);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].stable_fingerprint, "same-fingerprint");
    }

    #[test]
    fn analysis_db_assigns_deterministic_ids_and_preserves_shared_source() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\n".to_string(),
        );
        let span = test_span(file, 1);

        let function = db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "main".to_string(),
            span: span.clone(),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        let import = db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "fmt".to_string(),
            span: span.clone(),
            language: Language::Go,
        });
        let branch = db.push_branch(BranchObligation {
            id: BranchId(99),
            function: Some(function),
            file,
            decision_span: span,
            condition_text: "err != nil".to_string(),
            edge_label: "true".to_string(),
            is_error_path: true,
            stable_fingerprint: "branch".to_string(),
        });

        assert_eq!(file, FileId(0));
        assert_eq!(function, FunctionId(0));
        assert_eq!(import, ImportId(0));
        assert_eq!(branch, BranchId(0));

        let stored = db.file(file).expect("source file exists");
        let shared: Arc<str> = Arc::clone(&stored.source);
        assert_eq!(&*shared, "package main\n");
    }

    #[test]
    fn analysis_db_assigns_package_ids_deterministically() {
        let mut db = AnalysisDb::new();
        let first_file = db.add_file(
            PathBuf::from("payment.go"),
            "payment.go".to_string(),
            "package payment\n".to_string(),
        );
        let second_file = db.add_file(
            PathBuf::from("billing.go"),
            "billing.go".to_string(),
            "package billing\n".to_string(),
        );

        let first = db.push_package(PackageFact {
            id: PackageId(99),
            file: first_file,
            name: "payment".to_string(),
            span: test_span(first_file, 1),
            language: Language::Go,
        });
        let second = db.push_package(PackageFact {
            id: PackageId(99),
            file: second_file,
            name: "billing".to_string(),
            span: test_span(second_file, 1),
            language: Language::Go,
        });

        assert_eq!(first, PackageId(0));
        assert_eq!(second, PackageId(1));
        assert_eq!(db.packages()[0].id, PackageId(0));
        assert_eq!(db.packages()[1].id, PackageId(1));
    }

    #[test]
    fn analysis_db_exposes_package_facts() {
        let mut db = AnalysisDb::new();
        let first_file = db.add_file(
            PathBuf::from("payment.go"),
            "payment.go".to_string(),
            "package payment\n".to_string(),
        );
        let second_file = db.add_file(
            PathBuf::from("billing.go"),
            "billing.go".to_string(),
            "package billing\n".to_string(),
        );
        let first_span = test_span(first_file, 1);
        let second_span = test_span(second_file, 1);

        db.push_package(PackageFact {
            id: PackageId(99),
            file: first_file,
            name: "payment".to_string(),
            span: first_span.clone(),
            language: Language::Go,
        });
        db.push_package(PackageFact {
            id: PackageId(99),
            file: second_file,
            name: "billing".to_string(),
            span: second_span.clone(),
            language: Language::Go,
        });

        assert_eq!(db.packages().len(), 2);
        assert_eq!(db.packages()[0].file, first_file);
        assert_eq!(db.packages()[0].name, "payment");
        assert_eq!(db.packages()[0].span, first_span);
        assert_eq!(db.packages()[0].language, Language::Go);
        assert_eq!(db.packages()[1].file, second_file);
        assert_eq!(db.packages()[1].name, "billing");
        assert_eq!(db.packages()[1].span, second_span);
        assert_eq!(db.packages()[1].language, Language::Go);
    }

    #[test]
    fn module_relationship_core_contract_stores_relationship_facts_with_stable_ids() {
        let mut db = AnalysisDb::new();
        let from_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import { Button } from './button';\nimport React from 'react';\n".to_string(),
        );
        let target_file = db.add_file(
            PathBuf::from("src/button.ts"),
            "src/button.ts".to_string(),
            "export function Button() {}\n".to_string(),
        );
        let span = test_span(from_file, 1);
        let local_import = db.push_import(ImportFact {
            id: ImportId(99),
            file: from_file,
            package: None,
            path: "./button".to_string(),
            span: span.clone(),
            language: Language::TypeScript,
        });
        let external_import = db.push_import(ImportFact {
            id: ImportId(99),
            file: from_file,
            package: None,
            path: "react".to_string(),
            span,
            language: Language::TypeScript,
        });

        db.replace_module_graph_facts(
            vec![
                ResolvedImportFact {
                    id: ResolvedImportId(99),
                    import: local_import,
                    from_file,
                    target_node: Some(ModuleNodeId(1)),
                    status: ResolutionStatus::Resolved,
                    precision: ResolutionPrecision::ExactFile,
                    reason: None,
                },
                ResolvedImportFact {
                    id: ResolvedImportId(99),
                    import: external_import,
                    from_file,
                    target_node: Some(ModuleNodeId(2)),
                    status: ResolutionStatus::External,
                    precision: ResolutionPrecision::ExternalPackage,
                    reason: None,
                },
            ],
            vec![
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::File,
                    label: "src/app.ts".to_string(),
                    file: Some(from_file),
                    package: None,
                    language: Some(Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::File,
                    label: "src/button.ts".to_string(),
                    file: Some(target_file),
                    package: None,
                    language: Some(Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::External,
                    label: "react".to_string(),
                    file: None,
                    package: None,
                    language: Some(Language::TypeScript),
                },
            ],
            vec![
                ModuleEdge {
                    id: ModuleEdgeId(99),
                    from: ModuleNodeId(0),
                    to: ModuleNodeId(1),
                    import: Some(local_import),
                    resolved_import: Some(ResolvedImportId(0)),
                    kind: ModuleEdgeKind::Imports,
                    status: ResolutionStatus::Resolved,
                },
                ModuleEdge {
                    id: ModuleEdgeId(99),
                    from: ModuleNodeId(0),
                    to: ModuleNodeId(2),
                    import: Some(external_import),
                    resolved_import: Some(ResolvedImportId(1)),
                    kind: ModuleEdgeKind::DependsOn,
                    status: ResolutionStatus::External,
                },
            ],
        );

        assert_eq!(db.resolved_imports()[0].id, ResolvedImportId(0));
        assert_eq!(db.resolved_imports()[1].id, ResolvedImportId(1));
        assert_eq!(db.module_nodes()[0].id, ModuleNodeId(0));
        assert_eq!(db.module_nodes()[1].id, ModuleNodeId(1));
        assert_eq!(db.module_nodes()[2].id, ModuleNodeId(2));
        assert_eq!(db.module_edges()[0].id, ModuleEdgeId(0));
        assert_eq!(db.module_edges()[1].id, ModuleEdgeId(1));
        assert_eq!(
            db.module_edges()[1].resolved_import,
            Some(ResolvedImportId(1))
        );
    }

    #[test]
    fn symbol_fact_contract_preserves_provider_ids_and_indexes_queries() {
        let mut db = AnalysisDb::new();
        let app_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function Button() { return theme; }\n".to_string(),
        );
        let theme_file = db.add_file(
            PathBuf::from("src/theme.ts"),
            "src/theme.ts".to_string(),
            "export const theme = {};\n".to_string(),
        );

        db.replace_symbol_graph_facts(
            vec![
                SymbolFact {
                    id: SymbolId(0xfeed_beef),
                    language: Language::TypeScript,
                    name: "Button".to_string(),
                    qualified_name: "src/app.ts::Button".to_string(),
                    kind: SymbolKind::Function,
                    namespace: SymbolNamespace::Value,
                    file: Some(app_file),
                    package: None,
                    module: Some(ModuleNodeId(10)),
                    owner: None,
                    primary_span: Some(test_span(app_file, 1)),
                    is_exported: true,
                    stable_key: "ts|src/app.ts|value|function|Button|1:1".to_string(),
                    precision: SymbolPrecision::ExactLocal,
                },
                SymbolFact {
                    id: SymbolId(0xabc0_1234),
                    language: Language::TypeScript,
                    name: "theme".to_string(),
                    qualified_name: "src/theme.ts::theme".to_string(),
                    kind: SymbolKind::Constant,
                    namespace: SymbolNamespace::Value,
                    file: Some(theme_file),
                    package: None,
                    module: Some(ModuleNodeId(11)),
                    owner: None,
                    primary_span: Some(test_span(theme_file, 1)),
                    is_exported: true,
                    stable_key: "ts|src/theme.ts|value|constant|theme|1:1".to_string(),
                    precision: SymbolPrecision::ModuleLinked,
                },
            ],
            vec![DefinitionFact {
                id: DefinitionId(0x1010_2020),
                symbol: SymbolId(0xfeed_beef),
                language: Language::TypeScript,
                name: "Button".to_string(),
                qualified_name: "src/app.ts::Button".to_string(),
                kind: DefinitionKind::Declaration,
                namespace: SymbolNamespace::Value,
                file: Some(app_file),
                package: None,
                module: Some(ModuleNodeId(10)),
                owner: None,
                primary_span: Some(test_span(app_file, 1)),
                is_primary: true,
                is_exported: true,
                stable_key: "ts|src/app.ts|definition|Button|1:1".to_string(),
                precision: SymbolPrecision::ExactLocal,
            }],
            vec![
                ReferenceFact {
                    id: ReferenceId(0x3030_4040),
                    language: Language::TypeScript,
                    name: "theme".to_string(),
                    qualified_name: "src/theme.ts::theme".to_string(),
                    kind: ReferenceKind::Read,
                    namespace: SymbolNamespace::Value,
                    file: Some(app_file),
                    package: None,
                    module: Some(ModuleNodeId(10)),
                    owner: Some(SymbolId(0xfeed_beef)),
                    primary_span: Some(test_span(app_file, 1)),
                    target: Some(SymbolId(0xabc0_1234)),
                    candidates: Vec::new(),
                    stable_key: "ts|src/app.ts|reference|theme|1:28".to_string(),
                    status: SymbolResolutionStatus::Resolved,
                    precision: SymbolPrecision::ModuleLinked,
                },
                ReferenceFact {
                    id: ReferenceId(0x5050_6060),
                    language: Language::TypeScript,
                    name: "missing".to_string(),
                    qualified_name: "missing".to_string(),
                    kind: ReferenceKind::Read,
                    namespace: SymbolNamespace::Value,
                    file: Some(app_file),
                    package: None,
                    module: Some(ModuleNodeId(10)),
                    owner: Some(SymbolId(0xfeed_beef)),
                    primary_span: Some(test_span(app_file, 2)),
                    target: None,
                    candidates: vec![SymbolId(0xfeed_beef), SymbolId(0xabc0_1234)],
                    stable_key: "ts|src/app.ts|reference|missing|2:1".to_string(),
                    status: SymbolResolutionStatus::Ambiguous,
                    precision: SymbolPrecision::Ambiguous,
                },
            ],
        );

        assert_eq!(db.symbols()[0].id, SymbolId(0xfeed_beef));
        assert_eq!(db.definitions()[0].id, DefinitionId(0x1010_2020));
        assert_eq!(db.references()[0].id, ReferenceId(0x3030_4040));
        assert_eq!(
            db.symbol_by_id(SymbolId(0xabc0_1234))
                .map(|symbol| symbol.name.as_str()),
            Some("theme")
        );
        assert_eq!(
            db.symbols_for_file(app_file)
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>(),
            vec![SymbolId(0xfeed_beef)]
        );
        assert_eq!(
            db.symbols_by_name("Button")
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>(),
            vec![SymbolId(0xfeed_beef)]
        );
        assert_eq!(
            db.definitions_for_symbol(SymbolId(0xfeed_beef))
                .map(|definition| definition.id)
                .collect::<Vec<_>>(),
            vec![DefinitionId(0x1010_2020)]
        );
        assert_eq!(
            db.definition_for_symbol(SymbolId(0xfeed_beef))
                .map(|definition| definition.id),
            Some(DefinitionId(0x1010_2020))
        );
        assert_eq!(
            db.references_to_symbol(SymbolId(0xabc0_1234))
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            vec![ReferenceId(0x3030_4040)]
        );
        assert_eq!(
            db.references_for_file(app_file)
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            vec![ReferenceId(0x3030_4040), ReferenceId(0x5050_6060)]
        );

        let precision_statuses = [
            SymbolPrecision::ExactSemantic,
            SymbolPrecision::ExactLocal,
            SymbolPrecision::ModuleLinked,
            SymbolPrecision::Heuristic,
            SymbolPrecision::Unresolved,
            SymbolPrecision::Ambiguous,
            SymbolPrecision::SetupMissing,
            SymbolPrecision::Unsupported,
        ];
        assert_eq!(precision_statuses.len(), 8);

        let resolution_statuses = [
            SymbolResolutionStatus::Resolved,
            SymbolResolutionStatus::Unresolved,
            SymbolResolutionStatus::Ambiguous,
            SymbolResolutionStatus::SetupMissing,
            SymbolResolutionStatus::Unsupported,
        ];
        assert_eq!(resolution_statuses.len(), 5);

        let capabilities = Capabilities::new().references();
        assert!(capabilities.references);
        assert!(capabilities.symbols);
    }

    #[test]
    fn module_relationship_core_contract_statuses_are_representable() {
        let statuses = [
            ResolutionStatus::Resolved,
            ResolutionStatus::External,
            ResolutionStatus::Unresolved,
            ResolutionStatus::SetupMissing,
            ResolutionStatus::Dynamic,
            ResolutionStatus::Unsupported,
        ];
        let reasons = [
            UnresolvedReason::NotFound,
            UnresolvedReason::SetupMissing,
            UnresolvedReason::DynamicExpression,
            UnresolvedReason::UnsupportedLanguage,
            UnresolvedReason::UnsupportedImport,
            UnresolvedReason::ResolverError,
            UnresolvedReason::OutsideWorkspace,
        ];

        assert!(matches!(statuses[0], ResolutionStatus::Resolved));
        assert!(matches!(statuses[1], ResolutionStatus::External));
        assert!(matches!(statuses[2], ResolutionStatus::Unresolved));
        assert!(matches!(statuses[3], ResolutionStatus::SetupMissing));
        assert!(matches!(statuses[4], ResolutionStatus::Dynamic));
        assert!(matches!(statuses[5], ResolutionStatus::Unsupported));
        assert_eq!(reasons.len(), 7);
    }

    #[test]
    fn module_relationship_core_contract_public_enums_match_with_wildcard() {
        fn status_name(status: ResolutionStatus) -> &'static str {
            match status {
                ResolutionStatus::Resolved => "resolved",
                _ => "not-resolved",
            }
        }

        fn node_kind_name(kind: ModuleNodeKind) -> &'static str {
            match kind {
                ModuleNodeKind::File => "file",
                _ => "other",
            }
        }

        fn edge_kind_name(kind: ModuleEdgeKind) -> &'static str {
            match kind {
                ModuleEdgeKind::Imports => "imports",
                _ => "other",
            }
        }

        fn precision_name(precision: ResolutionPrecision) -> &'static str {
            match precision {
                ResolutionPrecision::ExactFile => "exact-file",
                _ => "other",
            }
        }

        fn reason_name(reason: UnresolvedReason) -> &'static str {
            match reason {
                UnresolvedReason::NotFound => "not-found",
                _ => "other",
            }
        }

        assert_eq!(status_name(ResolutionStatus::Resolved), "resolved");
        assert_eq!(node_kind_name(ModuleNodeKind::Package), "other");
        assert_eq!(edge_kind_name(ModuleEdgeKind::Contains), "other");
        assert_eq!(
            precision_name(ResolutionPrecision::ExternalPackage),
            "other"
        );
        assert_eq!(reason_name(UnresolvedReason::SetupMissing), "other");
    }

    #[test]
    fn analysis_db_exposes_all_phase3_fact_families() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/component.tsx"),
            "src/component.tsx".to_string(),
            "export function Button() { return <button aria-label=\"Save\" /> }\n".to_string(),
        );
        let span = test_span(file, 1);
        let function = db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "Button".to_string(),
            span: span.clone(),
            language: Language::Tsx,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: vec!["render".to_string()],
        });
        let import = db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: Some("react".to_string()),
            path: "react".to_string(),
            span: span.clone(),
            language: Language::Tsx,
        });
        let branch = db.push_branch(BranchObligation {
            id: BranchId(99),
            function: Some(function),
            file,
            decision_span: span.clone(),
            condition_text: "enabled".to_string(),
            edge_label: "true".to_string(),
            is_error_path: false,
            stable_fingerprint: "branch".to_string(),
        });
        db.push_test(TestFact {
            file,
            function: Some(function),
            name: "Button test".to_string(),
            span: span.clone(),
            evidence_terms: vec!["render".to_string()],
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_coverage(CoverageFact {
            branch,
            covered: Some(true),
            source: "synthetic-coverage".to_string(),
        });
        db.push_ts_component(TsComponentFact {
            file,
            function: Some(function),
            name: "Button".to_string(),
            span: span.clone(),
        });
        db.push_string_literal(StringLiteralFact {
            file,
            value: "Save".to_string(),
            span: span.clone(),
            language: Language::Tsx,
        });
        db.push_jsx_attribute(JsxAttributeFact {
            file,
            name: "aria-label".to_string(),
            value: Some("Save".to_string()),
            span,
        });

        assert_eq!(db.files()[0].id, file);
        assert_eq!(db.functions()[0].id, function);
        assert_eq!(db.imports()[0].id, import);
        assert_eq!(db.branches()[0].id, branch);
        assert_eq!(db.tests()[0].name, "Button test");
        assert_eq!(db.coverage()[0].covered, Some(true));
        assert_eq!(db.ts_components()[0].name, "Button");
        assert_eq!(db.string_literals()[0].value, "Save");
        assert_eq!(db.jsx_attributes()[0].name, "aria-label");
    }

    #[test]
    fn span_from_byte_range_handles_utf8_newlines_and_empty_ranges() {
        let source = "aé\nβ\n";
        let file = FileId(7);

        let utf8 = span_from_byte_range(file, source, 1, 3);
        assert_eq!(utf8.diagnostic_range(), diagnostic_range(1, 2, 1, 3));

        let newline = span_from_byte_range(file, source, 3, 4);
        assert_eq!(newline.diagnostic_range(), diagnostic_range(1, 3, 2, 1));

        let empty = span_from_byte_range(file, source, 4, 4);
        assert_eq!(empty.diagnostic_range(), diagnostic_range(2, 1, 2, 1));

        let clamped = span_from_byte_range(file, source, source.len() + 10, source.len() + 20);
        assert_eq!(clamped.start_byte as usize, source.len());
        assert_eq!(clamped.end_byte as usize, source.len());
        assert_eq!(clamped.diagnostic_range(), diagnostic_range(3, 1, 3, 1));
    }

    #[test]
    fn rule_pattern_matches_prefix() {
        assert!(rule_id_matches("examples/*", "examples/ts-no-raw-colors"));
        assert!(!rule_id_matches("custom/*", "examples/ts-no-raw-colors"));
    }

    proptest! {
        #[test]
        fn span_from_byte_range_is_monotonic_for_char_boundaries(source in "\\PC*") {
            let mut offsets: Vec<usize> = source.char_indices().map(|(idx, _)| idx).collect();
            offsets.push(source.len());

            for start in &offsets {
                for end in offsets.iter().filter(|end| *end >= start) {
                    let span = span_from_byte_range(FileId(0), &source, *start, *end);
                    let range = span.diagnostic_range();
                    prop_assert!(
                        (range.end_line, range.end_col) >= (range.start_line, range.start_col),
                        "range {range:?} from offsets {start}..{end} in {source:?}"
                    );
                }
            }
        }
    }
}
