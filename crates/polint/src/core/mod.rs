use crate::analysis::access_paths::facts::AccessPathFact;
use crate::analysis::access_paths::store::AccessPathStore;
use crate::analysis::adaptation::facts::{AcceptedModelFact, RejectedModelFact};
use crate::analysis::aliases::facts::{AliasAnswerFact, AliasPrecision, AliasStatus};
use crate::analysis::aliases::store::AliasStore;
use crate::analysis::calls::facts::{
    CallAlgorithm, CallEdgeKind, CallPrecision, CallSiteFact, CallSyntaxKind, CallTargetFact,
    CallTargetStatus, UnresolvedCallFact, UnresolvedCallReason,
};
use crate::analysis::calls::store::{CallOutput, CallStore};
use crate::analysis::cfg::facts::{
    BasicBlockFact, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, CfgPrecision, CfgStatus,
    ControlDependenceFact, DominatorFact, PostDominatorFact, ReachabilityFact,
    UnsupportedControlFlowFact,
};
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis::data_flow::facts::{
    DataFlowBudgetFact, DataFlowConfidence, DataFlowEdgeFact, DataFlowModelFact, DataFlowNodeFact,
    DataFlowPrecision, DataFlowStatus, DataFlowValidation,
};
use crate::analysis::data_flow::provider::DATA_FLOW_PROVIDER_ID;
use crate::analysis::data_flow::store::{DataFlowOutput, DataFlowStore};
use crate::analysis::domains::facts::{
    DomainEventFact, DomainObservationFact, DomainPrecision, DomainStatus,
};
use crate::analysis::domains::store::{DomainOutput, DomainStore};
use crate::analysis::entrypoints::facts::{
    EntrypointFact, EntrypointPrecision, EntrypointStatus, FrameworkDispatchEdgeFact,
    TrustBoundaryFact, UnresolvedFrameworkFact,
};
use crate::analysis::entrypoints::store::{EntrypointOutput, EntrypointStore};
use crate::analysis::error::AnalysisError;
use crate::analysis::evidence::facts::{
    EvidenceBundleFact, EvidenceConfidence, EvidenceEdgeFact, EvidenceNodeFact,
    EvidenceOmittedRegionFact, EvidencePathFact, EvidencePrecision, EvidenceProvenance,
    EvidenceReplayKeyFact, EvidenceSliceFact, EvidenceStatus, EvidenceUnknownFact,
    EvidenceValidation,
};
use crate::analysis::evidence::provider::EVIDENCE_PROVIDER_ID;
use crate::analysis::evidence::store::{EvidenceOutput, EvidenceStore};
use crate::analysis::extensions::sinks::{ExtensionFactConfidence, ExtensionFactPrecision};
use crate::analysis::extensions::store::{
    AcceptedExtensionFact, ExtensionActivationRow, ExtensionOutput, RejectedExtensionFact,
};
use crate::analysis::identity::facts::IdentityRecord;
use crate::analysis::identity::provider::valid_call_site_ids;
use crate::analysis::identity::store::{IdentityProviderOutput, IdentityStore};
use crate::analysis::ids::CallSiteId;
use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
use crate::analysis::mir::op::{MirOperation, UnsupportedSemanticFact};
use crate::analysis::places::{PlaceFact, PlaceStatus};
use crate::analysis::points_to::facts::{
    PointsToConstraintFact, PointsToPrecision, PointsToSetFact, PointsToStatus,
};
use crate::analysis::points_to::store::PointsToStore;
use crate::analysis::reachability::facts::{CallReachabilityFact, ReachabilityRootFact};
use crate::analysis::reachability::store::{ReachabilityProviderOutput, ReachabilityStore};
use crate::analysis::refined_calls::facts::{
    RefinedCallConfidence, RefinedCallEdgeFact, RefinedCallTier, RefinedCallValidation,
};
use crate::analysis::refined_calls::provider::REFINED_CALLS_PROVIDER_ID;
use crate::analysis::refined_calls::store::{RefinedCallOutput, RefinedCallStore};
use crate::analysis::semantic_graph::constraints::ConstraintFact;
use crate::analysis::semantic_graph::facts::{SemanticEdgeFact, SemanticNodeFact};
use crate::analysis::semantic_graph::store::{SemanticGraphOutput, SemanticGraphStore};
use crate::analysis::solver::budget::BudgetStatus;
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis::solver::store::{SolverOutput, SolverStore};
use crate::analysis::store::SemanticStore;
use crate::analysis::summaries::facts::{
    SummaryDomainKind, SummaryEventFact, SummaryFact, SummaryPrecision, SummaryStatus,
};
use crate::analysis::summaries::store::{SummaryOutput, SummaryStore};
use crate::analysis::types::facts::{
    NarrowedTypeFact, TypeConfidence, TypeFact, TypePrecision, TypeStatus,
};
use crate::analysis::types::provider::TYPE_VALUE_ALIAS_PROVIDER_ID;
use crate::analysis::types::store::{TypeStore, TypeValueAliasOutput};
use crate::analysis::values::facts::{AllocationTokenFact, ValueFact, ValuePrecision, ValueStatus};
use crate::analysis::values::store::ValueStore;
use crate::analysis_kernel::{
    FactConfidence, FactFamily, FactMeta, FactMetaStore, FactPrecision, FactRef, MissingFactMeta,
    ValidationStatus, resolution_metadata, resolution_status_metadata, stable_key_from_parts,
    symbol_metadata,
};
use crate::diagnostics::{
    Diagnostic, Severity, TextRange as DiagnosticRange, dedupe_diagnostics, fingerprint,
};
use crate::go::semantic::facts::{
    GoSemanticAddressTakenFact, GoSemanticCallsiteFact, GoSemanticDynamicDispatchFact,
    GoSemanticFunctionFact, GoSemanticInstantiatedTypeFact, GoSemanticMethodSetFact,
    GoSemanticPackageErrorFact, GoSemanticPackageFact, GoSemanticRtaEdgeFact,
};
use crate::go::semantic::store::{GoSemanticFactsOutput, GoSemanticStore, GoSemanticStoreReport};
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
use crate::ts::object_model::facts::{
    TsObjectAllocationFact, TsPropertyReadFact, TsPropertyWriteFact, TsPrototypeLinkFact,
    TsReceiverBindingFact,
};
use crate::ts::object_model::store::{TsObjectModelOutput, TsObjectModelStore};
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
pub(crate) const CFG_PROVIDER_ID: &str = "polint.cfg";
pub(crate) const CALLS_PROVIDER_ID: &str = "polint.calls";
pub(crate) const POLINT_ABSTRACT_DOMAINS_PROVIDER_ID: &str = "polint.abstract_domains";
pub(crate) const POLINT_DIRECT_SUMMARIES_PROVIDER_ID: &str = "polint.direct_summaries";
pub(crate) const ENTRYPOINTS_PROVIDER_ID: &str = "polint.entrypoints";
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
            "ts" | "mts" | "cts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "js" | "mjs" | "cjs" => Self::JavaScript,
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

pub(crate) const TS_JS_MODULE_FUNCTION_NAME: &str = "<polint:module>";

pub(crate) fn is_synthetic_ts_js_module_function(function: &FunctionFact) -> bool {
    function.language.is_ts_family() && function.name == TS_JS_MODULE_FUNCTION_NAME
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

#[derive(Debug, Clone)]
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
    cfg_functions: Vec<CfgFunctionFact>,
    cfg_nodes: Vec<CfgNodeFact>,
    cfg_blocks: Vec<BasicBlockFact>,
    cfg_edges: Vec<CfgEdgeFact>,
    cfg_reachability: Vec<ReachabilityFact>,
    cfg_dominators: Vec<DominatorFact>,
    cfg_postdominators: Vec<PostDominatorFact>,
    cfg_control_dependence: Vec<ControlDependenceFact>,
    unsupported_control_flow: Vec<UnsupportedControlFlowFact>,
    call_sites: Vec<CallSiteFact>,
    call_targets: Vec<CallTargetFact>,
    unresolved_calls: Vec<UnresolvedCallFact>,
    call_store: Option<CallStore>,
    identity_records: Vec<IdentityRecord>,
    identity_store: Option<IdentityStore>,
    refined_call_edges: Vec<RefinedCallEdgeFact>,
    refined_call_store: Option<RefinedCallStore>,
    data_flow_nodes: Vec<DataFlowNodeFact>,
    data_flow_edges: Vec<DataFlowEdgeFact>,
    data_flow_models: Vec<DataFlowModelFact>,
    data_flow_budgets: Vec<DataFlowBudgetFact>,
    data_flow_store: Option<DataFlowStore>,
    evidence_nodes: Vec<EvidenceNodeFact>,
    evidence_edges: Vec<EvidenceEdgeFact>,
    evidence_bundles: Vec<EvidenceBundleFact>,
    evidence_paths: Vec<EvidencePathFact>,
    evidence_slices: Vec<EvidenceSliceFact>,
    evidence_unknowns: Vec<EvidenceUnknownFact>,
    evidence_omitted_regions: Vec<EvidenceOmittedRegionFact>,
    evidence_replay_keys: Vec<EvidenceReplayKeyFact>,
    evidence_store: Option<EvidenceStore>,
    abstract_domain_observations: Vec<DomainObservationFact>,
    abstract_domain_events: Vec<DomainEventFact>,
    abstract_domain_store: Option<DomainStore>,
    summary_facts: Vec<SummaryFact>,
    summary_events: Vec<SummaryEventFact>,
    summary_store: Option<SummaryStore>,
    extension_activations: Vec<ExtensionActivationRow>,
    extension_facts: Vec<AcceptedExtensionFact>,
    #[allow(
        dead_code,
        reason = "Rejected extension audit rows are surfaced by the extension provider/debug wiring in the next Phase 34 plan."
    )]
    rejected_extension_facts: Vec<RejectedExtensionFact>,
    adaptation_model_facts: Vec<AcceptedModelFact>,
    rejected_adaptation_model_facts: Vec<RejectedModelFact>,
    entrypoint_facts: Vec<EntrypointFact>,
    trust_boundary_facts: Vec<TrustBoundaryFact>,
    dispatch_edge_facts: Vec<FrameworkDispatchEdgeFact>,
    unresolved_framework_facts: Vec<UnresolvedFrameworkFact>,
    entrypoint_store: Option<EntrypointStore>,
    reachability_roots: Vec<ReachabilityRootFact>,
    reachability_marks: Vec<CallReachabilityFact>,
    semantic_nodes: Vec<SemanticNodeFact>,
    semantic_edges: Vec<SemanticEdgeFact>,
    semantic_constraints: Vec<ConstraintFact>,
    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    ts_object_allocations: Vec<TsObjectAllocationFact>,
    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    ts_property_writes: Vec<TsPropertyWriteFact>,
    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    ts_property_reads: Vec<TsPropertyReadFact>,
    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    ts_receiver_bindings: Vec<TsReceiverBindingFact>,
    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    ts_prototype_links: Vec<TsPrototypeLinkFact>,
    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    ts_object_model_store: Option<TsObjectModelStore>,
    solver_derived_edges: Vec<DerivedEdgeFact>,
    solver_budget_status: BudgetStatus,
    solver_budget_reasons: BTreeSet<String>,
    go_semantic_packages: Vec<GoSemanticPackageFact>,
    go_semantic_functions: Vec<GoSemanticFunctionFact>,
    go_semantic_callsites: Vec<GoSemanticCallsiteFact>,
    go_semantic_method_sets: Vec<GoSemanticMethodSetFact>,
    go_semantic_address_taken: Vec<GoSemanticAddressTakenFact>,
    go_semantic_instantiated_types: Vec<GoSemanticInstantiatedTypeFact>,
    go_semantic_dynamic_dispatch: Vec<GoSemanticDynamicDispatchFact>,
    go_semantic_rta_edges: Vec<GoSemanticRtaEdgeFact>,
    go_semantic_package_errors: Vec<GoSemanticPackageErrorFact>,
    type_facts: Vec<TypeFact>,
    narrowed_type_facts: Vec<NarrowedTypeFact>,
    value_facts: Vec<ValueFact>,
    allocation_tokens: Vec<AllocationTokenFact>,
    access_path_facts: Vec<AccessPathFact>,
    points_to_constraints: Vec<PointsToConstraintFact>,
    points_to_sets: Vec<PointsToSetFact>,
    alias_answers: Vec<AliasAnswerFact>,
    type_store: Option<TypeStore>,
    value_store: Option<ValueStore>,
    access_path_store: Option<AccessPathStore>,
    points_to_store: Option<PointsToStore>,
    alias_store: Option<AliasStore>,
    path_contexts: Option<crate::path_context::PathContextIndex>,
    /// Diff-to-target-ref facts, injected by the host for `polint review`.
    ///
    /// This is the first externally injected fact family: it is set by the
    /// runner via [`AnalysisDb::set_changeset`] after the kernel runs, not
    /// derived by a provider. It is `None` under `polint check` (so the
    /// `ChangedFiles` view is empty there) and excluded from all cache digests.
    changeset: Option<ChangeSetFacts>,
}

impl Default for AnalysisDb {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            fact_meta: FactMetaStore::default(),
            packages: Vec::new(),
            functions: Vec::new(),
            imports: Vec::new(),
            resolved_imports: Vec::new(),
            module_nodes: Vec::new(),
            module_edges: Vec::new(),
            workspace_roots: Vec::new(),
            topology_packages: Vec::new(),
            source_sets: Vec::new(),
            dependency_requirements: Vec::new(),
            resolved_dependency_edges: Vec::new(),
            import_to_package_edges: Vec::new(),
            repo_topology_overlays: Vec::new(),
            scopes: Vec::new(),
            semantic_imports: Vec::new(),
            exports: Vec::new(),
            aliases: Vec::new(),
            resolution_facts: Vec::new(),
            generated_symbols: Vec::new(),
            stable_exports: Vec::new(),
            scopes_by_id: BTreeMap::new(),
            semantic_imports_by_id: BTreeMap::new(),
            exports_by_id: BTreeMap::new(),
            aliases_by_id: BTreeMap::new(),
            resolution_facts_by_id: BTreeMap::new(),
            generated_symbols_by_id: BTreeMap::new(),
            stable_exports_by_id: BTreeMap::new(),
            symbols: Vec::new(),
            definitions: Vec::new(),
            references: Vec::new(),
            symbols_by_id: BTreeMap::new(),
            definitions_by_symbol: BTreeMap::new(),
            references_by_target: BTreeMap::new(),
            symbols_by_file: BTreeMap::new(),
            references_by_file: BTreeMap::new(),
            symbols_by_name: BTreeMap::new(),
            branches: Vec::new(),
            tests: Vec::new(),
            coverage: Vec::new(),
            file_metrics: Vec::new(),
            function_metrics: Vec::new(),
            complexity_metrics: Vec::new(),
            ts_components: Vec::new(),
            ts_classes: Vec::new(),
            string_literals: Vec::new(),
            jsx_attributes: Vec::new(),
            semantic: None,
            cfg_functions: Vec::new(),
            cfg_nodes: Vec::new(),
            cfg_blocks: Vec::new(),
            cfg_edges: Vec::new(),
            cfg_reachability: Vec::new(),
            cfg_dominators: Vec::new(),
            cfg_postdominators: Vec::new(),
            cfg_control_dependence: Vec::new(),
            unsupported_control_flow: Vec::new(),
            call_sites: Vec::new(),
            call_targets: Vec::new(),
            unresolved_calls: Vec::new(),
            call_store: None,
            identity_records: Vec::new(),
            identity_store: None,
            refined_call_edges: Vec::new(),
            refined_call_store: None,
            data_flow_nodes: Vec::new(),
            data_flow_edges: Vec::new(),
            data_flow_models: Vec::new(),
            data_flow_budgets: Vec::new(),
            data_flow_store: None,
            evidence_nodes: Vec::new(),
            evidence_edges: Vec::new(),
            evidence_bundles: Vec::new(),
            evidence_paths: Vec::new(),
            evidence_slices: Vec::new(),
            evidence_unknowns: Vec::new(),
            evidence_omitted_regions: Vec::new(),
            evidence_replay_keys: Vec::new(),
            evidence_store: None,
            abstract_domain_observations: Vec::new(),
            abstract_domain_events: Vec::new(),
            abstract_domain_store: None,
            summary_facts: Vec::new(),
            summary_events: Vec::new(),
            summary_store: None,
            extension_activations: Vec::new(),
            extension_facts: Vec::new(),
            rejected_extension_facts: Vec::new(),
            adaptation_model_facts: Vec::new(),
            rejected_adaptation_model_facts: Vec::new(),
            entrypoint_facts: Vec::new(),
            trust_boundary_facts: Vec::new(),
            dispatch_edge_facts: Vec::new(),
            unresolved_framework_facts: Vec::new(),
            entrypoint_store: None,
            reachability_roots: Vec::new(),
            reachability_marks: Vec::new(),
            semantic_nodes: Vec::new(),
            semantic_edges: Vec::new(),
            semantic_constraints: Vec::new(),
            ts_object_allocations: Vec::new(),
            ts_property_writes: Vec::new(),
            ts_property_reads: Vec::new(),
            ts_receiver_bindings: Vec::new(),
            ts_prototype_links: Vec::new(),
            ts_object_model_store: None,
            solver_derived_edges: Vec::new(),
            solver_budget_status: BudgetStatus::NotRun,
            solver_budget_reasons: BTreeSet::new(),
            go_semantic_packages: Vec::new(),
            go_semantic_functions: Vec::new(),
            go_semantic_callsites: Vec::new(),
            go_semantic_method_sets: Vec::new(),
            go_semantic_address_taken: Vec::new(),
            go_semantic_instantiated_types: Vec::new(),
            go_semantic_dynamic_dispatch: Vec::new(),
            go_semantic_rta_edges: Vec::new(),
            go_semantic_package_errors: Vec::new(),
            type_facts: Vec::new(),
            narrowed_type_facts: Vec::new(),
            value_facts: Vec::new(),
            allocation_tokens: Vec::new(),
            access_path_facts: Vec::new(),
            points_to_constraints: Vec::new(),
            points_to_sets: Vec::new(),
            alias_answers: Vec::new(),
            type_store: None,
            value_store: None,
            access_path_store: None,
            points_to_store: None,
            alias_store: None,
            path_contexts: None,
            changeset: None,
        }
    }
}

impl AnalysisDb {
    pub fn new() -> Self {
        Self::default()
    }

