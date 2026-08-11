//! Minimal [`FactDatabase`] used by analysis unit tests in place of the facade
//! `AnalysisDb`.

use std::any::Any;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use polint_analysis_api::{
    BranchObligation, CachedFileFacts, CoverageFact, FactDatabase, FactFamily, FactMetaStore,
    FactStore, FactStoreEntry, FunctionFact, ImportFact, JsxAttributeFact, PackageFact, SourceFile,
    StringLiteralFact, TestFact, TsClassFact, TsComponentFact,
};
#[cfg(test)]
use polint_core::test_stable_key_interner;
use polint_core::{
    BranchId, FileId, FunctionId, ImportId, Language, PackageId, StableKeyInterner, fingerprint,
};

use crate::access_paths::store::AccessPathStore;
use crate::aliases::store::AliasStore;
use crate::calls::store::CallStore;
use crate::data_flow::store::DataFlowStore;
use crate::domains::store::DomainStore;
use crate::entrypoints::store::EntrypointStore;
use crate::evidence::store::EvidenceStore;
use crate::fact_store::{
    ACCESS_PATH_STORE_FAMILY, ADAPTATION_STORE_FAMILY, ALIAS_STORE_FAMILY, AdaptationFactStore,
    CALL_STORE_FAMILY, CFG_STORE_FAMILY, CfgFactStore, DATA_FLOW_STORE_FAMILY, DOMAIN_STORE_FAMILY,
    ENTRYPOINT_STORE_FAMILY, EVIDENCE_STORE_FAMILY, EXTENSION_STORE_FAMILY, ExtensionFactStore,
    IDENTITY_STORE_FAMILY, POINTS_TO_STORE_FAMILY, REACHABILITY_STORE_FAMILY,
    REFINED_CALL_STORE_FAMILY, SEMANTIC_GRAPH_STORE_FAMILY, SEMANTIC_MIR_STORE_FAMILY,
    SOLVER_STORE_FAMILY, SUMMARY_STORE_FAMILY, TYPE_STORE_FAMILY, VALUE_STORE_FAMILY,
};
use crate::identity::store::IdentityStore;
use crate::points_to::store::PointsToStore;
use crate::reachability::store::ReachabilityStore;
use crate::refined_calls::store::RefinedCallStore;
use crate::semantic_graph::store::SemanticGraphStore;
use crate::solver::store::SolverStore;
use crate::store::SemanticStore;
use crate::summaries::store::SummaryStore;
use crate::types::store::TypeStore;
use crate::values::store::ValueStore;

#[derive(Debug)]
pub struct LocalAnalysisDb {
    files: Vec<SourceFile>,
    stable_keys: StableKeyInterner,
    fact_meta: FactMetaStore,
    fact_stores: BTreeMap<FactFamily, FactStoreEntry>,
    packages: Vec<PackageFact>,
    functions: Vec<FunctionFact>,
    imports: Vec<ImportFact>,
    branches: Vec<BranchObligation>,
    tests: Vec<TestFact>,
    coverage: Vec<CoverageFact>,
    string_literals: Vec<StringLiteralFact>,
    ts_components: Vec<TsComponentFact>,
    ts_classes: Vec<TsClassFact>,
    jsx_attributes: Vec<JsxAttributeFact>,
}

impl Default for LocalAnalysisDb {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalAnalysisDb {
    pub fn new() -> Self {
        let mut fact_stores = BTreeMap::new();
        macro_rules! put {
            ($fam:expr, $store:expr) => {{
                let mut store = $store;
                FactStore::clear(&mut store);
                fact_stores.insert($fam, FactStoreEntry::new(store));
            }};
        }
        put!(CFG_STORE_FAMILY, CfgFactStore::default());
        put!(CALL_STORE_FAMILY, CallStore::default());
        put!(IDENTITY_STORE_FAMILY, IdentityStore::default());
        put!(REFINED_CALL_STORE_FAMILY, RefinedCallStore::default());
        put!(DATA_FLOW_STORE_FAMILY, DataFlowStore::default());
        put!(EVIDENCE_STORE_FAMILY, EvidenceStore::default());
        put!(DOMAIN_STORE_FAMILY, DomainStore::default());
        put!(SUMMARY_STORE_FAMILY, SummaryStore::default());
        put!(ENTRYPOINT_STORE_FAMILY, EntrypointStore::default());
        put!(TYPE_STORE_FAMILY, TypeStore::default());
        put!(VALUE_STORE_FAMILY, ValueStore::default());
        put!(ACCESS_PATH_STORE_FAMILY, AccessPathStore::default());
        put!(POINTS_TO_STORE_FAMILY, PointsToStore::default());
        put!(ALIAS_STORE_FAMILY, AliasStore::default());
        put!(EXTENSION_STORE_FAMILY, ExtensionFactStore::default());
        put!(ADAPTATION_STORE_FAMILY, AdaptationFactStore::default());
        put!(REACHABILITY_STORE_FAMILY, ReachabilityStore::default());
        put!(SEMANTIC_GRAPH_STORE_FAMILY, SemanticGraphStore::default());
        put!(SOLVER_STORE_FAMILY, SolverStore::default());
        put!(SEMANTIC_MIR_STORE_FAMILY, SemanticStore::default());

        Self {
            files: Vec::new(),
            stable_keys: {
                #[cfg(test)]
                {
                    test_stable_key_interner()
                }
                #[cfg(not(test))]
                {
                    StableKeyInterner::default()
                }
            },
            fact_meta: FactMetaStore::default(),
            fact_stores,
            packages: Vec::new(),
            functions: Vec::new(),
            imports: Vec::new(),
            branches: Vec::new(),
            tests: Vec::new(),
            coverage: Vec::new(),
            string_literals: Vec::new(),
            ts_components: Vec::new(),
            ts_classes: Vec::new(),
            jsx_attributes: Vec::new(),
        }
    }

