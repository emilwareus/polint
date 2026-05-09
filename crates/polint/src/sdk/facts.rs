//! Typed fact views used by macro-derived rules.
//!
//! A rule requests facts by accepting these view types as parameters. The
//! `#[polint::rule]` macro derives capabilities from the view types and builds
//! the matching views at runtime.

use crate::core::{
    AnalysisDb, BranchObligation, CoverageFact, FileId, FunctionFact, FunctionId, ImportFact,
    JsxAttributeFact, Language, PackageFact, SourceFile, StringLiteralFact, TestFact, TsClassFact,
    TsComponentFact,
};

/// Public source-file view. Requesting this view maps to the `syntax` capability.
#[derive(Clone, Copy)]
pub struct SourceFiles<'a> {
    db: &'a AnalysisDb,
}

impl<'a> SourceFiles<'a> {
    /// Returns all analyzed source files in deterministic database order.
    pub fn all(self) -> &'a [SourceFile] {
        self.db.files()
    }

    /// Iterates all analyzed source files in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, SourceFile> {
        self.db.files().iter()
    }

    /// Returns the source file for a stable file ID.
    pub fn get(self, file: FileId) -> Option<&'a SourceFile> {
        self.db.file(file)
    }

    /// Returns the language for a stable file ID.
    pub fn language(self, file: FileId) -> Option<Language> {
        self.get(file).map(|source| source.language)
    }

    /// Returns a display path for a file ID, or `<unknown>` if missing.
    pub fn path_for(self, file: FileId) -> String {
        self.db.path_for(file)
    }
}

/// Package fact view. Requesting this view maps to the `syntax` capability.
#[derive(Clone, Copy)]
pub struct Packages<'a> {
    db: &'a AnalysisDb,
}

impl<'a> Packages<'a> {
    /// Returns all package facts in deterministic database order.
    pub fn all(self) -> &'a [PackageFact] {
        self.db.packages()
    }

    /// Iterates all package facts in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, PackageFact> {
        self.db.packages().iter()
    }
}

/// Function fact view. Requesting this view maps to the `syntax` capability.
#[derive(Clone, Copy)]
pub struct Functions<'a> {
    db: &'a AnalysisDb,
}

impl<'a> Functions<'a> {
    /// Returns all function facts in deterministic database order.
    pub fn all(self) -> &'a [FunctionFact] {
        self.db.functions()
    }

    /// Iterates all function facts in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, FunctionFact> {
        self.db.functions().iter()
    }

    /// Returns function facts for a file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a FunctionFact> {
        self.db
            .functions()
            .iter()
            .filter(move |function| function.file == file)
    }
}

/// Import fact view. Requesting this view maps to the `imports` capability.
#[derive(Clone, Copy)]
pub struct Imports<'a> {
    db: &'a AnalysisDb,
}

impl<'a> Imports<'a> {
    /// Returns all import facts in deterministic database order.
    pub fn all(self) -> &'a [ImportFact] {
        self.db.imports()
    }

    /// Iterates all import facts in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, ImportFact> {
        self.db.imports().iter()
    }

    /// Returns import facts for a file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a ImportFact> {
        self.db
            .imports()
            .iter()
            .filter(move |import| import.file == file)
    }

    /// Returns syntactic import graph edges as `(source file, import)` pairs.
    pub fn edges(self) -> impl Iterator<Item = (&'a SourceFile, &'a ImportFact)> {
        self.db
            .imports()
            .iter()
            .filter_map(move |import| self.db.file(import.file).map(|file| (file, import)))
    }
}

/// Branch obligation fact view. Requesting this view maps to the `branch_obligations` capability.
#[derive(Clone, Copy)]
pub struct BranchObligations<'a> {
    db: &'a AnalysisDb,
}

impl<'a> BranchObligations<'a> {
    /// Returns all branch obligations in deterministic database order.
    pub fn all(self) -> &'a [BranchObligation] {
        self.db.branches()
    }

    /// Iterates all branch obligations in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, BranchObligation> {
        self.db.branches().iter()
    }

    /// Returns branch obligations for a file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a BranchObligation> {
        self.db
            .branches()
            .iter()
            .filter(move |branch| branch.file == file)
    }

    /// Returns branch obligations for a function without cloning facts.
    pub fn for_function(self, function: FunctionId) -> impl Iterator<Item = &'a BranchObligation> {
        self.db
            .branches()
            .iter()
            .filter(move |branch| branch.function == Some(function))
    }
}

/// Go test fact view. Requesting this view maps to the `go_tests` capability.
#[derive(Clone, Copy)]
pub struct GoTests<'a> {
    db: &'a AnalysisDb,
}

impl<'a> GoTests<'a> {
    /// Returns all Go test facts in deterministic database order.
    pub fn all(self) -> &'a [TestFact] {
        self.db.tests()
    }

    /// Iterates all Go test facts in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, TestFact> {
        self.db.tests().iter()
    }

    /// Returns Go test facts for a file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a TestFact> {
        self.db.tests().iter().filter(move |test| test.file == file)
    }

    /// Returns Go tests related to a source file.
    pub fn related_for_file(self, file: FileId) -> Vec<&'a TestFact> {
        let Some(source_file) = self.db.file(file) else {
            return Vec::new();
        };
        if source_file.language != Language::Go {
            return Vec::new();
        }

        let source_path = std::path::Path::new(&source_file.relative_path);
        let source_name = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();

        if source_name.ends_with("_test.go") {
            return self.for_file(file).collect();
        }

        let source_dir = source_path.parent();
        self.db
            .tests()
            .iter()
            .filter(|test| {
                if test.file == file {
                    return true;
                }
                let Some(test_file) = self.db.file(test.file) else {
                    return false;
                };
                if test_file.language != Language::Go {
                    return false;
                }
                let test_path = std::path::Path::new(&test_file.relative_path);
                let test_name = test_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                test_name.ends_with("_test.go") && test_path.parent() == source_dir
            })
            .collect()
    }
}