    /// Injects diff-to-target-ref facts for `polint review`.
    ///
    /// Called by the host runner after the kernel runs and before rules
    /// execute, so the `ChangedFiles` fact view can read the diff. The
    /// changeset is excluded from all cache digests by construction (it is set
    /// post-kernel), so a changing diff never busts the analysis cache.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "The host runner wires set_changeset from --changed-files in the polint review command (Task 4)."
        )
    )]
    pub(crate) fn set_changeset(&mut self, changeset: ChangeSetFacts) {
        self.changeset = Some(changeset);
    }

    /// Returns the injected changeset, or `None` under `polint check`.
    pub(crate) fn changeset(&self) -> Option<&ChangeSetFacts> {
        self.changeset.as_ref()
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

    pub(crate) fn replace_cfg_facts(&mut self, output: CfgOutput) -> Result<(), AnalysisError> {
        let output = output.normalized();
        self.cfg_functions = output.functions;
        self.cfg_nodes = output.nodes;
        self.cfg_blocks = output.blocks;
        self.cfg_edges = output.edges;
        self.cfg_reachability = output.reachability;
        self.cfg_dominators = output.dominators;
        self.cfg_postdominators = output.postdominators;
        self.cfg_control_dependence = output.control_dependence;
        self.unsupported_control_flow = output.unsupported;
        self.refresh_cfg_metadata();
        Ok(())
    }

    pub(crate) fn replace_call_facts(
        &mut self,
        mut output: CallOutput,
    ) -> Result<(), AnalysisError> {
        self.populate_call_owner_symbols(&mut output);
        let store = CallStore::from_output(output)?;
        self.call_sites = store.sites().to_vec();
        self.call_targets = store.targets().to_vec();
        self.unresolved_calls = store.unresolved().to_vec();
        self.call_store = Some(store);
        self.refresh_call_metadata();
        Ok(())
    }

    pub(crate) fn replace_identity_facts(
        &mut self,
        output: IdentityProviderOutput,
    ) -> Result<(), AnalysisError> {
        let valid_sites = valid_call_site_ids(self);
        let valid_targets = self
            .call_targets
            .iter()
            .map(|target| target.id)
            .collect::<BTreeSet<_>>();
        let store = IdentityStore::from_output(output, &valid_sites, &valid_targets)?;
        self.identity_records = store.records().to_vec();
        self.identity_store = Some(store);
        Ok(())
    }

    pub(crate) fn identity_records(&self) -> &[IdentityRecord] {
        &self.identity_records
    }

    /// Injects identity records directly, bypassing store-level reference
    /// validation, so validation diagnostics (the defense-in-depth layer) can be
    /// exercised even for records that the store would have rejected.
    #[cfg(test)]
    pub(crate) fn set_identity_records_for_test(&mut self, records: Vec<IdentityRecord>) {
        self.identity_records = records;
        self.identity_store = None;
    }

    #[allow(dead_code)]
    pub(crate) fn identity_store(&self) -> Option<&IdentityStore> {
        self.identity_store.as_ref()
    }

    pub(crate) fn replace_refined_call_facts(
        &mut self,
        output: RefinedCallOutput,
    ) -> Result<(), AnalysisError> {
        let store = RefinedCallStore::from_output(output)?;
        self.refined_call_edges = store.edges().to_vec();
        self.refined_call_store = Some(store);
        self.refresh_refined_call_metadata();
        Ok(())
    }

    pub(crate) fn replace_data_flow_facts(
        &mut self,
        output: DataFlowOutput,
    ) -> Result<(), AnalysisError> {
        let store = DataFlowStore::from_output(output)?;
        self.data_flow_nodes = store.nodes().to_vec();
        self.data_flow_edges = store.edges().to_vec();
        self.data_flow_models = store.models().to_vec();
        self.data_flow_budgets = store.budgets().to_vec();
        self.data_flow_store = Some(store);
        self.refresh_data_flow_metadata();
        Ok(())
    }

    pub(crate) fn replace_evidence_facts(
        &mut self,
        output: EvidenceOutput,
    ) -> Result<(), AnalysisError> {
        let store = EvidenceStore::from_output(output)?;
        self.evidence_nodes = store.nodes().to_vec();
        self.evidence_edges = store.edges().to_vec();
        self.evidence_bundles = store.bundles().to_vec();
        self.evidence_paths = store.paths().to_vec();
        self.evidence_slices = store.slices().to_vec();
        self.evidence_unknowns = store.unknowns().to_vec();
        self.evidence_omitted_regions = store.omitted_regions().to_vec();
        self.evidence_replay_keys = store.replay_keys().to_vec();
        self.evidence_store = Some(store);
        self.refresh_evidence_metadata();
        Ok(())
    }

    pub(crate) fn replace_abstract_domain_facts(&mut self, output: DomainOutput) {
        let store = DomainStore::from_output(output);
        self.abstract_domain_observations = store.observations().to_vec();
        self.abstract_domain_events = store.events().to_vec();
        self.abstract_domain_store = Some(store);
        self.refresh_abstract_domain_metadata();
    }

    fn populate_call_owner_symbols(&self, output: &mut CallOutput) {
        if output.sites.iter().all(|site| site.owner_symbol.is_some()) {
            return;
        }

        let function_symbols = self
            .functions
            .iter()
            .filter_map(|function| {
                let symbol = self
                    .symbols
                    .iter()
                    .find(|symbol| {
                        symbol.file == Some(function.file)
                            && symbol.name == function.name
                            && symbol.primary_span.as_ref().is_some_and(|span| {
                                span == &function.span || Self::span_is_within(span, &function.span)
                            })
                    })
                    .map(|symbol| symbol.id)
                    .or_else(|| {
                        self.definitions
                            .iter()
                            .find(|definition| {
                                definition.file == Some(function.file)
                                    && definition.name == function.name
                                    && definition.primary_span.as_ref().is_some_and(|span| {
                                        span == &function.span
                                            || Self::span_is_within(span, &function.span)
                                    })
                            })
                            .map(|definition| definition.symbol)
                    });
                symbol.map(|symbol| (function.id, symbol))
            })
            .collect::<BTreeMap<_, _>>();

        for site in &mut output.sites {
            if site.owner_symbol.is_none() {
                site.owner_symbol = function_symbols.get(&site.caller).copied();
            }
        }
    }

    fn span_is_within(inner: &Span, outer: &Span) -> bool {
        inner.file == outer.file
            && inner.start_byte >= outer.start_byte
            && inner.end_byte <= outer.end_byte
    }

    pub(crate) fn call_sites(&self) -> &[CallSiteFact] {
        &self.call_sites
    }

    pub(crate) fn call_targets(&self) -> &[CallTargetFact] {
        &self.call_targets
    }

    pub(crate) fn unresolved_calls(&self) -> &[UnresolvedCallFact] {
        &self.unresolved_calls
    }

    #[allow(dead_code)]
    pub(crate) fn call_store(&self) -> Option<&CallStore> {
        self.call_store.as_ref()
    }

    pub(crate) fn refined_call_edges(&self) -> &[RefinedCallEdgeFact] {
        &self.refined_call_edges
    }

    #[allow(dead_code)]
    pub(crate) fn refined_call_store(&self) -> Option<&RefinedCallStore> {
        self.refined_call_store.as_ref()
    }

    pub(crate) fn data_flow_nodes(&self) -> &[DataFlowNodeFact] {
        &self.data_flow_nodes
    }

    pub(crate) fn data_flow_edges(&self) -> &[DataFlowEdgeFact] {
        &self.data_flow_edges
    }

    pub(crate) fn data_flow_models(&self) -> &[DataFlowModelFact] {
        &self.data_flow_models
    }

    pub(crate) fn data_flow_budgets(&self) -> &[DataFlowBudgetFact] {
        &self.data_flow_budgets
    }

    #[allow(dead_code)]
    pub(crate) fn data_flow_store(&self) -> Option<&DataFlowStore> {
        self.data_flow_store.as_ref()
    }

    pub(crate) fn evidence_nodes(&self) -> &[EvidenceNodeFact] {
        &self.evidence_nodes
    }

    pub(crate) fn evidence_edges(&self) -> &[EvidenceEdgeFact] {
        &self.evidence_edges
    }

    pub(crate) fn evidence_bundles(&self) -> &[EvidenceBundleFact] {
        &self.evidence_bundles
    }

    pub(crate) fn evidence_paths(&self) -> &[EvidencePathFact] {
        &self.evidence_paths
    }

    pub(crate) fn evidence_slices(&self) -> &[EvidenceSliceFact] {
        &self.evidence_slices
    }

    pub(crate) fn evidence_unknowns(&self) -> &[EvidenceUnknownFact] {
        &self.evidence_unknowns
    }

    pub(crate) fn evidence_omitted_regions(&self) -> &[EvidenceOmittedRegionFact] {
        &self.evidence_omitted_regions
    }

    pub(crate) fn evidence_replay_keys(&self) -> &[EvidenceReplayKeyFact] {
        &self.evidence_replay_keys
    }

    #[allow(dead_code)]
    pub(crate) fn evidence_store(&self) -> Option<&EvidenceStore> {
        self.evidence_store.as_ref()
    }

    pub(crate) fn abstract_domain_observations(&self) -> &[DomainObservationFact] {
        &self.abstract_domain_observations
    }

    pub(crate) fn abstract_domain_events(&self) -> &[DomainEventFact] {
        &self.abstract_domain_events
    }

    #[allow(dead_code)]
    pub(crate) fn abstract_domain_store(&self) -> Option<&DomainStore> {
        self.abstract_domain_store.as_ref()
    }

    pub(crate) fn replace_summary_facts(&mut self, output: SummaryOutput) {
        let store =
            SummaryStore::from_output(output).expect("summary output should produce a valid store");
        self.summary_facts = store.all_summaries().to_vec();
        self.summary_events = store.all_events().to_vec();
        self.summary_store = Some(store);
        self.refresh_summary_metadata();
    }

    #[allow(
        dead_code,
        reason = "Extension fact replacement is wired into the kernel provider in the next Phase 34 plan."
    )]
    pub(crate) fn replace_extension_facts(&mut self, output: ExtensionOutput) {
        let output = output.normalized();
        self.extension_activations = output.activations;
        self.extension_facts = output.accepted;
        self.rejected_extension_facts = output.rejected;
        self.refresh_extension_metadata();
    }

    pub(crate) fn summary_facts(&self) -> &[SummaryFact] {
        &self.summary_facts
    }

    pub(crate) fn summary_events(&self) -> &[SummaryEventFact] {
        &self.summary_events
    }

    #[allow(dead_code)]
    pub(crate) fn summary_store(&self) -> Option<&SummaryStore> {
        self.summary_store.as_ref()
    }

    pub(crate) fn extension_facts(&self) -> &[AcceptedExtensionFact] {
        &self.extension_facts
    }

    pub(crate) fn extension_activations(&self) -> &[ExtensionActivationRow] {
        &self.extension_activations
    }

    #[allow(
        dead_code,
        reason = "Rejected extension audit rows are surfaced by the extension provider/debug wiring in the next Phase 34 plan."
    )]
    pub(crate) fn rejected_extension_facts(&self) -> &[RejectedExtensionFact] {
        &self.rejected_extension_facts
    }

    pub(crate) fn replace_adaptation_model_facts(
        &mut self,
        accepted: Vec<AcceptedModelFact>,
        rejected: Vec<RejectedModelFact>,
    ) {
        self.adaptation_model_facts = accepted;
        self.rejected_adaptation_model_facts = rejected;
        self.refresh_adaptation_model_metadata();
    }

    pub(crate) fn adaptation_model_facts(&self) -> &[AcceptedModelFact] {
        &self.adaptation_model_facts
    }

    #[allow(
        dead_code,
        reason = "Rejected adaptation model audit rows are surfaced by eval fixture observation wiring."
    )]
    pub(crate) fn rejected_adaptation_model_facts(&self) -> &[RejectedModelFact] {
        &self.rejected_adaptation_model_facts
    }

    pub(crate) fn replace_entrypoint_facts(
        &mut self,
        output: EntrypointOutput,
    ) -> Result<(), AnalysisError> {
        let store = EntrypointStore::from_output(output)?;
        self.entrypoint_facts = store.entrypoints().to_vec();
        self.trust_boundary_facts = store.trust_boundaries().to_vec();
        self.dispatch_edge_facts = store.dispatch_edges().to_vec();
        self.unresolved_framework_facts = store.unresolved().to_vec();
        self.entrypoint_store = Some(store);
        self.refresh_entrypoint_metadata();
        Ok(())
    }

    pub(crate) fn entrypoint_facts(&self) -> &[EntrypointFact] {
        &self.entrypoint_facts
    }

    #[allow(
        dead_code,
        reason = "Reachability fact replacement is wired into the kernel provider in the next Phase 43 plan-01 task (provider/kernel splice)."
    )]
    pub(crate) fn replace_reachability_facts(
        &mut self,
        output: ReachabilityProviderOutput,
    ) -> Result<(), AnalysisError> {
        let valid_function_ids = self.functions.iter().map(|row| row.id).collect();
        let valid_entrypoint_ids = self.entrypoint_facts.iter().map(|row| row.id).collect();
        let store =
            ReachabilityStore::from_output(output, &valid_function_ids, &valid_entrypoint_ids)?;
        self.reachability_roots = store.roots().to_vec();
        self.reachability_marks = store.marks().to_vec();
        Ok(())
    }

    #[allow(
        dead_code,
        reason = "Reachability roots are consumed by validation, debug, and the kernel provider wiring in Phase 43 plan-01."
    )]
    pub(crate) fn reachability_roots(&self) -> &[ReachabilityRootFact] {
        &self.reachability_roots
    }

    #[allow(
        dead_code,
        reason = "Reachability marks are populated by the marking traversal in Phase 43 plan-02 and read by debug/eval."
    )]
    pub(crate) fn reachability_marks(&self) -> &[CallReachabilityFact] {
        &self.reachability_marks
    }

    /// Stores the normalized semantic-graph nodes/edges/constraints (GRAPH-01),
    /// mirroring [`Self::replace_reachability_facts`]. Construction runs through
    /// [`SemanticGraphStore::from_output`], which normalizes (stable-key sort + dense
    /// ID assignment) and referentially validates every edge endpoint and constraint
    /// node reference — a dangling reference returns [`AnalysisError::InvalidFact`] so
    /// the db is never left holding a malformed graph.
    pub(crate) fn replace_semantic_graph_facts(
        &mut self,
        output: SemanticGraphOutput,
    ) -> Result<(), AnalysisError> {
        let store = SemanticGraphStore::from_output(output)?;
        self.semantic_nodes = store.nodes().to_vec();
        self.semantic_edges = store.edges().to_vec();
        self.semantic_constraints = store.constraints().to_vec();
        Ok(())
    }

    pub(crate) fn semantic_nodes(&self) -> &[SemanticNodeFact] {
        &self.semantic_nodes
    }

    pub(crate) fn semantic_edges(&self) -> &[SemanticEdgeFact] {
        &self.semantic_edges
    }

    pub(crate) fn semantic_constraints(&self) -> &[ConstraintFact] {
        &self.semantic_constraints
    }

    /// Stores the private TS object/property/prototype/receiver rows used by the
    /// Phase 50 semantic-graph lowering. Construction runs through
    /// [`TsObjectModelStore::try_from_output`], which preserves deterministic
    /// normalization and rejects duplicate stable keys before stale rows are replaced.
    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    pub(crate) fn replace_ts_object_model_facts(
        &mut self,
        output: TsObjectModelOutput,
    ) -> Result<(), AnalysisError> {
        let store = TsObjectModelStore::try_from_output(output)?;
        self.ts_object_allocations = store.allocations().to_vec();
        self.ts_property_writes = store.property_writes().to_vec();
        self.ts_property_reads = store.property_reads().to_vec();
        self.ts_receiver_bindings = store.receiver_bindings().to_vec();
        self.ts_prototype_links = store.prototype_links().to_vec();
        self.ts_object_model_store = Some(store);
        Ok(())
    }

    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    pub(crate) fn ts_object_allocations(&self) -> &[TsObjectAllocationFact] {
        &self.ts_object_allocations
    }

    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    pub(crate) fn ts_property_writes(&self) -> &[TsPropertyWriteFact] {
        &self.ts_property_writes
    }

    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    pub(crate) fn ts_property_reads(&self) -> &[TsPropertyReadFact] {
        &self.ts_property_reads
    }

    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    pub(crate) fn ts_receiver_bindings(&self) -> &[TsReceiverBindingFact] {
        &self.ts_receiver_bindings
    }

    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    pub(crate) fn ts_prototype_links(&self) -> &[TsPrototypeLinkFact] {
        &self.ts_prototype_links
    }

    #[allow(
        dead_code,
        reason = "Phase 50 task 3 stores TS object-model rows before semantic-graph lowering consumes them in task 4."
    )]
    pub(crate) fn ts_object_model_store(&self) -> Option<&TsObjectModelStore> {
        self.ts_object_model_store.as_ref()
    }

    /// Stores the normalized solver-derived edges (GRAPH-03/GRAPH-04), mirroring
    /// [`Self::replace_semantic_graph_facts`]. Construction runs through
    /// [`SolverStore::from_output`], which normalizes (stable-key sort + dense ID
    /// assignment) and referentially validates duplicate stable keys + the precision
    /// ceiling (D-06) — a malformed row returns [`AnalysisError::InvalidFact`] so the
    /// db is never left holding a malformed solver output.
    pub(crate) fn replace_solver_facts(
        &mut self,
        output: SolverOutput,
    ) -> Result<(), AnalysisError> {
        let store = SolverStore::from_output(output)?;
        self.solver_derived_edges = store.derived_edges().to_vec();
        self.solver_budget_status = store.budget_status();
        self.solver_budget_reasons = store.budget_reasons().clone();
        Ok(())
    }

    /// The stored solver-derived edges. Consumed by the provider tests today and by
    /// Phase 52's GRAPH-05 refined_calls rework (which projects over solver output);
    /// no production read exists yet, so the accessor is dead-code in a non-test build
    /// until that consumer lands (the facts are stored unconditionally so the
    /// determinism gate observes them).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn solver_derived_edges(&self) -> &[DerivedEdgeFact] {
        &self.solver_derived_edges
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn solver_budget_status(&self) -> BudgetStatus {
        self.solver_budget_status
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn solver_budget_reasons(&self) -> &BTreeSet<String> {
        &self.solver_budget_reasons
    }

    /// Store the Go semantic facts, returning the resilience report (malformed RTA-signal
    /// harvest rows dropped, FIX 3; plus duplicate structural rows collapsed keep-first,
    /// FIX-08) so the provider can surface observable diagnostics. All counts are zero on a
    /// clean frontend run.
    pub(crate) fn replace_go_semantic_facts(
        &mut self,
        output: GoSemanticFactsOutput,
    ) -> Result<GoSemanticStoreReport, AnalysisError> {
        let store = GoSemanticStore::from_output(output)?;
        self.go_semantic_packages = store.output().packages.clone();
        self.go_semantic_functions = store.output().functions.clone();
        self.go_semantic_callsites = store.output().callsites.clone();
        self.go_semantic_method_sets = store.output().method_sets.clone();
        self.go_semantic_address_taken = store.output().address_taken.clone();
        self.go_semantic_instantiated_types = store.output().instantiated_types.clone();
        self.go_semantic_dynamic_dispatch = store.output().dynamic_dispatch.clone();
        self.go_semantic_rta_edges = store.output().rta_edges.clone();
        self.go_semantic_package_errors = store.output().package_errors.clone();
        Ok(store.report())
    }

    /// The normalized Go semantic output currently stored in the database.
    ///
    /// Used by the provider after `replace_go_semantic_facts` so its output digest certifies
    /// the rows that survived store-time resilience passes (invalid harvest-row drops and
    /// duplicate structural-key collapse), not the raw sidecar/lowering rows.
    pub(crate) fn go_semantic_facts_output(&self) -> GoSemanticFactsOutput {
        GoSemanticFactsOutput {
            packages: self.go_semantic_packages.clone(),
            functions: self.go_semantic_functions.clone(),
            callsites: self.go_semantic_callsites.clone(),
            method_sets: self.go_semantic_method_sets.clone(),
            address_taken: self.go_semantic_address_taken.clone(),
            instantiated_types: self.go_semantic_instantiated_types.clone(),
            dynamic_dispatch: self.go_semantic_dynamic_dispatch.clone(),
            rta_edges: self.go_semantic_rta_edges.clone(),
            package_errors: self.go_semantic_package_errors.clone(),
        }
    }

    pub(crate) fn go_semantic_packages(&self) -> &[GoSemanticPackageFact] {
        &self.go_semantic_packages
    }

    pub(crate) fn go_semantic_functions(&self) -> &[GoSemanticFunctionFact] {
        &self.go_semantic_functions
    }

    pub(crate) fn go_semantic_callsites(&self) -> &[GoSemanticCallsiteFact] {
        &self.go_semantic_callsites
    }

    #[allow(
        dead_code,
        reason = "Method-set facts are stored privately for Phase 48 receiver/RTA expansion."
    )]
    pub(crate) fn go_semantic_method_sets(&self) -> &[GoSemanticMethodSetFact] {
        &self.go_semantic_method_sets
    }

    #[allow(
        dead_code,
        reason = "Address-taken facts are stored privately for the Plan 2 go_rta dispatch-candidate set (GO-05)."
    )]
    pub(crate) fn go_semantic_address_taken(&self) -> &[GoSemanticAddressTakenFact] {
        &self.go_semantic_address_taken
    }

    #[allow(
        dead_code,
        reason = "Instantiated-type facts are stored privately for the Plan 2 go_rta rapid-type filter (GO-05)."
    )]
    pub(crate) fn go_semantic_instantiated_types(&self) -> &[GoSemanticInstantiatedTypeFact] {
        &self.go_semantic_instantiated_types
    }

    #[allow(
        dead_code,
        reason = "Dynamic-dispatch detail is stored privately for the Plan 2 go_rta method-set matching (GO-05)."
    )]
    pub(crate) fn go_semantic_dynamic_dispatch(&self) -> &[GoSemanticDynamicDispatchFact] {
        &self.go_semantic_dynamic_dispatch
    }

    #[cfg(test)]
    pub(crate) fn go_semantic_rta_edges(&self) -> &[GoSemanticRtaEdgeFact] {
        &self.go_semantic_rta_edges
    }

    #[allow(
        dead_code,
        reason = "Package-load errors are stored privately for capability diagnostics once the provider is kernel-wired."
    )]
    pub(crate) fn go_semantic_package_errors(&self) -> &[GoSemanticPackageErrorFact] {
        &self.go_semantic_package_errors
    }

    pub(crate) fn trust_boundary_facts(&self) -> &[TrustBoundaryFact] {
        &self.trust_boundary_facts
    }

    pub(crate) fn dispatch_edge_facts(&self) -> &[FrameworkDispatchEdgeFact] {
        &self.dispatch_edge_facts
    }

    pub(crate) fn unresolved_framework_facts(&self) -> &[UnresolvedFrameworkFact] {
        &self.unresolved_framework_facts
    }

    pub(crate) fn replace_type_value_alias_facts(&mut self, output: TypeValueAliasOutput) {
        let output = output.normalized();
        let type_store = TypeStore::from_output(output.types);
        let value_store = ValueStore::from_output(output.values);
        let access_path_store = AccessPathStore::from_output(output.access_paths);
        let points_to_store = PointsToStore::from_output(output.points_to);
        let alias_store = AliasStore::from_output(output.aliases);

        self.type_facts = type_store.types().to_vec();
        self.narrowed_type_facts = type_store.narrowed().to_vec();
        self.value_facts = value_store.values().to_vec();
        self.allocation_tokens = value_store.allocations().to_vec();
        self.access_path_facts = access_path_store.access_paths().to_vec();
        self.points_to_constraints = points_to_store.constraints().to_vec();
        self.points_to_sets = points_to_store.sets().to_vec();
        self.alias_answers = alias_store.answers().to_vec();
        self.type_store = Some(type_store);
        self.value_store = Some(value_store);
        self.access_path_store = Some(access_path_store);
        self.points_to_store = Some(points_to_store);
        self.alias_store = Some(alias_store);
        self.refresh_type_value_alias_metadata();
    }

    pub(crate) fn type_facts(&self) -> &[TypeFact] {
        &self.type_facts
    }

    #[allow(dead_code)]
    pub(crate) fn narrowed_type_facts(&self) -> &[NarrowedTypeFact] {
        &self.narrowed_type_facts
    }

    pub(crate) fn value_facts(&self) -> &[ValueFact] {
        &self.value_facts
    }

    #[allow(dead_code)]
    pub(crate) fn allocation_tokens(&self) -> &[AllocationTokenFact] {
        &self.allocation_tokens
    }

    pub(crate) fn access_path_facts(&self) -> &[AccessPathFact] {
        &self.access_path_facts
    }

    #[allow(dead_code)]
    pub(crate) fn points_to_constraints(&self) -> &[PointsToConstraintFact] {
        &self.points_to_constraints
    }

    pub(crate) fn points_to_sets(&self) -> &[PointsToSetFact] {
        &self.points_to_sets
    }

    pub(crate) fn alias_answers(&self) -> &[AliasAnswerFact] {
        &self.alias_answers
    }

    #[allow(dead_code)]
    pub(crate) fn call_sites_by_caller(&self, caller: FunctionId) -> Vec<&CallSiteFact> {
        self.call_store
            .as_ref()
            .map_or_else(Vec::new, |store| store.sites_by_caller(caller))
    }

    #[allow(dead_code)]
    pub(crate) fn call_targets_by_site(&self, site: CallSiteId) -> Vec<&CallTargetFact> {
        self.call_store
            .as_ref()
            .map_or_else(Vec::new, |store| store.targets_by_site(site))
    }

    #[allow(dead_code)]
    pub(crate) fn outgoing_calls_by_function(&self, caller: FunctionId) -> Vec<&CallTargetFact> {
        self.call_store
            .as_ref()
            .map_or_else(Vec::new, |store| store.outgoing_by_function(caller))
    }

    #[allow(dead_code)]
    pub(crate) fn outgoing_calls_by_symbol(&self, caller: SymbolId) -> Vec<&CallTargetFact> {
        self.call_store
            .as_ref()
            .map_or_else(Vec::new, |store| store.outgoing_by_symbol(caller))
    }

    #[allow(dead_code)]
    pub(crate) fn incoming_calls_by_symbol(&self, target: SymbolId) -> Vec<&CallTargetFact> {
        self.call_store
            .as_ref()
            .map_or_else(Vec::new, |store| store.incoming_by_symbol(target))
    }

    #[allow(dead_code)]
    pub(crate) fn incoming_calls_by_function(&self, target: FunctionId) -> Vec<&CallTargetFact> {
        self.call_store
            .as_ref()
            .map_or_else(Vec::new, |store| store.incoming_by_function(target))
    }

    #[allow(dead_code)]
    pub(crate) fn unresolved_calls_by_reason(
        &self,
        reason: UnresolvedCallReason,
    ) -> Vec<&UnresolvedCallFact> {
        self.call_store
            .as_ref()
            .map_or_else(Vec::new, |store| store.unresolved_by_reason(reason))
    }

    #[allow(dead_code)]
    pub(crate) fn unresolved_calls_by_status(
        &self,
        status: CallTargetStatus,
    ) -> Vec<&UnresolvedCallFact> {
        self.call_store
            .as_ref()
            .map_or_else(Vec::new, |store| store.unresolved_by_status(status))
    }

    fn refresh_call_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::CallSite);
        self.fact_meta.remove_family(FactFamily::CallTarget);
        self.fact_meta.remove_family(FactFamily::UnresolvedCall);

        let site_metadata = self
            .call_sites
            .iter()
            .map(|fact| (fact.id.0, self.call_site_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in site_metadata {
            self.record_fact_meta(FactFamily::CallSite, run_id, metadata);
        }

        let target_metadata = self
            .call_targets
            .iter()
            .map(|fact| (fact.id.0, self.call_target_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in target_metadata {
            self.record_fact_meta(FactFamily::CallTarget, run_id, metadata);
        }

        let unresolved_metadata = self
            .unresolved_calls
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.unresolved_call_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in unresolved_metadata {
            self.record_fact_meta(FactFamily::UnresolvedCall, run_id, metadata);
        }
    }

    fn refresh_refined_call_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::RefinedCallEdge);

        let edge_metadata = self
            .refined_call_edges
            .iter()
            .map(|fact| (fact.id.0, self.refined_call_edge_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in edge_metadata {
            self.record_fact_meta(FactFamily::RefinedCallEdge, run_id, metadata);
        }
    }

    fn refresh_data_flow_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::DataFlowNode);
        self.fact_meta.remove_family(FactFamily::DataFlowEdge);
        self.fact_meta.remove_family(FactFamily::DataFlowModel);
        self.fact_meta.remove_family(FactFamily::DataFlowBudget);

        let node_metadata = self
            .data_flow_nodes
            .iter()
            .map(|fact| (fact.id.0, self.data_flow_node_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in node_metadata {
            self.record_fact_meta(FactFamily::DataFlowNode, run_id, metadata);
        }

        let edge_metadata = self
            .data_flow_edges
            .iter()
            .map(|fact| (fact.id.0, self.data_flow_edge_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in edge_metadata {
            self.record_fact_meta(FactFamily::DataFlowEdge, run_id, metadata);
        }

        let model_metadata = self
            .data_flow_models
            .iter()
            .map(|fact| (fact.id.0, self.data_flow_model_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in model_metadata {
            self.record_fact_meta(FactFamily::DataFlowModel, run_id, metadata);
        }

        let budget_metadata = self
            .data_flow_budgets
            .iter()
            .map(|fact| (fact.id.0, self.data_flow_budget_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in budget_metadata {
            self.record_fact_meta(FactFamily::DataFlowBudget, run_id, metadata);
        }
    }

    fn refresh_evidence_metadata(&mut self) {
        for family in [
            FactFamily::EvidenceNode,
            FactFamily::EvidenceEdge,
            FactFamily::EvidenceBundle,
            FactFamily::EvidencePath,
            FactFamily::EvidenceSlice,
            FactFamily::EvidenceUnknown,
            FactFamily::EvidenceOmittedRegion,
            FactFamily::EvidenceReplayKey,
        ] {
            self.fact_meta.remove_family(family);
        }

        let node_metadata = self
            .evidence_nodes
            .iter()
            .map(|fact| (fact.id.0, self.evidence_node_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in node_metadata {
            self.record_fact_meta(FactFamily::EvidenceNode, run_id, metadata);
        }

        let edge_metadata = self
            .evidence_edges
            .iter()
            .map(|fact| (fact.id.0, self.evidence_edge_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in edge_metadata {
            self.record_fact_meta(FactFamily::EvidenceEdge, run_id, metadata);
        }

        let bundle_metadata = self
            .evidence_bundles
            .iter()
            .map(|fact| (fact.id.0, self.evidence_bundle_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in bundle_metadata {
            self.record_fact_meta(FactFamily::EvidenceBundle, run_id, metadata);
        }

        let path_metadata = self
            .evidence_paths
            .iter()
            .map(|fact| (fact.id.0, self.evidence_path_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in path_metadata {
            self.record_fact_meta(FactFamily::EvidencePath, run_id, metadata);
        }

        let slice_metadata = self
            .evidence_slices
            .iter()
            .map(|fact| (fact.id.0, self.evidence_slice_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in slice_metadata {
            self.record_fact_meta(FactFamily::EvidenceSlice, run_id, metadata);
        }

        let unknown_metadata = self
            .evidence_unknowns
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.evidence_unknown_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in unknown_metadata {
            self.record_fact_meta(FactFamily::EvidenceUnknown, run_id, metadata);
        }

        let omitted_metadata = self
            .evidence_omitted_regions
            .iter()
            .map(|fact| (fact.id.0, self.evidence_omitted_region_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in omitted_metadata {
            self.record_fact_meta(FactFamily::EvidenceOmittedRegion, run_id, metadata);
        }

        let replay_metadata = self
            .evidence_replay_keys
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.evidence_replay_key_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in replay_metadata {
            self.record_fact_meta(FactFamily::EvidenceReplayKey, run_id, metadata);
        }
    }

    fn refresh_abstract_domain_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::DomainObservation);
        self.fact_meta.remove_family(FactFamily::DomainEvent);

        let observation_metadata = self
            .abstract_domain_observations
            .iter()
            .map(|fact| (fact.id.0, self.domain_observation_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in observation_metadata {
            self.record_fact_meta(FactFamily::DomainObservation, run_id, metadata);
        }

        let event_metadata = self
            .abstract_domain_events
            .iter()
            .map(|fact| (fact.id.0, self.domain_event_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in event_metadata {
            self.record_fact_meta(FactFamily::DomainEvent, run_id, metadata);
        }
    }

    fn refresh_summary_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::SummaryControl);
        self.fact_meta.remove_family(FactFamily::SummaryCall);
        self.fact_meta.remove_family(FactFamily::SummaryMemory);
        self.fact_meta.remove_family(FactFamily::SummaryTito);
        self.fact_meta.remove_family(FactFamily::SummaryEvent);

        let summary_metadata = self
            .summary_facts
            .iter()
            .map(|fact| {
                let family = summary_domain_to_fact_family(fact.domain);
                (family, fact.id.0, self.summary_fact_metadata(fact))
            })
            .collect::<Vec<_>>();
        for (family, run_id, metadata) in summary_metadata {
            self.record_fact_meta(family, run_id, metadata);
        }

        let event_metadata = self
            .summary_events
            .iter()
            .map(|fact| (fact.id.0, self.summary_event_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in event_metadata {
            self.record_fact_meta(FactFamily::SummaryEvent, run_id, metadata);
        }
    }

    #[allow(
        dead_code,
        reason = "Extension metadata refresh is reached through extension provider wiring in the next Phase 34 plan."
    )]
    fn refresh_extension_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::ExtensionFact);
        let metadata = self
            .extension_facts
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, extension_fact_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in metadata {
            self.record_fact_meta(FactFamily::ExtensionFact, run_id, metadata);
        }
    }

    fn refresh_adaptation_model_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::AdaptationModel);
        let metadata = self
            .adaptation_model_facts
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, adaptation_model_fact_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in metadata {
            self.record_fact_meta(FactFamily::AdaptationModel, run_id, metadata);
        }
    }

    fn refresh_entrypoint_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::Entrypoint);
        self.fact_meta.remove_family(FactFamily::TrustBoundary);
        self.fact_meta.remove_family(FactFamily::DispatchEdge);
        self.fact_meta
            .remove_family(FactFamily::UnresolvedFramework);

        let entrypoint_metadata = self
            .entrypoint_facts
            .iter()
            .map(|fact| (fact.id.0, self.entrypoint_fact_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in entrypoint_metadata {
            self.record_fact_meta(FactFamily::Entrypoint, run_id, metadata);
        }

        let trust_boundary_metadata = self
            .trust_boundary_facts
            .iter()
            .map(|fact| (fact.id.0, self.trust_boundary_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in trust_boundary_metadata {
            self.record_fact_meta(FactFamily::TrustBoundary, run_id, metadata);
        }

        let dispatch_edge_metadata = self
            .dispatch_edge_facts
            .iter()
            .map(|fact| (fact.id.0, self.dispatch_edge_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in dispatch_edge_metadata {
            self.record_fact_meta(FactFamily::DispatchEdge, run_id, metadata);
        }

        let unresolved_metadata = self
            .unresolved_framework_facts
            .iter()
            .map(|fact| (fact.id.0, self.unresolved_framework_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in unresolved_metadata {
            self.record_fact_meta(FactFamily::UnresolvedFramework, run_id, metadata);
        }
    }

    fn refresh_type_value_alias_metadata(&mut self) {
        for family in [
            FactFamily::Type,
            FactFamily::NarrowedType,
            FactFamily::Value,
            FactFamily::AllocationToken,
            FactFamily::AccessPath,
            FactFamily::PointsToConstraint,
            FactFamily::PointsToSet,
            FactFamily::AliasAnswer,
        ] {
            self.fact_meta.remove_family(family);
        }

        let type_metadata = self
            .type_facts
            .iter()
            .map(|fact| (fact.id.0, self.type_fact_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in type_metadata {
            self.record_fact_meta(FactFamily::Type, run_id, metadata);
        }

        let narrowed_metadata = self
            .narrowed_type_facts
            .iter()
            .map(|fact| (fact.id.0, self.narrowed_type_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in narrowed_metadata {
            self.record_fact_meta(FactFamily::NarrowedType, run_id, metadata);
        }

        let value_metadata = self
            .value_facts
            .iter()
            .map(|fact| (fact.id.0, self.value_fact_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in value_metadata {
            self.record_fact_meta(FactFamily::Value, run_id, metadata);
        }

        let allocation_metadata = self
            .allocation_tokens
            .iter()
            .map(|fact| (fact.id.0, self.allocation_token_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in allocation_metadata {
            self.record_fact_meta(FactFamily::AllocationToken, run_id, metadata);
        }

        let access_path_metadata = self
            .access_path_facts
            .iter()
            .map(|fact| (fact.id.0, self.access_path_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in access_path_metadata {
            self.record_fact_meta(FactFamily::AccessPath, run_id, metadata);
        }

        let points_to_constraint_metadata = self
            .points_to_constraints
            .iter()
            .map(|fact| (fact.id.0, self.points_to_constraint_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in points_to_constraint_metadata {
            self.record_fact_meta(FactFamily::PointsToConstraint, run_id, metadata);
        }

        let points_to_set_metadata = self
            .points_to_sets
            .iter()
            .map(|fact| (fact.id.0, self.points_to_set_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in points_to_set_metadata {
            self.record_fact_meta(FactFamily::PointsToSet, run_id, metadata);
        }

        let alias_metadata = self
            .alias_answers
            .iter()
            .map(|fact| (fact.id.0, self.alias_answer_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in alias_metadata {
            self.record_fact_meta(FactFamily::AliasAnswer, run_id, metadata);
        }
    }

    fn type_fact_metadata(&self, fact: &TypeFact) -> FactMeta {
        let (precision, confidence) =
            type_metadata_precision(fact.status, fact.precision, Some(fact.confidence));
        fact_meta_from_stable_key(
            FactFamily::Type,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("phase", format!("{:?}", fact.phase)),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.option_source_file_key(fact.file)),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                ("subject", format!("{:?}", fact.subject)),
                ("shape", format!("{:?}", fact.shape)),
                ("provenance", format!("{:?}", fact.provenance)),
            ]),
        )
    }

    fn narrowed_type_metadata(&self, fact: &NarrowedTypeFact) -> FactMeta {
        let (precision, confidence) = type_metadata_precision(fact.status, fact.precision, None);
        fact_meta_from_stable_key(
            FactFamily::NarrowedType,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("language", language_label(fact.language).to_string()),
                (
                    "place_key",
                    self.fact_stable_key(FactFamily::Place, fact.place.0),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.clone()),
            ]),
        )
    }

    fn value_fact_metadata(&self, fact: &ValueFact) -> FactMeta {
        let (precision, confidence) = value_metadata_precision(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Value,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("language", language_label(fact.language).to_string()),
                ("subject", format!("{:?}", fact.subject)),
                ("kind", format!("{:?}", fact.kind)),
                ("provenance", format!("{:?}", fact.provenance)),
            ]),
        )
    }

    fn allocation_token_metadata(&self, fact: &AllocationTokenFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::AllocationToken,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            FactPrecision::SetupAware,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("language", language_label(fact.language).to_string()),
                (
                    "source_place",
                    fact.source_place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                ("provenance", format!("{:?}", fact.provenance)),
            ]),
        )
    }

    fn access_path_metadata(&self, fact: &AccessPathFact) -> FactMeta {
        let precision = match fact.status {
            crate::analysis::access_paths::facts::AccessPathStatus::Resolved => {
                FactPrecision::SetupAware
            }
            crate::analysis::access_paths::facts::AccessPathStatus::Partial => {
                FactPrecision::Ambiguous
            }
            crate::analysis::access_paths::facts::AccessPathStatus::Unknown => {
                FactPrecision::Unresolved
            }
            crate::analysis::access_paths::facts::AccessPathStatus::Unsupported => {
                FactPrecision::Unsupported
            }
            crate::analysis::access_paths::facts::AccessPathStatus::BudgetExceeded => {
                FactPrecision::Heuristic
            }
        };
        fact_meta_from_stable_key(
            FactFamily::AccessPath,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("language", language_label(fact.language).to_string()),
                (
                    "base_key",
                    self.fact_stable_key(FactFamily::Place, fact.base.0),
                ),
                ("projection_count", fact.projections.len().to_string()),
            ]),
        )
    }

    fn points_to_constraint_metadata(&self, fact: &PointsToConstraintFact) -> FactMeta {
        let (precision, confidence) = points_to_metadata_precision(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::PointsToConstraint,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("kind", format!("{:?}", fact.kind)),
            ]),
        )
    }

    fn points_to_set_metadata(&self, fact: &PointsToSetFact) -> FactMeta {
        let (precision, confidence) = points_to_metadata_precision(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::PointsToSet,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("budget", format!("{:?}", fact.budget)),
                ("variable", format!("{:?}", fact.variable)),
                ("objects", format!("{:?}", fact.objects)),
            ]),
        )
    }

    fn alias_answer_metadata(&self, fact: &AliasAnswerFact) -> FactMeta {
        let (precision, confidence) = alias_metadata_precision(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::AliasAnswer,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("reason", format!("{:?}", fact.reason)),
                ("left", format!("{:?}", fact.left)),
                ("right", format!("{:?}", fact.right)),
                ("evidence", fact.evidence.join("\n")),
            ]),
        )
    }

    fn entrypoint_fact_metadata(&self, fact: &EntrypointFact) -> FactMeta {
        let (precision, confidence) = entrypoint_precision_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Entrypoint,
            ENTRYPOINTS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("kind", format!("{:?}", fact.kind)),
                ("language", language_label(fact.language).to_string()),
                ("framework", fact.framework_id.clone()),
                ("file_key", self.source_file_key(fact.registration_file)),
                (
                    "function_key",
                    self.function_key(
                        fact.target_function,
                        &fact.framework_id,
                        &fact.registration_span,
                    ),
                ),
                ("provenance", format!("{:?}", fact.provenance)),
            ]),
        )
    }

    fn trust_boundary_metadata(&self, fact: &TrustBoundaryFact) -> FactMeta {
        let (precision, confidence) =
            entrypoint_precision_metadata(EntrypointStatus::Resolved, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::TrustBoundary,
            ENTRYPOINTS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("source_kind", format!("{:?}", fact.source_kind)),
                ("entrypoint_key", fact.entrypoint_stable_key.clone()),
                ("precision", format!("{:?}", fact.precision)),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.source_file_key(fact.file)),
            ]),
        )
    }

    fn dispatch_edge_metadata(&self, fact: &FrameworkDispatchEdgeFact) -> FactMeta {
        let (precision, confidence) =
            entrypoint_precision_metadata(EntrypointStatus::Resolved, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::DispatchEdge,
            ENTRYPOINTS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("edge_kind", format!("{:?}", fact.edge_kind)),
                ("from_source", fact.from_source.clone()),
                ("precision", format!("{:?}", fact.precision)),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.source_file_key(fact.file)),
            ]),
        )
    }

    fn unresolved_framework_metadata(&self, fact: &UnresolvedFrameworkFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::UnresolvedFramework,
            ENTRYPOINTS_PROVIDER_ID,
            FactPrecision::SetupAware,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("framework", fact.framework_id.clone()),
                ("precision", format!("{:?}", fact.precision)),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.source_file_key(fact.file)),
            ]),
        )
    }

    fn summary_fact_metadata(&self, fact: &SummaryFact) -> FactMeta {
        let (precision, confidence) = summary_precision_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            summary_domain_to_fact_family(fact.domain),
            POLINT_DIRECT_SUMMARIES_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", fact.status.as_str().to_string()),
                ("precision", fact.precision.as_str().to_string()),
                ("domain", fact.domain.as_str().to_string()),
                ("callable", fact.callable_stable_key.clone()),
                ("provenance", fact.provenance.as_str().to_string()),
                ("payload_digest", fact.payload_digest.clone()),
            ]),
        )
    }

    fn summary_event_metadata(&self, fact: &SummaryEventFact) -> FactMeta {
        let (precision, confidence) = summary_precision_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::SummaryEvent,
            POLINT_DIRECT_SUMMARIES_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", fact.status.as_str().to_string()),
                ("precision", fact.precision.as_str().to_string()),
                ("domain", fact.domain.as_str().to_string()),
                ("callable", fact.callable_stable_key.clone()),
                ("event_kind", fact.event_kind.clone()),
                ("reason", fact.reason.clone()),
            ]),
        )
    }

    fn refresh_cfg_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::CfgFunction);
        self.fact_meta.remove_family(FactFamily::CfgNode);
        self.fact_meta.remove_family(FactFamily::BasicBlock);
        self.fact_meta.remove_family(FactFamily::CfgEdge);
        self.fact_meta.remove_family(FactFamily::CfgReachability);
        self.fact_meta.remove_family(FactFamily::CfgDominator);
        self.fact_meta.remove_family(FactFamily::CfgPostDominator);
        self.fact_meta
            .remove_family(FactFamily::CfgControlDependence);
        self.fact_meta
            .remove_family(FactFamily::UnsupportedControlFlow);

        let function_metadata = self
            .cfg_functions
            .iter()
            .map(|fact| (fact.id.0, self.cfg_function_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in function_metadata {
            self.record_fact_meta(FactFamily::CfgFunction, run_id, metadata);
        }

        let node_metadata = self
            .cfg_nodes
            .iter()
            .map(|fact| (fact.id.0, self.cfg_node_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in node_metadata {
            self.record_fact_meta(FactFamily::CfgNode, run_id, metadata);
        }

        let block_metadata = self
            .cfg_blocks
            .iter()
            .map(|fact| (fact.id.0, self.cfg_block_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in block_metadata {
            self.record_fact_meta(FactFamily::BasicBlock, run_id, metadata);
        }

        let edge_metadata = self
            .cfg_edges
            .iter()
            .map(|fact| (fact.id.0, self.cfg_edge_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in edge_metadata {
            self.record_fact_meta(FactFamily::CfgEdge, run_id, metadata);
        }

        let reachability_metadata = self
            .cfg_reachability
            .iter()
            .map(|fact| (fact.id.0, self.cfg_reachability_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in reachability_metadata {
            self.record_fact_meta(FactFamily::CfgReachability, run_id, metadata);
        }

        let dominator_metadata = self
            .cfg_dominators
            .iter()
            .map(|fact| (fact.id.0, self.cfg_dominator_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in dominator_metadata {
            self.record_fact_meta(FactFamily::CfgDominator, run_id, metadata);
        }

        let postdominator_metadata = self
            .cfg_postdominators
            .iter()
            .map(|fact| (fact.id.0, self.cfg_postdominator_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in postdominator_metadata {
            self.record_fact_meta(FactFamily::CfgPostDominator, run_id, metadata);
        }

        let dependence_metadata = self
            .cfg_control_dependence
            .iter()
            .map(|fact| (fact.id.0, self.cfg_control_dependence_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in dependence_metadata {
            self.record_fact_meta(FactFamily::CfgControlDependence, run_id, metadata);
        }

        let unsupported_metadata = self
            .unsupported_control_flow
            .iter()
            .map(|fact| (fact.id.0, self.unsupported_control_flow_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in unsupported_metadata {
            self.record_fact_meta(FactFamily::UnsupportedControlFlow, run_id, metadata);
        }
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
        for site in self.call_sites() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::CallSite, site.id.0);
        }
        for target in self.call_targets() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::CallTarget, target.id.0);
        }
        for (run_id, _unresolved) in self.unresolved_calls().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::UnresolvedCall,
                run_id as u64,
            );
        }
        for edge in self.refined_call_edges() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::RefinedCallEdge, edge.id.0);
        }
        for node in self.data_flow_nodes() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::DataFlowNode, node.id.0);
        }
        for edge in self.data_flow_edges() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::DataFlowEdge, edge.id.0);
        }
        for model in self.data_flow_models() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::DataFlowModel, model.id.0);
        }
        for budget in self.data_flow_budgets() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::DataFlowBudget, budget.id.0);
        }
        for node in self.evidence_nodes() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::EvidenceNode, node.id.0);
        }
        for edge in self.evidence_edges() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::EvidenceEdge, edge.id.0);
        }
        for bundle in self.evidence_bundles() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::EvidenceBundle, bundle.id.0);
        }
        for path in self.evidence_paths() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::EvidencePath, path.id.0);
        }
        for slice in self.evidence_slices() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::EvidenceSlice, slice.id.0);
        }
        for (run_id, _unknown) in self.evidence_unknowns().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::EvidenceUnknown,
                run_id as u64,
            );
        }
        for omitted in self.evidence_omitted_regions() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::EvidenceOmittedRegion,
                omitted.id.0,
            );
        }
        for (run_id, _replay) in self.evidence_replay_keys().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::EvidenceReplayKey,
                run_id as u64,
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
        for (run_id, _fact) in self.extension_facts().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::ExtensionFact, run_id as u64);
        }
        for (run_id, _fact) in self.adaptation_model_facts().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::AdaptationModel,
                run_id as u64,
            );
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

    pub(crate) fn cfg_functions(&self) -> &[CfgFunctionFact] {
        &self.cfg_functions
    }

    pub(crate) fn cfg_nodes(&self) -> &[CfgNodeFact] {
        &self.cfg_nodes
    }

    pub(crate) fn cfg_blocks(&self) -> &[BasicBlockFact] {
        &self.cfg_blocks
    }

    pub(crate) fn cfg_edges(&self) -> &[CfgEdgeFact] {
        &self.cfg_edges
    }

    pub(crate) fn cfg_reachability(&self) -> &[ReachabilityFact] {
        &self.cfg_reachability
    }

    pub(crate) fn cfg_dominators(&self) -> &[DominatorFact] {
        &self.cfg_dominators
    }

    pub(crate) fn cfg_postdominators(&self) -> &[PostDominatorFact] {
        &self.cfg_postdominators
    }

    pub(crate) fn cfg_control_dependence(&self) -> &[ControlDependenceFact] {
        &self.cfg_control_dependence
    }

    pub(crate) fn unsupported_control_flow(&self) -> &[UnsupportedControlFlowFact] {
        &self.unsupported_control_flow
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

    fn call_site_metadata(&self, fact: &CallSiteFact) -> FactMeta {
        let (precision, confidence) = call_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CallSite,
            CALLS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                ("kind", call_syntax_kind_label(fact.kind).to_string()),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.source_file_key(fact.file)),
                ("caller_key", self.function_key(fact.caller, "", &fact.span)),
                (
                    "owner_symbol_key",
                    fact.owner_symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                (
                    "operation_key",
                    self.fact_stable_key(FactFamily::MirOperation, fact.operation.0),
                ),
                ("span", span_metadata_value(&fact.span)),
            ]),
        )
    }

    fn call_target_metadata(&self, fact: &CallTargetFact) -> FactMeta {
        let (precision, confidence) = call_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CallTarget,
            CALLS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                (
                    "algorithm",
                    call_algorithm_label(fact.algorithm).to_string(),
                ),
                (
                    "edge_kind",
                    call_edge_kind_label(fact.edge_kind).to_string(),
                ),
                (
                    "reason",
                    fact.reason
                        .map(call_unresolved_reason_label)
                        .map(str::to_string)
                        .unwrap_or_else(none_value),
                ),
                (
                    "site_key",
                    self.fact_stable_key(FactFamily::CallSite, fact.site.0),
                ),
                (
                    "caller_key",
                    self.fact_stable_key(FactFamily::Function, fact.caller.0),
                ),
                (
                    "target_function_key",
                    fact.target_function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "target_symbol_key",
                    fact.target_symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn refined_call_edge_metadata(&self, fact: &RefinedCallEdgeFact) -> FactMeta {
        let (precision, status_confidence) = call_status_metadata(fact.status, fact.precision);
        let confidence = refined_call_confidence_metadata(fact.confidence, status_confidence);
        let validation = refined_call_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::RefinedCallEdge,
            REFINED_CALLS_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                (
                    "algorithm",
                    call_algorithm_label(fact.algorithm).to_string(),
                ),
                (
                    "edge_kind",
                    call_edge_kind_label(fact.edge_kind).to_string(),
                ),
                ("tier", refined_call_tier_label(fact.tier).to_string()),
                (
                    "validation",
                    refined_call_validation_label(fact.validation).to_string(),
                ),
                (
                    "reason",
                    fact.reason
                        .map(call_unresolved_reason_label)
                        .map(str::to_string)
                        .unwrap_or_else(none_value),
                ),
                (
                    "site_key",
                    self.fact_stable_key(FactFamily::CallSite, fact.site.0),
                ),
                (
                    "base_target_key",
                    fact.base_target
                        .map(|target| self.fact_stable_key(FactFamily::CallTarget, target.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "caller_key",
                    self.fact_stable_key(FactFamily::Function, fact.caller.0),
                ),
                (
                    "target_function_key",
                    fact.target_function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "target_symbol_key",
                    fact.target_symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "synthetic_target",
                    fact.synthetic_target.clone().unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.join("\n")),
                ("inputs", fact.input_stable_keys.join("\n")),
            ]),
        )
    }

    fn data_flow_node_metadata(&self, fact: &DataFlowNodeFact) -> FactMeta {
        let model = fact
            .model
            .and_then(|id| self.data_flow_models.iter().find(|model| model.id == id));
        let (status, data_flow_precision, data_flow_confidence, data_flow_validation, model_key) =
            model.map_or(
                (
                    DataFlowStatus::Present,
                    DataFlowPrecision::Syntax,
                    DataFlowConfidence::High,
                    DataFlowValidation::Native,
                    none_value(),
                ),
                |model| {
                    (
                        model.status,
                        model.precision,
                        model.confidence,
                        model.validation,
                        model.stable_key.clone(),
                    )
                },
            );
        let (precision, status_confidence) = data_flow_status_metadata(status, data_flow_precision);
        let confidence = data_flow_confidence_metadata(data_flow_confidence, status_confidence);
        let validation = data_flow_validation_metadata(data_flow_validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::DataFlowNode,
            DATA_FLOW_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("status", data_flow_status_label(status).to_string()),
                (
                    "precision",
                    data_flow_precision_label(data_flow_precision).to_string(),
                ),
                (
                    "validation",
                    data_flow_validation_label(data_flow_validation).to_string(),
                ),
                ("language", language_label(fact.language).to_string()),
                (
                    "file_key",
                    fact.file
                        .map(|file| self.source_file_key(file))
                        .unwrap_or_else(none_value),
                ),
                (
                    "function_key",
                    fact.function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "symbol_key",
                    fact.symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "reference_key",
                    fact.reference
                        .map(|reference| self.fact_stable_key(FactFamily::Reference, reference.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "call_site_key",
                    fact.call_site
                        .map(|site| self.fact_stable_key(FactFamily::CallSite, site.0))
                        .unwrap_or_else(none_value),
                ),
                ("model_key", model_key),
                ("span", option_span_metadata_value(fact.span.as_ref())),
            ]),
        )
    }

    fn data_flow_edge_metadata(&self, fact: &DataFlowEdgeFact) -> FactMeta {
        let (precision, status_confidence) = data_flow_status_metadata(fact.status, fact.precision);
        let confidence = data_flow_confidence_metadata(fact.confidence, status_confidence);
        let validation = data_flow_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::DataFlowEdge,
            DATA_FLOW_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("algorithm", format!("{:?}", fact.algorithm)),
                ("status", data_flow_status_label(fact.status).to_string()),
                (
                    "precision",
                    data_flow_precision_label(fact.precision).to_string(),
                ),
                (
                    "validation",
                    data_flow_validation_label(fact.validation).to_string(),
                ),
                (
                    "from_key",
                    self.fact_stable_key(FactFamily::DataFlowNode, fact.from.0),
                ),
                (
                    "to_key",
                    self.fact_stable_key(FactFamily::DataFlowNode, fact.to.0),
                ),
                (
                    "call_site_key",
                    fact.call_site
                        .map(|site| self.fact_stable_key(FactFamily::CallSite, site.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "call_target_key",
                    fact.call_target
                        .map(|target| self.fact_stable_key(FactFamily::CallTarget, target.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "refined_call_key",
                    fact.refined_call
                        .map(|edge| self.fact_stable_key(FactFamily::RefinedCallEdge, edge.0))
                        .unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.join("\n")),
                ("inputs", fact.input_stable_keys.join("\n")),
            ]),
        )
    }

    fn data_flow_model_metadata(&self, fact: &DataFlowModelFact) -> FactMeta {
        let (precision, status_confidence) = data_flow_status_metadata(fact.status, fact.precision);
        let confidence = data_flow_confidence_metadata(fact.confidence, status_confidence);
        let validation = data_flow_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::DataFlowModel,
            DATA_FLOW_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("language", language_label(fact.language).to_string()),
                ("provider_id", fact.provider_id.clone()),
                ("model_id", fact.model_id.clone().unwrap_or_else(none_value)),
                (
                    "source_key",
                    fact.source_stable_key.clone().unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.join("\n")),
                ("payload_labels", fact.payload_labels.join("\n")),
            ]),
        )
    }

    fn data_flow_budget_metadata(&self, fact: &DataFlowBudgetFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::DataFlowBudget,
            DATA_FLOW_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("status", data_flow_status_label(fact.status).to_string()),
                ("limit", fact.limit.to_string()),
                ("observed", fact.observed.to_string()),
            ]),
        )
    }

    fn evidence_node_metadata(&self, fact: &EvidenceNodeFact) -> FactMeta {
        let (precision, status_confidence) = evidence_status_metadata(fact.status, fact.precision);
        let confidence = evidence_confidence_metadata(fact.confidence, status_confidence);
        let validation = evidence_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::EvidenceNode,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("status", evidence_status_label(fact.status).to_string()),
                (
                    "precision",
                    evidence_precision_label(fact.precision).to_string(),
                ),
                (
                    "provenance",
                    evidence_provenance_label(fact.provenance).to_string(),
                ),
                (
                    "validation",
                    evidence_validation_label(fact.validation).to_string(),
                ),
                ("language", language_label(fact.language).to_string()),
                (
                    "file_key",
                    fact.file
                        .map(|file| self.source_file_key(file))
                        .unwrap_or_else(none_value),
                ),
                (
                    "function_key",
                    fact.function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                ("span", option_span_metadata_value(fact.span.as_ref())),
                ("sources", fact.source_fact_stable_keys.join("\n")),
            ]),
        )
    }

    fn evidence_edge_metadata(&self, fact: &EvidenceEdgeFact) -> FactMeta {
        let (precision, status_confidence) = evidence_status_metadata(fact.status, fact.precision);
        let confidence = evidence_confidence_metadata(fact.confidence, status_confidence);
        let validation = evidence_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::EvidenceEdge,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                (
                    "precision",
                    evidence_precision_label(fact.precision).to_string(),
                ),
                (
                    "provenance",
                    evidence_provenance_label(fact.provenance).to_string(),
                ),
                (
                    "validation",
                    evidence_validation_label(fact.validation).to_string(),
                ),
                (
                    "from_key",
                    self.fact_stable_key(FactFamily::EvidenceNode, fact.from.0),
                ),
                (
                    "to_key",
                    self.fact_stable_key(FactFamily::EvidenceNode, fact.to.0),
                ),
                (
                    "call_site_key",
                    fact.call_site
                        .map(|site| self.fact_stable_key(FactFamily::CallSite, site.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "summary_key",
                    fact.summary_stable_key.clone().unwrap_or_else(none_value),
                ),
                ("sources", fact.source_fact_stable_keys.join("\n")),
            ]),
        )
    }

    fn evidence_bundle_metadata(&self, fact: &EvidenceBundleFact) -> FactMeta {
        let (precision, status_confidence) = evidence_status_metadata(fact.status, fact.precision);
        let confidence = evidence_confidence_metadata(fact.confidence, status_confidence);
        let validation = evidence_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::EvidenceBundle,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("diagnostic_key", fact.diagnostic_stable_key.clone()),
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                (
                    "precision",
                    evidence_precision_label(fact.precision).to_string(),
                ),
                ("selected_paths", fact.selected_paths.len().to_string()),
                ("selected_slices", fact.selected_slices.len().to_string()),
                (
                    "replay_key",
                    fact.replay_key.clone().unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn evidence_path_metadata(&self, fact: &EvidencePathFact) -> FactMeta {
        let (precision, confidence) =
            evidence_status_metadata(fact.status, EvidencePrecision::Heuristic);
        fact_meta_from_stable_key(
            FactFamily::EvidencePath,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                ("rank", fact.rank.to_string()),
                ("node_count", fact.nodes.len().to_string()),
                ("edge_count", fact.edges.len().to_string()),
                ("hidden_node_count", fact.hidden_node_count.to_string()),
            ]),
        )
    }

    fn evidence_slice_metadata(&self, fact: &EvidenceSliceFact) -> FactMeta {
        let (precision, confidence) =
            evidence_status_metadata(fact.status, EvidencePrecision::Heuristic);
        fact_meta_from_stable_key(
            FactFamily::EvidenceSlice,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                ("root_count", fact.root_nodes.len().to_string()),
                ("node_count", fact.nodes.len().to_string()),
                ("edge_count", fact.edges.len().to_string()),
            ]),
        )
    }

    fn evidence_unknown_metadata(&self, fact: &EvidenceUnknownFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::EvidenceUnknown,
            EVIDENCE_PROVIDER_ID,
            FactPrecision::Unresolved,
            FactConfidence::Low,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("message", fact.message.clone()),
                ("sources", fact.source_fact_stable_keys.join("\n")),
            ]),
        )
    }

    fn evidence_omitted_region_metadata(&self, fact: &EvidenceOmittedRegionFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::EvidenceOmittedRegion,
            EVIDENCE_PROVIDER_ID,
            FactPrecision::Unresolved,
            FactConfidence::Low,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("hidden_node_count", fact.hidden_node_count.to_string()),
                ("hidden_edge_count", fact.hidden_edge_count.to_string()),
                (
                    "budget_label",
                    fact.budget_label.clone().unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn evidence_replay_key_metadata(&self, fact: &EvidenceReplayKeyFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::EvidenceReplayKey,
            EVIDENCE_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("graph_schema", fact.graph_schema.clone()),
                ("max_paths", fact.query_budget.max_paths.to_string()),
                ("max_nodes", fact.query_budget.max_nodes.to_string()),
                ("max_edges", fact.query_budget.max_edges.to_string()),
                ("max_depth", fact.query_budget.max_depth.to_string()),
                ("ranking", format!("{:?}", fact.ranking)),
                ("renderer", format!("{:?}", fact.renderer)),
                ("upstream", fact.upstream_digest_keys.join("\n")),
            ]),
        )
    }

    fn unresolved_call_metadata(&self, fact: &UnresolvedCallFact) -> FactMeta {
        let (precision, confidence) = call_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::UnresolvedCall,
            CALLS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                (
                    "algorithm",
                    call_algorithm_label(fact.algorithm).to_string(),
                ),
                (
                    "reason",
                    call_unresolved_reason_label(fact.reason).to_string(),
                ),
                (
                    "site_key",
                    self.fact_stable_key(FactFamily::CallSite, fact.site.0),
                ),
                (
                    "caller_key",
                    self.fact_stable_key(FactFamily::Function, fact.caller.0),
                ),
            ]),
        )
    }

    fn domain_observation_metadata(&self, fact: &DomainObservationFact) -> FactMeta {
        let (precision, confidence) = domain_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::DomainObservation,
            POLINT_ABSTRACT_DOMAINS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", fact.status.as_str().to_string()),
                ("precision", fact.precision.as_str().to_string()),
                ("slot", fact.slot.as_str().to_string()),
                ("location", fact.location.as_str().to_string()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                (
                    "block",
                    fact.block
                        .map(|block| block.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                ("value", fact.value.stable_parts().join("\n")),
            ]),
        )
    }

    fn domain_event_metadata(&self, fact: &DomainEventFact) -> FactMeta {
        let (precision, confidence) = domain_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::DomainEvent,
            POLINT_ABSTRACT_DOMAINS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", fact.status.as_str().to_string()),
                ("precision", fact.precision.as_str().to_string()),
                (
                    "slot",
                    fact.slot
                        .map(|slot| slot.as_str().to_string())
                        .unwrap_or_else(none_value),
                ),
                ("reason", fact.reason.clone()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                (
                    "block",
                    fact.block
                        .map(|block| block.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn cfg_function_metadata(&self, fact: &CfgFunctionFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgFunction,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                ("language", language_label(fact.language).to_string()),
                ("path", self.path_for(fact.file)),
                ("span", span_metadata_value(&fact.span)),
            ]),
        )
    }

    fn cfg_node_metadata(&self, fact: &CfgNodeFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgNode,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("kind", cfg_node_kind_label(fact.kind).to_string()),
                (
                    "function_key",
                    self.fact_stable_key(FactFamily::CfgFunction, fact.cfg_function.0),
                ),
                ("operation_ordinal", fact.operation_ordinal.to_string()),
                ("span", option_span_metadata_value(fact.span.as_ref())),
            ]),
        )
    }

    fn cfg_block_metadata(&self, fact: &BasicBlockFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::BasicBlock,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("kind", basic_block_kind_label(fact.kind).to_string()),
                (
                    "function_key",
                    self.fact_stable_key(FactFamily::CfgFunction, fact.cfg_function.0),
                ),
                ("reachable", fact.reachable.to_string()),
                ("reverse_postorder", fact.reverse_postorder.to_string()),
            ]),
        )
    }

    fn cfg_edge_metadata(&self, fact: &CfgEdgeFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgEdge,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("kind", cfg_edge_kind_label(fact.kind).to_string()),
                (
                    "function_key",
                    self.fact_stable_key(FactFamily::CfgFunction, fact.cfg_function.0),
                ),
                ("from_block", fact.from_block.0.to_string()),
                ("to_block", fact.to_block.0.to_string()),
            ]),
        )
    }

    fn cfg_reachability_metadata(&self, fact: &ReachabilityFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgReachability,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("block", fact.block.0.to_string()),
                ("reachable", fact.reachable.to_string()),
            ]),
        )
    }

    fn cfg_dominator_metadata(&self, fact: &DominatorFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgDominator,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("dominator", fact.dominator.0.to_string()),
                ("dominated", fact.dominated.0.to_string()),
                ("immediate", fact.immediate.to_string()),
            ]),
        )
    }

    fn cfg_postdominator_metadata(&self, fact: &PostDominatorFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgPostDominator,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("postdominator", fact.postdominator.0.to_string()),
                ("postdominated", fact.postdominated.0.to_string()),
                ("immediate", fact.immediate.to_string()),
            ]),
        )
    }

    fn cfg_control_dependence_metadata(&self, fact: &ControlDependenceFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgControlDependence,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("edge", fact.controlling_edge.0.to_string()),
                (
                    "edge_kind",
                    cfg_edge_kind_label(fact.controlling_edge_kind).to_string(),
                ),
                ("controlled_block", fact.controlled_block.0.to_string()),
            ]),
        )
    }

    fn unsupported_control_flow_metadata(&self, fact: &UnsupportedControlFlowFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::UnsupportedControlFlow,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("language", language_label(fact.language).to_string()),
                ("path", self.path_for(fact.file)),
                ("span", span_metadata_value(&fact.span)),
                ("construct", fact.construct.clone()),
                ("source_evidence", fact.source_evidence.clone()),
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

    fn option_source_file_key(&self, file: Option<FileId>) -> String {
        file.map(|file| self.source_file_key(file))
            .unwrap_or_else(none_value)
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

#[allow(
    dead_code,
    reason = "Extension fact metadata is reached through extension provider wiring in the next Phase 34 plan."
)]
fn extension_fact_metadata(fact: &AcceptedExtensionFact) -> FactMeta {
    let producer_id = leaked_extension_producer_id(&fact.extension_id, &fact.provider_id);
    let precision = extension_precision_metadata(fact.precision);
    let confidence = extension_confidence_metadata(fact.confidence);
    let payload_extra_parts = [
        ("family", fact.fact_family.clone()),
        ("bindings", fact.binding_refs.join(",")),
        ("evidence", fact.evidence.join(",")),
        ("payload", fact.payload_labels.join(",")),
        ("status", format!("{:?}", fact.status)),
    ];
    let payload_digest = metadata_payload_digest(&fact.stable_key, &payload_extra_parts);

    FactMeta {
        stable_key: fact.stable_key.clone(),
        producer_id,
        layer_id: producer_id,
        precision,
        confidence,
        validation: ValidationStatus::SchemaValidated,
        payload_digest,
    }
}

