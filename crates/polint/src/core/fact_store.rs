//! Provider-owned fact stores behind a keyed registry on [`super::AnalysisDb`].
//!
//! Each store owns the vectors (and indexes) for one provider group. The SDK and
//! `AnalysisDb` accessors stay typed; the registry holds `dyn FactStore` for
//! later eviction and language-neutral core layout.

use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;

use crate::analysis::calls::store::CallStore;
use crate::analysis::cfg::facts::{
    BasicBlockFact, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, ControlDependenceFact,
    DominatorFact, PostDominatorFact, ReachabilityFact, UnsupportedControlFlowFact,
};
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis_kernel::FactFamily;
use crate::core::facts::{
    BranchObligation, ComplexityMetricFact, CoverageFact, DefinitionFact, FileMetricFact,
    FunctionFact, FunctionMetricFact, ImportFact, JsxAttributeFact, ModuleEdge, ModuleNode,
    PackageFact, ReferenceFact, ResolvedImportFact, StringLiteralFact, SymbolFact, TestFact,
    TsClassFact, TsComponentFact,
};
use crate::core::ids::{BranchId, FileId, FunctionId, ImportId, PackageId, SymbolId};
use crate::go::semantic::store::GoSemanticStore;
use crate::module_graph::topology::{
    DependencyRequirementFact, ImportToPackageFact, RepoTopologyOverlayFact,
    ResolvedDependencyEdgeFact, SourceSetFact, TopologyOutput, TopologyPackageFact,
    WorkspaceRootFact,
};
use crate::symbol_graph::semantic::{
    AliasFact, AliasId, ExportFact, ExportId, GeneratedSymbolFact, GeneratedSymbolId,
    ResolutionFact, ResolutionId, ScopeFact, ScopeId, SemanticImportFact, SemanticImportId,
    StableExportId, StableExportIdentity,
};
use crate::ts::object_model::store::TsObjectModelStore;

/// Erased provider-owned fact container. Not public — rule authors use SDK views.
pub(crate) trait FactStore: Any + Send + Sync {
    fn family(&self) -> FactFamily;
    fn clear(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clone_box(&self) -> Box<dyn FactStore>;
}

/// Cloneable / debug wrapper so [`super::AnalysisDb`] can keep `derive(Clone, Debug)`.
pub(crate) struct FactStoreEntry(Box<dyn FactStore>);

impl FactStoreEntry {
    pub(crate) fn new(store: impl FactStore + 'static) -> Self {
        Self(Box::new(store))
    }

    pub(crate) fn as_store(&self) -> &dyn FactStore {
        self.0.as_ref()
    }

    pub(crate) fn as_store_mut(&mut self) -> &mut dyn FactStore {
        self.0.as_mut()
    }
}

impl Clone for FactStoreEntry {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl fmt::Debug for FactStoreEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FactStoreEntry")
            .field("family", &self.0.family())
            .finish_non_exhaustive()
    }
}

/// Syntax facts produced by `polint.go.syntax` (and shared `functions` /
/// `imports` rows also written by `polint.ts.syntax` through the same accessors).
#[derive(Debug, Clone, Default)]
pub(crate) struct GoSyntaxStore {
    pub(crate) packages: Vec<PackageFact>,
    pub(crate) functions: Vec<FunctionFact>,
    pub(crate) imports: Vec<ImportFact>,
    pub(crate) branches: Vec<BranchObligation>,
    pub(crate) tests: Vec<TestFact>,
}

impl GoSyntaxStore {
    pub(crate) fn packages(&self) -> &[PackageFact] {
        &self.packages
    }

    pub(crate) fn functions(&self) -> &[FunctionFact] {
        &self.functions
    }

    pub(crate) fn imports(&self) -> &[ImportFact] {
        &self.imports
    }

    pub(crate) fn branches(&self) -> &[BranchObligation] {
        &self.branches
    }

    pub(crate) fn tests(&self) -> &[TestFact] {
        &self.tests
    }

    pub(crate) fn push_package(&mut self, mut fact: PackageFact) -> PackageId {
        let id = PackageId(self.packages.len() as u64);
        fact.id = id;
        self.packages.push(fact);
        id
    }

    pub(crate) fn push_function(&mut self, mut fact: FunctionFact) -> FunctionId {
        let id = FunctionId(self.functions.len() as u64);
        fact.id = id;
        self.functions.push(fact);
        id
    }

