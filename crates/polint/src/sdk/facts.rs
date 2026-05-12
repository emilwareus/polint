//! Typed fact views used by macro-derived rules.
//!
//! A rule requests facts by accepting these view types as parameters. The
//! `#[polint::rule]` macro derives capabilities from the view types and builds
//! the matching views at runtime.

use crate::core::{
    AnalysisDb, BranchObligation, ComplexityMetricFact, CoverageFact, FileId, FileMetricFact,
    FunctionFact, FunctionId, FunctionMetricFact, ImportFact, ImportId, JsxAttributeFact, Language,
    ModuleEdge, ModuleNode, ModuleNodeId, PackageFact, ResolutionStatus, ResolvedImportFact,
    SourceFile, StringLiteralFact, TestFact, TsClassFact, TsComponentFact,
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

/// Source-file metric view. Requesting this view maps to the `file_metrics` capability.
#[derive(Clone, Copy)]
pub struct FileMetrics<'a> {
    db: &'a AnalysisDb,
}

impl<'a> FileMetrics<'a> {
    /// Returns all derived file metrics in deterministic database order.
    pub fn all(self) -> &'a [FileMetricFact] {
        self.db.file_metrics()
    }

    /// Iterates all derived file metrics in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, FileMetricFact> {
        self.db.file_metrics().iter()
    }

    /// Returns derived metrics for one source file.
    pub fn get(self, file: FileId) -> Option<&'a FileMetricFact> {
        self.db
            .file_metrics()
            .iter()
            .find(|metric| metric.file == file)
    }

    /// Returns file metrics for one language.
    pub fn for_language(self, language: Language) -> impl Iterator<Item = &'a FileMetricFact> {
        self.db
            .file_metrics()
            .iter()
            .filter(move |metric| metric.language == language)
    }
}

/// Function-size metric view. Requesting this view maps to the `function_metrics` capability.
#[derive(Clone, Copy)]
pub struct FunctionMetrics<'a> {
    db: &'a AnalysisDb,
}

impl<'a> FunctionMetrics<'a> {
    /// Returns all derived function-size metrics in deterministic database order.
    pub fn all(self) -> &'a [FunctionMetricFact] {
        self.db.function_metrics()
    }

    /// Iterates all derived function-size metrics in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, FunctionMetricFact> {
        self.db.function_metrics().iter()
    }

    /// Returns function-size metrics for a file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a FunctionMetricFact> {
        self.db
            .function_metrics()
            .iter()
            .filter(move |metric| metric.file == file)
    }

    /// Returns function-size metrics for one function.
    pub fn get(self, function: FunctionId) -> Option<&'a FunctionMetricFact> {
        self.db
            .function_metrics()
            .iter()
            .find(|metric| metric.function == function)
    }
}

/// Complexity metric view. Requesting this view maps to the `complexity_metrics` capability.
#[derive(Clone, Copy)]
pub struct ComplexityMetrics<'a> {
    db: &'a AnalysisDb,
}

impl<'a> ComplexityMetrics<'a> {
    /// Returns all derived complexity metrics in deterministic database order.
    pub fn all(self) -> &'a [ComplexityMetricFact] {
        self.db.complexity_metrics()
    }

    /// Iterates all derived complexity metrics in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, ComplexityMetricFact> {
        self.db.complexity_metrics().iter()
    }

    /// Returns complexity metrics for a file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a ComplexityMetricFact> {
        self.db
            .complexity_metrics()
            .iter()
            .filter(move |metric| metric.file == file)
    }

    /// Returns complexity metrics for one function.
    pub fn get(self, function: FunctionId) -> Option<&'a ComplexityMetricFact> {
        self.db
            .complexity_metrics()
            .iter()
            .find(|metric| metric.function == function)
    }

    /// Iterates functions whose cyclomatic complexity is greater than `max`.
    pub fn over(self, max: u32) -> impl Iterator<Item = &'a ComplexityMetricFact> {
        self.db
            .complexity_metrics()
            .iter()
            .filter(move |metric| metric.cyclomatic_complexity > max)
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

/// Resolved import fact view. Requesting this view maps to the `resolved_imports` capability.
#[derive(Clone, Copy)]
pub struct ResolvedImports<'a> {
    db: &'a AnalysisDb,
}