fn adaptation_model_fact_metadata(fact: &AcceptedModelFact) -> FactMeta {
    FactMeta {
        stable_key: fact.fact.stable_key.clone(),
        producer_id: "polint.adaptation.model",
        layer_id: "polint.adaptation.model",
        precision: FactPrecision::Heuristic,
        confidence: FactConfidence::Medium,
        validation: ValidationStatus::SchemaValidated,
        payload_digest: metadata_payload_digest(
            &fact.fact.stable_key,
            &[
                ("model_path", fact.fact.model_path.clone()),
                ("source_pattern", fact.fact.source_pattern.clone()),
                ("target_pattern", fact.fact.target_pattern.clone()),
                ("confidence", fact.fact.confidence.as_str().to_string()),
                ("language", fact.fact.language.as_str().to_string()),
                ("scope", fact.fact.scope.clone()),
                ("evidence", fact.fact.evidence.join(",")),
            ],
        ),
    }
}

#[allow(
    dead_code,
    reason = "Extension producer ids are reached through extension provider wiring in the next Phase 34 plan."
)]
fn leaked_extension_producer_id(extension_id: &str, provider_id: &str) -> &'static str {
    Box::leak(format!("polint.extension.{extension_id}.{provider_id}").into_boxed_str())
}

#[allow(
    dead_code,
    reason = "Extension precision mapping is reached through extension provider wiring in the next Phase 34 plan."
)]
fn extension_precision_metadata(precision: ExtensionFactPrecision) -> FactPrecision {
    match precision {
        ExtensionFactPrecision::Exact => FactPrecision::Exact,
        ExtensionFactPrecision::SetupAware => FactPrecision::SetupAware,
        ExtensionFactPrecision::Heuristic => FactPrecision::Heuristic,
        ExtensionFactPrecision::GeneratedUnvalidated => FactPrecision::Heuristic,
    }
}