    pub(crate) fn push_import(&mut self, mut fact: ImportFact) -> ImportId {
        let id = ImportId(self.imports.len() as u64);
        fact.id = id;
        self.imports.push(fact);
        id
    }

    pub(crate) fn push_branch(&mut self, mut fact: BranchObligation) -> BranchId {
        let id = BranchId(self.branches.len() as u64);
        fact.id = id;
        self.branches.push(fact);
        id
    }

    pub(crate) fn push_test(&mut self, fact: TestFact) -> u64 {
        let run_id = self.tests.len() as u64;
        self.tests.push(fact);
        run_id
    }
}

impl FactStore for GoSyntaxStore {
    fn family(&self) -> FactFamily {
        // Primary key for the multi-family syntax group in the registry map.
        FactFamily::Package
    }

    fn clear(&mut self) {
        self.packages.clear();
        self.functions.clear();
        self.imports.clear();
        self.branches.clear();
        self.tests.clear();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`GoSyntaxStore`] in `AnalysisDb::fact_stores`.
pub(crate) const GO_SYNTAX_STORE_FAMILY: FactFamily = FactFamily::Package;

/// TS/JSX syntax facts produced by `polint.ts.syntax`.
#[derive(Debug, Clone, Default)]
pub(crate) struct TsSyntaxStore {
    pub(crate) ts_components: Vec<TsComponentFact>,
    pub(crate) ts_classes: Vec<TsClassFact>,
    pub(crate) string_literals: Vec<StringLiteralFact>,
    pub(crate) jsx_attributes: Vec<JsxAttributeFact>,
}

impl TsSyntaxStore {
    pub(crate) fn ts_components(&self) -> &[TsComponentFact] {
        &self.ts_components
    }

    pub(crate) fn ts_classes(&self) -> &[TsClassFact] {
        &self.ts_classes
    }

    pub(crate) fn string_literals(&self) -> &[StringLiteralFact] {
        &self.string_literals
    }

    pub(crate) fn jsx_attributes(&self) -> &[JsxAttributeFact] {
        &self.jsx_attributes
    }

    pub(crate) fn push_ts_component(&mut self, fact: TsComponentFact) -> u64 {
        let run_id = self.ts_components.len() as u64;
        self.ts_components.push(fact);
        run_id
    }

    pub(crate) fn push_ts_class(&mut self, fact: TsClassFact) -> u64 {
        let run_id = self.ts_classes.len() as u64;
        self.ts_classes.push(fact);
        run_id
    }

    pub(crate) fn push_string_literal(&mut self, fact: StringLiteralFact) -> u64 {
        let run_id = self.string_literals.len() as u64;
        self.string_literals.push(fact);
        run_id
    }

    pub(crate) fn push_jsx_attribute(&mut self, fact: JsxAttributeFact) -> u64 {
        let run_id = self.jsx_attributes.len() as u64;
        self.jsx_attributes.push(fact);
        run_id
    }
}

impl FactStore for TsSyntaxStore {
    fn family(&self) -> FactFamily {
        FactFamily::TsComponent
    }

    fn clear(&mut self) {
        self.ts_components.clear();
        self.ts_classes.clear();
        self.string_literals.clear();
        self.jsx_attributes.clear();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`TsSyntaxStore`] in `AnalysisDb::fact_stores`.
pub(crate) const TS_SYNTAX_STORE_FAMILY: FactFamily = FactFamily::TsComponent;

/// CFG facts produced by `polint.cfg`.
#[derive(Debug, Clone, Default)]
pub(crate) struct CfgFactStore {
    output: CfgOutput,
}

impl CfgFactStore {
    pub(crate) fn replace(&mut self, output: CfgOutput) {
        self.output = output;
    }

    pub(crate) fn functions(&self) -> &[CfgFunctionFact] {
        &self.output.functions
    }

    pub(crate) fn nodes(&self) -> &[CfgNodeFact] {
        &self.output.nodes
    }

    pub(crate) fn blocks(&self) -> &[BasicBlockFact] {
        &self.output.blocks
    }

    pub(crate) fn edges(&self) -> &[CfgEdgeFact] {
        &self.output.edges
    }

    pub(crate) fn reachability(&self) -> &[ReachabilityFact] {
        &self.output.reachability
    }

    pub(crate) fn dominators(&self) -> &[DominatorFact] {
        &self.output.dominators
    }

    pub(crate) fn postdominators(&self) -> &[PostDominatorFact] {
        &self.output.postdominators
    }