    /// Inherent helpers so unit-test fixtures do not need [`FactDatabase`] in scope.
    pub fn add_file(&mut self, path: PathBuf, relative_path: String, source: String) -> FileId {
        FactDatabase::add_file(self, path, relative_path, source)
    }

    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        FactDatabase::file(self, id)
    }

    pub fn stable_key_interner(&self) -> StableKeyInterner {
        FactDatabase::stable_key_interner(self)
    }

    pub fn functions(&self) -> &[FunctionFact] {
        FactDatabase::functions(self)
    }

    pub fn push_function(&mut self, fact: FunctionFact) -> FunctionId {
        FactDatabase::push_function(self, fact)
    }

    pub fn push_package(&mut self, fact: PackageFact) -> PackageId {
        FactDatabase::push_package(self, fact)
    }

    pub fn fact_meta(&self) -> &FactMetaStore {
        FactDatabase::fact_meta(self)
    }

    pub fn resolve_stable_key(&self, id: polint_core::StableKeyId) -> std::sync::Arc<str> {
        crate::AnalysisHost::resolve_stable_key(self, id)
    }

    pub fn replace_summary_facts(&mut self, output: crate::summaries::store::SummaryOutput) {
        crate::AnalysisHost::replace_summary_facts(self, output)
    }

    pub fn summary_facts(&self) -> &[crate::summaries::facts::SummaryFact] {
        crate::AnalysisHost::summary_facts(self)
    }

    pub fn summary_events(&self) -> &[crate::summaries::facts::SummaryEventFact] {
        crate::AnalysisHost::summary_events(self)
    }

    pub fn summary_store(&self) -> Option<&crate::summaries::store::SummaryStore> {
        Some(crate::AnalysisHost::summary_store(self))
    }

    pub fn replace_abstract_domain_facts(&mut self, output: crate::domains::store::DomainOutput) {
        crate::AnalysisHost::replace_abstract_domain_facts(self, output)
    }

    pub fn abstract_domain_observations(&self) -> &[crate::domains::facts::DomainObservationFact] {
        crate::AnalysisHost::domain_store_inner(self).observations()
    }

    pub fn fact_meta_mut_for_test(&mut self) -> &mut FactMetaStore {
        crate::AnalysisHost::fact_meta_mut_for_test(self)
    }

    pub fn files(&self) -> &[SourceFile] {
        FactDatabase::files(self)
    }

    pub fn path_for(&self, file: FileId) -> String {
        FactDatabase::path_for(self, file)
    }

    pub fn packages(&self) -> &[PackageFact] {
        FactDatabase::packages(self)
    }

    pub fn imports(&self) -> &[ImportFact] {
        FactDatabase::imports(self)
    }

    pub fn mir_bodies(&self) -> &[crate::mir_body::MirBody] {
        crate::AnalysisHost::mir_bodies(self)
    }

    pub fn mir_operations(&self) -> &[crate::mir_op::MirOperation] {
        crate::AnalysisHost::mir_operations(self)
    }

    pub fn mir_blocks(&self) -> &[crate::mir_body::MirBlock] {
        crate::AnalysisHost::mir_blocks(self)
    }

    pub fn mir_places(&self) -> &[crate::places::PlaceFact] {
        crate::AnalysisHost::mir_places(self)
    }

    pub fn mir_place_types(&self) -> &[crate::places::PlaceTypeFact] {
        crate::AnalysisHost::mir_place_types(self)
    }

    pub fn call_sites(&self) -> &[crate::calls::facts::CallSiteFact] {
        crate::AnalysisHost::call_sites(self)
    }

    pub fn call_targets(&self) -> &[crate::calls::facts::CallTargetFact] {
        crate::AnalysisHost::call_targets(self)
    }

    pub fn unresolved_calls(&self) -> &[crate::calls::facts::UnresolvedCallFact] {
        crate::AnalysisHost::unresolved_calls(self)
    }

    pub fn unsupported_semantics(&self) -> &[crate::mir_op::UnsupportedSemanticFact] {
        crate::AnalysisHost::unsupported_semantics(self)
    }

    pub fn replace_semantic_mir(
        &mut self,
        output: crate::mir_body::MirOutput,
    ) -> Result<(), crate::AnalysisError> {
        crate::AnalysisHost::replace_semantic_mir(self, output)
    }
}