#[allow(
    dead_code,
    reason = "Extension confidence mapping is reached through extension provider wiring in the next Phase 34 plan."
)]
fn extension_confidence_metadata(confidence: ExtensionFactConfidence) -> FactConfidence {
    match confidence {
        ExtensionFactConfidence::High => FactConfidence::High,
        ExtensionFactConfidence::Medium => FactConfidence::Medium,
        ExtensionFactConfidence::Low => FactConfidence::Low,
    }
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

fn fact_meta_from_stable_key_with_validation<const EXTRA: usize>(
    _family: FactFamily,
    producer_id: &'static str,
    precision: FactPrecision,
    confidence: FactConfidence,
    validation: ValidationStatus,
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
        validation,
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

fn cfg_status_metadata(
    status: CfgStatus,
    precision: CfgPrecision,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match (status, precision) {
        (CfgStatus::Resolved, CfgPrecision::ExactSyntax) => FactPrecision::Syntax,
        (CfgStatus::Resolved, CfgPrecision::ExactLowered | CfgPrecision::SetupAware) => {
            FactPrecision::SetupAware
        }
        (_, CfgPrecision::Conservative | CfgPrecision::Heuristic) => FactPrecision::Heuristic,
        (CfgStatus::Partial, _) => FactPrecision::Heuristic,
        (CfgStatus::Unknown, _) | (_, CfgPrecision::Unknown) => FactPrecision::Unresolved,
        (CfgStatus::Unsupported, _) | (_, CfgPrecision::Unsupported) => FactPrecision::Unsupported,
    };
    let confidence = match status {
        CfgStatus::Resolved => FactConfidence::High,
        CfgStatus::Partial => FactConfidence::Medium,
        CfgStatus::Unknown | CfgStatus::Unsupported => FactConfidence::Low,
    };
    (fact_precision, confidence)
}

fn call_status_metadata(
    status: CallTargetStatus,
    precision: CallPrecision,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match status {
        CallTargetStatus::Resolved => match precision {
            CallPrecision::Exact | CallPrecision::SetupAware => FactPrecision::SetupAware,
            CallPrecision::Conservative | CallPrecision::Heuristic => FactPrecision::Heuristic,
            CallPrecision::Ambiguous => FactPrecision::Ambiguous,
            CallPrecision::Unknown => FactPrecision::Unresolved,
            CallPrecision::Unsupported => FactPrecision::Unsupported,
        },
        CallTargetStatus::Ambiguous => FactPrecision::Ambiguous,
        CallTargetStatus::Unresolved | CallTargetStatus::BudgetExceeded => {
            FactPrecision::Unresolved
        }
        CallTargetStatus::Unsupported | CallTargetStatus::Rejected => FactPrecision::Unsupported,
        CallTargetStatus::SetupMissing => FactPrecision::SetupMissing,
    };
    let confidence = match status {
        CallTargetStatus::Resolved => FactConfidence::High,
        CallTargetStatus::Ambiguous => FactConfidence::Medium,
        CallTargetStatus::Unresolved
        | CallTargetStatus::Unsupported
        | CallTargetStatus::SetupMissing
        | CallTargetStatus::BudgetExceeded
        | CallTargetStatus::Rejected => FactConfidence::Low,
    };
    (fact_precision, confidence)
}

fn domain_status_metadata(
    status: DomainStatus,
    precision: DomainPrecision,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match status {
        DomainStatus::Present => match precision {
            DomainPrecision::ExactLocal => FactPrecision::SetupAware,
            DomainPrecision::SetupAware => FactPrecision::SetupAware,
            DomainPrecision::Conservative | DomainPrecision::Heuristic => FactPrecision::Heuristic,
            DomainPrecision::Unknown => FactPrecision::Unresolved,
            DomainPrecision::Unsupported => FactPrecision::Unsupported,
        },
        DomainStatus::Top => FactPrecision::Ambiguous,
        DomainStatus::Unknown | DomainStatus::BudgetExceeded => FactPrecision::Unresolved,
        DomainStatus::Unsupported => FactPrecision::Unsupported,
        DomainStatus::SetupMissing => FactPrecision::SetupMissing,
    };
    let confidence = match status {
        DomainStatus::Present => match precision {
            DomainPrecision::ExactLocal | DomainPrecision::SetupAware => FactConfidence::High,
            DomainPrecision::Conservative | DomainPrecision::Heuristic => FactConfidence::Medium,
            DomainPrecision::Unknown | DomainPrecision::Unsupported => FactConfidence::Low,
        },
        DomainStatus::Top => FactConfidence::Medium,
        DomainStatus::Unknown
        | DomainStatus::Unsupported
        | DomainStatus::SetupMissing
        | DomainStatus::BudgetExceeded => FactConfidence::Low,
    };
    (fact_precision, confidence)
}

fn entrypoint_precision_metadata(
    status: EntrypointStatus,
    precision: EntrypointPrecision,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match status {
        EntrypointStatus::Resolved => match precision {
            EntrypointPrecision::ResolvedStatic | EntrypointPrecision::SetupAware => {
                FactPrecision::SetupAware
            }
            EntrypointPrecision::Heuristic | EntrypointPrecision::Conservative => {
                FactPrecision::Heuristic
            }
            EntrypointPrecision::Unknown => FactPrecision::Unresolved,
        },
        EntrypointStatus::Partial => FactPrecision::Ambiguous,
        EntrypointStatus::Unresolved => FactPrecision::Unresolved,
        EntrypointStatus::SetupMissing => FactPrecision::SetupMissing,
        EntrypointStatus::Unsupported => FactPrecision::Unsupported,
    };
    let confidence = match status {
        EntrypointStatus::Resolved => match precision {
            EntrypointPrecision::ResolvedStatic | EntrypointPrecision::SetupAware => {
                FactConfidence::High
            }
            EntrypointPrecision::Heuristic | EntrypointPrecision::Conservative => {
                FactConfidence::Medium
            }
            EntrypointPrecision::Unknown => FactConfidence::Low,
        },
        EntrypointStatus::Partial => FactConfidence::Medium,
        EntrypointStatus::Unresolved
        | EntrypointStatus::SetupMissing
        | EntrypointStatus::Unsupported => FactConfidence::Low,
    };
    (fact_precision, confidence)
}

fn type_metadata_precision(
    status: TypeStatus,
    precision: TypePrecision,
    confidence: Option<TypeConfidence>,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match status {
        TypeStatus::SetupMissing => FactPrecision::SetupMissing,
        TypeStatus::Unsupported => FactPrecision::Unsupported,
        TypeStatus::Unknown => FactPrecision::Unresolved,
        TypeStatus::BudgetExceeded => FactPrecision::Heuristic,
        TypeStatus::Present => match precision {
            TypePrecision::ExactLocal => FactPrecision::Exact,
            TypePrecision::SetupAware => FactPrecision::SetupAware,
            TypePrecision::Conservative | TypePrecision::Heuristic => FactPrecision::Heuristic,
            TypePrecision::Unknown => FactPrecision::Unresolved,
            TypePrecision::Unsupported => FactPrecision::Unsupported,
        },
    };
    let confidence = match confidence.unwrap_or(TypeConfidence::Medium) {
        TypeConfidence::High => FactConfidence::High,
        TypeConfidence::Medium => FactConfidence::Medium,
        TypeConfidence::Low => FactConfidence::Low,
    };
    (fact_precision, confidence)
}

fn value_metadata_precision(
    status: ValueStatus,
    precision: ValuePrecision,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match status {
        ValueStatus::SetupMissing => FactPrecision::SetupMissing,
        ValueStatus::Unsupported => FactPrecision::Unsupported,
        ValueStatus::Unknown => FactPrecision::Unresolved,
        ValueStatus::BudgetExceeded => FactPrecision::Heuristic,
        ValueStatus::Present => match precision {
            ValuePrecision::ExactLocal => FactPrecision::SetupAware,
            ValuePrecision::SetupAware => FactPrecision::SetupAware,
            ValuePrecision::Conservative | ValuePrecision::Heuristic => FactPrecision::Heuristic,
            ValuePrecision::Unknown => FactPrecision::Unresolved,
            ValuePrecision::Unsupported => FactPrecision::Unsupported,
        },
    };
    (fact_precision, FactConfidence::Medium)
}

fn points_to_metadata_precision(
    status: PointsToStatus,
    precision: PointsToPrecision,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match status {
        PointsToStatus::SetupMissing => FactPrecision::SetupMissing,
        PointsToStatus::Unsupported => FactPrecision::Unsupported,
        PointsToStatus::Unknown => FactPrecision::Unresolved,
        PointsToStatus::BudgetExceeded => FactPrecision::Heuristic,
        PointsToStatus::Present => match precision {
            PointsToPrecision::LocalFlowSensitive => FactPrecision::SetupAware,
            PointsToPrecision::FlowInsensitive
            | PointsToPrecision::SummaryProjected
            | PointsToPrecision::Heuristic => FactPrecision::Heuristic,
            PointsToPrecision::Unknown => FactPrecision::Unresolved,
            PointsToPrecision::Unsupported => FactPrecision::Unsupported,
        },
    };
    (fact_precision, FactConfidence::Medium)
}

fn alias_metadata_precision(
    status: AliasStatus,
    precision: AliasPrecision,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match status {
        AliasStatus::NoAlias | AliasStatus::MustAlias => match precision {
            AliasPrecision::ExactLocal
            | AliasPrecision::FlowInsensitive
            | AliasPrecision::SetupAware
            | AliasPrecision::Conservative => FactPrecision::SetupAware,
            AliasPrecision::Heuristic => FactPrecision::Heuristic,
            AliasPrecision::Unknown => FactPrecision::Unresolved,
            AliasPrecision::Unsupported => FactPrecision::Unsupported,
        },
        AliasStatus::MayAlias | AliasStatus::PartialAlias => FactPrecision::Ambiguous,
        AliasStatus::Unknown => FactPrecision::Unresolved,
    };
    let confidence = match status {
        AliasStatus::NoAlias | AliasStatus::MustAlias => FactConfidence::High,
        AliasStatus::MayAlias | AliasStatus::PartialAlias => FactConfidence::Medium,
        AliasStatus::Unknown => FactConfidence::Low,
    };
    (fact_precision, confidence)
}

fn summary_domain_to_fact_family(domain: SummaryDomainKind) -> FactFamily {
    match domain {
        SummaryDomainKind::ControlEffects => FactFamily::SummaryControl,
        SummaryDomainKind::CallEffects => FactFamily::SummaryCall,
        SummaryDomainKind::MemoryEffects => FactFamily::SummaryMemory,
        SummaryDomainKind::DataFlowTito => FactFamily::SummaryTito,
    }
}

fn summary_precision_metadata(
    status: SummaryStatus,
    precision: SummaryPrecision,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match status {
        SummaryStatus::Present => match precision {
            SummaryPrecision::Local | SummaryPrecision::SetupAware => FactPrecision::SetupAware,
            SummaryPrecision::Heuristic => FactPrecision::Heuristic,
            SummaryPrecision::UnknownTop => FactPrecision::Unresolved,
        },
        SummaryStatus::Unknown | SummaryStatus::BudgetExceeded => FactPrecision::Unresolved,
        SummaryStatus::Unsupported => FactPrecision::Unsupported,
        SummaryStatus::SetupMissing => FactPrecision::SetupMissing,
    };
    let confidence = match status {
        SummaryStatus::Present => match precision {
            SummaryPrecision::Local | SummaryPrecision::SetupAware => FactConfidence::High,
            SummaryPrecision::Heuristic => FactConfidence::Medium,
            SummaryPrecision::UnknownTop => FactConfidence::Low,
        },
        SummaryStatus::Unknown
        | SummaryStatus::Unsupported
        | SummaryStatus::SetupMissing
        | SummaryStatus::BudgetExceeded => FactConfidence::Low,
    };
    (fact_precision, confidence)
}

fn call_status_label(status: CallTargetStatus) -> &'static str {
    match status {
        CallTargetStatus::Resolved => "resolved",
        CallTargetStatus::Ambiguous => "ambiguous",
        CallTargetStatus::Unresolved => "unresolved",
        CallTargetStatus::Unsupported => "unsupported",
        CallTargetStatus::SetupMissing => "setup_missing",
        CallTargetStatus::BudgetExceeded => "budget_exceeded",
        CallTargetStatus::Rejected => "rejected",
    }
}

fn call_precision_label(precision: CallPrecision) -> &'static str {
    match precision {
        CallPrecision::Exact => "exact",
        CallPrecision::SetupAware => "setup_aware",
        CallPrecision::Conservative => "conservative",
        CallPrecision::Heuristic => "heuristic",
        CallPrecision::Ambiguous => "ambiguous",
        CallPrecision::Unknown => "unknown",
        CallPrecision::Unsupported => "unsupported",
    }
}

