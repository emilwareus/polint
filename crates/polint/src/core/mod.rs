use crate::analysis::error::AnalysisError;
use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
use crate::analysis::mir::op::{MirOperation, UnsupportedSemanticFact};
use crate::analysis::places::{PlaceFact, PlaceStatus};
use crate::analysis::store::SemanticStore;
use crate::analysis_kernel::{
    FactConfidence, FactFamily, FactMeta, FactMetaStore, FactPrecision, FactRef, MissingFactMeta,
    ValidationStatus, resolution_metadata, resolution_status_metadata, stable_key_from_parts,
    symbol_metadata,
};
use crate::diagnostics::{
    Diagnostic, Severity, TextRange as DiagnosticRange, dedupe_diagnostics, fingerprint,
};
use crate::module_graph::topology::{
    DependencyRequirementFact, ImportToPackageFact, RepoTopologyOverlayFact,
    ResolvedDependencyEdgeFact, SourceSetFact, TopologyOutput, TopologyPackageFact,
    TopologyPrecision, WorkspaceRootFact,
};
use crate::rule_error::RuleResult;
use crate::rule_manifest::{FactViewRequirement, RuleManifest};
use crate::symbol_graph::semantic::{
    AliasFact, AliasId, ExportFact, ExportId, GeneratedSymbolFact, GeneratedSymbolId,
    ResolutionFact, ResolutionId, ScopeFact, ScopeId, SemanticImportFact, SemanticImportId,
    SemanticStatus, StableExportId, StableExportIdentity,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SOURCE_PROVIDER_ID: &str = "polint.source";
const GO_SYNTAX_PROVIDER_ID: &str = "polint.go.syntax";
const TS_SYNTAX_PROVIDER_ID: &str = "polint.ts.syntax";
const MODULE_GRAPH_PROVIDER_ID: &str = "polint.module_graph";
const MODULE_TOPOLOGY_PROVIDER_ID: &str = "polint.module_topology";
const SYMBOL_GRAPH_PROVIDER_ID: &str = "polint.symbol_graph";
pub(crate) const SEMANTIC_MIR_PROVIDER_ID: &str = "polint.semantic_mir";
const METRICS_PROVIDER_ID: &str = "polint.metrics";
const FUNCTION_SIZE_METRIC_NAME: &str = "function_size";
const CYCLOMATIC_COMPLEXITY_METRIC_NAME: &str = "cyclomatic_complexity";

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
    fact_meta: FactMetaStore,
    packages: Vec<PackageFact>,
    functions: Vec<FunctionFact>,
    imports: Vec<ImportFact>,
    resolved_imports: Vec<ResolvedImportFact>,
    module_nodes: Vec<ModuleNode>,
    module_edges: Vec<ModuleEdge>,
    workspace_roots: Vec<WorkspaceRootFact>,
    topology_packages: Vec<TopologyPackageFact>,
    source_sets: Vec<SourceSetFact>,
    dependency_requirements: Vec<DependencyRequirementFact>,
    resolved_dependency_edges: Vec<ResolvedDependencyEdgeFact>,
    import_to_package_edges: Vec<ImportToPackageFact>,
    repo_topology_overlays: Vec<RepoTopologyOverlayFact>,
    scopes: Vec<ScopeFact>,
    semantic_imports: Vec<SemanticImportFact>,
    exports: Vec<ExportFact>,
    aliases: Vec<AliasFact>,
    resolution_facts: Vec<ResolutionFact>,
    generated_symbols: Vec<GeneratedSymbolFact>,
    stable_exports: Vec<StableExportIdentity>,
    scopes_by_id: BTreeMap<ScopeId, usize>,
    semantic_imports_by_id: BTreeMap<SemanticImportId, usize>,
    exports_by_id: BTreeMap<ExportId, usize>,
    aliases_by_id: BTreeMap<AliasId, usize>,
    resolution_facts_by_id: BTreeMap<ResolutionId, usize>,
    generated_symbols_by_id: BTreeMap<GeneratedSymbolId, usize>,
    stable_exports_by_id: BTreeMap<StableExportId, usize>,
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
    semantic: Option<SemanticStore>,
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
        let metadata = source_file_metadata(&relative_path, language, &content_hash);
        self.files.push(SourceFile {
            id,
            path,
            relative_path,
            language,
            source,
            content_hash,
        });
        self.record_fact_meta(FactFamily::SourceFile, u64::from(id.0), metadata);
        id
    }

    pub fn push_package(&mut self, mut fact: PackageFact) -> PackageId {
        let id = PackageId(self.packages.len() as u64);
        fact.id = id;
        let metadata = self.package_metadata(&fact);
        self.packages.push(fact);
        self.record_fact_meta(FactFamily::Package, id.0, metadata);
        id
    }

    pub fn push_function(&mut self, mut fact: FunctionFact) -> FunctionId {
        let id = FunctionId(self.functions.len() as u64);
        fact.id = id;
        let metadata = self.function_metadata(&fact);
        self.functions.push(fact);
        self.record_fact_meta(FactFamily::Function, id.0, metadata);
        id
    }

    pub fn push_import(&mut self, mut fact: ImportFact) -> ImportId {
        let id = ImportId(self.imports.len() as u64);
        fact.id = id;
        let metadata = self.import_metadata(&fact);
        self.imports.push(fact);
        self.record_fact_meta(FactFamily::Import, id.0, metadata);
        id
    }

    pub fn push_branch(&mut self, mut fact: BranchObligation) -> BranchId {
        let id = BranchId(self.branches.len() as u64);
        fact.id = id;
        let metadata = self.branch_metadata(&fact);
        self.branches.push(fact);
        self.record_fact_meta(FactFamily::BranchObligation, id.0, metadata);
        id
    }

    pub fn push_test(&mut self, fact: TestFact) {
        let run_id = self.tests.len() as u64;
        let metadata = self.test_metadata(&fact);
        self.tests.push(fact);
        self.record_fact_meta(FactFamily::Test, run_id, metadata);
    }

    pub fn push_coverage(&mut self, fact: CoverageFact) {
        let run_id = self.coverage.len() as u64;
        let metadata = self.coverage_metadata(&fact);
        self.coverage.push(fact);
        self.record_fact_meta(FactFamily::Coverage, run_id, metadata);
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
        self.refresh_metric_metadata();
    }

    pub(crate) fn replace_module_graph_facts(
        &mut self,
        mut resolved_imports: Vec<ResolvedImportFact>,
        mut module_nodes: Vec<ModuleNode>,
        mut module_edges: Vec<ModuleEdge>,
    ) {
        let resolved_import_ids = resolved_imports
            .iter()
            .enumerate()
            .map(|(index, fact)| (fact.id, ResolvedImportId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        let module_node_ids = module_nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, ModuleNodeId(index as u64)))
            .collect::<BTreeMap<_, _>>();

        for (index, fact) in resolved_imports.iter_mut().enumerate() {
            fact.id = ResolvedImportId(index as u64);
            if let Some(target_node) = fact.target_node
                && let Some(remapped) = module_node_ids.get(&target_node)
            {
                fact.target_node = Some(*remapped);
            }
        }
        for (index, node) in module_nodes.iter_mut().enumerate() {
            node.id = ModuleNodeId(index as u64);
        }
        for (index, edge) in module_edges.iter_mut().enumerate() {
            edge.id = ModuleEdgeId(index as u64);
            if let Some(remapped) = module_node_ids.get(&edge.from) {
                edge.from = *remapped;
            }
            if let Some(remapped) = module_node_ids.get(&edge.to) {
                edge.to = *remapped;
            }
            if let Some(resolved_import) = edge.resolved_import
                && let Some(remapped) = resolved_import_ids.get(&resolved_import)
            {
                edge.resolved_import = Some(*remapped);
            }
        }

        self.resolved_imports = resolved_imports;
        self.module_nodes = module_nodes;
        self.module_edges = module_edges;
        self.refresh_module_graph_metadata();
    }

    pub(crate) fn replace_topology_facts(&mut self, output: TopologyOutput) {
        let output = output.normalized();
        self.workspace_roots = output.workspace_roots;
        self.topology_packages = output.packages;
        self.source_sets = output.source_sets;
        self.dependency_requirements = output.dependency_requirements;
        self.resolved_dependency_edges = output.resolved_dependency_edges;
        self.import_to_package_edges = output.import_to_package_edges;
        self.repo_topology_overlays = output.overlays;
        self.refresh_topology_metadata();
    }

    pub(crate) fn replace_import_to_package_facts(&mut self, edges: Vec<ImportToPackageFact>) {
        let output = TopologyOutput {
            import_to_package_edges: edges,
            ..TopologyOutput::default()
        }
        .normalized();
        self.import_to_package_edges = output.import_to_package_edges;
        self.refresh_import_to_package_metadata();
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
        self.refresh_symbol_graph_metadata();
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "semantic index replacement accepts every internal semantic row family explicitly"
    )]
    pub(crate) fn replace_semantic_index_facts(
        &mut self,
        mut scopes: Vec<ScopeFact>,
        mut semantic_imports: Vec<SemanticImportFact>,
        mut exports: Vec<ExportFact>,
        mut aliases: Vec<AliasFact>,
        mut resolutions: Vec<ResolutionFact>,
        mut generated_symbols: Vec<GeneratedSymbolFact>,
        mut stable_exports: Vec<StableExportIdentity>,
    ) {
        normalize_scope_facts(&mut scopes);
        let scope_ids = scopes
            .iter()
            .enumerate()
            .map(|(index, scope)| (scope.id, ScopeId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        for (index, scope) in scopes.iter_mut().enumerate() {
            scope.id = ScopeId(index as u64);
            if let Some(parent) = scope.parent
                && let Some(remapped) = scope_ids.get(&parent)
            {
                scope.parent = Some(*remapped);
            }
        }

        normalize_semantic_import_facts(&mut semantic_imports);
        for (index, import) in semantic_imports.iter_mut().enumerate() {
            import.id = SemanticImportId(index as u64);
            if let Some(scope) = import.scope
                && let Some(remapped) = scope_ids.get(&scope)
            {
                import.scope = Some(*remapped);
            }
        }

        normalize_export_facts(&mut exports);
        let export_ids = exports
            .iter()
            .enumerate()
            .map(|(index, export)| (export.id, ExportId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        for (index, export) in exports.iter_mut().enumerate() {
            export.id = ExportId(index as u64);
            if let Some(scope) = export.scope
                && let Some(remapped) = scope_ids.get(&scope)
            {
                export.scope = Some(*remapped);
            }
        }

        normalize_alias_facts(&mut aliases);
        for (index, alias) in aliases.iter_mut().enumerate() {
            alias.id = AliasId(index as u64);
        }

        normalize_resolution_facts(&mut resolutions);
        for (index, resolution) in resolutions.iter_mut().enumerate() {
            resolution.id = ResolutionId(index as u64);
        }

        normalize_generated_symbol_facts(&mut generated_symbols);
        for (index, generated) in generated_symbols.iter_mut().enumerate() {
            generated.id = GeneratedSymbolId(index as u64);
        }

        normalize_stable_export_identities(&mut stable_exports);
        for (index, stable_export) in stable_exports.iter_mut().enumerate() {
            stable_export.id = StableExportId(index as u64);
            if let Some(remapped) = export_ids.get(&stable_export.export) {
                stable_export.export = *remapped;
            }
        }

        self.scopes = scopes;
        self.semantic_imports = semantic_imports;
        self.exports = exports;
        self.aliases = aliases;
        self.resolution_facts = resolutions;
        self.generated_symbols = generated_symbols;
        self.stable_exports = stable_exports;
        self.rebuild_semantic_index_indexes();
        self.refresh_semantic_index_metadata();
    }

    pub(crate) fn replace_semantic_mir(&mut self, output: MirOutput) -> Result<(), AnalysisError> {
        self.semantic = Some(SemanticStore::from_output(output)?);
        self.refresh_semantic_mir_metadata();
        Ok(())
    }

    fn refresh_semantic_mir_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::MirBody);
        self.fact_meta.remove_family(FactFamily::MirOperation);
        self.fact_meta.remove_family(FactFamily::Place);
        self.fact_meta
            .remove_family(FactFamily::UnsupportedSemantic);

        let body_metadata = self
            .mir_bodies()
            .iter()
            .map(|body| (body.id.0, self.mir_body_metadata(body)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in body_metadata {
            self.record_fact_meta(FactFamily::MirBody, run_id, metadata);
        }

        let operation_metadata = self
            .mir_operations()
            .iter()
            .map(|operation| (operation.id.0, self.mir_operation_metadata(operation)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in operation_metadata {
            self.record_fact_meta(FactFamily::MirOperation, run_id, metadata);
        }

        let place_metadata = self
            .mir_places()
            .iter()
            .map(|place| (place.id.0, self.place_metadata(place)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in place_metadata {
            self.record_fact_meta(FactFamily::Place, run_id, metadata);
        }

        let unsupported_metadata = self
            .unsupported_semantics()
            .iter()
            .map(|row| (row.id.0, self.unsupported_semantic_metadata(row)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in unsupported_metadata {
            self.record_fact_meta(FactFamily::UnsupportedSemantic, run_id, metadata);
        }
    }

    fn refresh_module_graph_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::ModuleNode);
        self.fact_meta.remove_family(FactFamily::ResolvedImport);
        self.fact_meta.remove_family(FactFamily::ModuleEdge);

        let node_metadata = self
            .module_nodes
            .iter()
            .map(|node| (node.id.0, self.module_node_metadata(node)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in node_metadata {
            self.record_fact_meta(FactFamily::ModuleNode, run_id, metadata);
        }

        let resolved_metadata = self
            .resolved_imports
            .iter()
            .map(|fact| (fact.id.0, self.resolved_import_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in resolved_metadata {
            self.record_fact_meta(FactFamily::ResolvedImport, run_id, metadata);
        }

        let edge_metadata = self
            .module_edges
            .iter()
            .map(|edge| (edge.id.0, self.module_edge_metadata(edge)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in edge_metadata {
            self.record_fact_meta(FactFamily::ModuleEdge, run_id, metadata);
        }
    }

    fn refresh_topology_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::WorkspaceRoot);
        self.fact_meta.remove_family(FactFamily::TopologyPackage);
        self.fact_meta.remove_family(FactFamily::SourceSet);
        self.fact_meta
            .remove_family(FactFamily::DependencyRequirement);
        self.fact_meta
            .remove_family(FactFamily::ResolvedDependencyEdge);
        self.fact_meta
            .remove_family(FactFamily::RepoTopologyOverlay);
        self.refresh_import_to_package_metadata();

        let root_metadata = self
            .workspace_roots
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::WorkspaceRoot,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in root_metadata {
            self.record_fact_meta(FactFamily::WorkspaceRoot, run_id, metadata);
        }

        let package_metadata = self
            .topology_packages
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::TopologyPackage,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in package_metadata {
            self.record_fact_meta(FactFamily::TopologyPackage, run_id, metadata);
        }

        let source_set_metadata = self
            .source_sets
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::SourceSet,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in source_set_metadata {
            self.record_fact_meta(FactFamily::SourceSet, run_id, metadata);
        }

        let requirement_metadata = self
            .dependency_requirements
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::DependencyRequirement,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in requirement_metadata {
            self.record_fact_meta(FactFamily::DependencyRequirement, run_id, metadata);
        }

        let resolved_metadata = self
            .resolved_dependency_edges
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::ResolvedDependencyEdge,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in resolved_metadata {
            self.record_fact_meta(FactFamily::ResolvedDependencyEdge, run_id, metadata);
        }

        let overlay_metadata = self
            .repo_topology_overlays
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::RepoTopologyOverlay,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in overlay_metadata {
            self.record_fact_meta(FactFamily::RepoTopologyOverlay, run_id, metadata);
        }
    }

    fn refresh_import_to_package_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::ImportToPackage);

        let metadata = self
            .import_to_package_edges
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::ImportToPackage,
                        MODULE_TOPOLOGY_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in metadata {
            self.record_fact_meta(FactFamily::ImportToPackage, run_id, metadata);
        }
    }

    fn refresh_semantic_index_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::Scope);
        self.fact_meta.remove_family(FactFamily::SemanticImport);
        self.fact_meta.remove_family(FactFamily::Export);
        self.fact_meta.remove_family(FactFamily::Alias);
        self.fact_meta.remove_family(FactFamily::Resolution);
        self.fact_meta.remove_family(FactFamily::GeneratedSymbol);
        self.fact_meta.remove_family(FactFamily::StableExport);

        let scope_metadata = self
            .scopes
            .iter()
            .map(|scope| {
                (
                    scope.id.0,
                    self.semantic_fact_metadata(FactFamily::Scope, &scope.stable_key, scope.status),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in scope_metadata {
            self.record_fact_meta(FactFamily::Scope, run_id, metadata);
        }

        let import_metadata = self
            .semantic_imports
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(
                        FactFamily::SemanticImport,
                        &fact.stable_key,
                        fact.status,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in import_metadata {
            self.record_fact_meta(FactFamily::SemanticImport, run_id, metadata);
        }

        let export_metadata = self
            .exports
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(FactFamily::Export, &fact.stable_key, fact.status),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in export_metadata {
            self.record_fact_meta(FactFamily::Export, run_id, metadata);
        }

        let alias_metadata = self
            .aliases
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(FactFamily::Alias, &fact.stable_key, fact.status),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in alias_metadata {
            self.record_fact_meta(FactFamily::Alias, run_id, metadata);
        }

        let resolution_metadata = self
            .resolution_facts
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(
                        FactFamily::Resolution,
                        &fact.stable_key,
                        fact.status,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in resolution_metadata {
            self.record_fact_meta(FactFamily::Resolution, run_id, metadata);
        }

        let generated_metadata = self
            .generated_symbols
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(
                        FactFamily::GeneratedSymbol,
                        &fact.stable_key,
                        fact.status,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in generated_metadata {
            self.record_fact_meta(FactFamily::GeneratedSymbol, run_id, metadata);
        }

        let stable_export_metadata = self
            .stable_exports
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(
                        FactFamily::StableExport,
                        &fact.stable_key,
                        fact.status,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in stable_export_metadata {
            self.record_fact_meta(FactFamily::StableExport, run_id, metadata);
        }
    }

    fn refresh_symbol_graph_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::Symbol);
        self.fact_meta.remove_family(FactFamily::Definition);
        self.fact_meta.remove_family(FactFamily::Reference);

        let symbol_metadata = self
            .symbols
            .iter()
            .map(|symbol| (symbol.id.0, self.symbol_fact_metadata(symbol)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in symbol_metadata {
            self.record_fact_meta(FactFamily::Symbol, run_id, metadata);
        }

        let definition_metadata = self
            .definitions
            .iter()
            .map(|definition| (definition.id.0, self.definition_fact_metadata(definition)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in definition_metadata {
            self.record_fact_meta(FactFamily::Definition, run_id, metadata);
        }

        let reference_metadata = self
            .references
            .iter()
            .map(|reference| (reference.id.0, self.reference_fact_metadata(reference)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in reference_metadata {
            self.record_fact_meta(FactFamily::Reference, run_id, metadata);
        }
    }

    fn refresh_metric_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::FileMetric);
        self.fact_meta.remove_family(FactFamily::FunctionMetric);
        self.fact_meta.remove_family(FactFamily::ComplexityMetric);

        let file_metadata = self
            .file_metrics
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.file_metric_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in file_metadata {
            self.record_fact_meta(FactFamily::FileMetric, run_id, metadata);
        }

        let function_metadata = self
            .function_metrics
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.function_metric_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in function_metadata {
            self.record_fact_meta(FactFamily::FunctionMetric, run_id, metadata);
        }

        let complexity_metadata = self
            .complexity_metrics
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.complexity_metric_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in complexity_metadata {
            self.record_fact_meta(FactFamily::ComplexityMetric, run_id, metadata);
        }
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

    fn rebuild_semantic_index_indexes(&mut self) {
        self.scopes_by_id.clear();
        self.semantic_imports_by_id.clear();
        self.exports_by_id.clear();
        self.aliases_by_id.clear();
        self.resolution_facts_by_id.clear();
        self.generated_symbols_by_id.clear();
        self.stable_exports_by_id.clear();

        for (index, scope) in self.scopes.iter().enumerate() {
            self.scopes_by_id.insert(scope.id, index);
        }
        for (index, import) in self.semantic_imports.iter().enumerate() {
            self.semantic_imports_by_id.insert(import.id, index);
        }
        for (index, export) in self.exports.iter().enumerate() {
            self.exports_by_id.insert(export.id, index);
        }
        for (index, alias) in self.aliases.iter().enumerate() {
            self.aliases_by_id.insert(alias.id, index);
        }
        for (index, resolution) in self.resolution_facts.iter().enumerate() {
            self.resolution_facts_by_id.insert(resolution.id, index);
        }
        for (index, generated) in self.generated_symbols.iter().enumerate() {
            self.generated_symbols_by_id.insert(generated.id, index);
        }
        for (index, stable_export) in self.stable_exports.iter().enumerate() {
            self.stable_exports_by_id.insert(stable_export.id, index);
        }
    }

    pub fn push_ts_component(&mut self, fact: TsComponentFact) {
        let run_id = self.ts_components.len() as u64;
        let metadata = self.ts_component_metadata(&fact);
        self.ts_components.push(fact);
        self.record_fact_meta(FactFamily::TsComponent, run_id, metadata);
    }

    pub fn push_ts_class(&mut self, fact: TsClassFact) {
        let run_id = self.ts_classes.len() as u64;
        let metadata = self.ts_class_metadata(&fact);
        self.ts_classes.push(fact);
        self.record_fact_meta(FactFamily::TsClass, run_id, metadata);
    }

    pub fn push_string_literal(&mut self, fact: StringLiteralFact) {
        let run_id = self.string_literals.len() as u64;
        let metadata = self.string_literal_metadata(&fact);
        self.string_literals.push(fact);
        self.record_fact_meta(FactFamily::StringLiteral, run_id, metadata);
    }

    pub fn push_jsx_attribute(&mut self, fact: JsxAttributeFact) {
        let run_id = self.jsx_attributes.len() as u64;
        let metadata = self.jsx_attribute_metadata(&fact);
        self.jsx_attributes.push(fact);
        self.record_fact_meta(FactFamily::JsxAttribute, run_id, metadata);
    }

    pub(crate) fn fact_meta(&self) -> &FactMetaStore {
        &self.fact_meta
    }

    #[cfg(test)]
    pub(crate) fn fact_meta_mut_for_test(&mut self) -> &mut FactMetaStore {
        &mut self.fact_meta
    }

    #[cfg(test)]
    pub(crate) fn remove_fact_metadata_for_test(&mut self, fact_ref: FactRef) -> Option<FactMeta> {
        self.fact_meta.remove_for_test(fact_ref)
    }

    pub(crate) fn metadata_for(&self, fact_ref: FactRef) -> Option<&FactMeta> {
        self.fact_meta().get(fact_ref)
    }

    pub(crate) fn missing_fact_metadata(&self) -> Vec<MissingFactMeta> {
        let mut missing = Vec::new();

        for file in self.files() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::SourceFile,
                u64::from(file.id.0),
            );
        }
        for package in self.packages() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Package, package.id.0);
        }
        for function in self.functions() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Function, function.id.0);
        }
        for import in self.imports() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Import, import.id.0);
        }
        for resolved_import in self.resolved_imports() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::ResolvedImport,
                resolved_import.id.0,
            );
        }
        for module_node in self.module_nodes() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::ModuleNode, module_node.id.0);
        }
        for module_edge in self.module_edges() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::ModuleEdge, module_edge.id.0);
        }
        for root in self.workspace_roots() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::WorkspaceRoot, root.id.0);
        }
        for package in self.topology_packages() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::TopologyPackage,
                package.id.0,
            );
        }
        for source_set in self.source_sets() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::SourceSet, source_set.id.0);
        }
        for requirement in self.dependency_requirements() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::DependencyRequirement,
                requirement.id.0,
            );
        }
        for edge in self.resolved_dependency_edges() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::ResolvedDependencyEdge,
                edge.id.0,
            );
        }
        for edge in self.import_to_package_edges() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::ImportToPackage, edge.id.0);
        }
        for overlay in self.repo_topology_overlays() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::RepoTopologyOverlay,
                overlay.id.0,
            );
        }
        for scope in self.scopes() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Scope, scope.id.0);
        }
        for import in self.semantic_imports() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::SemanticImport, import.id.0);
        }
        for export in self.exports() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Export, export.id.0);
        }
        for alias in self.aliases() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Alias, alias.id.0);
        }
        for resolution in self.resolution_facts() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Resolution, resolution.id.0);
        }
        for generated in self.generated_symbols() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::GeneratedSymbol,
                generated.id.0,
            );
        }
        for stable_export in self.stable_exports() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::StableExport,
                stable_export.id.0,
            );
        }
        for body in self.mir_bodies() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::MirBody, body.id.0);
        }
        for operation in self.mir_operations() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::MirOperation, operation.id.0);
        }
        for place in self.mir_places() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Place, place.id.0);
        }
        for row in self.unsupported_semantics() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::UnsupportedSemantic,
                row.id.0,
            );
        }
        for symbol in self.symbols() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Symbol, symbol.id.0);
        }
        for definition in self.definitions() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Definition, definition.id.0);
        }
        for reference in self.references() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Reference, reference.id.0);
        }
        for branch in self.branches() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::BranchObligation,
                branch.id.0,
            );
        }
        for (run_id, _test) in self.tests().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Test, run_id as u64);
        }
        for (run_id, _coverage) in self.coverage().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Coverage, run_id as u64);
        }
        for (run_id, _file_metric) in self.file_metrics().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::FileMetric, run_id as u64);
        }
        for (run_id, _function_metric) in self.function_metrics().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::FunctionMetric,
                run_id as u64,
            );
        }
        for (run_id, _complexity_metric) in self.complexity_metrics().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::ComplexityMetric,
                run_id as u64,
            );
        }
        for (run_id, _component) in self.ts_components().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::TsComponent, run_id as u64);
        }
        for (run_id, _class) in self.ts_classes().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::TsClass, run_id as u64);
        }
        for (run_id, _literal) in self.string_literals().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::StringLiteral, run_id as u64);
        }
        for (run_id, _attribute) in self.jsx_attributes().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::JsxAttribute, run_id as u64);
        }

        missing.sort_by(|left, right| {
            (left.family.label(), left.run_id).cmp(&(right.family.label(), right.run_id))
        });
        missing
    }

    fn push_missing_fact_metadata(
        &self,
        missing: &mut Vec<MissingFactMeta>,
        family: FactFamily,
        run_id: u64,
    ) {
        if self.metadata_for(FactRef::new(family, run_id)).is_none() {
            missing.push(MissingFactMeta { family, run_id });
        }
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

    pub(crate) fn workspace_roots(&self) -> &[WorkspaceRootFact] {
        &self.workspace_roots
    }

    pub(crate) fn topology_packages(&self) -> &[TopologyPackageFact] {
        &self.topology_packages
    }

    pub(crate) fn source_sets(&self) -> &[SourceSetFact] {
        &self.source_sets
    }

    pub(crate) fn dependency_requirements(&self) -> &[DependencyRequirementFact] {
        &self.dependency_requirements
    }

    pub(crate) fn resolved_dependency_edges(&self) -> &[ResolvedDependencyEdgeFact] {
        &self.resolved_dependency_edges
    }

    pub(crate) fn import_to_package_edges(&self) -> &[ImportToPackageFact] {
        &self.import_to_package_edges
    }

    pub(crate) fn repo_topology_overlays(&self) -> &[RepoTopologyOverlayFact] {
        &self.repo_topology_overlays
    }

    pub(crate) fn scopes(&self) -> &[ScopeFact] {
        &self.scopes
    }

    pub(crate) fn semantic_imports(&self) -> &[SemanticImportFact] {
        &self.semantic_imports
    }

    pub(crate) fn exports(&self) -> &[ExportFact] {
        &self.exports
    }

    pub(crate) fn aliases(&self) -> &[AliasFact] {
        &self.aliases
    }

    pub(crate) fn resolution_facts(&self) -> &[ResolutionFact] {
        &self.resolution_facts
    }

    pub(crate) fn generated_symbols(&self) -> &[GeneratedSymbolFact] {
        &self.generated_symbols
    }

    pub(crate) fn stable_exports(&self) -> &[StableExportIdentity] {
        &self.stable_exports
    }

    pub(crate) fn semantic_store(&self) -> Option<&SemanticStore> {
        self.semantic.as_ref()
    }

    pub(crate) fn mir_bodies(&self) -> &[MirBody] {
        self.semantic_store().map_or(&[], SemanticStore::mir_bodies)
    }

    pub(crate) fn mir_operations(&self) -> &[MirOperation] {
        self.semantic_store()
            .map_or(&[], SemanticStore::mir_operations)
    }

    pub(crate) fn mir_places(&self) -> &[PlaceFact] {
        self.semantic_store().map_or(&[], SemanticStore::places)
    }

    pub(crate) fn unsupported_semantics(&self) -> &[UnsupportedSemanticFact] {
        self.semantic_store()
            .map_or(&[], SemanticStore::unsupported_semantics)
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

    pub(crate) fn symbols_for_file(&self, file: FileId) -> impl Iterator<Item = &SymbolFact> + '_ {
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
                indexes
                    .iter()
                    .filter_map(|index| self.references.get(*index))
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
                indexes
                    .iter()
                    .filter_map(|index| self.references.get(*index))
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

    fn record_fact_meta(&mut self, family: FactFamily, run_id: u64, meta: FactMeta) {
        let reference = FactRef::new(family, run_id);
        let _insert = self.fact_meta.insert(reference, meta);
        debug_assert!(self.metadata_for(reference).is_some());
    }

    fn package_metadata(&self, fact: &PackageFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Package,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([]),
        )
    }

    fn function_metadata(&self, fact: &FunctionFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Function,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([
                ("is_test", fact.is_test.to_string()),
                ("is_exported", fact.is_exported.to_string()),
                (
                    "cyclomatic_complexity",
                    fact.cyclomatic_complexity.to_string(),
                ),
                ("calls", fact.calls.join("\n")),
            ]),
        )
    }

    fn import_metadata(&self, fact: &ImportFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Import,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("import_path", fact.path.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([(
                "package",
                fact.package.clone().unwrap_or_else(|| "none".to_string()),
            )]),
        )
    }

    fn branch_metadata(&self, fact: &BranchObligation) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::BranchObligation,
            GO_SYNTAX_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("stable_fingerprint", fact.stable_fingerprint.clone()),
            ]),
            stable_parts([
                ("function", option_function_id(fact.function)),
                ("span", span_metadata_value(&fact.decision_span)),
                ("condition_text", fact.condition_text.clone()),
                ("edge_label", fact.edge_label.clone()),
                ("is_error_path", fact.is_error_path.to_string()),
            ]),
        )
    }

    fn test_metadata(&self, fact: &TestFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Test,
            GO_SYNTAX_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([
                ("function", option_function_id(fact.function)),
                ("evidence_terms", fact.evidence_terms.join("\n")),
                ("assertion_count", fact.assertion_count.to_string()),
                ("subtest_count", fact.subtest_count.to_string()),
                ("subtest_names", fact.subtest_names.join("\n")),
                ("table_rows", fact.table_rows.to_string()),
            ]),
        )
    }

    fn coverage_metadata(&self, fact: &CoverageFact) -> FactMeta {
        let branch = self.branches.iter().find(|branch| branch.id == fact.branch);
        let (path, branch_fingerprint, precision, confidence) = if let Some(branch) = branch {
            (
                self.path_for(branch.file),
                branch.stable_fingerprint.clone(),
                FactPrecision::SetupAware,
                FactConfidence::Medium,
            )
        } else {
            (
                "<unknown>".to_string(),
                format!("unresolved:{}", fact.branch.0),
                FactPrecision::Unsupported,
                FactConfidence::Low,
            )
        };

        fact_meta_from_parts(
            FactFamily::Coverage,
            branch
                .map(|branch| syntax_provider_for_file(self.file(branch.file)))
                .unwrap_or(GO_SYNTAX_PROVIDER_ID),
            precision,
            confidence,
            stable_parts([
                ("path", path),
                ("branch_fingerprint", branch_fingerprint),
                ("source", fact.source.clone()),
            ]),
            stable_parts([
                ("branch", fact.branch.0.to_string()),
                ("covered", option_bool(fact.covered)),
            ]),
        )
    }

    fn file_metric_metadata(&self, fact: &FileMetricFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::FileMetric,
            METRICS_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([("file_key", self.source_file_key(fact.file))]),
            stable_parts([
                ("language", language_label(fact.language).to_string()),
                ("line_count", fact.line_count.to_string()),
                (
                    "non_empty_line_count",
                    fact.non_empty_line_count.to_string(),
                ),
                ("byte_count", fact.byte_count.to_string()),
                ("function_count", fact.function_count.to_string()),
            ]),
        )
    }

    fn function_metric_metadata(&self, fact: &FunctionMetricFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::FunctionMetric,
            METRICS_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                (
                    "function_key",
                    self.function_key(fact.function, &fact.name, &fact.span),
                ),
                ("metric_name", FUNCTION_SIZE_METRIC_NAME.to_string()),
            ]),
            stable_parts([
                ("file_key", self.source_file_key(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("line_count", fact.line_count.to_string()),
                ("byte_count", fact.byte_count.to_string()),
            ]),
        )
    }

    fn complexity_metric_metadata(&self, fact: &ComplexityMetricFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::ComplexityMetric,
            METRICS_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                (
                    "function_key",
                    self.function_key(fact.function, &fact.name, &fact.span),
                ),
                ("metric_name", CYCLOMATIC_COMPLEXITY_METRIC_NAME.to_string()),
            ]),
            stable_parts([
                ("file_key", self.source_file_key(fact.file)),
                ("language", language_label(fact.language).to_string()),
                (
                    "cyclomatic_complexity",
                    fact.cyclomatic_complexity.to_string(),
                ),
            ]),
        )
    }

    fn module_node_metadata(&self, node: &ModuleNode) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::ModuleNode,
            MODULE_GRAPH_PROVIDER_ID,
            FactPrecision::SetupAware,
            FactConfidence::High,
            stable_parts([
                ("kind", module_node_kind_label(node.kind).to_string()),
                ("label", node.label.clone()),
                ("path", option_file_path(self, node.file)),
                (
                    "package_key",
                    node.package
                        .map(|package| self.fact_stable_key(FactFamily::Package, package.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "language",
                    node.language
                        .map(|language| language_label(language).to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
            stable_parts([("id", node.id.0.to_string())]),
        )
    }

    fn resolved_import_metadata(&self, fact: &ResolvedImportFact) -> FactMeta {
        let (precision, confidence) = resolution_metadata(fact.precision, fact.status);
        fact_meta_from_parts(
            FactFamily::ResolvedImport,
            MODULE_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            stable_parts([
                (
                    "import_key",
                    self.fact_stable_key(FactFamily::Import, fact.import.0),
                ),
                ("from_path", self.path_for(fact.from_file)),
                (
                    "target_node_key",
                    fact.target_node
                        .map(|node| self.fact_stable_key(FactFamily::ModuleNode, node.0))
                        .unwrap_or_else(none_value),
                ),
                ("status", resolution_status_label(fact.status).to_string()),
                (
                    "precision",
                    resolution_precision_label(fact.precision).to_string(),
                ),
                (
                    "reason",
                    fact.reason
                        .map(|reason| unresolved_reason_label(reason).to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("import", fact.import.0.to_string()),
                ("from_file", u64::from(fact.from_file.0).to_string()),
                (
                    "target_node",
                    fact.target_node
                        .map(|node| node.0.to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn module_edge_metadata(&self, edge: &ModuleEdge) -> FactMeta {
        let (precision, confidence) = resolution_status_metadata(edge.status);
        fact_meta_from_parts(
            FactFamily::ModuleEdge,
            MODULE_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            stable_parts([
                (
                    "from_node_key",
                    self.fact_stable_key(FactFamily::ModuleNode, edge.from.0),
                ),
                (
                    "to_node_key",
                    self.fact_stable_key(FactFamily::ModuleNode, edge.to.0),
                ),
                (
                    "import_key",
                    edge.import
                        .map(|import| self.fact_stable_key(FactFamily::Import, import.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "resolved_import_key",
                    edge.resolved_import
                        .map(|resolved| {
                            self.fact_stable_key(FactFamily::ResolvedImport, resolved.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                ("kind", module_edge_kind_label(edge.kind).to_string()),
                ("status", resolution_status_label(edge.status).to_string()),
            ]),
            stable_parts([
                ("id", edge.id.0.to_string()),
                ("from", edge.from.0.to_string()),
                ("to", edge.to.0.to_string()),
            ]),
        )
    }

    fn symbol_fact_metadata(&self, fact: &SymbolFact) -> FactMeta {
        let (precision, confidence) = symbol_metadata(fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Symbol,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("qualified_name", fact.qualified_name.clone()),
                (
                    "precision",
                    symbol_precision_label(fact.precision).to_string(),
                ),
                ("path", option_file_path(self, fact.file)),
                (
                    "span",
                    option_span_metadata_value(fact.primary_span.as_ref()),
                ),
            ]),
        )
    }

    fn definition_fact_metadata(&self, fact: &DefinitionFact) -> FactMeta {
        let (precision, confidence) = symbol_metadata(fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Definition,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("symbol", fact.symbol.0.to_string()),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("qualified_name", fact.qualified_name.clone()),
                (
                    "precision",
                    symbol_precision_label(fact.precision).to_string(),
                ),
                ("path", option_file_path(self, fact.file)),
                (
                    "span",
                    option_span_metadata_value(fact.primary_span.as_ref()),
                ),
            ]),
        )
    }

    fn reference_fact_metadata(&self, fact: &ReferenceFact) -> FactMeta {
        let (precision, confidence) = symbol_metadata(fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Reference,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("qualified_name", fact.qualified_name.clone()),
                (
                    "precision",
                    symbol_precision_label(fact.precision).to_string(),
                ),
                (
                    "status",
                    symbol_resolution_status_label(fact.status).to_string(),
                ),
                ("path", option_file_path(self, fact.file)),
                (
                    "span",
                    option_span_metadata_value(fact.primary_span.as_ref()),
                ),
                (
                    "target",
                    fact.target
                        .map(|target| target.0.to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn semantic_fact_metadata(
        &self,
        family: FactFamily,
        stable_key: &str,
        status: SemanticStatus,
    ) -> FactMeta {
        let (precision, confidence) = semantic_status_metadata(status);
        fact_meta_from_stable_key(
            family,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            stable_key.to_string(),
            stable_parts([("status", semantic_status_label(status).to_string())]),
        )
    }

    fn mir_body_metadata(&self, body: &MirBody) -> FactMeta {
        let (precision, confidence) = mir_status_metadata(body.status);
        fact_meta_from_stable_key(
            FactFamily::MirBody,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            body.stable_key.clone(),
            stable_parts([
                ("status", mir_status_label(body.status).to_string()),
                ("language", language_label(body.language).to_string()),
                ("file_key", self.source_file_key(body.file)),
                (
                    "function_key",
                    self.function_key(body.function, "", &body.span),
                ),
                ("owner_stable_key", body.owner_stable_key.clone()),
                (
                    "package",
                    body.package
                        .map(|package| package.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                (
                    "module",
                    body.module
                        .map(|module| module.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                ("span", span_metadata_value(&body.span)),
            ]),
        )
    }

    fn mir_operation_metadata(&self, operation: &MirOperation) -> FactMeta {
        let (precision, confidence) = mir_status_metadata(operation.status);
        fact_meta_from_stable_key(
            FactFamily::MirOperation,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            operation.stable_key.clone(),
            stable_parts([
                ("status", mir_status_label(operation.status).to_string()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, operation.body.0),
                ),
                ("ordinal", operation.ordinal.to_string()),
                ("span", span_metadata_value(&operation.span)),
            ]),
        )
    }

    fn place_metadata(&self, place: &PlaceFact) -> FactMeta {
        let (precision, confidence) = place_status_metadata(place.status);
        fact_meta_from_stable_key(
            FactFamily::Place,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            place.stable_key.clone(),
            stable_parts([
                ("status", place_status_label(place.status).to_string()),
                ("language", language_label(place.language).to_string()),
                ("path", option_file_path(self, place.file)),
                ("function", option_function_id(place.function)),
                ("projection_count", place.projections.len().to_string()),
            ]),
        )
    }

    fn unsupported_semantic_metadata(&self, row: &UnsupportedSemanticFact) -> FactMeta {
        let (precision, confidence) = mir_status_metadata(row.status);
        fact_meta_from_stable_key(
            FactFamily::UnsupportedSemantic,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            row.stable_key.clone(),
            stable_parts([
                ("status", mir_status_label(row.status).to_string()),
                ("language", language_label(row.language).to_string()),
                ("path", self.path_for(row.file)),
                ("span", span_metadata_value(&row.span)),
                ("construct", row.construct.clone()),
                ("source_evidence", row.source_evidence.clone()),
                (
                    "body_key",
                    row.body
                        .map(|body| self.fact_stable_key(FactFamily::MirBody, body.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    row.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                (
                    "affected_places",
                    row.affected_places
                        .iter()
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
            ]),
        )
    }

    fn fact_stable_key(&self, family: FactFamily, run_id: u64) -> String {
        self.metadata_for(FactRef::new(family, run_id))
            .map(|metadata| metadata.stable_key.clone())
            .unwrap_or_else(|| format!("<missing:{}:{run_id}>", family.label()))
    }

    fn source_file_key(&self, file: FileId) -> String {
        self.metadata_for(FactRef::new(FactFamily::SourceFile, u64::from(file.0)))
            .map(|metadata| metadata.stable_key.clone())
            .unwrap_or_else(|| self.path_for(file).replace('\\', "/"))
    }

    fn function_key(&self, function: FunctionId, name: &str, span: &Span) -> String {
        self.metadata_for(FactRef::new(FactFamily::Function, function.0))
            .map(|metadata| metadata.stable_key.clone())
            .unwrap_or_else(|| {
                stable_key_from_parts(
                    FactFamily::Function,
                    &[
                        ("path", self.path_for(span.file)),
                        ("name", name.to_string()),
                        ("span", span_metadata_value(span)),
                    ],
                )
            })
    }

    fn ts_component_metadata(&self, fact: &TsComponentFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::TsComponent,
            TS_SYNTAX_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([("function", option_function_id(fact.function))]),
        )
    }

    fn ts_class_metadata(&self, fact: &TsClassFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::TsClass,
            TS_SYNTAX_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([
                ("is_exported", fact.is_exported.to_string()),
                ("is_component_like", fact.is_component_like.to_string()),
            ]),
        )
    }

    fn string_literal_metadata(&self, fact: &StringLiteralFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::StringLiteral,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("value", fact.value.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([]),
        )
    }

    fn jsx_attribute_metadata(&self, fact: &JsxAttributeFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::JsxAttribute,
            TS_SYNTAX_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("value", option_string(fact.value.as_deref())),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([]),
        )
    }
}

fn normalize_scope_facts(facts: &mut [ScopeFact]) {
    for fact in facts.iter_mut() {
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
}

fn normalize_semantic_import_facts(facts: &mut [SemanticImportFact]) {
    for fact in facts.iter_mut() {
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
}

fn normalize_export_facts(facts: &mut [ExportFact]) {
    for fact in facts.iter_mut() {
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
}

fn normalize_alias_facts(facts: &mut [AliasFact]) {
    for fact in facts.iter_mut() {
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
}

fn normalize_resolution_facts(facts: &mut [ResolutionFact]) {
    for fact in facts.iter_mut() {
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
}

fn normalize_generated_symbol_facts(facts: &mut [GeneratedSymbolFact]) {
    for fact in facts.iter_mut() {
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
}

fn normalize_stable_export_identities(facts: &mut [StableExportIdentity]) {
    for fact in facts.iter_mut() {
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
}

fn source_file_metadata(relative_path: &str, language: Language, content_hash: &str) -> FactMeta {
    fact_meta_from_parts(
        FactFamily::SourceFile,
        SOURCE_PROVIDER_ID,
        FactPrecision::Exact,
        FactConfidence::High,
        stable_parts([
            ("path", relative_path.to_string()),
            ("content_hash", content_hash.to_string()),
        ]),
        stable_parts([("language", language_label(language).to_string())]),
    )
}

fn fact_meta_from_parts<const STABLE: usize, const EXTRA: usize>(
    family: FactFamily,
    producer_id: &'static str,
    precision: FactPrecision,
    confidence: FactConfidence,
    stable_parts: [(&'static str, String); STABLE],
    payload_extra_parts: [(&'static str, String); EXTRA],
) -> FactMeta {
    let stable_key = stable_key_from_parts(family, &stable_parts);
    let mut payload_parts = stable_parts.to_vec();
    payload_parts.extend(payload_extra_parts);
    let payload_digest = metadata_payload_digest(&stable_key, &payload_parts);

    FactMeta {
        stable_key,
        producer_id,
        layer_id: producer_id,
        precision,
        confidence,
        validation: ValidationStatus::NativeTrusted,
        payload_digest,
    }
}

fn fact_meta_from_stable_key<const EXTRA: usize>(
    _family: FactFamily,
    producer_id: &'static str,
    precision: FactPrecision,
    confidence: FactConfidence,
    stable_key: String,
    payload_extra_parts: [(&'static str, String); EXTRA],
) -> FactMeta {
    let payload_parts = payload_extra_parts.to_vec();
    let payload_digest = metadata_payload_digest(&stable_key, &payload_parts);

    FactMeta {
        stable_key,
        producer_id,
        layer_id: producer_id,
        precision,
        confidence,
        validation: ValidationStatus::NativeTrusted,
        payload_digest,
    }
}

fn topology_fact_metadata(
    family: FactFamily,
    producer_id: &'static str,
    precision: TopologyPrecision,
    stable_key: &str,
) -> FactMeta {
    let (precision, confidence) = topology_precision_metadata(precision);
    fact_meta_from_stable_key(
        family,
        producer_id,
        precision,
        confidence,
        stable_key.to_string(),
        stable_parts([]),
    )
}

fn stable_parts<const N: usize>(parts: [(&'static str, String); N]) -> [(&'static str, String); N] {
    parts
}

fn metadata_payload_digest(stable_key: &str, parts: &[(&'static str, String)]) -> String {
    let mut normalized = parts
        .iter()
        .map(|(label, value)| format!("{label}={}", metadata_value(value)))
        .collect::<Vec<_>>();
    normalized.sort();

    let mut digest_parts = Vec::with_capacity(normalized.len() + 1);
    digest_parts.push(stable_key.to_string());
    digest_parts.extend(normalized);
    let digest_refs = digest_parts.iter().map(String::as_str).collect::<Vec<_>>();
    fingerprint(&digest_refs)
}

fn metadata_value(value: &str) -> String {
    value.replace('\\', "/")
}

fn span_metadata_value(span: &Span) -> String {
    format!(
        "{}-{}:{}:{}-{}:{}",
        span.start_byte,
        span.end_byte,
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col
    )
}

fn option_span_metadata_value(span: Option<&Span>) -> String {
    span.map(span_metadata_value).unwrap_or_else(none_value)
}

fn language_label(language: Language) -> &'static str {
    match language {
        Language::Go => "go",
        Language::TypeScript => "typescript",
        Language::Tsx => "tsx",
        Language::JavaScript => "javascript",
        Language::Jsx => "jsx",
        Language::Unknown => "unknown",
    }
}

fn module_node_kind_label(kind: ModuleNodeKind) -> &'static str {
    match kind {
        ModuleNodeKind::File => "file",
        ModuleNodeKind::Package => "package",
        ModuleNodeKind::Module => "module",
        ModuleNodeKind::External => "external",
    }
}

fn module_edge_kind_label(kind: ModuleEdgeKind) -> &'static str {
    match kind {
        ModuleEdgeKind::Contains => "contains",
        ModuleEdgeKind::Imports => "imports",
        ModuleEdgeKind::DependsOn => "depends_on",
    }
}

fn resolution_status_label(status: ResolutionStatus) -> &'static str {
    match status {
        ResolutionStatus::Resolved => "resolved",
        ResolutionStatus::External => "external",
        ResolutionStatus::Unresolved => "unresolved",
        ResolutionStatus::SetupMissing => "setup_missing",
        ResolutionStatus::Dynamic => "dynamic",
        ResolutionStatus::Unsupported => "unsupported",
    }
}

fn resolution_precision_label(precision: ResolutionPrecision) -> &'static str {
    match precision {
        ResolutionPrecision::ExactFile => "exact_file",
        ResolutionPrecision::Package => "package",
        ResolutionPrecision::ExternalPackage => "external_package",
        ResolutionPrecision::Heuristic => "heuristic",
        ResolutionPrecision::None => "none",
    }
}

fn unresolved_reason_label(reason: UnresolvedReason) -> &'static str {
    match reason {
        UnresolvedReason::NotFound => "not_found",
        UnresolvedReason::SetupMissing => "setup_missing",
        UnresolvedReason::DynamicExpression => "dynamic_expression",
        UnresolvedReason::UnsupportedLanguage => "unsupported_language",
        UnresolvedReason::UnsupportedImport => "unsupported_import",
        UnresolvedReason::ResolverError => "resolver_error",
        UnresolvedReason::OutsideWorkspace => "outside_workspace",
    }
}

fn symbol_precision_label(precision: SymbolPrecision) -> &'static str {
    match precision {
        SymbolPrecision::ExactSemantic => "exact_semantic",
        SymbolPrecision::ExactLocal => "exact_local",
        SymbolPrecision::ModuleLinked => "module_linked",
        SymbolPrecision::Heuristic => "heuristic",
        SymbolPrecision::Unresolved => "unresolved",
        SymbolPrecision::Ambiguous => "ambiguous",
        SymbolPrecision::SetupMissing => "setup_missing",
        SymbolPrecision::Unsupported => "unsupported",
    }
}

fn symbol_resolution_status_label(status: SymbolResolutionStatus) -> &'static str {
    match status {
        SymbolResolutionStatus::Resolved => "resolved",
        SymbolResolutionStatus::Unresolved => "unresolved",
        SymbolResolutionStatus::Ambiguous => "ambiguous",
        SymbolResolutionStatus::SetupMissing => "setup_missing",
        SymbolResolutionStatus::Unsupported => "unsupported",
    }
}

fn semantic_status_metadata(status: SemanticStatus) -> (FactPrecision, FactConfidence) {
    match status {
        SemanticStatus::Resolved | SemanticStatus::Generated => {
            (FactPrecision::SetupAware, FactConfidence::High)
        }
        SemanticStatus::Ambiguous => (FactPrecision::Ambiguous, FactConfidence::Medium),
        SemanticStatus::Unresolved => (FactPrecision::Unresolved, FactConfidence::Medium),
        SemanticStatus::Cycle | SemanticStatus::Unsupported => {
            (FactPrecision::Unsupported, FactConfidence::Low)
        }
        SemanticStatus::Dynamic => (FactPrecision::Heuristic, FactConfidence::Low),
        SemanticStatus::External => (FactPrecision::SetupAware, FactConfidence::Medium),
        SemanticStatus::SetupMissing => (FactPrecision::SetupMissing, FactConfidence::High),
    }
}

fn mir_status_metadata(status: MirStatus) -> (FactPrecision, FactConfidence) {
    match status {
        MirStatus::Resolved => (FactPrecision::SetupAware, FactConfidence::High),
        MirStatus::Partial => (FactPrecision::Heuristic, FactConfidence::Medium),
        MirStatus::Unknown => (FactPrecision::Unresolved, FactConfidence::Low),
        MirStatus::Unsupported => (FactPrecision::Unsupported, FactConfidence::Low),
    }
}

fn place_status_metadata(status: PlaceStatus) -> (FactPrecision, FactConfidence) {
    match status {
        PlaceStatus::Resolved => (FactPrecision::SetupAware, FactConfidence::High),
        PlaceStatus::Partial => (FactPrecision::Heuristic, FactConfidence::Medium),
        PlaceStatus::Unknown => (FactPrecision::Unresolved, FactConfidence::Low),
        PlaceStatus::Unsupported => (FactPrecision::Unsupported, FactConfidence::Low),
    }
}

fn topology_precision_metadata(precision: TopologyPrecision) -> (FactPrecision, FactConfidence) {
    match precision {
        TopologyPrecision::ExactStatic | TopologyPrecision::ExactLockfile => {
            (FactPrecision::SetupAware, FactConfidence::High)
        }
        TopologyPrecision::Heuristic => (FactPrecision::Heuristic, FactConfidence::Medium),
        TopologyPrecision::Unknown => (FactPrecision::Unresolved, FactConfidence::Low),
        TopologyPrecision::Unsupported => (FactPrecision::Unsupported, FactConfidence::Low),
    }
}

fn semantic_status_label(status: SemanticStatus) -> &'static str {
    match status {
        SemanticStatus::Resolved => "resolved",
        SemanticStatus::Ambiguous => "ambiguous",
        SemanticStatus::Unresolved => "unresolved",
        SemanticStatus::Cycle => "cycle",
        SemanticStatus::Generated => "generated",
        SemanticStatus::Dynamic => "dynamic",
        SemanticStatus::External => "external",
        SemanticStatus::SetupMissing => "setup_missing",
        SemanticStatus::Unsupported => "unsupported",
    }
}

fn mir_status_label(status: MirStatus) -> &'static str {
    match status {
        MirStatus::Resolved => "resolved",
        MirStatus::Partial => "partial",
        MirStatus::Unknown => "unknown",
        MirStatus::Unsupported => "unsupported",
    }
}

fn place_status_label(status: PlaceStatus) -> &'static str {
    match status {
        PlaceStatus::Resolved => "resolved",
        PlaceStatus::Partial => "partial",
        PlaceStatus::Unknown => "unknown",
        PlaceStatus::Unsupported => "unsupported",
    }
}

fn syntax_provider_for_language(language: Language) -> &'static str {
    if language.is_ts_family() {
        TS_SYNTAX_PROVIDER_ID
    } else if language == Language::Go {
        GO_SYNTAX_PROVIDER_ID
    } else {
        SOURCE_PROVIDER_ID
    }
}

fn syntax_provider_for_file(file: Option<&SourceFile>) -> &'static str {
    file.map(|file| syntax_provider_for_language(file.language))
        .unwrap_or(GO_SYNTAX_PROVIDER_ID)
}

fn option_function_id(function: Option<FunctionId>) -> String {
    function
        .map(|function| function.0.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn option_file_path(db: &AnalysisDb, file: Option<FileId>) -> String {
    file.map(|file| db.path_for(file))
        .unwrap_or_else(none_value)
}

fn none_value() -> String {
    "<none>".to_string()
}

fn option_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn option_string(value: Option<&str>) -> String {
    value
        .map(|value| format!("some:{value}"))
        .unwrap_or_else(|| "none".to_string())
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
    fact_views: Arc<[FactViewRequirement]>,
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
            fact_views: Arc::from(Vec::new().into_boxed_slice()),
            run: Arc::new(run),
        }
    }

    pub(crate) fn from_parts_with_fact_views<M, C, R>(
        meta: M,
        capabilities: C,
        fact_views: Vec<FactViewRequirement>,
        run: R,
    ) -> Self
    where
        M: Fn() -> RuleMeta + Send + Sync + 'static,
        C: Fn() -> Capabilities + Send + Sync + 'static,
        R: Fn(&AnalysisDb, &mut RuleCtx<'_>) -> RuleResult + Send + Sync + 'static,
    {
        Self {
            meta: Arc::new(meta),
            capabilities: Arc::new(capabilities),
            fact_views: Arc::from(fact_views.into_boxed_slice()),
            run: Arc::new(run),
        }
    }

    pub(crate) fn meta(&self) -> RuleMeta {
        (self.meta)()
    }

    pub(crate) fn capabilities(&self) -> Capabilities {
        (self.capabilities)()
    }

    pub(crate) fn manifest(&self, options: Option<&RuleOptions>) -> RuleManifest {
        RuleManifest::from_parts(
            self.meta(),
            self.capabilities(),
            self.fact_views.iter().cloned().collect(),
            options,
        )
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
    use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId, UnsupportedId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{
        AssignMode, ConservativeAction, MirOperation, MirOperationKind, MirValue,
        UnsupportedDomain, UnsupportedPrecision, UnsupportedSemanticFact,
    };
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::analysis_kernel::{
        FactConfidence, FactFamily, FactPrecision, FactRef, ValidationStatus,
    };
    use crate::sdk::facts::{
        BranchObligations, FactView, Functions, GoTests, Imports, JsxAttributes, Packages,
        SourceFiles, StringLiterals, TsClasses, TsComponents,
    };
    use crate::symbol_graph::semantic::{
        AliasFact, ExportFact, GeneratedSymbolFact, ResolutionFact, ScopeFact, ScopeId, ScopeKind,
        SemanticImportFact, SemanticStatus, StableExportIdentity,
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

    fn test_scope(name: &str, file: FileId, status: SemanticStatus) -> ScopeFact {
        let scope_path = vec![name.to_string()];
        ScopeFact {
            id: ScopeId(99),
            language: Language::TypeScript,
            file: Some(file),
            package: None,
            module: None,
            parent: None,
            stable_key: ScopeFact::stable_key_for(
                Language::TypeScript,
                &scope_path,
                Some(format!("file:{}", file.0)),
                None,
                None,
                ScopeKind::Function,
                status,
            ),
            scope_path,
            kind: ScopeKind::Function,
            status,
        }
    }

    fn test_mir_body(id: u64, file: FileId, stable_key: &str) -> MirBody {
        MirBody {
            id: MirBodyId(id),
            language: Language::TypeScript,
            file,
            function: FunctionId(id),
            package: None,
            module: None,
            owner_stable_key: format!("function:{stable_key}"),
            span: test_span(file, 1),
            stable_key: stable_key.to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn test_place(id: u64, file: FileId, stable_key: &str) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::TypeScript,
            file: Some(file),
            function: Some(FunctionId(0)),
            root: PlaceRoot::Local {
                function: FunctionId(0),
                name: stable_key.to_string(),
            },
            projections: Vec::new(),
            stable_key: stable_key.to_string(),
            status: PlaceStatus::Resolved,
        }
    }

    fn test_mir_operation(
        id: u64,
        body: MirBodyId,
        place: PlaceId,
        value: PlaceId,
        stable_key: &str,
    ) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body,
            ordinal: id as u32,
            span: test_span(FileId(0), 1),
            kind: MirOperationKind::Assign {
                place,
                value: MirValue::Place(value),
                mode: AssignMode::Overwrite,
            },
            stable_key: stable_key.to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn test_unsupported(stable_key: &str) -> UnsupportedSemanticFact {
        UnsupportedSemanticFact {
            id: UnsupportedId(9),
            body: None,
            operation: None,
            language: Language::TypeScript,
            file: FileId(0),
            span: test_span(FileId(0), 1),
            construct: "dynamic-property".to_string(),
            source_evidence: "target[key]".to_string(),
            affected_places: Vec::new(),
            affected_domains: vec![UnsupportedDomain::Mir],
            conservative_action: ConservativeAction::HavocAffectedPlaces,
            precision: UnsupportedPrecision::Unsupported,
            status: MirStatus::Unsupported,
            stable_key: stable_key.to_string(),
        }
    }

    mod semantic_mir_storage {
        use super::*;

        #[test]
        fn replace_semantic_mir_removes_stale_rows_from_prior_run() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let first = MirOutput {
                bodies: vec![test_mir_body(9, file, "body:first")],
                places: vec![test_place(9, file, "place:first")],
                operations: vec![test_mir_operation(
                    9,
                    MirBodyId(9),
                    PlaceId(9),
                    PlaceId(9),
                    "op:first",
                )],
                unsupported: vec![test_unsupported("unsupported:first")],
            };
            let second = MirOutput {
                bodies: vec![test_mir_body(4, file, "body:second")],
                places: vec![test_place(4, file, "place:second")],
                operations: vec![test_mir_operation(
                    4,
                    MirBodyId(4),
                    PlaceId(4),
                    PlaceId(4),
                    "op:second",
                )],
                unsupported: Vec::new(),
            };

            db.replace_semantic_mir(first).expect("first MIR replace");
            db.replace_semantic_mir(second).expect("second MIR replace");

            assert_eq!(
                db.mir_bodies()
                    .iter()
                    .map(|body| (body.id, body.stable_key.as_str()))
                    .collect::<Vec<_>>(),
                vec![(MirBodyId(0), "body:second")]
            );
            assert_eq!(db.mir_operations()[0].stable_key, "op:second");
            assert_eq!(db.mir_places()[0].stable_key, "place:second");
            assert!(db.unsupported_semantics().is_empty());
        }

        #[test]
        fn replace_semantic_mir_reassigns_ids_by_stable_key_order() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let output = MirOutput {
                bodies: vec![
                    test_mir_body(20, file, "body:z"),
                    test_mir_body(10, file, "body:a"),
                ],
                places: vec![
                    test_place(20, file, "place:z"),
                    test_place(10, file, "place:a"),
                ],
                operations: vec![
                    test_mir_operation(20, MirBodyId(20), PlaceId(20), PlaceId(10), "op:z"),
                    test_mir_operation(10, MirBodyId(10), PlaceId(10), PlaceId(20), "op:a"),
                ],
                unsupported: vec![
                    test_unsupported("unsupported:z"),
                    test_unsupported("unsupported:a"),
                ],
            };

            db.replace_semantic_mir(output)
                .expect("semantic MIR replace");

            assert_eq!(
                db.mir_bodies()
                    .iter()
                    .map(|body| (body.id, body.stable_key.as_str()))
                    .collect::<Vec<_>>(),
                vec![(MirBodyId(0), "body:a"), (MirBodyId(1), "body:z")]
            );
            assert_eq!(
                db.mir_operations()
                    .iter()
                    .map(|operation| (operation.id, operation.body, operation.stable_key.as_str()))
                    .collect::<Vec<_>>(),
                vec![
                    (MirOpId(0), MirBodyId(0), "op:a"),
                    (MirOpId(1), MirBodyId(1), "op:z"),
                ]
            );
            assert_eq!(
                db.mir_places()
                    .iter()
                    .map(|place| (place.id, place.stable_key.as_str()))
                    .collect::<Vec<_>>(),
                vec![(PlaceId(0), "place:a"), (PlaceId(1), "place:z")]
            );
            assert_eq!(
                db.unsupported_semantics()
                    .iter()
                    .map(|row| (row.id, row.stable_key.as_str()))
                    .collect::<Vec<_>>(),
                vec![
                    (UnsupportedId(0), "unsupported:a"),
                    (UnsupportedId(1), "unsupported:z"),
                ]
            );
            let store = db.semantic_store().expect("semantic store exists");
            assert_eq!(
                store
                    .mir_body(MirBodyId(1))
                    .map(|body| body.stable_key.as_str()),
                Some("body:z")
            );
            assert_eq!(
                store
                    .mir_operation(MirOpId(0))
                    .map(|operation| operation.stable_key.as_str()),
                Some("op:a")
            );
            assert_eq!(
                store
                    .place(PlaceId(0))
                    .map(|place| place.stable_key.as_str()),
                Some("place:a")
            );
            assert_eq!(
                store
                    .unsupported_semantic(UnsupportedId(1))
                    .map(|row| row.stable_key.as_str()),
                Some("unsupported:z")
            );
        }

        #[test]
        fn replace_semantic_mir_rejects_dangling_operation_references() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let output = MirOutput {
                bodies: vec![test_mir_body(0, file, "body:a")],
                places: vec![test_place(0, file, "place:a")],
                operations: vec![test_mir_operation(
                    0,
                    MirBodyId(99),
                    PlaceId(0),
                    PlaceId(0),
                    "op:dangling",
                )],
                unsupported: Vec::new(),
            };

            let error = db
                .replace_semantic_mir(output)
                .expect_err("dangling MIR body reference should fail");

            assert!(error.to_string().contains("dangling MIR operation body"));
        }
    }

    mod semantic_mir_metadata {
        use super::*;

        fn replace_with_semantic_rows(db: &mut AnalysisDb, file: FileId) {
            db.replace_semantic_mir(MirOutput {
                bodies: vec![test_mir_body(2, file, "body:metadata")],
                places: vec![test_place(2, file, "place:metadata")],
                operations: vec![test_mir_operation(
                    2,
                    MirBodyId(2),
                    PlaceId(2),
                    PlaceId(2),
                    "op:metadata",
                )],
                unsupported: vec![test_unsupported("unsupported:metadata")],
            })
            .expect("semantic MIR replace");
        }

        #[test]
        fn replace_semantic_mir_records_metadata_for_every_stored_row() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );

            replace_with_semantic_rows(&mut db, file);

            for family in [
                FactFamily::MirBody,
                FactFamily::MirOperation,
                FactFamily::Place,
                FactFamily::UnsupportedSemantic,
            ] {
                let metadata = db
                    .metadata_for(FactRef::new(family, 0))
                    .expect("semantic MIR metadata exists");
                assert_eq!(metadata.producer_id, "polint.semantic_mir");
                assert_eq!(metadata.layer_id, "polint.semantic_mir");
                assert_eq!(metadata.validation, ValidationStatus::NativeTrusted);
                assert_ne!(metadata.precision, FactPrecision::Exact);
            }
        }

        #[test]
        fn semantic_mir_missing_metadata_reports_rows_when_refresh_is_bypassed() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );

            replace_with_semantic_rows(&mut db, file);
            for family in [
                FactFamily::MirBody,
                FactFamily::MirOperation,
                FactFamily::Place,
                FactFamily::UnsupportedSemantic,
            ] {
                db.remove_fact_metadata_for_test(FactRef::new(family, 0));
            }

            assert_eq!(
                db.missing_fact_metadata(),
                vec![
                    MissingFactMeta {
                        family: FactFamily::MirBody,
                        run_id: 0,
                    },
                    MissingFactMeta {
                        family: FactFamily::MirOperation,
                        run_id: 0,
                    },
                    MissingFactMeta {
                        family: FactFamily::Place,
                        run_id: 0,
                    },
                    MissingFactMeta {
                        family: FactFamily::UnsupportedSemantic,
                        run_id: 0,
                    },
                ]
            );
        }

        #[test]
        fn semantic_mir_metadata_maps_unknown_and_unsupported_to_low_precision() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let mut body = test_mir_body(1, file, "body:unknown");
            body.status = MirStatus::Unknown;
            let mut place = test_place(1, file, "place:partial");
            place.status = PlaceStatus::Partial;

            db.replace_semantic_mir(MirOutput {
                bodies: vec![body],
                places: vec![place],
                operations: Vec::new(),
                unsupported: vec![test_unsupported("unsupported:metadata")],
            })
            .expect("semantic MIR replace");

            assert_eq!(
                db.metadata_for(FactRef::new(FactFamily::MirBody, 0))
                    .map(|metadata| (metadata.precision, metadata.confidence)),
                Some((FactPrecision::Unresolved, FactConfidence::Low))
            );
            assert_eq!(
                db.metadata_for(FactRef::new(FactFamily::Place, 0))
                    .map(|metadata| (metadata.precision, metadata.confidence)),
                Some((FactPrecision::Heuristic, FactConfidence::Medium))
            );
            assert_eq!(
                db.metadata_for(FactRef::new(FactFamily::UnsupportedSemantic, 0))
                    .map(|metadata| (metadata.precision, metadata.confidence)),
                Some((FactPrecision::Unsupported, FactConfidence::Low))
            );
        }
    }

    mod semantic_mir_storage_public_boundary {
        use std::fs;
        use std::path::Path;

        const FORBIDDEN_PUBLIC_TOKENS: &[&str] = &[
            "MirBody",
            "MirOperation",
            "PlaceFact",
            "SemanticStore",
            "UnsupportedSemanticFact",
            "polint.semantic_mir",
            "semantic-mir-facts",
        ];

        fn assert_no_forbidden_tokens(label: &str, source: &str) {
            for token in FORBIDDEN_PUBLIC_TOKENS {
                assert!(
                    !source.contains(token),
                    "{label} leaked private semantic MIR token `{token}`"
                );
            }
        }

        #[test]
        fn sdk_runner_and_bench_sources_do_not_leak_semantic_mir_storage() {
            let sources = [
                ("sdk/mod.rs", include_str!("../sdk/mod.rs")),
                ("sdk/facts.rs", include_str!("../sdk/facts.rs")),
                ("runner/mod.rs", include_str!("../runner/mod.rs")),
                ("lib.rs", include_str!("../lib.rs")),
            ];

            for (label, source) in sources {
                assert_no_forbidden_tokens(label, source);
            }
        }

        #[test]
        fn crate_root_keeps_analysis_module_crate_private_and_out_of_bench() {
            let lib = include_str!("../lib.rs");
            assert!(lib.contains("pub(crate) mod analysis;"));

            let bench_surface = lib.split("pub mod _bench").nth(1).unwrap_or_default();
            assert!(!bench_surface.contains("pub mod analysis"));
            assert!(!bench_surface.contains("pub use crate::analysis"));
            assert_no_forbidden_tokens("_bench", bench_surface);
        }

        #[test]
        fn docs_and_readme_do_not_advertise_private_semantic_mir_facts() {
            let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
            let repo_root = manifest_dir
                .parent()
                .and_then(Path::parent)
                .expect("workspace root");
            let docs_root = repo_root.join("docs/facts");
            let mut sources = vec![(
                "README.md".to_string(),
                fs::read_to_string(repo_root.join("README.md")).expect("README.md"),
            )];

            for entry in fs::read_dir(&docs_root).expect("docs/facts exists") {
                let entry = entry.expect("docs/facts entry");
                if entry.file_type().expect("docs/facts file type").is_file() {
                    sources.push((
                        entry.path().display().to_string(),
                        fs::read_to_string(entry.path()).expect("docs/facts source"),
                    ));
                }
            }

            for (label, source) in sources {
                assert_no_forbidden_tokens(&label, &source);
            }
        }
    }

    fn topology_output(prefix: &str) -> crate::module_graph::topology::TopologyOutput {
        use crate::module_graph::topology::{
            DependencyRequirementFact, DependencyRequirementId, ImportContextKind,
            ImportToPackageFact, ImportToPackageId, ImportToPackageStatus, RepoTopologyOverlayFact,
            RepoTopologyOverlayId, RepoTopologyOverlayKind, ResolvedDependencyEdgeFact,
            ResolvedDependencyEdgeId, ResolvedDependencyKind, SourceSetFact, SourceSetId,
            SourceSetKind, TopologyOutput, TopologyPackageFact, TopologyPackageId,
            TopologyPackageKind, TopologyPrecision, TopologyStatus, WorkspaceRootFact,
            WorkspaceRootId, WorkspaceRootKind,
        };

        TopologyOutput {
            workspace_roots: vec![WorkspaceRootFact {
                id: WorkspaceRootId(99),
                kind: WorkspaceRootKind::Repository,
                root_path: ".".to_string(),
                manifest_path: None,
                language: None,
                stable_key: format!("{prefix}:root"),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            packages: vec![TopologyPackageFact {
                id: TopologyPackageId(99),
                workspace_root: Some(WorkspaceRootId(99)),
                package: None,
                module_node: None,
                kind: TopologyPackageKind::Workspace,
                name: format!("{prefix}-package"),
                version: None,
                path: ".".to_string(),
                language: Some(Language::TypeScript),
                stable_key: format!("{prefix}:package"),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            source_sets: vec![SourceSetFact {
                id: SourceSetId(99),
                package: Some(TopologyPackageId(99)),
                root: Some(WorkspaceRootId(99)),
                kind: SourceSetKind::Source,
                path: "src".to_string(),
                language: Some(Language::TypeScript),
                files: vec![FileId(0)],
                stable_key: format!("{prefix}:source-set"),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            dependency_requirements: vec![DependencyRequirementFact {
                id: DependencyRequirementId(99),
                from_package: Some(TopologyPackageId(99)),
                target_package: None,
                target_name: "react".to_string(),
                version_requirement: Some("^18".to_string()),
                kind: crate::module_graph::topology::RequirementKind::Runtime,
                manifest_path: Some("package.json".to_string()),
                stable_key: format!("{prefix}:requirement"),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            resolved_dependency_edges: vec![ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(99),
                requirement: Some(DependencyRequirementId(99)),
                from_package: Some(TopologyPackageId(99)),
                to_package: None,
                package_name: "react".to_string(),
                resolved_version: Some("18.2.0".to_string()),
                kind: ResolvedDependencyKind::Lockfile,
                stable_key: format!("{prefix}:resolved"),
                producer_id: "test",
                precision: TopologyPrecision::ExactLockfile,
                status: TopologyStatus::Resolved,
            }],
            import_to_package_edges: vec![ImportToPackageFact {
                id: ImportToPackageId(99),
                syntax_import: None,
                resolved_import: None,
                semantic_import_stable_key: None,
                from_file: Some(FileId(0)),
                from_package: Some(TopologyPackageId(99)),
                to_package: None,
                target_node: None,
                from_package_stable_key: Some(format!("{prefix}:package")),
                to_package_stable_key: None,
                source_set_stable_key: Some(format!("{prefix}:source-set")),
                import_path: "react".to_string(),
                context: ImportContextKind::Source,
                stable_key: format!("{prefix}:import-to-package"),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: ImportToPackageStatus::Resolved,
            }],
            overlays: vec![RepoTopologyOverlayFact {
                id: RepoTopologyOverlayId(99),
                root: Some(WorkspaceRootId(99)),
                package: Some(TopologyPackageId(99)),
                source_set: Some(SourceSetId(99)),
                kind: RepoTopologyOverlayKind::OwnershipZone,
                label: "team-platform".to_string(),
                path: Some("src".to_string()),
                stable_key: format!("{prefix}:overlay"),
                producer_id: "test",
                precision: TopologyPrecision::Heuristic,
                status: TopologyStatus::Present,
            }],
        }
    }

    #[test]
    fn topology_storage_replaces_rows_and_rebuilds_metadata() {
        let mut db = AnalysisDb::new();
        db.replace_topology_facts(topology_output("first"));
        db.replace_topology_facts(topology_output("second"));

        assert_eq!(db.workspace_roots().len(), 1);
        assert_eq!(db.workspace_roots()[0].id.0, 0);
        assert_eq!(db.workspace_roots()[0].stable_key, "second:root");
        assert_eq!(db.topology_packages()[0].id.0, 0);
        assert_eq!(db.source_sets()[0].id.0, 0);
        assert_eq!(db.dependency_requirements()[0].id.0, 0);
        assert_eq!(db.resolved_dependency_edges()[0].id.0, 0);
        assert_eq!(db.import_to_package_edges()[0].id.0, 0);
        assert_eq!(db.repo_topology_overlays()[0].id.0, 0);
        assert!(
            db.metadata_for(FactRef::new(FactFamily::WorkspaceRoot, 1))
                .is_none()
        );
        assert!(
            db.metadata_for(FactRef::new(FactFamily::WorkspaceRoot, 0))
                .is_some()
        );
    }

    #[test]
    fn topology_storage_replaces_import_to_package_edges_only() {
        let mut db = AnalysisDb::new();
        db.replace_topology_facts(topology_output("base"));
        let mut edges = topology_output("updated").import_to_package_edges;
        edges[0].id = crate::module_graph::topology::ImportToPackageId(42);

        db.replace_import_to_package_facts(edges);

        assert_eq!(db.workspace_roots()[0].stable_key, "base:root");
        assert_eq!(db.topology_packages()[0].stable_key, "base:package");
        assert_eq!(db.source_sets()[0].stable_key, "base:source-set");
        assert_eq!(
            db.dependency_requirements()[0].stable_key,
            "base:requirement"
        );
        assert_eq!(
            db.resolved_dependency_edges()[0].stable_key,
            "base:resolved"
        );
        assert_eq!(db.repo_topology_overlays()[0].stable_key, "base:overlay");
        assert_eq!(
            db.import_to_package_edges()[0].stable_key,
            "updated:import-to-package"
        );
        assert_eq!(db.import_to_package_edges()[0].id.0, 0);
    }

    #[test]
    fn topology_storage_records_provider_metadata_for_every_row() {
        let mut db = AnalysisDb::new();
        db.replace_topology_facts(topology_output("meta"));

        for family in [
            FactFamily::WorkspaceRoot,
            FactFamily::TopologyPackage,
            FactFamily::SourceSet,
            FactFamily::DependencyRequirement,
            FactFamily::ResolvedDependencyEdge,
            FactFamily::RepoTopologyOverlay,
        ] {
            let metadata = db
                .metadata_for(FactRef::new(family, 0))
                .expect("topology metadata exists");
            assert_eq!(metadata.producer_id, MODULE_GRAPH_PROVIDER_ID);
        }

        let metadata = db
            .metadata_for(FactRef::new(FactFamily::ImportToPackage, 0))
            .expect("import-to-package metadata exists");
        assert_eq!(metadata.producer_id, MODULE_TOPOLOGY_PROVIDER_ID);
    }

    #[test]
    fn source_file_metadata_records_provider_and_stable_key_inputs() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src\\main.go"),
            "src\\main.go".to_string(),
            "package main\n".to_string(),
        );

        let metadata = db
            .metadata_for(FactRef::new(FactFamily::SourceFile, u64::from(file.0)))
            .expect("source metadata should be recorded");

        assert_eq!(metadata.producer_id, "polint.source");
        assert_eq!(metadata.layer_id, "polint.source");
        assert_eq!(metadata.precision, FactPrecision::Exact);
        assert_eq!(metadata.confidence, FactConfidence::High);
        assert_eq!(metadata.validation, ValidationStatus::NativeTrusted);
        assert!(metadata.stable_key.contains("4:path=11:src/main.go"));
        assert!(metadata.stable_key.contains("12:content_hash="));
        assert!(
            db.fact_meta_mut_for_test()
                .get(FactRef::new(FactFamily::SourceFile, u64::from(file.0)))
                .is_some()
        );
    }

    #[test]
    fn syntax_metadata_uses_language_specific_producers() {
        let mut db = AnalysisDb::new();
        let go_file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nimport \"fmt\"\n".to_string(),
        );
        let ts_file = db.add_file(
            PathBuf::from("src/component.tsx"),
            "src/component.tsx".to_string(),
            "export class Button {}".to_string(),
        );
        let go_span = test_span(go_file, 1);
        let ts_span = test_span(ts_file, 1);

        let import = db.push_import(ImportFact {
            id: ImportId(999),
            file: go_file,
            package: None,
            path: "fmt".to_string(),
            span: go_span,
            language: Language::Go,
        });
        db.push_ts_class(TsClassFact {
            file: ts_file,
            name: "Button".to_string(),
            span: ts_span,
            is_exported: true,
            is_component_like: true,
        });

        let import_meta = db
            .metadata_for(FactRef::new(FactFamily::Import, import.0))
            .expect("import metadata should be recorded");
        let class_meta = db
            .metadata_for(FactRef::new(FactFamily::TsClass, 0))
            .expect("TS class metadata should be recorded");

        assert_eq!(import_meta.producer_id, "polint.go.syntax");
        assert_eq!(import_meta.precision, FactPrecision::Syntax);
        assert_eq!(class_meta.producer_id, "polint.ts.syntax");
        assert_eq!(class_meta.precision, FactPrecision::Syntax);
    }

    #[test]
    fn restore_file_facts_recreates_metadata_for_cached_syntax_facts() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/component.tsx"),
            "src/component.tsx".to_string(),
            "export function Button() { return <button aria-label=\"Save\" /> }\n".to_string(),
        );
        let span = test_span(file, 1);

        db.restore_file_facts(
            file,
            CachedFileFacts {
                packages: vec![PackageFact {
                    id: PackageId(99),
                    file,
                    name: "main".to_string(),
                    span: span.clone(),
                    language: Language::Go,
                }],
                functions: vec![FunctionFact {
                    id: FunctionId(99),
                    file,
                    name: "Button".to_string(),
                    span: span.clone(),
                    language: Language::Tsx,
                    is_test: false,
                    is_exported: true,
                    cyclomatic_complexity: 1,
                    calls: vec!["render".to_string()],
                }],
                imports: vec![ImportFact {
                    id: ImportId(99),
                    file,
                    package: Some("react".to_string()),
                    path: "react".to_string(),
                    span: span.clone(),
                    language: Language::Tsx,
                }],
                branches: vec![BranchObligation {
                    id: BranchId(99),
                    function: Some(FunctionId(99)),
                    file,
                    decision_span: span.clone(),
                    condition_text: "enabled".to_string(),
                    edge_label: "true".to_string(),
                    is_error_path: false,
                    stable_fingerprint: "branch".to_string(),
                }],
                tests: vec![TestFact {
                    file,
                    function: Some(FunctionId(99)),
                    name: "TestButton".to_string(),
                    span: span.clone(),
                    evidence_terms: vec!["render".to_string()],
                    assertion_count: 1,
                    subtest_count: 0,
                    subtest_names: Vec::new(),
                    table_rows: 0,
                }],
                coverage: vec![CoverageFact {
                    branch: BranchId(99),
                    covered: Some(true),
                    source: "synthetic".to_string(),
                }],
                ts_components: vec![TsComponentFact {
                    file,
                    function: Some(FunctionId(99)),
                    name: "Button".to_string(),
                    span: span.clone(),
                }],
                ts_classes: vec![TsClassFact {
                    file,
                    name: "Dialog".to_string(),
                    span: span.clone(),
                    is_exported: true,
                    is_component_like: false,
                }],
                string_literals: vec![StringLiteralFact {
                    file,
                    value: "Save".to_string(),
                    span: span.clone(),
                    language: Language::Tsx,
                }],
                jsx_attributes: vec![JsxAttributeFact {
                    file,
                    name: "aria-label".to_string(),
                    value: Some("Save".to_string()),
                    span,
                }],
            },
        );

        for family in [
            FactFamily::Package,
            FactFamily::Function,
            FactFamily::Import,
            FactFamily::BranchObligation,
            FactFamily::Test,
            FactFamily::Coverage,
            FactFamily::TsComponent,
            FactFamily::TsClass,
            FactFamily::StringLiteral,
            FactFamily::JsxAttribute,
        ] {
            assert!(
                db.metadata_for(FactRef::new(family, 0)).is_some(),
                "missing restored metadata for {family:?}"
            );
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
    fn semantic_index_storage_replaces_rows_and_rebuilds_metadata() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function handler() {}\n".to_string(),
        );
        let stale = test_scope("stale", file, SemanticStatus::Resolved);
        let beta = test_scope("bravo", file, SemanticStatus::Resolved);
        let alpha = test_scope("alpha", file, SemanticStatus::SetupMissing);

        db.replace_semantic_index_facts(
            vec![stale],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        db.replace_semantic_index_facts(
            vec![beta, alpha],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(
            db.scopes()
                .iter()
                .map(|scope| (scope.id.0, scope.scope_path.as_slice()))
                .collect::<Vec<_>>(),
            vec![
                (0, &["alpha".to_string()][..]),
                (1, &["bravo".to_string()][..]),
            ]
        );
        assert!(
            db.metadata_for(FactRef::new(FactFamily::Scope, 0))
                .is_some()
        );
        assert!(
            db.metadata_for(FactRef::new(FactFamily::Scope, 2))
                .is_none()
        );
    }

    #[test]
    fn semantic_index_storage_reports_missing_metadata_when_refresh_is_bypassed() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function handler() {}\n".to_string(),
        );

        db.replace_semantic_index_facts(
            vec![test_scope("root", file, SemanticStatus::Resolved)],
            Vec::<SemanticImportFact>::new(),
            Vec::<ExportFact>::new(),
            Vec::<AliasFact>::new(),
            Vec::<ResolutionFact>::new(),
            Vec::<GeneratedSymbolFact>::new(),
            Vec::<StableExportIdentity>::new(),
        );
        db.remove_fact_metadata_for_test(FactRef::new(FactFamily::Scope, 0));

        assert_eq!(
            db.missing_fact_metadata(),
            vec![MissingFactMeta {
                family: FactFamily::Scope,
                run_id: 0,
            }]
        );
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
    fn module_relationship_core_contract_remaps_relationship_ids_when_normalizing_inputs() {
        let mut db = AnalysisDb::new();
        let from_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import { Button } from './button';\n".to_string(),
        );
        let target_file = db.add_file(
            PathBuf::from("src/button.ts"),
            "src/button.ts".to_string(),
            "export function Button() {}\n".to_string(),
        );
        let import = db.push_import(ImportFact {
            id: ImportId(99),
            file: from_file,
            package: None,
            path: "./button".to_string(),
            span: test_span(from_file, 1),
            language: Language::TypeScript,
        });

        db.replace_module_graph_facts(
            vec![ResolvedImportFact {
                id: ResolvedImportId(40),
                import,
                from_file,
                target_node: Some(ModuleNodeId(42)),
                status: ResolutionStatus::Resolved,
                precision: ResolutionPrecision::ExactFile,
                reason: None,
            }],
            vec![
                ModuleNode {
                    id: ModuleNodeId(41),
                    kind: ModuleNodeKind::File,
                    label: "src/app.ts".to_string(),
                    file: Some(from_file),
                    package: None,
                    language: Some(Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(42),
                    kind: ModuleNodeKind::File,
                    label: "src/button.ts".to_string(),
                    file: Some(target_file),
                    package: None,
                    language: Some(Language::TypeScript),
                },
            ],
            vec![ModuleEdge {
                id: ModuleEdgeId(43),
                from: ModuleNodeId(41),
                to: ModuleNodeId(42),
                import: Some(import),
                resolved_import: Some(ResolvedImportId(40)),
                kind: ModuleEdgeKind::Imports,
                status: ResolutionStatus::Resolved,
            }],
        );

        assert_eq!(db.resolved_imports()[0].target_node, Some(ModuleNodeId(1)));
        assert_eq!(db.module_edges()[0].from, ModuleNodeId(0));
        assert_eq!(db.module_edges()[0].to, ModuleNodeId(1));
        assert_eq!(
            db.module_edges()[0].resolved_import,
            Some(ResolvedImportId(0))
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