impl FactDatabase for LocalAnalysisDb {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn store(&self, family: FactFamily) -> Option<&dyn FactStore> {
        self.fact_stores.get(&family).map(|entry| entry.as_store())
    }

    fn store_mut(&mut self, family: FactFamily) -> Option<&mut dyn FactStore> {
        self.fact_stores
            .get_mut(&family)
            .map(|entry| entry.as_store_mut())
    }

    fn fact_meta(&self) -> &FactMetaStore {
        &self.fact_meta
    }

    fn fact_meta_mut(&mut self) -> &mut FactMetaStore {
        &mut self.fact_meta
    }

    fn stable_key_interner(&self) -> StableKeyInterner {
        self.stable_keys.clone()
    }

    fn files(&self) -> &[SourceFile] {
        &self.files
    }

    fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    fn path_for(&self, file: FileId) -> String {
        self.file(file)
            .map(|file| file.relative_path.clone())
            .unwrap_or_else(|| "<unknown>".to_string())
    }

    fn add_file(&mut self, path: PathBuf, relative_path: String, source: String) -> FileId {
        let language = Language::from_path(&path);
        let content_hash = fingerprint(&[&source]);
        self.add_source_file(
            path,
            relative_path,
            language,
            Arc::from(source),
            content_hash,
        )
    }

    fn add_source_file(
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

    fn push_package(&mut self, mut fact: PackageFact) -> PackageId {
        let id = PackageId(self.packages.len() as u64);
        fact.id = id;
        self.packages.push(fact);
        id
    }

    fn push_function(&mut self, mut fact: FunctionFact) -> FunctionId {
        let id = FunctionId(self.functions.len() as u64);
        fact.id = id;
        self.functions.push(fact);
        id
    }

    fn push_import(&mut self, mut fact: ImportFact) -> ImportId {
        let id = ImportId(self.imports.len() as u64);
        fact.id = id;
        self.imports.push(fact);
        id
    }

    fn push_branch(&mut self, mut fact: BranchObligation) -> BranchId {
        let id = BranchId(self.branches.len() as u64);
        fact.id = id;
        self.branches.push(fact);
        id
    }

    fn push_test(&mut self, fact: TestFact) {
        self.tests.push(fact);
    }

    fn push_coverage(&mut self, fact: CoverageFact) {
        self.coverage.push(fact);
    }

    fn push_string_literal(&mut self, fact: StringLiteralFact) {
        self.string_literals.push(fact);
    }

    fn push_ts_component(&mut self, fact: TsComponentFact) {
        self.ts_components.push(fact);
    }

    fn push_ts_class(&mut self, fact: TsClassFact) {
        self.ts_classes.push(fact);
    }

    fn push_jsx_attribute(&mut self, fact: JsxAttributeFact) {
        self.jsx_attributes.push(fact);
    }

    fn packages(&self) -> &[PackageFact] {
        &self.packages
    }

    fn functions(&self) -> &[FunctionFact] {
        &self.functions
    }

    fn imports(&self) -> &[ImportFact] {
        &self.imports
    }

    fn branches(&self) -> &[BranchObligation] {
        &self.branches
    }

    fn tests(&self) -> &[TestFact] {
        &self.tests
    }

    fn coverage(&self) -> &[CoverageFact] {
        &self.coverage
    }

    fn string_literals(&self) -> &[StringLiteralFact] {
        &self.string_literals
    }

    fn ts_components(&self) -> &[TsComponentFact] {
        &self.ts_components
    }

    fn ts_classes(&self) -> &[TsClassFact] {
        &self.ts_classes
    }

    fn jsx_attributes(&self) -> &[JsxAttributeFact] {
        &self.jsx_attributes
    }

    fn module_nodes(&self) -> &[polint_analysis_api::ModuleNode] {
        &[]
    }

    fn facts_for_file(&self, file: FileId) -> CachedFileFacts {
        CachedFileFacts {
            packages: self
                .packages()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            functions: self
                .functions()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            imports: self
                .imports()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            branches: self
                .branches()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            tests: self
                .tests()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            coverage: self.coverage().to_vec(),
            ts_components: self
                .ts_components()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            ts_classes: self
                .ts_classes()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            string_literals: self
                .string_literals()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            jsx_attributes: self
                .jsx_attributes()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
        }
    }

    fn restore_file_facts(&mut self, file: FileId, facts: CachedFileFacts) {
        for mut package in facts.packages {
            package.file = file;
            package.span.file = file;
            self.push_package(package);
        }
        for mut function in facts.functions {
            function.file = file;
            function.span.file = file;
            self.push_function(function);
        }
        for mut import in facts.imports {
            import.file = file;
            import.span.file = file;
            self.push_import(import);
        }
        for mut branch in facts.branches {
            branch.file = file;
            branch.decision_span.file = file;
            self.push_branch(branch);
        }
        for mut test in facts.tests {
            test.file = file;
            test.span.file = file;
            self.push_test(test);
        }
        for coverage in facts.coverage {
            self.push_coverage(coverage);
        }
        for mut component in facts.ts_components {
            component.file = file;
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