fn call_syntax_kind_label(kind: CallSyntaxKind) -> &'static str {
    match kind {
        CallSyntaxKind::Function => "function",
        CallSyntaxKind::Method => "method",
        CallSyntaxKind::Constructor => "constructor",
        CallSyntaxKind::StaticMember => "static_member",
        CallSyntaxKind::Member => "member",
        CallSyntaxKind::Index => "index",
        CallSyntaxKind::Super => "super",
        CallSyntaxKind::Import => "import",
        CallSyntaxKind::New => "new",
        CallSyntaxKind::TaggedTemplate => "tagged_template",
        CallSyntaxKind::GoRoutine => "go_routine",
        CallSyntaxKind::Deferred => "deferred",
        CallSyntaxKind::DynamicImport => "dynamic_import",
        CallSyntaxKind::Require => "require",
        CallSyntaxKind::FunctionValue => "function_value",
        CallSyntaxKind::Unknown => "unknown",
    }
}

fn call_edge_kind_label(kind: CallEdgeKind) -> &'static str {
    match kind {
        CallEdgeKind::Direct => "direct",
        CallEdgeKind::Constructor => "constructor",
        CallEdgeKind::StaticMember => "static_member",
        CallEdgeKind::MethodDirect => "method_direct",
        CallEdgeKind::Method => "method",
        CallEdgeKind::FunctionValue => "function_value",
        CallEdgeKind::Synthetic => "synthetic",
        CallEdgeKind::Spawn => "spawn",
        CallEdgeKind::Deferred => "deferred",
        CallEdgeKind::Unknown => "unknown",
    }
}

