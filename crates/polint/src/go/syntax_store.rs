//! Go (and shared) syntax fact store registered on the host [`FactDatabase`].

use std::any::Any;

use crate::analysis_api::{
    BranchObligation, FactFamily, FactStore, FunctionFact, GoTypeDeclFact, ImportFact, PackageFact,
    TestFact,
};
use crate::internal_core::{BranchId, FunctionId, ImportId, PackageId};

/// Syntax facts produced by `polint.go.syntax` (packages/functions/imports/branches/tests).
///
/// Shared function/import rows may also be written by the TS frontend through the same host
/// accessors; the store itself is owned by this crate.
#[derive(Debug, Clone, Default)]
pub struct GoSyntaxStore {
    pub packages: Vec<PackageFact>,
    pub functions: Vec<FunctionFact>,
    pub imports: Vec<ImportFact>,
    pub branches: Vec<BranchObligation>,
    pub tests: Vec<TestFact>,
    pub go_types: Vec<GoTypeDeclFact>,
}

impl GoSyntaxStore {
    pub fn packages(&self) -> &[PackageFact] {
        &self.packages
    }

    pub fn functions(&self) -> &[FunctionFact] {
        &self.functions
    }

    pub fn imports(&self) -> &[ImportFact] {
        &self.imports
    }

    pub fn branches(&self) -> &[BranchObligation] {
        &self.branches
    }

    pub fn tests(&self) -> &[TestFact] {
        &self.tests
    }

    pub fn push_package(&mut self, mut fact: PackageFact) -> PackageId {
        let id = PackageId::from_raw(self.packages.len() as u64);
        fact.id = id;
        self.packages.push(fact);
        id
    }

    pub fn push_function(&mut self, mut fact: FunctionFact) -> FunctionId {
        let id = FunctionId::from_raw(self.functions.len() as u64);
        fact.id = id;
        self.functions.push(fact);
        id
    }

    pub fn push_import(&mut self, mut fact: ImportFact) -> ImportId {
        let id = ImportId::from_raw(self.imports.len() as u64);
        fact.id = id;
        self.imports.push(fact);
        id
    }

    pub fn push_branch(&mut self, mut fact: BranchObligation) -> BranchId {
        let id = BranchId::from_raw(self.branches.len() as u64);
        fact.id = id;
        self.branches.push(fact);
        id
    }

    pub fn push_test(&mut self, fact: TestFact) -> u64 {
        let run_id = self.tests.len() as u64;
        self.tests.push(fact);
        run_id
    }

    pub fn go_types(&self) -> &[GoTypeDeclFact] {
        &self.go_types
    }

    pub fn push_go_type(&mut self, fact: GoTypeDeclFact) {
        self.go_types.push(fact);
    }
}

impl FactStore for GoSyntaxStore {
    fn family(&self) -> FactFamily {
        FactFamily::Package
    }

    fn clear(&mut self) {
        self.packages.clear();
        self.functions.clear();
        self.imports.clear();
        self.branches.clear();
        self.tests.clear();
        self.go_types.clear();
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

/// Registry key used for [`GoSyntaxStore`] in the host fact-store map.
pub const GO_SYNTAX_STORE_FAMILY: FactFamily = FactFamily::Package;