impl<'a> ResolvedImports<'a> {
    /// Returns all resolved import facts in deterministic database order.
    pub fn all(self) -> &'a [ResolvedImportFact] {
        self.db.resolved_imports()
    }

    /// Iterates all resolved import facts in deterministic database order.
    pub fn iter(self) -> std::slice::Iter<'a, ResolvedImportFact> {
        self.db.resolved_imports().iter()
    }

    /// Returns resolved import facts for a source file without cloning facts.
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a ResolvedImportFact> {
        self.db
            .resolved_imports()
            .iter()
            .filter(move |resolved| resolved.from_file == file)
    }

    /// Returns setup-missing, dynamic, unsupported, or unresolved imports for a source file.
    pub fn unresolved_for_file(self, file: FileId) -> impl Iterator<Item = &'a ResolvedImportFact> {
        self.for_file(file).filter(|resolved| {
            matches!(
                resolved.status,
                ResolutionStatus::Unresolved
                    | ResolutionStatus::SetupMissing
                    | ResolutionStatus::Dynamic
                    | ResolutionStatus::Unsupported
            )
        })
    }

    /// Returns the resolved import record for a syntactic import ID.
    pub fn for_import(self, import: ImportId) -> Option<&'a ResolvedImportFact> {
        self.db
            .resolved_imports()
            .iter()
            .find(|resolved| resolved.import == import)
    }
}

/// Module relationship graph fact view. Requesting this view maps to the `module_graph` capability.
#[derive(Clone, Copy)]
pub struct ModuleGraphFacts<'a> {
    db: &'a AnalysisDb,
}