fn call_algorithm_label(algorithm: CallAlgorithm) -> &'static str {
    match algorithm {
        CallAlgorithm::SyntaxOnly => "syntax_only",
        CallAlgorithm::DirectReference => "direct_reference",
        CallAlgorithm::ImportBinding => "import_binding",
        CallAlgorithm::ConstructorBinding => "constructor_binding",
        CallAlgorithm::StaticMember => "static_member",
        CallAlgorithm::DirectMember => "direct_member",
        CallAlgorithm::GoStatic => "go_static",
        CallAlgorithm::GoCha => "go_cha",
        CallAlgorithm::GoRta => "go_rta",
        CallAlgorithm::GoVta => "go_vta",
        CallAlgorithm::FunctionTokenFlow => "function_token_flow",
        CallAlgorithm::ThisMethodFlow => "this_method_flow",
        CallAlgorithm::TypeHierarchy => "type_hierarchy",
        CallAlgorithm::PointsTo => "points_to",
        CallAlgorithm::SummaryAssisted => "summary_assisted",
        CallAlgorithm::FrameworkModel => "framework_model",
        CallAlgorithm::RepoModel => "repo_model",
        CallAlgorithm::Unsupported => "unsupported",
    }
}

fn call_unresolved_reason_label(reason: UnresolvedCallReason) -> &'static str {
    match reason {
        UnresolvedCallReason::FunctionValue => "function_value",
        UnresolvedCallReason::DynamicProperty => "dynamic_property",
        UnresolvedCallReason::InterfaceDispatch => "interface_dispatch",
        UnresolvedCallReason::Eval => "eval",
        UnresolvedCallReason::CallApplyBind => "call_apply_bind",
        UnresolvedCallReason::FrameworkDispatch => "framework_dispatch",
        UnresolvedCallReason::Reflection => "reflection",
        UnresolvedCallReason::GoroutineBoundary => "goroutine_boundary",
        UnresolvedCallReason::DynamicImport => "dynamic_import",
        UnresolvedCallReason::ProxyOrAccessor => "proxy_or_accessor",
        UnresolvedCallReason::MissingSemanticReference => "missing_semantic_reference",
        UnresolvedCallReason::MissingImportResolution => "missing_import_resolution",
        UnresolvedCallReason::SetupMissing => "setup_missing",
        UnresolvedCallReason::UnsupportedSyntax => "unsupported_syntax",
        UnresolvedCallReason::BudgetExceeded => "budget_exceeded",
        UnresolvedCallReason::UnknownCallee => "unknown_callee",
        UnresolvedCallReason::Unknown => "unknown",
    }
}

fn refined_call_tier_label(tier: RefinedCallTier) -> &'static str {
    match tier {
        RefinedCallTier::DirectOnly => "direct_only",
        RefinedCallTier::DirectPlusFramework => "direct_plus_framework",
        RefinedCallTier::TypeValueFunctionToken => "type_value_function_token",
        RefinedCallTier::SummaryAssisted => "summary_assisted",
        RefinedCallTier::PointsToAssisted => "points_to_assisted",
        RefinedCallTier::ExtensionModel => "extension_model",
        RefinedCallTier::AllAccepted => "all_accepted",
    }
}

fn refined_call_validation_label(validation: RefinedCallValidation) -> &'static str {
    match validation {
        RefinedCallValidation::Native => "native",
        RefinedCallValidation::ReferentiallyValidated => "referentially_validated",
        RefinedCallValidation::ExtensionValidated => "extension_validated",
        RefinedCallValidation::Rejected => "rejected",
    }
}

fn refined_call_validation_metadata(validation: RefinedCallValidation) -> ValidationStatus {
    match validation {
        RefinedCallValidation::Native => ValidationStatus::NativeTrusted,
        RefinedCallValidation::ReferentiallyValidated => ValidationStatus::ReferentiallyValidated,
        RefinedCallValidation::ExtensionValidated => ValidationStatus::SchemaValidated,
        RefinedCallValidation::Rejected => ValidationStatus::ConflictRejected,
    }
}

fn refined_call_confidence_metadata(
    confidence: RefinedCallConfidence,
    fallback: FactConfidence,
) -> FactConfidence {
    let requested = match confidence {
        RefinedCallConfidence::High => FactConfidence::High,
        RefinedCallConfidence::Medium => FactConfidence::Medium,
        RefinedCallConfidence::Low => FactConfidence::Low,
    };
    match (requested, fallback) {
        (FactConfidence::Low, _) | (_, FactConfidence::Low) => FactConfidence::Low,
        (FactConfidence::Medium, _) | (_, FactConfidence::Medium) => FactConfidence::Medium,
        (FactConfidence::High, FactConfidence::High) => FactConfidence::High,
    }
}

fn data_flow_status_metadata(
    status: DataFlowStatus,
    precision: DataFlowPrecision,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match status {
        DataFlowStatus::Present => match precision {
            DataFlowPrecision::Exact => FactPrecision::Exact,
            DataFlowPrecision::SetupAware => FactPrecision::SetupAware,
            DataFlowPrecision::Syntax => FactPrecision::Syntax,
            DataFlowPrecision::Conservative | DataFlowPrecision::Heuristic => {
                FactPrecision::Heuristic
            }
            DataFlowPrecision::Unknown => FactPrecision::Unresolved,
        },
        DataFlowStatus::Unknown | DataFlowStatus::BudgetExceeded => FactPrecision::Unresolved,
        DataFlowStatus::Unsupported | DataFlowStatus::Rejected => FactPrecision::Unsupported,
        DataFlowStatus::SetupMissing => FactPrecision::SetupMissing,
    };
    let confidence = match status {
        DataFlowStatus::Present => match precision {
            DataFlowPrecision::Exact
            | DataFlowPrecision::SetupAware
            | DataFlowPrecision::Syntax => FactConfidence::High,
            DataFlowPrecision::Conservative | DataFlowPrecision::Heuristic => {
                FactConfidence::Medium
            }
            DataFlowPrecision::Unknown => FactConfidence::Low,
        },
        DataFlowStatus::Unknown
        | DataFlowStatus::Unsupported
        | DataFlowStatus::SetupMissing
        | DataFlowStatus::BudgetExceeded
        | DataFlowStatus::Rejected => FactConfidence::Low,
    };
    (fact_precision, confidence)
}

fn data_flow_confidence_metadata(
    confidence: DataFlowConfidence,
    fallback: FactConfidence,
) -> FactConfidence {
    let requested = match confidence {
        DataFlowConfidence::High => FactConfidence::High,
        DataFlowConfidence::Medium => FactConfidence::Medium,
        DataFlowConfidence::Low => FactConfidence::Low,
    };
    match (requested, fallback) {
        (FactConfidence::Low, _) | (_, FactConfidence::Low) => FactConfidence::Low,
        (FactConfidence::Medium, _) | (_, FactConfidence::Medium) => FactConfidence::Medium,
        (FactConfidence::High, FactConfidence::High) => FactConfidence::High,
    }
}