    pub(crate) fn control_dependence(&self) -> &[ControlDependenceFact] {
        &self.output.control_dependence
    }

    pub(crate) fn unsupported(&self) -> &[UnsupportedControlFlowFact] {
        &self.output.unsupported
    }
}

impl FactStore for CfgFactStore {
    fn family(&self) -> FactFamily {
        FactFamily::CfgFunction
    }

    fn clear(&mut self) {
        self.output = CfgOutput::empty();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`CfgFactStore`] in `AnalysisDb::fact_stores`.
pub(crate) const CFG_STORE_FAMILY: FactFamily = FactFamily::CfgFunction;

impl FactStore for CallStore {
    fn family(&self) -> FactFamily {
        FactFamily::CallSite
    }

    fn clear(&mut self) {
        *self = CallStore::default();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`CallStore`] in `AnalysisDb::fact_stores`.
pub(crate) const CALL_STORE_FAMILY: FactFamily = FactFamily::CallSite;

impl FactStore for GoSemanticStore {
    fn family(&self) -> FactFamily {
        FactFamily::GoSemantic
    }

    fn clear(&mut self) {
        *self = GoSemanticStore::default();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`GoSemanticStore`] in `AnalysisDb::fact_stores`.
pub(crate) const GO_SEMANTIC_STORE_FAMILY: FactFamily = FactFamily::GoSemantic;

/// Module-graph facts produced by `polint.module_graph`.
#[derive(Debug, Clone, Default)]
pub(crate) struct ModuleGraphStore {
    pub(crate) resolved_imports: Vec<ResolvedImportFact>,
    pub(crate) module_nodes: Vec<ModuleNode>,
    pub(crate) module_edges: Vec<ModuleEdge>,
}

impl ModuleGraphStore {
    pub(crate) fn resolved_imports(&self) -> &[ResolvedImportFact] {
        &self.resolved_imports
    }

    pub(crate) fn module_nodes(&self) -> &[ModuleNode] {
        &self.module_nodes
    }

    pub(crate) fn module_edges(&self) -> &[ModuleEdge] {
        &self.module_edges
    }

    pub(crate) fn replace(
        &mut self,
        resolved_imports: Vec<ResolvedImportFact>,
        module_nodes: Vec<ModuleNode>,
        module_edges: Vec<ModuleEdge>,
    ) {
        self.resolved_imports = resolved_imports;
        self.module_nodes = module_nodes;
        self.module_edges = module_edges;
    }
}

impl FactStore for ModuleGraphStore {
    fn family(&self) -> FactFamily {
        FactFamily::ResolvedImport
    }

    fn clear(&mut self) {
        *self = ModuleGraphStore::default();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`ModuleGraphStore`] in `AnalysisDb::fact_stores`.
pub(crate) const MODULE_GRAPH_STORE_FAMILY: FactFamily = FactFamily::ResolvedImport;

/// Topology facts produced by `polint.module_topology`.
#[derive(Debug, Clone, Default)]
pub(crate) struct ModuleTopologyStore {
    output: TopologyOutput,
}

impl ModuleTopologyStore {
    pub(crate) fn from_output(output: TopologyOutput) -> Self {
        Self { output }
    }

    pub(crate) fn workspace_roots(&self) -> &[WorkspaceRootFact] {
        &self.output.workspace_roots
    }

    pub(crate) fn topology_packages(&self) -> &[TopologyPackageFact] {
        &self.output.packages
    }

    pub(crate) fn source_sets(&self) -> &[SourceSetFact] {
        &self.output.source_sets
    }

    pub(crate) fn dependency_requirements(&self) -> &[DependencyRequirementFact] {
        &self.output.dependency_requirements
    }

    pub(crate) fn resolved_dependency_edges(&self) -> &[ResolvedDependencyEdgeFact] {
        &self.output.resolved_dependency_edges
    }

    pub(crate) fn import_to_package_edges(&self) -> &[ImportToPackageFact] {
        &self.output.import_to_package_edges
    }

    pub(crate) fn repo_topology_overlays(&self) -> &[RepoTopologyOverlayFact] {
        &self.output.overlays
    }

    pub(crate) fn replace_import_to_package_edges(&mut self, edges: Vec<ImportToPackageFact>) {
        self.output.import_to_package_edges = edges;
    }
}

impl FactStore for ModuleTopologyStore {
    fn family(&self) -> FactFamily {
        FactFamily::WorkspaceRoot
    }

