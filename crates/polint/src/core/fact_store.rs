//! Provider-owned fact stores behind a keyed registry on [`super::AnalysisDb`].
//!
//! Each store owns the vectors (and indexes) for one provider group. The SDK and
//! `AnalysisDb` accessors stay typed; the registry holds `dyn FactStore` for
//! later eviction and language-neutral core layout.

use std::any::Any;
use std::fmt;

use crate::analysis_kernel::FactFamily;
use crate::core::facts::{BranchObligation, FunctionFact, ImportFact, PackageFact, TestFact};
use crate::core::ids::{BranchId, FunctionId, ImportId, PackageId};

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