/// TS/JS component fact view. Requesting this view maps to the `ts_components` capability.
#[derive(Clone, Copy)]
pub struct TsComponents<'a> {
    db: &'a AnalysisDb,
}

impl<'a> TsComponents<'a> {
    /// Returns all component facts in deterministic database order.
    pub fn all(self) -> &'a [TsComponentFact] {
        self.db.ts_components()
    }

    /// Iterates all component facts in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, TsComponentFact> {
        self.db.ts_components().iter()
    }

    /// Returns component facts for a file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a TsComponentFact> {
        self.db
            .ts_components()
            .iter()
            .filter(move |component| component.file == file)
    }
}

/// TS/JS class fact view. Requesting this view maps to the `ts_classes` capability.
#[derive(Clone, Copy)]
pub struct TsClasses<'a> {
    db: &'a AnalysisDb,
}

impl<'a> TsClasses<'a> {
    /// Returns all class facts in deterministic database order.
    pub fn all(self) -> &'a [TsClassFact] {
        self.db.ts_classes()
    }

    /// Iterates all class facts in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, TsClassFact> {
        self.db.ts_classes().iter()
    }

    /// Returns class facts for a file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a TsClassFact> {
        self.db
            .ts_classes()
            .iter()
            .filter(move |class| class.file == file)
    }
}

/// String-literal fact view. Requesting this view maps to the `string_literals` capability.
#[derive(Clone, Copy)]
pub struct StringLiterals<'a> {
    db: &'a AnalysisDb,
}

impl<'a> StringLiterals<'a> {
    /// Returns all string literal facts in deterministic database order.
    pub fn all(self) -> &'a [StringLiteralFact] {
        self.db.string_literals()
    }

    /// Iterates all string literal facts in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, StringLiteralFact> {
        self.db.string_literals().iter()
    }

    /// Returns string literal facts for a file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a StringLiteralFact> {
        self.db
            .string_literals()
            .iter()
            .filter(move |literal| literal.file == file)
    }
}

/// JSX attribute fact view. Requesting this view maps to the `jsx_attributes` capability.
#[derive(Clone, Copy)]
pub struct JsxAttributes<'a> {
    db: &'a AnalysisDb,
}

impl<'a> JsxAttributes<'a> {
    /// Returns all JSX attribute facts in deterministic database order.
    pub fn all(self) -> &'a [JsxAttributeFact] {
        self.db.jsx_attributes()
    }

    /// Iterates all JSX attribute facts in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, JsxAttributeFact> {
        self.db.jsx_attributes().iter()
    }

    /// Returns JSX attribute facts for a file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a JsxAttributeFact> {
        self.db
            .jsx_attributes()
            .iter()
            .filter(move |attribute| attribute.file == file)
    }
}

/// Reserved CFG fact view. Requesting this view currently maps to unsupported `cfg`.
#[derive(Clone, Copy)]
pub struct Cfg<'a> {
    _db: &'a AnalysisDb,
}

/// Reserved call-graph fact view. Requesting this view currently maps to unsupported `call_graph`.
#[derive(Clone, Copy)]
pub struct CallGraph<'a> {
    _db: &'a AnalysisDb,
}

/// Reserved dataflow fact view. Requesting this view currently maps to unsupported `dataflow`.
#[derive(Clone, Copy)]
pub struct DataFlow<'a> {
    _db: &'a AnalysisDb,
}

/// Reserved coverage fact view. Requesting this view currently maps to unsupported `coverage_facts`.
#[derive(Clone, Copy)]
pub struct CoverageFacts<'a> {
    db: &'a AnalysisDb,
}

impl<'a> CoverageFacts<'a> {
    /// Returns all coverage facts currently stored in the database.
    pub fn all(self) -> &'a [CoverageFact] {
        self.db.coverage()
    }

    /// Iterates all coverage facts currently stored in the database.
    pub fn iter(self) -> std::slice::Iter<'a, CoverageFact> {
        self.db.coverage().iter()
    }
}

/// Reserved test-suite metric view. Requesting this view currently maps to unsupported `test_suite_metrics`.
#[derive(Clone, Copy)]
pub struct TestSuiteMetrics<'a> {
    _db: &'a AnalysisDb,
}

/// Hidden trait used by the `#[polint::rule]` macro to construct fact views.
#[doc(hidden)]
pub trait FactView<'a>: Sized {
    /// Builds a view for the current analysis database.
    fn build(db: &'a AnalysisDb) -> Self;
}

macro_rules! impl_fact_view {
    ($ty:ident) => {
        impl<'a> FactView<'a> for $ty<'a> {
            fn build(db: &'a AnalysisDb) -> Self {
                Self { db }
            }
        }
    };
    ($ty:ident, $field:ident) => {
        impl<'a> FactView<'a> for $ty<'a> {
            fn build(db: &'a AnalysisDb) -> Self {
                Self { $field: db }
            }
        }
    };
}

impl_fact_view!(SourceFiles);
impl_fact_view!(Packages);
impl_fact_view!(Functions);
impl_fact_view!(Imports);
impl_fact_view!(BranchObligations);
impl_fact_view!(GoTests);
impl_fact_view!(TsComponents);
impl_fact_view!(TsClasses);
impl_fact_view!(StringLiterals);
impl_fact_view!(JsxAttributes);
impl_fact_view!(CoverageFacts);
impl_fact_view!(Cfg, _db);
impl_fact_view!(CallGraph, _db);
impl_fact_view!(DataFlow, _db);
impl_fact_view!(TestSuiteMetrics, _db);
