//! Minimal [`FactDatabase`] used as a per-file scratch pad while parsing Go sources.

use std::any::Any;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use polint_analysis_api::{
    BranchObligation, CachedFileFacts, CoverageFact, FactDatabase, FactFamily, FactMetaStore,
    FactStore, FactStoreEntry, FunctionFact, ImportFact, JsxAttributeFact, PackageFact, SourceFile,
    StringLiteralFact, TestFact, TsClassFact, TsComponentFact,
};
use polint_core::{
    BranchId, FileId, FunctionId, ImportId, Language, PackageId, StableKeyInterner, fingerprint,
};

use crate::semantic::store::{GO_SEMANTIC_STORE_FAMILY, GoSemanticStore};
use crate::syntax_store::{GO_SYNTAX_STORE_FAMILY, GoSyntaxStore};

#[derive(Debug, Default)]
pub(crate) struct LocalFactDb {
    files: Vec<SourceFile>,
    stable_keys: StableKeyInterner,
    fact_meta: FactMetaStore,
    fact_stores: BTreeMap<FactFamily, FactStoreEntry>,
    string_literals: Vec<StringLiteralFact>,
    coverage: Vec<CoverageFact>,
    ts_components: Vec<TsComponentFact>,
    ts_classes: Vec<TsClassFact>,
    jsx_attributes: Vec<JsxAttributeFact>,
}

impl LocalFactDb {
    pub(crate) fn new() -> Self {
        let mut fact_stores = BTreeMap::new();
        fact_stores.insert(
            GO_SYNTAX_STORE_FAMILY,
            FactStoreEntry::new(GoSyntaxStore::default()),
        );
        fact_stores.insert(
            GO_SEMANTIC_STORE_FAMILY,
            FactStoreEntry::new(GoSemanticStore::default()),
        );
        Self {
            files: Vec::new(),
            stable_keys: StableKeyInterner::default(),
            fact_meta: FactMetaStore::default(),
            fact_stores,
            string_literals: Vec::new(),
            coverage: Vec::new(),
            ts_components: Vec::new(),
            ts_classes: Vec::new(),
            jsx_attributes: Vec::new(),
        }
    }

    fn go_syntax(&self) -> &GoSyntaxStore {
        self.fact_stores
            .get(&GO_SYNTAX_STORE_FAMILY)
            .and_then(|entry| entry.as_store().as_any().downcast_ref::<GoSyntaxStore>())
            .expect("GoSyntaxStore installed")
    }

    fn go_syntax_mut(&mut self) -> &mut GoSyntaxStore {
        self.fact_stores
            .get_mut(&GO_SYNTAX_STORE_FAMILY)
            .and_then(|entry| {
                entry
                    .as_store_mut()
                    .as_any_mut()
                    .downcast_mut::<GoSyntaxStore>()
            })
            .expect("GoSyntaxStore installed")
    }
}

impl FactDatabase for LocalFactDb {
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

    fn push_package(&mut self, fact: PackageFact) -> PackageId {
        self.go_syntax_mut().push_package(fact)
    }

    fn push_function(&mut self, fact: FunctionFact) -> FunctionId {
        self.go_syntax_mut().push_function(fact)
    }

    fn push_import(&mut self, fact: ImportFact) -> ImportId {
        self.go_syntax_mut().push_import(fact)
    }

    fn push_branch(&mut self, fact: BranchObligation) -> BranchId {
        self.go_syntax_mut().push_branch(fact)
    }

    fn push_test(&mut self, fact: TestFact) {
        let _ = self.go_syntax_mut().push_test(fact);
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
        self.go_syntax().packages()
    }

    fn functions(&self) -> &[FunctionFact] {
        self.go_syntax().functions()
    }

    fn imports(&self) -> &[ImportFact] {
        self.go_syntax().imports()
    }

    fn branches(&self) -> &[BranchObligation] {
        self.go_syntax().branches()
    }

    fn tests(&self) -> &[TestFact] {
        self.go_syntax().tests()
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

    fn symbols(&self) -> &[polint_analysis_api::SymbolFact] {
        &[]
    }

    fn definitions(&self) -> &[polint_analysis_api::DefinitionFact] {
        &[]
    }

    fn references(&self) -> &[polint_analysis_api::ReferenceFact] {
        &[]
    }

    fn replace_symbol_facts(
        &mut self,
        _symbols: Vec<polint_analysis_api::SymbolFact>,
        _definitions: Vec<polint_analysis_api::DefinitionFact>,
        _references: Vec<polint_analysis_api::ReferenceFact>,
    ) {
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
            coverage: self
                .coverage()
                .iter()
                .filter(|fact| {
                    self.branches()
                        .iter()
                        .any(|branch| branch.id == fact.branch && branch.file == file)
                })
                .cloned()
                .collect(),
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
            let restored = self.push_function(function);
            function_ids.insert(cached_id, restored);
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
            let restored = self.push_branch(branch);
            branch_ids.insert(cached_id, restored);
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
