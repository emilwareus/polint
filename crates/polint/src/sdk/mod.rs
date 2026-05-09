//! Public rule-authoring API for polint.
//!
//! Repo-local and example rule authors should start with
//! `use polint::sdk::prelude::*;`. The prelude re-exports the stable rule
//! authoring types, fact views, diagnostics, severity types, scope helpers
//! ([`scope::file_in_scope`], [`scope::glob_matches`]), and [`RuleResult`](prelude::RuleResult)
//! ([`RuleError`](prelude::RuleError)) without depending on `polint::core` directly.

#![deny(missing_docs)]

pub mod facts;
pub mod scope;

use crate::core::{FileId, TestFact};
use facts::GoTests;

/// Collects [`TestFact`] references for `file` from a typed [`GoTests`] fact view.
pub fn collect_go_tests<'a>(tests: GoTests<'a>, file: FileId) -> Vec<&'a TestFact> {
    tests.for_file(file).collect()
}

/// Re-exports of the stable rule-authoring surface.
///
/// Importing `use polint::sdk::prelude::*;` is the recommended way to write a rule
/// — it pulls in the rule contract, the fact types, the diagnostic/severity types,
/// the path-scoping helpers, and [`RuleResult`](crate::sdk::prelude::RuleResult) in one star-import.
pub mod prelude {
    pub use crate::core::{
        BranchId, BranchObligation, CapabilitySupport, CapabilitySupportStatus,
        CapabilitySupportView, CoverageFact, FileId, FunctionFact, FunctionId, ImportFact,
        ImportId, JsxAttributeFact, Language, NodeId, PackageFact, PackageId, Rule,
        RuleConfigValue, RuleCtx, RuleId, RuleOptions, SourceFile, Span, StringLiteralFact,
        TestFact, TextRange, TsClassFact, TsComponentFact,
    };
    pub use crate::diagnostics::{
        ColorChoice, Diagnostic, Evidence, Fix, JsonReportMeta, Label, OutputFormat,
        POLINT_REPORT_JSON_SCHEMA_V1_URL, PolintReport, PolintToolInfo, RenderOpts, Severity,
        Suggestion, TextRange as DiagnosticRange, diagnostics_from_json_report,
    };
    pub use crate::rule_error::{RuleError, RuleResult};
    pub use crate::sdk::collect_go_tests;
    pub use crate::sdk::facts::{
        BranchObligations, CallGraph, Cfg, CoverageFacts, Functions, GoTests, Imports,
        JsxAttributes, Packages, SourceFiles, StringLiterals, TestSuiteMetrics, TsClasses,
        TsComponents,
    };
    pub use crate::sdk::scope::{file_in_scope, file_matches_globs, glob_matches};
}

/// Hidden implementation details used by generated rule code.
#[doc(hidden)]
pub mod __private {
    pub use crate::core::{AnalysisDb, Capabilities, RuleMeta};
    pub use crate::sdk::facts::FactView;

    use crate::core::{Rule, RuleCtx};
    use crate::rule_error::RuleResult;

    /// Constructs an opaque rule for generated `#[polint::rule]` code.
    #[doc(hidden)]
    pub fn make_rule<M, C, R>(meta: M, capabilities: C, run: R) -> Rule
    where
        M: Fn() -> RuleMeta + Send + Sync + 'static,
        C: Fn() -> Capabilities + Send + Sync + 'static,
        R: Fn(&AnalysisDb, &mut RuleCtx<'_>) -> RuleResult + Send + Sync + 'static,
    {
        Rule::from_parts(meta, capabilities, run)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::AnalysisDb;
    use crate::sdk::facts::FactView;
    use crate::sdk::prelude::*;

    #[polint::rule(
        id = "examples/prelude-smoke",
        description = "Prelude smoke rule",
        severity = "warn"
    )]
    fn prelude_smoke(
        ctx: &mut RuleCtx<'_>,
        files: SourceFiles<'_>,
        imports: Imports<'_>,
        literals: StringLiterals<'_>,
        jsx: JsxAttributes<'_>,
    ) -> RuleResult {
        assert_eq!(files.iter().count(), 0);
        assert_eq!(imports.edges().count(), 0);
        assert!(literals.all().is_empty());
        assert!(jsx.all().is_empty());
        ctx.warn(&Span::point(FileId(0), 1, 1), "prelude warning");
        Ok(())
    }

    #[test]
    fn sdk_prelude_exports_rule_authoring_surface() {
        fn assert_exported<T>() {}
        assert_exported::<PackageFact>();
        assert_exported::<TsClassFact>();

        let db = AnalysisDb::new();
        let rule = prelude_smoke();
        let capabilities = rule.capabilities();
        assert!(capabilities.syntax);
        assert!(capabilities.imports);
        assert!(capabilities.string_literals);
        assert!(capabilities.jsx_attributes);

        let mut ctx = RuleCtx::new(&db, rule.meta(), RuleOptions::default());
        let tests = GoTests::build(&db);
        assert!(collect_go_tests(tests, FileId(0)).is_empty());
        rule.run(&db, &mut ctx).expect("prelude rule runs");
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
