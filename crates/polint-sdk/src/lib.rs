pub mod prelude {
    pub use anyhow::Result;
    pub use polint_core::{
        AnalysisDb, BranchId, BranchObligation, Capabilities, CoverageFact, FileId, FunctionFact,
        FunctionId, ImportFact, ImportId, JsxAttributeFact, Language, NodeId, PackageId, Rule,
        RuleCtx, RuleId, RuleMeta, RuleOptions, SourceFile, Span, StringLiteralFact, TestFact,
        TextRange, TsComponentFact,
    };
    pub use polint_diagnostics::{
        Diagnostic, Evidence, Fix, Label, OutputFormat, Severity, Suggestion,
        TextRange as DiagnosticRange,
    };
}
