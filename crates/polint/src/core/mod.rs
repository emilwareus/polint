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
    branches: Vec<BranchObligation>,
    tests: Vec<TestFact>,
    coverage: Vec<CoverageFact>,
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

    pub fn branches(&self) -> &[BranchObligation] {
        &self.branches
    }

    pub fn tests(&self) -> &[TestFact] {
        &self.tests
    }

    pub fn coverage(&self) -> &[CoverageFact] {
        &self.coverage
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
/// consumes without changing the `Rule` trait.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub syntax: bool,
    pub imports: bool,
    pub cfg: bool,
    pub call_graph: bool,
    pub go_tests: bool,
    pub branch_obligations: bool,
    pub coverage_facts: bool,
    pub test_suite_metrics: bool,
    pub ts_components: bool,
    pub ts_classes: bool,
    pub string_literals: bool,
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

    pub fn cfg(mut self) -> Self {
        self.cfg = true;
        self
    }

    pub fn call_graph(mut self) -> Self {
        self.call_graph = true;
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
}

/// A static-analysis rule that runs against an [`AnalysisDb`] through [`RuleCtx`].
///
/// Implementations should declare their metadata and capabilities, then report
/// diagnostics through the context instead of panicking or writing output.
pub trait Rule: Send + Sync {
    fn meta(&self) -> RuleMeta;
    fn capabilities(&self) -> Capabilities;
    fn run(&self, ctx: &mut RuleCtx<'_>) -> RuleResult;
}

/// Resolved per-rule options from configuration.
///
/// Built-in and repo-local rules can read these values through
/// [`RuleCtx::options`] to apply severity overrides, file filters, thresholds,
/// denied values, or import-boundary settings.
#[derive(Debug, Clone, Default)]
pub struct RuleOptions {
    pub severity: Option<Severity>,
    pub files: Vec<String>,
    pub allow_files: Vec<String>,
    pub allow: Vec<String>,
    pub max: Option<u32>,
    pub deny: Vec<String>,
    pub forbidden_imports: BTreeMap<String, Vec<String>>,
}

/// Borrowed execution context passed to a single rule run.
///
/// The context exposes stable, deterministic views of facts in [`AnalysisDb`]
/// and buffers diagnostics until the rule finishes.
pub struct RuleCtx<'a> {
    db: &'a AnalysisDb,
    diagnostics: Vec<Diagnostic>,
    rule: RuleMeta,
    options: RuleOptions,
}

impl<'a> RuleCtx<'a> {
    /// Creates a rule context for one rule execution.
    pub fn new(db: &'a AnalysisDb, rule: RuleMeta, options: RuleOptions) -> Self {
        Self {
            db,
            diagnostics: Vec::new(),
            rule,
            options,
        }
    }

    /// Returns the underlying analysis database for advanced read-only queries.
    pub fn db(&self) -> &AnalysisDb {
        self.db
    }

    /// Returns resolved options for the current rule.
    pub fn options(&self) -> &RuleOptions {
        &self.options
    }

    /// Returns all analyzed source files in deterministic database order.
    pub fn files(&self) -> &[SourceFile] {
        self.db.files()
    }

    /// Returns all package facts in deterministic database order.
    pub fn packages(&self) -> &[PackageFact] {
        self.db.packages()
    }

    /// Returns all function facts in deterministic database order.
    pub fn functions(&self) -> &[FunctionFact] {
        self.db.functions()
    }

    /// Returns all import facts in deterministic database order.
    pub fn imports(&self) -> &[ImportFact] {
        self.db.imports()
    }

    /// Returns all branch obligations in deterministic database order.
    pub fn branches(&self) -> &[BranchObligation] {
        self.db.branches()
    }

    /// Returns the source file for a stable file ID.
    pub fn source_file(&self, file: FileId) -> Option<&SourceFile> {
        self.db.file(file)
    }

    /// Returns function facts for a file without cloning facts.
    pub fn functions_for_file(&self, file: FileId) -> impl Iterator<Item = &FunctionFact> + '_ {
        self.db
            .functions()
            .iter()
            .filter(move |function| function.file == file)
    }

    /// Returns import facts for a file without cloning facts.
    pub fn imports_for_file(&self, file: FileId) -> impl Iterator<Item = &ImportFact> + '_ {
        self.db
            .imports()
            .iter()
            .filter(move |import| import.file == file)
    }

    /// Returns branch obligations for a function.
    ///
    /// This compatibility helper combines borrowed branch references for the
    /// requested function ID.
    pub fn branch_obligations(&self, function: FunctionId) -> Vec<&BranchObligation> {
        self.db
            .branches()
            .iter()
            .filter(|branch| branch.function == Some(function))
            .collect()
    }

    /// Returns branch obligations for a file without cloning facts.
    pub fn branch_obligations_for_file(
        &self,
        file: FileId,
    ) -> impl Iterator<Item = &BranchObligation> + '_ {
        self.db
            .branches()
            .iter()
            .filter(move |branch| branch.file == file)
    }

    /// Returns all Go test facts in deterministic database order.
    pub fn go_tests(&self) -> &[TestFact] {
        self.db.tests()
    }

    /// Returns Go test facts for a file without cloning facts.
    pub fn go_tests_for_file(&self, file: FileId) -> impl Iterator<Item = &TestFact> + '_ {
        self.db.tests().iter().filter(move |test| test.file == file)
    }

    /// Paths paired with `file`'s relative path under the named `[path_contexts]` rule.
    pub fn path_context_related(&self, pair_name: &str, file: FileId) -> Vec<String> {
        let Some(source) = self.source_file(file) else {
            return Vec::new();
        };
        self.db
            .path_context_related(pair_name, &source.relative_path)
    }

    /// Returns Go tests related to a source file.
    ///
    /// For a production `.go` file, this includes tests in the same file and
    /// tests from same-directory `_test.go` files. For a `_test.go` file, this
    /// includes only tests from that same test file.
    pub fn go_tests_for_related_file(&self, file: FileId) -> Vec<&TestFact> {
        let Some(source_file) = self.source_file(file) else {
            return Vec::new();
        };
        if source_file.language != Language::Go {
            return Vec::new();
        }

        let source_path = Path::new(&source_file.relative_path);
        let source_name = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();

        if source_name.ends_with("_test.go") {
            return self.go_tests_for_file(file).collect();
        }

        let source_dir = source_path.parent();
        self.db
            .tests()
            .iter()
            .filter(|test| {
                if test.file == file {
                    return true;
                }

                let Some(test_file) = self.source_file(test.file) else {
                    return false;
                };
                if test_file.language != Language::Go {
                    return false;
                }

                let test_path = Path::new(&test_file.relative_path);
                let test_name = test_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();

                test_name.ends_with("_test.go") && test_path.parent() == source_dir
            })
            .collect()
    }

    /// Returns TypeScript/JavaScript component facts in database order.
    pub fn ts_components(&self) -> &[TsComponentFact] {
        self.db.ts_components()
    }

    /// Returns TypeScript/JavaScript component facts for a file without cloning facts.
    pub fn ts_components_for_file(
        &self,
        file: FileId,
    ) -> impl Iterator<Item = &TsComponentFact> + '_ {
        self.db
            .ts_components()
            .iter()
            .filter(move |component| component.file == file)
    }

    /// Returns TypeScript/JavaScript class facts in database order.
    pub fn ts_classes(&self) -> &[TsClassFact] {
        self.db.ts_classes()
    }

    /// Returns TypeScript/JavaScript class facts for a file without cloning facts.
    pub fn ts_classes_for_file(&self, file: FileId) -> impl Iterator<Item = &TsClassFact> + '_ {
        self.db
            .ts_classes()
            .iter()
            .filter(move |class| class.file == file)
    }

    /// Returns string literal facts in deterministic database order.
    pub fn string_literals(&self) -> &[StringLiteralFact] {
        self.db.string_literals()
    }

    /// Returns string literal facts for a file without cloning facts.
    pub fn string_literals_for_file(
        &self,
        file: FileId,
    ) -> impl Iterator<Item = &StringLiteralFact> + '_ {
        self.db
            .string_literals()
            .iter()
            .filter(move |literal| literal.file == file)
    }

    /// Returns JSX attribute facts in deterministic database order.
    pub fn jsx_attributes(&self) -> &[JsxAttributeFact] {
        self.db.jsx_attributes()
    }

    /// Returns JSX attribute facts for a file without cloning facts.
    pub fn jsx_attributes_for_file(
        &self,
        file: FileId,
    ) -> impl Iterator<Item = &JsxAttributeFact> + '_ {
        self.db
            .jsx_attributes()
            .iter()
            .filter(move |attribute| attribute.file == file)
    }

    /// Returns syntactic import graph edges as `(source file, import)` pairs.
    ///
    /// Edges follow import fact insertion order from [`AnalysisDb`].
    pub fn import_edges(&self) -> impl Iterator<Item = (&SourceFile, &ImportFact)> + '_ {
        self.db
            .imports()
            .iter()
            .filter_map(move |import| self.source_file(import.file).map(|file| (file, import)))
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

