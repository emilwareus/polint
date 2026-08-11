pub use polint_analysis_api::{
    BranchObligation, ComplexityMetricFact, CoverageFact, DefinitionFact, DefinitionKind,
    FileMetricFact, FunctionFact, FunctionMetricFact, ImportFact, JsxAttributeFact, ModuleEdge,
    ModuleEdgeKind, ModuleNode, ModuleNodeKind, PackageFact, ReferenceFact, ReferenceKind,
    ResolutionPrecision, ResolutionStatus, ResolvedImportFact, SourceFile, StringLiteralFact,
    SymbolFact, SymbolKind, SymbolNamespace, SymbolPrecision, SymbolResolutionStatus, TestFact,
    TsClassFact, TsComponentFact, UnresolvedReason,
};
#[cfg(test)]
pub(crate) use polint_analysis_api::{CachedFileAnalysis, TS_JS_MODULE_FUNCTION_NAME};
pub(crate) use polint_analysis_api::{CachedFileFacts, is_synthetic_ts_js_module_function};
