//! Public rule-authoring API for polint.
//!
//! Repo-local and example rule authors should start with
//! `use polint::sdk::prelude::*;`. The prelude re-exports the stable core rule
//! contract, fact types, diagnostics, severity types, scope helpers
//! ([`scope::file_in_scope`], [`scope::glob_matches`]), and [`RuleResult`](prelude::RuleResult)
//! ([`RuleError`](prelude::RuleError)) for [`crate::core::Rule::run`] without depending on
//! `polint::core` directly.

#![deny(missing_docs)]

pub mod scope;

use crate::core::{FileId, RuleCtx, TestFact};

/// Collects [`TestFact`] references for `file` (same facts as [`RuleCtx::go_tests_for_file`], materialized).
pub fn collect_go_tests<'a>(ctx: &'a RuleCtx<'_>, file: FileId) -> Vec<&'a TestFact> {
    ctx.go_tests_for_file(file).collect()
}

/// Re-exports of the stable rule-authoring surface.
///
/// Importing `use polint::sdk::prelude::*;` is the recommended way to write a rule
/// — it pulls in the rule contract, the fact types, the diagnostic/severity types,
/// the path-scoping helpers, and [`RuleResult`](crate::sdk::prelude::RuleResult) in one star-import.
pub mod prelude {
    pub use crate::core::{
        AnalysisDb, BranchId, BranchObligation, Capabilities, CapabilitySupport,
        CapabilitySupportStatus, CapabilitySupportView, CoverageFact, FileId, FunctionFact,
        FunctionId, ImportFact, ImportId, JsxAttributeFact, Language, NodeId, PackageFact,
        PackageId, Rule, RuleConfigValue, RuleCtx, RuleId, RuleMeta, RuleOptions, SourceFile, Span,
        StringLiteralFact, TestFact, TextRange, TsClassFact, TsComponentFact,
    };
    pub use crate::diagnostics::{
        ColorChoice, Diagnostic, Evidence, Fix, JsonReportMeta, Label, OutputFormat,
        POLINT_REPORT_JSON_SCHEMA_V1_URL, PolintReport, PolintToolInfo, RenderOpts, Severity,
        Suggestion, TextRange as DiagnosticRange, diagnostics_from_json_report,
    };
    pub use crate::rule_error::{RuleError, RuleResult};
    pub use crate::sdk::collect_go_tests;
    pub use crate::sdk::scope::{file_in_scope, file_matches_globs, glob_matches};
}

#[cfg(test)]
mod tests {
    use crate::sdk::prelude::*;

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

        fn run(&self, ctx: &mut RuleCtx<'_>) -> RuleResult {
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
        assert!(collect_go_tests(&ctx, FileId(0)).is_empty());
        rule.run(&mut ctx).expect("prelude rule runs");
        let diagnostics = ctx.into_diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Warn);
        assert_eq!(diagnostics[0].file, "<unknown>");
    }

    #[test]
    fn sdk_prelude_exports_capability_support_view() {
        fn assert_exported<T>() {}
        assert_exported::<CapabilitySupport>();
        assert_exported::<CapabilitySupportStatus>();
        assert_exported::<CapabilitySupportView>();

        let view = CapabilitySupportView::empty();
        assert!(view.entries().is_empty());
    }
}