fn data_flow_validation_metadata(validation: DataFlowValidation) -> ValidationStatus {
    match validation {
        DataFlowValidation::Native => ValidationStatus::NativeTrusted,
        DataFlowValidation::ReferentiallyValidated => ValidationStatus::ReferentiallyValidated,
        DataFlowValidation::ExtensionValidated => ValidationStatus::SchemaValidated,
        DataFlowValidation::BudgetValidated => ValidationStatus::StableKeyValidated,
        DataFlowValidation::Rejected => ValidationStatus::ConflictRejected,
    }
}

fn data_flow_status_label(status: DataFlowStatus) -> &'static str {
    match status {
        DataFlowStatus::Present => "present",
        DataFlowStatus::Unknown => "unknown",
        DataFlowStatus::Unsupported => "unsupported",
        DataFlowStatus::SetupMissing => "setup_missing",
        DataFlowStatus::BudgetExceeded => "budget_exceeded",
        DataFlowStatus::Rejected => "rejected",
    }
}

fn data_flow_precision_label(precision: DataFlowPrecision) -> &'static str {
    match precision {
        DataFlowPrecision::Exact => "exact",
        DataFlowPrecision::SetupAware => "setup_aware",
        DataFlowPrecision::Syntax => "syntax",
        DataFlowPrecision::Conservative => "conservative",
        DataFlowPrecision::Heuristic => "heuristic",
        DataFlowPrecision::Unknown => "unknown",
    }
}

fn data_flow_validation_label(validation: DataFlowValidation) -> &'static str {
    match validation {
        DataFlowValidation::Native => "native",
        DataFlowValidation::ReferentiallyValidated => "referentially_validated",
        DataFlowValidation::ExtensionValidated => "extension_validated",
        DataFlowValidation::BudgetValidated => "budget_validated",
        DataFlowValidation::Rejected => "rejected",
    }
}

fn evidence_status_metadata(
    status: EvidenceStatus,
    precision: EvidencePrecision,
) -> (FactPrecision, FactConfidence) {
    let fact_precision = match status {
        EvidenceStatus::Present => match precision {
            EvidencePrecision::Exact => FactPrecision::SetupAware,
            EvidencePrecision::SetupAware => FactPrecision::SetupAware,
            EvidencePrecision::Syntax => FactPrecision::Syntax,
            EvidencePrecision::Conservative | EvidencePrecision::Heuristic => {
                FactPrecision::Heuristic
            }
            EvidencePrecision::Unknown => FactPrecision::Unresolved,
        },
        EvidenceStatus::Partial | EvidenceStatus::Unknown | EvidenceStatus::BudgetExceeded => {
            FactPrecision::Unresolved
        }
        EvidenceStatus::Unsupported | EvidenceStatus::Rejected => FactPrecision::Unsupported,
        EvidenceStatus::SetupMissing => FactPrecision::SetupMissing,
    };
    let confidence = match status {
        EvidenceStatus::Present => match precision {
            EvidencePrecision::Exact
            | EvidencePrecision::SetupAware
            | EvidencePrecision::Syntax => FactConfidence::High,
            EvidencePrecision::Conservative | EvidencePrecision::Heuristic => {
                FactConfidence::Medium
            }
            EvidencePrecision::Unknown => FactConfidence::Low,
        },
        EvidenceStatus::Partial => FactConfidence::Medium,
        EvidenceStatus::Unknown
        | EvidenceStatus::Unsupported
        | EvidenceStatus::SetupMissing
        | EvidenceStatus::BudgetExceeded
        | EvidenceStatus::Rejected => FactConfidence::Low,
    };
    (fact_precision, confidence)
}

fn evidence_confidence_metadata(
    confidence: EvidenceConfidence,
    fallback: FactConfidence,
) -> FactConfidence {
    let requested = match confidence {
        EvidenceConfidence::High => FactConfidence::High,
        EvidenceConfidence::Medium => FactConfidence::Medium,
        EvidenceConfidence::Low => FactConfidence::Low,
    };
    match (requested, fallback) {
        (FactConfidence::Low, _) | (_, FactConfidence::Low) => FactConfidence::Low,
        (FactConfidence::Medium, _) | (_, FactConfidence::Medium) => FactConfidence::Medium,
        (FactConfidence::High, FactConfidence::High) => FactConfidence::High,
    }
}

fn evidence_validation_metadata(validation: EvidenceValidation) -> ValidationStatus {
    match validation {
        EvidenceValidation::Native => ValidationStatus::NativeTrusted,
        EvidenceValidation::ReferentiallyValidated => ValidationStatus::ReferentiallyValidated,
        EvidenceValidation::ExtensionValidated => ValidationStatus::SchemaValidated,
        EvidenceValidation::BudgetValidated | EvidenceValidation::RendererValidated => {
            ValidationStatus::StableKeyValidated
        }
        EvidenceValidation::Rejected => ValidationStatus::ConflictRejected,
    }
}

fn evidence_status_label(status: EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Present => "present",
        EvidenceStatus::Partial => "partial",
        EvidenceStatus::Unknown => "unknown",
        EvidenceStatus::Unsupported => "unsupported",
        EvidenceStatus::SetupMissing => "setup_missing",
        EvidenceStatus::BudgetExceeded => "budget_exceeded",
        EvidenceStatus::Rejected => "rejected",
    }
}

fn evidence_precision_label(precision: EvidencePrecision) -> &'static str {
    match precision {
        EvidencePrecision::Exact => "exact",
        EvidencePrecision::SetupAware => "setup_aware",
        EvidencePrecision::Syntax => "syntax",
        EvidencePrecision::Conservative => "conservative",
        EvidencePrecision::Heuristic => "heuristic",
        EvidencePrecision::Unknown => "unknown",
    }
}

fn evidence_provenance_label(provenance: EvidenceProvenance) -> &'static str {
    match provenance {
        EvidenceProvenance::Native => "native",
        EvidenceProvenance::Summary => "summary",
        EvidenceProvenance::Extension => "extension",
        EvidenceProvenance::Model => "model",
        EvidenceProvenance::Query => "query",
        EvidenceProvenance::Synthetic => "synthetic",
    }
}

fn evidence_validation_label(validation: EvidenceValidation) -> &'static str {
    match validation {
        EvidenceValidation::Native => "native",
        EvidenceValidation::ReferentiallyValidated => "referentially_validated",
        EvidenceValidation::ExtensionValidated => "extension_validated",
        EvidenceValidation::BudgetValidated => "budget_validated",
        EvidenceValidation::RendererValidated => "renderer_validated",
        EvidenceValidation::Rejected => "rejected",
    }
}

fn cfg_status_label(status: CfgStatus) -> &'static str {
    match status {
        CfgStatus::Resolved => "resolved",
        CfgStatus::Partial => "partial",
        CfgStatus::Unknown => "unknown",
        CfgStatus::Unsupported => "unsupported",
    }
}

fn cfg_precision_label(precision: CfgPrecision) -> &'static str {
    match precision {
        CfgPrecision::ExactSyntax => "exact_syntax",
        CfgPrecision::ExactLowered => "exact_lowered",
        CfgPrecision::SetupAware => "setup_aware",
        CfgPrecision::Conservative => "conservative",
        CfgPrecision::Heuristic => "heuristic",
        CfgPrecision::Unknown => "unknown",
        CfgPrecision::Unsupported => "unsupported",
    }
}

fn cfg_view_label(view: crate::analysis::cfg::facts::CfgView) -> &'static str {
    match view {
        crate::analysis::cfg::facts::CfgView::NormalControl => "normal_control",
        crate::analysis::cfg::facts::CfgView::AbruptAware => "abrupt_aware",
        crate::analysis::cfg::facts::CfgView::ExceptionConservative => "exception_conservative",
    }
}

fn cfg_node_kind_label(kind: crate::analysis::cfg::facts::CfgNodeKind) -> &'static str {
    use crate::analysis::cfg::facts::CfgNodeKind;

    match kind {
        CfgNodeKind::Entry => "entry",
        CfgNodeKind::ExitNormal => "exit_normal",
        CfgNodeKind::ExitExceptional => "exit_exceptional",
        CfgNodeKind::Operation => "operation",
        CfgNodeKind::Condition => "condition",
        CfgNodeKind::CallSite => "call_site",
        CfgNodeKind::Return => "return",
        CfgNodeKind::Throw => "throw",
        CfgNodeKind::Panic => "panic",
        CfgNodeKind::Break => "break",
        CfgNodeKind::Continue => "continue",
        CfgNodeKind::Goto => "goto",
        CfgNodeKind::Yield => "yield",
        CfgNodeKind::Await => "await",
        CfgNodeKind::Defer => "defer",
        CfgNodeKind::RunDefers => "run_defers",
        CfgNodeKind::FinallyEnter => "finally_enter",
        CfgNodeKind::FinallyExit => "finally_exit",
        CfgNodeKind::Synthetic => "synthetic",
        CfgNodeKind::Unsupported => "unsupported",
    }
}

fn basic_block_kind_label(kind: crate::analysis::cfg::facts::BasicBlockKind) -> &'static str {
    use crate::analysis::cfg::facts::BasicBlockKind;

    match kind {
        BasicBlockKind::Entry => "entry",
        BasicBlockKind::ExitNormal => "exit_normal",
        BasicBlockKind::ExitExceptional => "exit_exceptional",
        BasicBlockKind::StraightLine => "straight_line",
        BasicBlockKind::Branch => "branch",
        BasicBlockKind::LoopHeader => "loop_header",
        BasicBlockKind::LoopBody => "loop_body",
        BasicBlockKind::Join => "join",
        BasicBlockKind::Cleanup => "cleanup",
        BasicBlockKind::Unreachable => "unreachable",
        BasicBlockKind::Synthetic => "synthetic",
    }
}

fn cfg_edge_kind_label(kind: crate::analysis::cfg::facts::CfgEdgeKind) -> &'static str {
    use crate::analysis::cfg::facts::CfgEdgeKind;

    match kind {
        CfgEdgeKind::Normal => "normal",
        CfgEdgeKind::True => "true",
        CfgEdgeKind::False => "false",
        CfgEdgeKind::SwitchCase => "switch_case",
        CfgEdgeKind::DefaultCase => "default_case",
        CfgEdgeKind::LoopEnter => "loop_enter",
        CfgEdgeKind::LoopBack => "loop_back",
        CfgEdgeKind::LoopExit => "loop_exit",
        CfgEdgeKind::Break => "break",
        CfgEdgeKind::Continue => "continue",
        CfgEdgeKind::Goto => "goto",
        CfgEdgeKind::Return => "return",
        CfgEdgeKind::Throw => "throw",
        CfgEdgeKind::ImplicitThrow => "implicit_throw",
        CfgEdgeKind::Panic => "panic",
        CfgEdgeKind::Recover => "recover",
        CfgEdgeKind::Finally => "finally",
        CfgEdgeKind::Cleanup => "cleanup",
        CfgEdgeKind::Defer => "defer",
        CfgEdgeKind::ShortCircuit => "short_circuit",
        CfgEdgeKind::OptionalChain => "optional_chain",
        CfgEdgeKind::Nullish => "nullish",
        CfgEdgeKind::YieldSuspend => "yield_suspend",
        CfgEdgeKind::YieldResume => "yield_resume",
        CfgEdgeKind::AwaitSuspend => "await_suspend",
        CfgEdgeKind::AwaitResume => "await_resume",
        CfgEdgeKind::Spawn => "spawn",
        CfgEdgeKind::Unreachable => "unreachable",
        CfgEdgeKind::Unknown => "unknown",
        CfgEdgeKind::Synthetic => "synthetic",
        CfgEdgeKind::Extension => "extension",
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

/// How a path changed relative to the target ref, in a `polint review` diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeStatus {
    /// The file is new on the working side.
    Added,
    /// The file existed on both sides and its content changed.
    Modified,
    /// The file was removed on the working side.
    Deleted,
    /// The file was renamed; the carried path is the new-side path.
    Renamed,
}

/// One changed file in a `polint review` diff against the target ref.
///
/// Crate-internal: rule authors read changed files through the `ChangedFiles`
/// fact view and its `ChangedFileRef` items, never this struct directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChangedFile {
    /// Repo-relative, `/`-normalized path, identical in form to `Diagnostic.file`.
    pub(crate) path: String,
    /// How this file changed relative to the target ref.
    pub(crate) status: ChangeStatus,
    /// New-side changed line ranges, inclusive and 1-based; empty for `Deleted`.
    pub(crate) new_line_ranges: Vec<(u32, u32)>,
}

/// The set of files changed in a `polint review` diff against the target ref.
///
/// Injected on the [`AnalysisDb`] by the host runner; read through the
/// `ChangedFiles` SDK fact view. Empty under `polint check`. Crate-internal:
/// it travels outer→host as a JSON cache file, so it derives `Serialize`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChangeSetFacts {
    /// Changed files, sorted by `path` for deterministic output.
    pub(crate) files: Vec<ChangedFile>,
}

/// Whether a rule runs under `polint check` (`Check`) or `polint review` (`Review`).
///
/// Authored via `#[polint::rule(..., kind = "check" | "review")]`. Defaults to
/// `Check`, so every existing rule keeps its behavior. `polint check` executes
/// only `Check` rules; `polint review` executes only `Review` rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// A normal rule that runs under `polint check`.
    #[default]
    Check,
    /// A diff-gated rule that runs under `polint review`.
    Review,
}