/// Test-only registry that exposes the [`Capabilities`]/`Rule` shape independent of `run_rules`.
/// Production runs use [`run_rules`] directly with `&[Arc<dyn Rule>]`.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct RuleRegistry {
    rules: Vec<Arc<dyn Rule>>,
}

#[cfg(test)]
impl RuleRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register<R>(&mut self, rule: R)
    where
        R: Rule + 'static,
    {
        self.rules.push(Arc::new(rule));
    }

    pub(crate) fn rules(&self) -> &[Arc<dyn Rule>] {
        &self.rules
    }
}

// Intentionally `pub` (not `pub(crate)`): `polint::_bench::core` glob-re-exports this for
// `polint-bench`, an external crate. `unreachable_pub` does not follow that path.
#[allow(unreachable_pub)]
pub fn run_rules(
    db: &AnalysisDb,
    rules: &[Arc<dyn Rule>],
    options: &BTreeMap<String, RuleOptions>,
    enabled: Option<&BTreeSet<String>>,
    parallel: bool,
) -> Vec<Diagnostic> {
    let run_one = |rule: &Arc<dyn Rule>| {
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
        let rule_options = options.get(&meta.id).cloned().unwrap_or_default();
        let mut ctx = RuleCtx::new(db, meta.clone(), rule_options);
        let result = catch_unwind(AssertUnwindSafe(|| rule.run(&mut ctx)));
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
    use anyhow::anyhow;
    use proptest::prelude::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[derive(Clone, Copy)]
    enum TestRuleBehavior {
        Report,
        Error,
        Panic,
        MetaPanic,
    }

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
    }

    impl Rule for TestRule {
        fn meta(&self) -> RuleMeta {
            if matches!(self.behavior, TestRuleBehavior::MetaPanic) {
                panic!("intentional metadata panic");
            }

            RuleMeta {
                id: self.id.to_string(),
                description: format!("Test rule {}", self.id),
                severity: self.severity,
            }
        }

        fn capabilities(&self) -> Capabilities {
            self.capabilities
        }

        fn run(&self, ctx: &mut RuleCtx<'_>) -> RuleResult {
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
    fn rule_ctx_exposes_ts_classes() {
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

        let ctx = RuleCtx::new(
            &db,
            RuleMeta {
                id: "examples/ts-classes".to_string(),
                description: "TS classes test".to_string(),
                severity: Severity::Warn,
            },
            RuleOptions::default(),
        );

        assert_eq!(ctx.ts_classes().len(), 1);
        assert_eq!(ctx.ts_classes()[0].name, db.ts_classes()[0].name);
        assert_eq!(ctx.ts_classes()[0].span, db.ts_classes()[0].span);
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

        let ctx = RuleCtx::new(
            &db,
            RuleMeta {
                id: "examples/sdk-query".to_string(),
                description: "SDK query helper test".to_string(),
                severity: Severity::Warn,
            },
            RuleOptions::default(),
        );

        assert_eq!(ctx.packages()[0].name, "payment");
        assert_eq!(ctx.branches()[0].condition_text, "err != nil");
        assert_eq!(
            ctx.source_file(go_file).unwrap().relative_path,
            "src/payment.go"
        );
        assert_eq!(
            ctx.functions_for_file(go_file)
                .map(|function| function.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Charge"]
        );
        assert_eq!(
            ctx.imports_for_file(go_file)
                .map(|import| import.path.as_str())
                .collect::<Vec<_>>(),
            vec!["context"]
        );
        assert_eq!(ctx.branch_obligations_for_file(go_file).count(), 1);
        assert_eq!(
            ctx.go_tests_for_file(go_file)
                .map(|test| test.name.as_str())
                .collect::<Vec<_>>(),
            vec!["TestCharge"]
        );
        assert_eq!(
            ctx.ts_components_for_file(ts_file)
                .map(|component| component.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Button"]
        );
        assert_eq!(
            ctx.ts_classes_for_file(ts_file)
                .map(|class| class.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Dialog"]
        );
        assert_eq!(
            ctx.string_literals_for_file(ts_file)
                .map(|literal| literal.value.as_str())
                .collect::<Vec<_>>(),
            vec!["Pay"]
        );
        assert_eq!(
            ctx.jsx_attributes_for_file(ts_file)
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

        let ctx = RuleCtx::new(
            &db,
            RuleMeta {
                id: "examples/import-edges".to_string(),
                description: "Import edge helper test".to_string(),
                severity: Severity::Warn,
            },
            RuleOptions::default(),
        );

        assert_eq!(
            ctx.import_edges()
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

        let ctx = RuleCtx::new(
            &db,
            RuleMeta {
                id: "examples/go-related-tests".to_string(),
                description: "Related Go test helper test".to_string(),
                severity: Severity::Warn,
            },
            RuleOptions::default(),
        );

        assert_eq!(
            ctx.go_tests_for_related_file(production_file)
                .iter()
                .map(|test| test.name.as_str())
                .collect::<Vec<_>>(),
            vec!["TestInline", "TestPayment"]
        );
        assert_eq!(
            ctx.go_tests_for_related_file(companion_file)
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
                .with_capabilities(Capabilities::new().imports().coverage_facts()),
        );

        let capabilities = registry.rules()[0].capabilities();
        assert!(capabilities.imports);
        assert!(capabilities.coverage_facts);
        assert!(!capabilities.jsx_attributes);
    }

    #[test]
    fn run_rules_filters_enabled_patterns_and_applies_severity_override() {
        let db = AnalysisDb::new();
        let rules: Vec<Arc<dyn Rule>> = vec![
            Arc::new(TestRule::report(
                "examples/allowed",
                Severity::Warn,
                "allowed",
            )),
            Arc::new(TestRule::report(
                "custom/blocked",
                Severity::Error,
                "blocked",
            )),
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
        let rules: Vec<Arc<dyn Rule>> = vec![
            Arc::new(TestRule::report("examples/one", Severity::Warn, "one")),
            Arc::new(TestRule::report("examples/two", Severity::Warn, "two")),
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
        let rules: Vec<Arc<dyn Rule>> = vec![
            Arc::new(TestRule::error("examples/error")),
            Arc::new(TestRule::panic("examples/panic")),
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
        let rules: Vec<Arc<dyn Rule>> = vec![Arc::new(TestRule::meta_panic())];

        let diagnostics = run_rules(&db, &rules, &BTreeMap::new(), None, false);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "internal/unknown");
        assert_eq!(diagnostics[0].file, "<workspace>");
        assert!(diagnostics[0].message.contains("rule metadata panicked"));
    }

    #[test]
    fn run_rules_parallel_matches_sequential() {
        let db = AnalysisDb::new();
        let rules: Vec<Arc<dyn Rule>> = vec![
            Arc::new(
                TestRule::report("examples/duplicate", Severity::Warn, "duplicate")
                    .with_message("same diagnostic")
                    .with_delay(Duration::from_millis(50)),
            ),
            Arc::new(
                TestRule::report("examples/duplicate", Severity::Error, "duplicate")
                    .with_message("same diagnostic"),
            ),
        ];

        let sequential = run_rules(&db, &rules, &BTreeMap::new(), None, false);
        let parallel = run_rules(&db, &rules, &BTreeMap::new(), None, true);

        assert_eq!(parallel, sequential);
    }

    #[test]
    fn run_rules_dedupes_duplicate_fingerprints() {
        let db = AnalysisDb::new();
        let rules: Vec<Arc<dyn Rule>> = vec![
            Arc::new(TestRule::report(
                "examples/duplicate-a",
                Severity::Warn,
                "same-fingerprint",
            )),
            Arc::new(TestRule::report(
                "examples/duplicate-b",
                Severity::Error,
                "same-fingerprint",
            )),
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
