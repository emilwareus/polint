//! Shared syntax fact row types produced by language frontends.
//!
//! These types are SourceFile-adjacent and owned here so frontends (`polint-go`,
//! `polint-ts`) can write them without depending on the facade.

use polint_core::{BranchId, Diagnostic, FileId, FunctionId, ImportId, Language, PackageId, Span};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
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

/// Name used for the synthetic per-module function row inserted for TS/JS files.
pub const TS_JS_MODULE_FUNCTION_NAME: &str = "<polint:module>";

pub fn is_synthetic_ts_js_module_function(function: &FunctionFact) -> bool {
    function.language.is_ts_family() && function.name == TS_JS_MODULE_FUNCTION_NAME
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PackageFact {
    pub id: PackageId,
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImportFact {
    pub id: ImportId,
    pub file: FileId,
    pub package: Option<String>,
    pub path: String,
    pub span: Span,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
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
#[non_exhaustive]
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
#[non_exhaustive]
pub struct CoverageFact {
    pub branch: BranchId,
    pub covered: Option<bool>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StringLiteralFact {
    pub file: FileId,
    pub value: String,
    pub span: Span,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TsComponentFact {
    pub file: FileId,
    pub function: Option<FunctionId>,
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TsClassFact {
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub is_exported: bool,
    pub is_component_like: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct JsxAttributeFact {
    pub file: FileId,
    pub name: String,
    pub value: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFileAnalysis {
    pub schema: String,
    pub diagnostics: Vec<Diagnostic>,
    pub facts: CachedFileFacts,
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
impl FunctionFact {
    /// Constructs a syntax function fact from its complete fields.
    #[expect(
        clippy::too_many_arguments,
        reason = "This constructor mirrors the complete non-exhaustive public fact schema."
    )]
    pub fn new(
        id: FunctionId,
        file: FileId,
        name: String,
        span: Span,
        language: Language,
        is_test: bool,
        is_exported: bool,
        cyclomatic_complexity: u32,
        calls: Vec<String>,
    ) -> Self {
        Self {
            id,
            file,
            name,
            span,
            language,
            is_test,
            is_exported,
            cyclomatic_complexity,
            calls,
        }
    }
}

impl PackageFact {
    /// Constructs a syntax package fact from its complete fields.
    pub fn new(id: PackageId, file: FileId, name: String, span: Span, language: Language) -> Self {
        Self {
            id,
            file,
            name,
            span,
            language,
        }
    }
}

impl ImportFact {
    /// Constructs a syntax import fact from its complete fields.
    pub fn new(
        id: ImportId,
        file: FileId,
        package: Option<String>,
        path: String,
        span: Span,
        language: Language,
    ) -> Self {
        Self {
            id,
            file,
            package,
            path,
            span,
            language,
        }
    }
}

impl BranchObligation {
    /// Constructs a branch obligation from its complete fields.
    #[expect(
        clippy::too_many_arguments,
        reason = "This constructor mirrors the complete non-exhaustive public fact schema."
    )]
    pub fn new(
        id: BranchId,
        function: Option<FunctionId>,
        file: FileId,
        decision_span: Span,
        condition_text: String,
        edge_label: String,
        is_error_path: bool,
        stable_fingerprint: String,
    ) -> Self {
        Self {
            id,
            function,
            file,
            decision_span,
            condition_text,
            edge_label,
            is_error_path,
            stable_fingerprint,
        }
    }
}

impl TestFact {
    /// Constructs a test fact from its complete fields.
    #[expect(
        clippy::too_many_arguments,
        reason = "This constructor mirrors the complete non-exhaustive public fact schema."
    )]
    pub fn new(
        file: FileId,
        function: Option<FunctionId>,
        name: String,
        span: Span,
        evidence_terms: Vec<String>,
        assertion_count: u32,
        subtest_count: u32,
        subtest_names: Vec<String>,
        table_rows: u32,
    ) -> Self {
        Self {
            file,
            function,
            name,
            span,
            evidence_terms,
            assertion_count,
            subtest_count,
            subtest_names,
            table_rows,
        }
    }
}

impl CoverageFact {
    /// Constructs a coverage fact from its complete fields.
    pub fn new(branch: BranchId, covered: Option<bool>, source: String) -> Self {
        Self {
            branch,
            covered,
            source,
        }
    }
}

impl StringLiteralFact {
    /// Constructs a string-literal fact from its complete fields.
    pub fn new(file: FileId, value: String, span: Span, language: Language) -> Self {
        Self {
            file,
            value,
            span,
            language,
        }
    }
}

impl TsComponentFact {
    /// Constructs a TypeScript component fact from its complete fields.
    pub fn new(file: FileId, function: Option<FunctionId>, name: String, span: Span) -> Self {
        Self {
            file,
            function,
            name,
            span,
        }
    }
}

impl TsClassFact {
    /// Constructs a TypeScript class fact from its complete fields.
    pub fn new(
        file: FileId,
        name: String,
        span: Span,
        is_exported: bool,
        is_component_like: bool,
    ) -> Self {
        Self {
            file,
            name,
            span,
            is_exported,
            is_component_like,
        }
    }
}

impl JsxAttributeFact {
    /// Constructs a JSX attribute fact from its complete fields.
    pub fn new(file: FileId, name: String, value: Option<String>, span: Span) -> Self {
        Self {
            file,
            name,
            value,
            span,
        }
    }
}