/// Static metadata for a rule as shown in diagnostics, config, and registries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMeta {
    pub id: String,
    pub description: String,
    pub severity: Severity,
    /// Which command runs this rule (`check` vs `review`); defaults to `check`.
    #[serde(default)]
    pub kind: RuleKind,
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
    /// Needs the `polint review` changeset (diff-to-target-ref facts).
    pub changeset: bool,
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

    pub fn changeset(mut self) -> Self {
        self.changeset = true;
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
            ("changeset", self.changeset),
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
        let started = std::time::Instant::now();
        let result = catch_unwind(AssertUnwindSafe(|| rule.run(db, &mut ctx)));
        tracing::info!(
            target: "polint::rules",
            rule = %meta.id,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "rule finished"
        );
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
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId, UnsupportedId};
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

    #[test]
    fn analysis_db_solver_budget_status_tracks_not_run_and_replacements() {
        let mut db = AnalysisDb::new();

        assert_eq!(db.solver_budget_status(), BudgetStatus::NotRun);
        assert!(db.solver_budget_reasons().is_empty());

        db.replace_solver_facts(SolverOutput::default())
            .expect("within-budget solver facts");
        assert_eq!(db.solver_budget_status(), BudgetStatus::WithinBudget);
        assert!(db.solver_budget_reasons().is_empty());

        db.replace_solver_facts(SolverOutput {
            budget_status: BudgetStatus::BudgetExceeded,
            budget_reasons: BTreeSet::from(["solver.max_steps".to_string()]),
            ..SolverOutput::default()
        })
        .expect("budget-exceeded solver facts");
        assert_eq!(db.solver_budget_status(), BudgetStatus::BudgetExceeded);
        assert_eq!(
            db.solver_budget_reasons(),
            &BTreeSet::from(["solver.max_steps".to_string()])
        );
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
                kind: RuleKind::Check,
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
            id: UnsupportedId(
                stable_key
                    .bytes()
                    .fold(0_u64, |sum, byte| sum + u64::from(byte)),
            ),
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

    fn test_call_site(
        id: u64,
        file: FileId,
        caller: FunctionId,
        stable_key: &str,
    ) -> crate::analysis::calls::facts::CallSiteFact {
        use crate::analysis::calls::facts::{
            CallCallee, CallPrecision, CallSiteFact, CallSyntaxKind, CallTargetStatus,
        };

        CallSiteFact {
            in_throw: false,
            id: CallSiteId(id),
            language: Language::TypeScript,
            file,
            caller,
            owner_symbol: Some(SymbolId(caller.0 + 100)),
            body: MirBodyId(id),
            operation: MirOpId(id),
            span: test_span(file, 1),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: stable_key.to_string(),
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            stable_key: stable_key.to_string(),
        }
    }

    fn test_call_target(
        id: u64,
        site: CallSiteId,
        caller: FunctionId,
        stable_key: &str,
    ) -> crate::analysis::calls::facts::CallTargetFact {
        use crate::analysis::calls::facts::{
            CallAlgorithm, CallEdgeKind, CallPrecision, CallProvenance, CallTargetFact,
            CallTargetStatus,
        };

        CallTargetFact {
            id: crate::analysis::ids::CallTargetId(id),
            site,
            caller,
            target_function: Some(FunctionId(id + 10)),
            target_symbol: Some(SymbolId(id + 20)),
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::Exact,
            stable_key: stable_key.to_string(),
        }
    }

    fn test_unresolved_call(
        site: CallSiteId,
        caller: FunctionId,
        stable_key: &str,
    ) -> crate::analysis::calls::facts::UnresolvedCallFact {
        use crate::analysis::calls::facts::{
            CallAlgorithm, CallPrecision, CallProvenance, CallTargetStatus, UnresolvedCallFact,
            UnresolvedCallReason,
        };

        UnresolvedCallFact {
            site,
            caller,
            status: CallTargetStatus::Unresolved,
            reason: UnresolvedCallReason::FunctionValue,
            algorithm: CallAlgorithm::SyntaxOnly,
            provenance: CallProvenance::MirShape,
            precision: CallPrecision::Unknown,
            stable_key: stable_key.to_string(),
        }
    }

    mod call_fact_storage {
        use super::*;
        use crate::analysis::calls::store::CallOutput;

        #[test]
        fn replace_call_facts_removes_stale_rows_from_previous_run() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { first(); second(); }\n".to_string(),
            );
            let first = CallOutput {
                sites: vec![test_call_site(1, file, FunctionId(1), "call-site:first")],
                targets: vec![test_call_target(
                    1,
                    CallSiteId(1),
                    FunctionId(1),
                    "call-target:first",
                )],
                unresolved: vec![test_unresolved_call(
                    CallSiteId(1),
                    FunctionId(1),
                    "unresolved:first",
                )],
            };
            let second = CallOutput {
                sites: vec![test_call_site(2, file, FunctionId(2), "call-site:second")],
                targets: Vec::new(),
                unresolved: Vec::new(),
            };

            db.replace_call_facts(first).expect("first call replace");
            assert!(db.call_store().is_some());
            assert_eq!(db.call_sites_by_caller(FunctionId(1)).len(), 1);
            assert_eq!(db.call_targets_by_site(CallSiteId(1)).len(), 1);
            assert_eq!(db.outgoing_calls_by_function(FunctionId(1)).len(), 1);
            assert_eq!(db.outgoing_calls_by_symbol(SymbolId(101)).len(), 1);
            assert_eq!(db.incoming_calls_by_symbol(SymbolId(21)).len(), 1);
            assert_eq!(db.incoming_calls_by_function(FunctionId(11)).len(), 1);
            assert_eq!(
                db.unresolved_calls_by_reason(
                    crate::analysis::calls::facts::UnresolvedCallReason::FunctionValue,
                )
                .len(),
                1
            );
            assert_eq!(
                db.unresolved_calls_by_status(
                    crate::analysis::calls::facts::CallTargetStatus::Unresolved,
                )
                .len(),
                1
            );

            db.replace_call_facts(second).expect("second call replace");

            assert_eq!(db.call_sites()[0].stable_key, "call-site:second");
            assert!(db.call_targets().is_empty());
            assert!(db.unresolved_calls().is_empty());
        }
    }

    mod ts_object_model_storage {
        use super::*;
        use crate::ts::object_model::facts::{
            TsObjectAllocationFact, TsObjectAllocationId, TsObjectAllocationKind,
            TsObjectModelStatus, TsPropertyKey, TsPropertyKeyKind, TsPropertyReadFact,
            TsPropertyReadId, TsPropertyWriteFact, TsPropertyWriteId, TsPrototypeLinkFact,
            TsPrototypeLinkId, TsPrototypeLinkKind, TsReceiverBindingFact, TsReceiverBindingId,
            TsReceiverBindingKind,
        };
        use crate::ts::object_model::store::TsObjectModelOutput;

        #[test]
        fn replace_ts_object_model_facts_removes_stale_rows_from_previous_run() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "const holder = { target() {} }; holder.target();\n".to_string(),
            );

            db.replace_ts_object_model_facts(full_output(file, "first"))
                .expect("first object-model replace");
            assert_eq!(db.ts_object_allocations().len(), 1);
            assert_eq!(db.ts_property_writes().len(), 1);
            assert_eq!(db.ts_property_reads().len(), 1);
            assert_eq!(db.ts_receiver_bindings().len(), 1);
            assert_eq!(db.ts_prototype_links().len(), 1);
            assert!(
                db.ts_object_model_store()
                    .expect("object-model store")
                    .allocation_by_stable_key("object:first")
                    .is_some()
            );

            db.replace_ts_object_model_facts(allocation_only_output(file, "second"))
                .expect("second object-model replace");

            assert_eq!(db.ts_object_allocations().len(), 1);
            assert_eq!(db.ts_object_allocations()[0].id, TsObjectAllocationId(0));
            assert_eq!(db.ts_object_allocations()[0].stable_key, "object:second");
            assert!(db.ts_property_writes().is_empty());
            assert!(db.ts_property_reads().is_empty());
            assert!(db.ts_receiver_bindings().is_empty());
            assert!(db.ts_prototype_links().is_empty());
            let store = db.ts_object_model_store().expect("object-model store");
            assert!(store.allocation_by_stable_key("object:first").is_none());
            assert!(store.allocation_by_stable_key("object:second").is_some());
        }

        #[test]
        fn replace_ts_object_model_facts_rejects_duplicate_stable_keys() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "const holder = {};\n".to_string(),
            );

            let error = db
                .replace_ts_object_model_facts(TsObjectModelOutput {
                    allocations: vec![
                        allocation(file, "object:dup", 1),
                        allocation(file, "object:dup", 2),
                    ],
                    property_writes: Vec::new(),
                    property_reads: Vec::new(),
                    receiver_bindings: Vec::new(),
                    prototype_links: Vec::new(),
                })
                .expect_err("duplicate stable key should be rejected");

            assert_eq!(
                error.to_string(),
                "invalid semantic fact from `polint.ts.object_model`: duplicate object allocation stable key `object:dup`"
            );
        }

        fn full_output(file: FileId, suffix: &str) -> TsObjectModelOutput {
            TsObjectModelOutput {
                allocations: vec![allocation(file, &format!("object:{suffix}"), 10)],
                property_writes: vec![property_write(file, &format!("write:{suffix}"), suffix)],
                property_reads: vec![property_read(file, &format!("read:{suffix}"), suffix)],
                receiver_bindings: vec![receiver_binding(file, &format!("receiver:{suffix}"))],
                prototype_links: vec![prototype_link(file, &format!("prototype:{suffix}"), suffix)],
            }
        }

        fn allocation_only_output(file: FileId, suffix: &str) -> TsObjectModelOutput {
            TsObjectModelOutput {
                allocations: vec![allocation(file, &format!("object:{suffix}"), 20)],
                property_writes: Vec::new(),
                property_reads: Vec::new(),
                receiver_bindings: Vec::new(),
                prototype_links: Vec::new(),
            }
        }

        fn allocation(file: FileId, stable_key: &str, id: u64) -> TsObjectAllocationFact {
            TsObjectAllocationFact {
                id: TsObjectAllocationId(id),
                file,
                span: test_span(file, 1),
                stable_key: stable_key.to_string(),
                lexical_parent_key: Some("scope:module".to_string()),
                inventory_function: None,
                inventory_function_stable_key: None,
                inventory_callsite: None,
                inventory_callsite_stable_key: None,
                kind: TsObjectAllocationKind::ObjectLiteral,
                status: TsObjectModelStatus::resolved(),
            }
        }

        fn property_write(file: FileId, stable_key: &str, suffix: &str) -> TsPropertyWriteFact {
            TsPropertyWriteFact {
                id: TsPropertyWriteId(99),
                file,
                span: test_span(file, 2),
                stable_key: stable_key.to_string(),
                base_object_stable_key: format!("object:{suffix}"),
                property_key: property_key(),
                value_function: None,
                value_function_stable_key: Some(format!("function:{suffix}")),
                value_object_stable_key: None,
                status: TsObjectModelStatus::resolved(),
            }
        }

        fn property_read(file: FileId, stable_key: &str, suffix: &str) -> TsPropertyReadFact {
            TsPropertyReadFact {
                id: TsPropertyReadId(99),
                file,
                span: test_span(file, 3),
                stable_key: stable_key.to_string(),
                base_object_stable_key: format!("object:{suffix}"),
                property_key: property_key(),
                destination_stable_key: Some(format!("place:{suffix}")),
                callsite: None,
                callsite_stable_key: Some(format!("callsite:{suffix}")),
                status: TsObjectModelStatus::resolved(),
            }
        }

        fn receiver_binding(file: FileId, stable_key: &str) -> TsReceiverBindingFact {
            TsReceiverBindingFact {
                id: TsReceiverBindingId(99),
                file,
                span: test_span(file, 4),
                stable_key: stable_key.to_string(),
                kind: TsReceiverBindingKind::MethodCall,
                callsite: None,
                callsite_stable_key: Some("callsite:first".to_string()),
                callee_function: None,
                callee_function_stable_key: Some("function:first".to_string()),
                receiver_object_stable_key: Some("object:first".to_string()),
                receiver_place_stable_key: Some("place:holder".to_string()),
                lexical_parent_key: Some("scope:module".to_string()),
                status: TsObjectModelStatus::resolved(),
            }
        }

        fn prototype_link(file: FileId, stable_key: &str, suffix: &str) -> TsPrototypeLinkFact {
            TsPrototypeLinkFact {
                id: TsPrototypeLinkId(99),
                file,
                span: test_span(file, 5),
                stable_key: stable_key.to_string(),
                kind: TsPrototypeLinkKind::ClassPrototype,
                object_stable_key: format!("object:{suffix}"),
                prototype_stable_key: format!("object:{suffix}:prototype"),
                property_key: None,
                status: TsObjectModelStatus::resolved(),
            }
        }

        fn property_key() -> TsPropertyKey {
            TsPropertyKey {
                kind: TsPropertyKeyKind::Static,
                value: Some("target".to_string()),
            }
        }
    }

    mod call_fact_metadata {
        use super::*;
        use crate::analysis::calls::facts::{CallPrecision, CallTargetStatus};
        use crate::analysis::calls::store::CallOutput;

        #[test]
        fn replace_call_facts_records_metadata_provider_and_family_labels() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { run(); }\n".to_string(),
            );

            db.replace_call_facts(CallOutput {
                sites: vec![test_call_site(0, file, FunctionId(1), "call-site:metadata")],
                targets: vec![test_call_target(
                    0,
                    CallSiteId(0),
                    FunctionId(1),
                    "call-target:metadata",
                )],
                unresolved: vec![test_unresolved_call(
                    CallSiteId(0),
                    FunctionId(1),
                    "unresolved:metadata",
                )],
            })
            .expect("call replace");

            for family in [
                FactFamily::CallSite,
                FactFamily::CallTarget,
                FactFamily::UnresolvedCall,
            ] {
                let metadata = db
                    .metadata_for(FactRef::new(family, 0))
                    .expect("call metadata exists");
                assert_eq!(metadata.producer_id, "polint.calls");
                assert_eq!(metadata.layer_id, "polint.calls");
                assert_eq!(metadata.validation, ValidationStatus::NativeTrusted);
                assert!(matches!(
                    family.label(),
                    "CallSite" | "CallTarget" | "UnresolvedCall"
                ));
            }
        }

        #[test]
        fn call_metadata_maps_unknown_statuses_to_non_exact_precision() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { target[key](); }\n".to_string(),
            );
            let mut site = test_call_site(0, file, FunctionId(1), "call-site:unsupported");
            site.status = CallTargetStatus::Unsupported;
            site.precision = CallPrecision::Unsupported;
            let mut target =
                test_call_target(0, CallSiteId(0), FunctionId(1), "call-target:setup-missing");
            target.status = CallTargetStatus::SetupMissing;
            target.precision = CallPrecision::Unknown;
            let unresolved =
                test_unresolved_call(CallSiteId(0), FunctionId(1), "unresolved:unknown");

            db.replace_call_facts(CallOutput {
                sites: vec![site],
                targets: vec![target],
                unresolved: vec![unresolved],
            })
            .expect("call replace");

            assert_ne!(
                db.metadata_for(FactRef::new(FactFamily::CallSite, 0))
                    .expect("call site metadata exists")
                    .precision,
                FactPrecision::Exact
            );
            assert_ne!(
                db.metadata_for(FactRef::new(FactFamily::CallTarget, 0))
                    .expect("call target metadata exists")
                    .precision,
                FactPrecision::Exact
            );
            assert_ne!(
                db.metadata_for(FactRef::new(FactFamily::UnresolvedCall, 0))
                    .expect("unresolved call metadata exists")
                    .precision,
                FactPrecision::Exact
            );
        }
    }

    mod data_flow_fact_metadata {
        use super::*;
        use crate::analysis::data_flow::facts::{
            DataFlowModelKind, DataFlowNodeKind, DataFlowProvenance,
        };

        #[test]
        fn model_backed_node_metadata_uses_model_precision_and_payload() {
            let mut db = AnalysisDb::new();

            db.replace_data_flow_facts(data_flow_output_with_model("model:source:first"))
                .expect("first data-flow replace");
            let first_metadata = db
                .metadata_for(FactRef::new(FactFamily::DataFlowNode, 0))
                .expect("node metadata")
                .clone();

            db.replace_data_flow_facts(data_flow_output_with_model("model:source:second"))
                .expect("second data-flow replace");
            let second_metadata = db
                .metadata_for(FactRef::new(FactFamily::DataFlowNode, 0))
                .expect("node metadata")
                .clone();

            assert_eq!(first_metadata.precision, FactPrecision::SetupAware);
            assert_eq!(
                first_metadata.validation,
                ValidationStatus::ReferentiallyValidated
            );
            assert_ne!(
                first_metadata.payload_digest,
                second_metadata.payload_digest
            );
        }

        fn data_flow_output_with_model(model_key: &str) -> DataFlowOutput {
            DataFlowOutput {
                nodes: vec![DataFlowNodeFact {
                    id: crate::analysis::ids::DataFlowNodeId(10),
                    kind: DataFlowNodeKind::Source,
                    language: Language::TypeScript,
                    file: None,
                    function: None,
                    body: None,
                    operation: None,
                    cfg_node: None,
                    place: None,
                    symbol: None,
                    reference: None,
                    call_site: None,
                    model: Some(crate::analysis::ids::DataFlowModelId(20)),
                    span: None,
                    stable_key: "node:source".to_string(),
                }],
                edges: Vec::new(),
                models: vec![DataFlowModelFact {
                    id: crate::analysis::ids::DataFlowModelId(20),
                    kind: DataFlowModelKind::Source,
                    language: Language::TypeScript,
                    provider_id: "test".to_string(),
                    model_id: None,
                    source_stable_key: None,
                    status: DataFlowStatus::Present,
                    precision: DataFlowPrecision::SetupAware,
                    validation: DataFlowValidation::ReferentiallyValidated,
                    confidence: DataFlowConfidence::High,
                    provenance: DataFlowProvenance::Native,
                    evidence: Vec::new(),
                    payload_labels: Vec::new(),
                    stable_key: model_key.to_string(),
                }],
                budgets: Vec::new(),
            }
        }
    }

    mod type_value_alias_metadata {
        use super::*;
        use crate::analysis::ids::{AbstractValueId, ValueFactId};
        use crate::analysis::values::facts::{ValueKind, ValueProvenance, ValueSubject};
        use crate::analysis::values::store::ValueOutput;

        #[test]
        fn exact_local_value_metadata_stays_within_setup_aware_provider_ceiling() {
            let mut db = AnalysisDb::new();
            db.replace_type_value_alias_facts(TypeValueAliasOutput {
                values: ValueOutput {
                    values: vec![ValueFact {
                        id: ValueFactId(0),
                        subject: ValueSubject::Synthetic("literal".to_string()),
                        value: AbstractValueId(0),
                        kind: ValueKind::String("\"ok\"".to_string()),
                        language: Language::TypeScript,
                        file: None,
                        function: None,
                        body: None,
                        precision: ValuePrecision::ExactLocal,
                        status: ValueStatus::Present,
                        provenance: ValueProvenance::Native,
                        stable_key: "value:literal".to_string(),
                    }],
                    allocations: Vec::new(),
                },
                ..TypeValueAliasOutput::default()
            });

            let metadata = db
                .metadata_for(FactRef::new(FactFamily::Value, 0))
                .expect("value metadata exists");

            assert_eq!(metadata.precision, FactPrecision::SetupAware);
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
                kind: RuleKind::Check,
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
                kind: RuleKind::Check,
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

    #[test]
    fn extension_facts_are_sidecar_metadata_and_rejections_are_audit_only() {
        let mut db = AnalysisDb::new();
        db.replace_extension_facts(ExtensionOutput {
            activations: vec![ExtensionActivationRow {
                extension_id: "demo".to_string(),
                provider_id: Some("routes".to_string()),
                status: crate::analysis::extensions::manifest::ExtensionActivationStatus::Active,
                diagnostic_count: 0,
                output_digest_inputs: Vec::new(),
                diagnostic_digest: "empty".to_string(),
            }],
            accepted: vec![AcceptedExtensionFact {
                extension_id: "demo".to_string(),
                provider_id: "routes".to_string(),
                fact_family: "extension.routes".to_string(),
                stable_key: "route:/a".to_string(),
                binding_refs: vec!["file:src/app.ts".to_string()],
                precision: ExtensionFactPrecision::Heuristic,
                confidence: ExtensionFactConfidence::Medium,
                status: crate::analysis::extensions::sinks::ExtensionFactStatus::Accepted,
                evidence: vec!["fixture".to_string()],
                payload_labels: vec!["method=GET".to_string()],
                payload_digest: "payload".to_string(),
            }],
            rejected: vec![RejectedExtensionFact {
                extension_id: "demo".to_string(),
                provider_id: "routes".to_string(),
                fact_family: "extension.routes".to_string(),
                stable_key: "route:/bad".to_string(),
                reason:
                    crate::analysis::extensions::validate::ExtensionRejectionReason::NativeConflict,
                evidence: vec!["fixture".to_string()],
            }],
        });

        assert_eq!(db.extension_facts().len(), 1);
        assert_eq!(db.extension_activations().len(), 1);
        assert_eq!(db.rejected_extension_facts().len(), 1);
        let metadata = db
            .metadata_for(FactRef::new(FactFamily::ExtensionFact, 0))
            .expect("extension metadata exists");
        assert_eq!(metadata.producer_id, "polint.extension.demo.routes");
        assert_eq!(metadata.layer_id, "polint.extension.demo.routes");
        assert_eq!(metadata.precision, FactPrecision::Heuristic);
        assert_eq!(metadata.validation, ValidationStatus::SchemaValidated);
    }

    #[test]
    fn evidence_exact_rows_do_not_exceed_setup_aware_metadata_ceiling() {
        let mut db = AnalysisDb::new();
        db.replace_evidence_facts(crate::analysis::evidence::store::EvidenceOutput {
            nodes: vec![EvidenceNodeFact {
                id: crate::analysis::ids::EvidenceNodeId(0),
                kind: crate::analysis::evidence::facts::EvidenceNodeKind::Operation,
                language: Language::Go,
                file: None,
                function: None,
                body: None,
                operation: None,
                cfg_node: None,
                place: None,
                symbol: None,
                reference: None,
                call_site: None,
                span: None,
                status: EvidenceStatus::Present,
                precision: EvidencePrecision::Exact,
                provenance: EvidenceProvenance::Native,
                validation: EvidenceValidation::Native,
                confidence: EvidenceConfidence::High,
                compact_label: None,
                source_fact_stable_keys: Vec::new(),
                stable_key: "evidence:node:exact".to_string(),
            }],
            ..crate::analysis::evidence::store::EvidenceOutput::empty()
        })
        .expect("valid evidence output");

        let metadata = db
            .metadata_for(FactRef::new(FactFamily::EvidenceNode, 0))
            .expect("evidence metadata exists");
        assert_eq!(metadata.producer_id, "polint.evidence");
        assert_eq!(metadata.precision, FactPrecision::SetupAware);
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