impl<'a> ModuleGraphFacts<'a> {
    /// Returns all module graph nodes in deterministic database order.
    pub fn nodes(self) -> &'a [ModuleNode] {
        self.db.module_nodes()
    }

    /// Returns all module graph edges in deterministic database order.
    pub fn edges(self) -> &'a [ModuleEdge] {
        self.db.module_edges()
    }

    /// Returns the first file node for a source file ID, if one exists.
    pub fn node_for_file(self, file: FileId) -> Option<ModuleNodeId> {
        self.db
            .module_nodes()
            .iter()
            .find(|node| node.file == Some(file))
            .map(|node| node.id)
    }

    /// Returns outgoing graph edges for a node without cloning facts.
    pub fn outgoing(self, node: ModuleNodeId) -> impl Iterator<Item = &'a ModuleEdge> {
        self.db
            .module_edges()
            .iter()
            .filter(move |edge| edge.from == node)
    }

    /// Returns incoming graph edges for a node without cloning facts.
    pub fn incoming(self, node: ModuleNodeId) -> impl Iterator<Item = &'a ModuleEdge> {
        self.db
            .module_edges()
            .iter()
            .filter(move |edge| edge.to == node)
    }

    /// Returns the resolution status attached to a dependency edge.
    pub fn dependency_status(self, edge: &ModuleEdge) -> ResolutionStatus {
        edge.status
    }

    /// Computes deterministic breadth-first reachability over resolved or external edges.
    pub fn reachable_from(self, node: ModuleNodeId) -> Vec<ModuleNodeId> {
        let mut seen = std::collections::BTreeSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut reachable = Vec::new();

        seen.insert(node);
        queue.push_back(node);

        while let Some(current) = queue.pop_front() {
            for edge in self.outgoing(current) {
                if !matches!(
                    edge.status,
                    ResolutionStatus::Resolved | ResolutionStatus::External
                ) {
                    continue;
                }

                if seen.insert(edge.to) {
                    reachable.push(edge.to);
                    queue.push_back(edge.to);
                }
            }
        }

        reachable
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
impl_fact_view!(FileMetrics);
impl_fact_view!(FunctionMetrics);
impl_fact_view!(ComplexityMetrics);
impl_fact_view!(Imports);
impl_fact_view!(ResolvedImports);
impl_fact_view!(ModuleGraphFacts);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        AnalysisDb, ComplexityMetricFact, DefinitionFact, DefinitionId, DefinitionKind,
        FileMetricFact, FunctionId, FunctionMetricFact, ImportFact, ModuleEdge, ModuleEdgeId,
        ModuleEdgeKind, ModuleNode, ModuleNodeId, ModuleNodeKind, ReferenceFact, ReferenceId,
        ReferenceKind, ResolutionPrecision, ResolutionStatus, ResolvedImportFact,
        ResolvedImportId, Span, SymbolFact, SymbolId, SymbolKind, SymbolNamespace,
        SymbolPrecision, SymbolResolutionStatus, UnresolvedReason,
    };
    use std::path::PathBuf;

    #[test]
    fn metric_views_query_by_file_function_language_and_threshold() {
        let mut db = AnalysisDb::new();
        let go_file = db.add_file(
            PathBuf::from("src/router.go"),
            "src/router.go".to_string(),
            "package app\nfunc route() {}\n".to_string(),
        );
        let ts_file = db.add_file(
            PathBuf::from("src/panel.ts"),
            "src/panel.ts".to_string(),
            "export function Panel() {}\n".to_string(),
        );
        let route_function = FunctionId(0);
        let panel_function = FunctionId(1);
        let route_span = Span {
            file: go_file,
            start_byte: 12,
            end_byte: 27,
            start_line: 2,
            start_col: 1,
            end_line: 2,
            end_col: 16,
        };
        let panel_span = Span {
            file: ts_file,
            start_byte: 0,
            end_byte: 26,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 27,
        };

        db.replace_metric_facts(
            vec![
                FileMetricFact {
                    file: go_file,
                    language: Language::Go,
                    line_count: 2,
                    non_empty_line_count: 2,
                    byte_count: 28,
                    function_count: 1,
                },
                FileMetricFact {
                    file: ts_file,
                    language: Language::TypeScript,
                    line_count: 1,
                    non_empty_line_count: 1,
                    byte_count: 27,
                    function_count: 1,
                },
            ],
            vec![
                FunctionMetricFact {
                    function: route_function,
                    file: go_file,
                    name: "route".to_string(),
                    span: route_span.clone(),
                    language: Language::Go,
                    line_count: 1,
                    byte_count: 15,
                },
                FunctionMetricFact {
                    function: panel_function,
                    file: ts_file,
                    name: "Panel".to_string(),
                    span: panel_span.clone(),
                    language: Language::TypeScript,
                    line_count: 1,
                    byte_count: 26,
                },
            ],
            vec![
                ComplexityMetricFact {
                    function: route_function,
                    file: go_file,
                    name: "route".to_string(),
                    span: route_span,
                    language: Language::Go,
                    cyclomatic_complexity: 1,
                },
                ComplexityMetricFact {
                    function: panel_function,
                    file: ts_file,
                    name: "Panel".to_string(),
                    span: panel_span,
                    language: Language::TypeScript,
                    cyclomatic_complexity: 3,
                },
            ],
        );

        let files = FileMetrics::build(&db);
        let functions = FunctionMetrics::build(&db);
        let complexity = ComplexityMetrics::build(&db);

        assert_eq!(
            files.get(go_file).map(|metric| metric.function_count),
            Some(1)
        );
        assert_eq!(
            files
                .for_language(Language::TypeScript)
                .map(|metric| metric.file)
                .collect::<Vec<_>>(),
            vec![ts_file]
        );
        assert_eq!(
            functions
                .for_file(go_file)
                .map(|metric| metric.name.as_str())
                .collect::<Vec<_>>(),
            vec!["route"]
        );
        assert_eq!(
            functions
                .get(panel_function)
                .map(|metric| metric.name.as_str()),
            Some("Panel")
        );
        assert_eq!(
            complexity
                .for_file(ts_file)
                .map(|metric| metric.cyclomatic_complexity)
                .collect::<Vec<_>>(),
            vec![3]
        );
        assert_eq!(
            complexity
                .over(1)
                .map(|metric| metric.function)
                .collect::<Vec<_>>(),
            vec![panel_function]
        );
    }

    #[test]
    fn module_graph_sdk_views_resolved_imports_queries() {
        let mut db = AnalysisDb::new();
        let source_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import { Button } from './button';\nimport missing from './missing';\n".to_string(),
        );
        let target_file = db.add_file(
            PathBuf::from("src/button.ts"),
            "src/button.ts".to_string(),
            "export function Button() {}\n".to_string(),
        );
        let span = Span::point(source_file, 1, 1);
        let local_import = db.push_import(ImportFact {
            id: crate::core::ImportId(99),
            file: source_file,
            package: None,
            path: "./button".to_string(),
            span: span.clone(),
            language: crate::core::Language::TypeScript,
        });
        let missing_import = db.push_import(ImportFact {
            id: crate::core::ImportId(99),
            file: source_file,
            package: None,
            path: "./missing".to_string(),
            span,
            language: crate::core::Language::TypeScript,
        });

        db.replace_module_graph_facts(
            vec![
                ResolvedImportFact {
                    id: ResolvedImportId(99),
                    import: local_import,
                    from_file: source_file,
                    target_node: Some(ModuleNodeId(1)),
                    status: ResolutionStatus::Resolved,
                    precision: ResolutionPrecision::ExactFile,
                    reason: None,
                },
                ResolvedImportFact {
                    id: ResolvedImportId(99),
                    import: missing_import,
                    from_file: source_file,
                    target_node: None,
                    status: ResolutionStatus::Unresolved,
                    precision: ResolutionPrecision::None,
                    reason: Some(UnresolvedReason::NotFound),
                },
            ],
            vec![
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::File,
                    label: "src/app.ts".to_string(),
                    file: Some(source_file),
                    package: None,
                    language: Some(crate::core::Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::File,
                    label: "src/button.ts".to_string(),
                    file: Some(target_file),
                    package: None,
                    language: Some(crate::core::Language::TypeScript),
                },
            ],
            Vec::new(),
        );

        let resolved = ResolvedImports::build(&db);
        assert_eq!(resolved.all().len(), 2);
        assert_eq!(
            resolved.iter().map(|fact| fact.id).collect::<Vec<_>>(),
            vec![ResolvedImportId(0), ResolvedImportId(1)]
        );
        assert_eq!(
            resolved
                .for_file(source_file)
                .map(|fact| fact.import)
                .collect::<Vec<_>>(),
            vec![local_import, missing_import]
        );
        assert_eq!(
            resolved
                .unresolved_for_file(source_file)
                .map(|fact| fact.import)
                .collect::<Vec<_>>(),
            vec![missing_import]
        );
        assert_eq!(
            resolved
                .for_import(local_import)
                .map(|fact| fact.target_node),
            Some(Some(ModuleNodeId(1)))
        );
    }

    #[test]
    fn module_graph_sdk_views_graph_queries_are_deterministic() {
        let mut db = AnalysisDb::new();
        let app_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import { Button } from './button';\nimport React from 'react';\n".to_string(),
        );
        let button_file = db.add_file(
            PathBuf::from("src/button.ts"),
            "src/button.ts".to_string(),
            "import { token } from './tokens';\nexport function Button() {}\n".to_string(),
        );
        let token_file = db.add_file(
            PathBuf::from("src/tokens.ts"),
            "src/tokens.ts".to_string(),
            "export const token = 'primary';\n".to_string(),
        );
        let app_import = db.push_import(ImportFact {
            id: crate::core::ImportId(99),
            file: app_file,
            package: None,
            path: "./button".to_string(),
            span: Span::point(app_file, 1, 1),
            language: crate::core::Language::TypeScript,
        });
        let react_import = db.push_import(ImportFact {
            id: crate::core::ImportId(99),
            file: app_file,
            package: None,
            path: "react".to_string(),
            span: Span::point(app_file, 2, 1),
            language: crate::core::Language::TypeScript,
        });
        let token_import = db.push_import(ImportFact {
            id: crate::core::ImportId(99),
            file: button_file,
            package: None,
            path: "./tokens".to_string(),
            span: Span::point(button_file, 1, 1),
            language: crate::core::Language::TypeScript,
        });

        db.replace_module_graph_facts(
            vec![
                ResolvedImportFact {
                    id: ResolvedImportId(99),
                    import: app_import,
                    from_file: app_file,
                    target_node: Some(ModuleNodeId(1)),
                    status: ResolutionStatus::Resolved,
                    precision: ResolutionPrecision::ExactFile,
                    reason: None,
                },
                ResolvedImportFact {
                    id: ResolvedImportId(99),
                    import: react_import,
                    from_file: app_file,
                    target_node: Some(ModuleNodeId(3)),
                    status: ResolutionStatus::External,
                    precision: ResolutionPrecision::ExternalPackage,
                    reason: None,
                },
                ResolvedImportFact {
                    id: ResolvedImportId(99),
                    import: token_import,
                    from_file: button_file,
                    target_node: Some(ModuleNodeId(2)),
                    status: ResolutionStatus::Resolved,
                    precision: ResolutionPrecision::ExactFile,
                    reason: None,
                },
            ],
            vec![
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::File,
                    label: "src/app.ts".to_string(),
                    file: Some(app_file),
                    package: None,
                    language: Some(crate::core::Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::File,
                    label: "src/button.ts".to_string(),
                    file: Some(button_file),
                    package: None,
                    language: Some(crate::core::Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::File,
                    label: "src/tokens.ts".to_string(),
                    file: Some(token_file),
                    package: None,
                    language: Some(crate::core::Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::External,
                    label: "react".to_string(),
                    file: None,
                    package: None,
                    language: Some(crate::core::Language::TypeScript),
                },
            ],
            vec![
                ModuleEdge {
                    id: ModuleEdgeId(99),
                    from: ModuleNodeId(0),
                    to: ModuleNodeId(1),
                    import: Some(app_import),
                    resolved_import: Some(ResolvedImportId(0)),
                    kind: ModuleEdgeKind::Imports,
                    status: ResolutionStatus::Resolved,
                },
                ModuleEdge {
                    id: ModuleEdgeId(99),
                    from: ModuleNodeId(0),
                    to: ModuleNodeId(3),
                    import: Some(react_import),
                    resolved_import: Some(ResolvedImportId(1)),
                    kind: ModuleEdgeKind::DependsOn,
                    status: ResolutionStatus::External,
                },
                ModuleEdge {
                    id: ModuleEdgeId(99),
                    from: ModuleNodeId(1),
                    to: ModuleNodeId(2),
                    import: Some(token_import),
                    resolved_import: Some(ResolvedImportId(2)),
                    kind: ModuleEdgeKind::Imports,
                    status: ResolutionStatus::Resolved,
                },
            ],
        );

        let graph = ModuleGraphFacts::build(&db);
        assert_eq!(graph.nodes().len(), 4);
        assert_eq!(graph.edges().len(), 3);
        assert_eq!(graph.node_for_file(app_file), Some(ModuleNodeId(0)));
        assert_eq!(
            graph
                .outgoing(ModuleNodeId(0))
                .map(|edge| edge.to)
                .collect::<Vec<_>>(),
            vec![ModuleNodeId(1), ModuleNodeId(3)]
        );
        assert_eq!(
            graph
                .incoming(ModuleNodeId(2))
                .map(|edge| edge.from)
                .collect::<Vec<_>>(),
            vec![ModuleNodeId(1)]
        );
        assert_eq!(
            graph.dependency_status(&graph.edges()[1]),
            ResolutionStatus::External
        );
        assert_eq!(
            graph.reachable_from(ModuleNodeId(0)),
            vec![ModuleNodeId(1), ModuleNodeId(3), ModuleNodeId(2)]
        );
    }

    #[test]
    fn symbol_sdk_views_query_borrowed_facts_deterministically() {
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
        let button = SymbolId(10);
        let theme = SymbolId(20);

        db.replace_symbol_graph_facts(
            vec![
                SymbolFact {
                    id: button,
                    language: Language::TypeScript,
                    name: "Button".to_string(),
                    qualified_name: "src/app.ts::Button".to_string(),
                    kind: SymbolKind::Function,
                    namespace: SymbolNamespace::Value,
                    file: Some(app_file),
                    package: None,
                    module: Some(ModuleNodeId(0)),
                    owner: None,
                    primary_span: Some(Span::point(app_file, 1, 1)),
                    is_exported: true,
                    stable_key: "ts|src/app.ts|Button".to_string(),
                    precision: SymbolPrecision::ExactLocal,
                },
                SymbolFact {
                    id: theme,
                    language: Language::TypeScript,
                    name: "theme".to_string(),
                    qualified_name: "src/theme.ts::theme".to_string(),
                    kind: SymbolKind::Constant,
                    namespace: SymbolNamespace::Value,
                    file: Some(theme_file),
                    package: None,
                    module: Some(ModuleNodeId(1)),
                    owner: None,
                    primary_span: Some(Span::point(theme_file, 1, 1)),
                    is_exported: true,
                    stable_key: "ts|src/theme.ts|theme".to_string(),
                    precision: SymbolPrecision::ModuleLinked,
                },
            ],
            vec![DefinitionFact {
                id: DefinitionId(30),
                symbol: button,
                language: Language::TypeScript,
                name: "Button".to_string(),
                qualified_name: "src/app.ts::Button".to_string(),
                kind: DefinitionKind::Declaration,
                namespace: SymbolNamespace::Value,
                file: Some(app_file),
                package: None,
                module: Some(ModuleNodeId(0)),
                owner: None,
                primary_span: Some(Span::point(app_file, 1, 1)),
                is_primary: true,
                is_exported: true,
                stable_key: "ts|src/app.ts|definition|Button".to_string(),
                precision: SymbolPrecision::ExactLocal,
            }],
            vec![
                ReferenceFact {
                    id: ReferenceId(40),
                    language: Language::TypeScript,
                    name: "theme".to_string(),
                    qualified_name: "src/theme.ts::theme".to_string(),
                    kind: ReferenceKind::Read,
                    namespace: SymbolNamespace::Value,
                    file: Some(app_file),
                    package: None,
                    module: Some(ModuleNodeId(0)),
                    owner: Some(button),
                    primary_span: Some(Span::point(app_file, 1, 28)),
                    target: Some(theme),
                    candidates: Vec::new(),
                    stable_key: "ts|src/app.ts|reference|theme".to_string(),
                    status: SymbolResolutionStatus::Resolved,
                    precision: SymbolPrecision::ModuleLinked,
                },
                ReferenceFact {
                    id: ReferenceId(50),
                    language: Language::TypeScript,
                    name: "missing".to_string(),
                    qualified_name: "missing".to_string(),
                    kind: ReferenceKind::Read,
                    namespace: SymbolNamespace::Value,
                    file: Some(app_file),
                    package: None,
                    module: Some(ModuleNodeId(0)),
                    owner: Some(button),
                    primary_span: Some(Span::point(app_file, 1, 35)),
                    target: None,
                    candidates: Vec::new(),
                    stable_key: "ts|src/app.ts|reference|missing".to_string(),
                    status: SymbolResolutionStatus::Unresolved,
                    precision: SymbolPrecision::Unresolved,
                },
                ReferenceFact {
                    id: ReferenceId(60),
                    language: Language::TypeScript,
                    name: "ambiguous".to_string(),
                    qualified_name: "ambiguous".to_string(),
                    kind: ReferenceKind::Read,
                    namespace: SymbolNamespace::Value,
                    file: Some(app_file),
                    package: None,
                    module: Some(ModuleNodeId(0)),
                    owner: Some(button),
                    primary_span: Some(Span::point(app_file, 1, 44)),
                    target: None,
                    candidates: vec![button, theme],
                    stable_key: "ts|src/app.ts|reference|ambiguous".to_string(),
                    status: SymbolResolutionStatus::Ambiguous,
                    precision: SymbolPrecision::Ambiguous,
                },
            ],
        );

        let symbols = Symbols::build(&db);
        let references = References::build(&db);

        assert_eq!(symbols.all().len(), 2);
        assert_eq!(
            symbols.iter().map(|symbol| symbol.id).collect::<Vec<_>>(),
            vec![button, theme]
        );
        assert!(std::ptr::eq(symbols.get(button).unwrap(), &symbols.all()[0]));
        assert_eq!(
            symbols
                .for_file(app_file)
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>(),
            vec![button]
        );
        assert_eq!(
            symbols
                .by_name("theme")
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>(),
            vec![theme]
        );
        assert_eq!(
            symbols.definition(button).map(|definition| definition.id),
            Some(DefinitionId(30))
        );
        assert_eq!(
            symbols
                .definitions(button)
                .map(|definition| definition.id)
                .collect::<Vec<_>>(),
            vec![DefinitionId(30)]
        );
        assert_eq!(
            symbols.exported().map(|symbol| symbol.id).collect::<Vec<_>>(),
            vec![button, theme]
        );

        assert_eq!(references.all().len(), 3);
        assert_eq!(
            references
                .iter()
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            vec![ReferenceId(40), ReferenceId(50), ReferenceId(60)]
        );
        assert_eq!(
            references
                .to(theme)
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            vec![ReferenceId(40)]
        );
        assert_eq!(
            references
                .for_file(app_file)
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            vec![ReferenceId(40), ReferenceId(50), ReferenceId(60)]
        );
        assert_eq!(
            references
                .unresolved()
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            vec![ReferenceId(50)]
        );
        assert_eq!(
            references
                .ambiguous()
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            vec![ReferenceId(60)]
        );
    }
}