    fn clear(&mut self) {
        *self = ModuleTopologyStore::default();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`ModuleTopologyStore`] in `AnalysisDb::fact_stores`.
pub(crate) const MODULE_TOPOLOGY_STORE_FAMILY: FactFamily = FactFamily::WorkspaceRoot;

/// Symbol-graph facts and derived indexes produced by `polint.symbol_graph`.
///
/// `symbols_by_name` stays keyed by `String` (not StableKeyId) until a later
/// coordinated interning change.
#[derive(Debug, Clone, Default)]
pub(crate) struct SymbolStore {
    pub(crate) symbols: Vec<SymbolFact>,
    pub(crate) definitions: Vec<DefinitionFact>,
    pub(crate) references: Vec<ReferenceFact>,
    pub(crate) symbols_by_id: BTreeMap<SymbolId, usize>,
    pub(crate) definitions_by_symbol: BTreeMap<SymbolId, Vec<usize>>,
    pub(crate) references_by_target: BTreeMap<SymbolId, Vec<usize>>,
    pub(crate) symbols_by_file: BTreeMap<FileId, Vec<usize>>,
    pub(crate) references_by_file: BTreeMap<FileId, Vec<usize>>,
    pub(crate) symbols_by_name: BTreeMap<String, Vec<usize>>,
}

impl SymbolStore {
    pub(crate) fn symbols(&self) -> &[SymbolFact] {
        &self.symbols
    }

    pub(crate) fn definitions(&self) -> &[DefinitionFact] {
        &self.definitions
    }

    pub(crate) fn references(&self) -> &[ReferenceFact] {
        &self.references
    }

