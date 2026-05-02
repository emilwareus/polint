//! `polint-sdk` is the public rule-authoring entry point for Polint.
//!
//! Repo-local and example rule authors should start with
//! `use polint_sdk::prelude::*;`. The prelude re-exports the stable core rule
//! contract, fact types, diagnostics, severity types, and `anyhow::Result`
//! needed for ordinary rule implementations without depending on `polint-core`
//! directly.

pub mod prelude {
    pub use anyhow::Result;
    pub use polint_core::{
        AnalysisDb, BranchId, BranchObligation, Capabilities, CoverageFact, FileId, FunctionFact,
        FunctionId, ImportFact, ImportId, JsxAttributeFact, Language, NodeId, PackageFact,
        PackageId, Rule, RuleCtx, RuleId, RuleMeta, RuleOptions, SourceFile, Span,
        StringLiteralFact, TestFact, TextRange, TsClassFact, TsComponentFact,
    };
    pub use polint_diagnostics::{
        Diagnostic, Evidence, Fix, Label, OutputFormat, Severity, Suggestion,
        TextRange as DiagnosticRange,
    };
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    struct PreludeSmokeRule;

    impl Rule for PreludeSmokeRule {
        fn meta(&self) -> RuleMeta {
            RuleMeta {
                id: "examples/prelude-smoke".to_string(),
                description: "Prelude smoke rule".to_string(),
                severity: Severity::Warn,
            }
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::new()
                .imports()
                .string_literals()
                .jsx_attributes()
        }

        fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
            assert!(ctx.files().is_empty());
            assert_eq!(ctx.import_edges().count(), 0);
            ctx.warn(&Span::point(FileId(0), 1, 1), "prelude warning");
            Ok(())
        }
    }

    #[test]
    fn sdk_prelude_exports_rule_authoring_surface() {
        fn assert_exported<T>() {}
        assert_exported::<PackageFact>();
        assert_exported::<TsClassFact>();

        let db = AnalysisDb::new();
        let rule = PreludeSmokeRule;
        let capabilities = rule.capabilities();
        assert!(capabilities.imports);
        assert!(capabilities.string_literals);
        assert!(capabilities.jsx_attributes);

        let mut ctx = RuleCtx::new(&db, rule.meta(), RuleOptions::default());
        rule.run(&mut ctx).expect("prelude rule runs");
        let diagnostics = ctx.into_diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Warn);
        assert_eq!(diagnostics[0].file, "<unknown>");
    }
}