    pub(crate) fn replace(
        &mut self,
        symbols: Vec<SymbolFact>,
        definitions: Vec<DefinitionFact>,
        references: Vec<ReferenceFact>,
    ) {
        self.symbols = symbols;
        self.definitions = definitions;
        self.references = references;
        self.rebuild_indexes();
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

    fn rebuild_indexes(&mut self) {
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
}

impl FactStore for SymbolStore {
    fn family(&self) -> FactFamily {
        FactFamily::Symbol
    }

    fn clear(&mut self) {
        *self = SymbolStore::default();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`SymbolStore`] in `AnalysisDb::fact_stores`.
pub(crate) const SYMBOL_STORE_FAMILY: FactFamily = FactFamily::Symbol;

/// Semantic-index facts and by-id indexes produced by the symbol-graph semantic layer.
#[derive(Debug, Clone, Default)]
pub(crate) struct SemanticIndexStore {
    pub(crate) scopes: Vec<ScopeFact>,
    pub(crate) semantic_imports: Vec<SemanticImportFact>,
    pub(crate) exports: Vec<ExportFact>,
    pub(crate) aliases: Vec<AliasFact>,
    pub(crate) resolution_facts: Vec<ResolutionFact>,
    pub(crate) generated_symbols: Vec<GeneratedSymbolFact>,
    pub(crate) stable_exports: Vec<StableExportIdentity>,
    pub(crate) scopes_by_id: BTreeMap<ScopeId, usize>,
    pub(crate) semantic_imports_by_id: BTreeMap<SemanticImportId, usize>,
    pub(crate) exports_by_id: BTreeMap<ExportId, usize>,
    pub(crate) aliases_by_id: BTreeMap<AliasId, usize>,
    pub(crate) resolution_facts_by_id: BTreeMap<ResolutionId, usize>,
    pub(crate) generated_symbols_by_id: BTreeMap<GeneratedSymbolId, usize>,
    pub(crate) stable_exports_by_id: BTreeMap<StableExportId, usize>,
}

impl SemanticIndexStore {
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

    #[expect(
        clippy::too_many_arguments,
        reason = "semantic index replacement accepts every internal semantic row family explicitly"
    )]
    pub(crate) fn replace(
        &mut self,
        scopes: Vec<ScopeFact>,
        semantic_imports: Vec<SemanticImportFact>,
        exports: Vec<ExportFact>,
        aliases: Vec<AliasFact>,
        resolution_facts: Vec<ResolutionFact>,
        generated_symbols: Vec<GeneratedSymbolFact>,
        stable_exports: Vec<StableExportIdentity>,
    ) {
        self.scopes = scopes;
        self.semantic_imports = semantic_imports;
        self.exports = exports;
        self.aliases = aliases;
        self.resolution_facts = resolution_facts;
        self.generated_symbols = generated_symbols;
        self.stable_exports = stable_exports;
        self.rebuild_indexes();
    }

    fn rebuild_indexes(&mut self) {
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
}

impl FactStore for SemanticIndexStore {
    fn family(&self) -> FactFamily {
        FactFamily::Scope
    }

    fn clear(&mut self) {
        *self = SemanticIndexStore::default();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`SemanticIndexStore`] in `AnalysisDb::fact_stores`.
pub(crate) const SEMANTIC_INDEX_STORE_FAMILY: FactFamily = FactFamily::Scope;

/// Coverage and metric facts produced by the metrics/coverage providers.
#[derive(Debug, Clone, Default)]
pub(crate) struct MetricsStore {
    pub(crate) coverage: Vec<CoverageFact>,
    pub(crate) file_metrics: Vec<FileMetricFact>,
    pub(crate) function_metrics: Vec<FunctionMetricFact>,
    pub(crate) complexity_metrics: Vec<ComplexityMetricFact>,
}

impl MetricsStore {
    pub(crate) fn coverage(&self) -> &[CoverageFact] {
        &self.coverage
    }

    pub(crate) fn file_metrics(&self) -> &[FileMetricFact] {
        &self.file_metrics
    }

    pub(crate) fn function_metrics(&self) -> &[FunctionMetricFact] {
        &self.function_metrics
    }

    pub(crate) fn complexity_metrics(&self) -> &[ComplexityMetricFact] {
        &self.complexity_metrics
    }

    pub(crate) fn push_coverage(&mut self, fact: CoverageFact) -> u64 {
        let run_id = self.coverage.len() as u64;
        self.coverage.push(fact);
        run_id
    }

    pub(crate) fn replace_metrics(
        &mut self,
        file_metrics: Vec<FileMetricFact>,
        function_metrics: Vec<FunctionMetricFact>,
        complexity_metrics: Vec<ComplexityMetricFact>,
    ) {
        self.file_metrics = file_metrics;
        self.function_metrics = function_metrics;
        self.complexity_metrics = complexity_metrics;
    }
}

impl FactStore for MetricsStore {
    fn family(&self) -> FactFamily {
        FactFamily::FileMetric
    }

    fn clear(&mut self) {
        *self = MetricsStore::default();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`MetricsStore`] in `AnalysisDb::fact_stores`.
pub(crate) const METRICS_STORE_FAMILY: FactFamily = FactFamily::FileMetric;

impl FactStore for TsObjectModelStore {
    fn family(&self) -> FactFamily {
        FactFamily::TsObjectModel
    }

    fn clear(&mut self) {
        *self = TsObjectModelStore::default();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`TsObjectModelStore`] in `AnalysisDb::fact_stores`.
pub(crate) const TS_OBJECT_MODEL_STORE_FAMILY: FactFamily = FactFamily::TsObjectModel;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AnalysisDb, FileId, Language, PackageFact, PackageId, Span};
    use std::path::PathBuf;

    #[test]
    fn analysis_db_serves_go_syntax_facts_from_registry_store() {
        let mut db = AnalysisDb::new();
        assert!(
            db.fact_store::<GoSyntaxStore>(GO_SYNTAX_STORE_FAMILY)
                .is_some(),
            "GoSyntaxStore must be registered at construction"
        );

        let file = db.add_file(
            PathBuf::from("pkg/x.go"),
            "pkg/x.go".to_string(),
            "package x\n".to_string(),
        );
        let id = db.push_package(PackageFact {
            id: PackageId(0),
            file,
            name: "x".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::Go,
        });
        assert_eq!(id, PackageId(0));
        assert_eq!(db.packages().len(), 1);
        assert_eq!(
            db.fact_store::<GoSyntaxStore>(GO_SYNTAX_STORE_FAMILY)
                .map(|store| store.packages().len()),
            Some(1)
        );
        assert_eq!(db.packages()[0].name, "x");
    }

    #[test]
    fn go_syntax_store_clear_empties_owned_vectors() {
        let mut store = GoSyntaxStore::default();
        store.packages.push(PackageFact {
            id: PackageId(0),
            file: FileId(0),
            name: "x".to_string(),
            span: Span::point(FileId(0), 1, 1),
            language: Language::Go,
        });
        let erased: &mut dyn FactStore = &mut store;
        erased.clear();
        assert_eq!(erased.family(), FactFamily::Package);
        assert!(store.packages.is_empty());
        assert!(store.functions.is_empty());
        assert!(store.imports.is_empty());
        assert!(store.branches.is_empty());
        assert!(store.tests.is_empty());
    }
}
