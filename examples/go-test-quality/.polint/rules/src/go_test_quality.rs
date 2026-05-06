// This is the whole policy for the go-test-quality example repo.
// It registers one local rule, local/go-test-quality, which heuristically flags
// oversized Go tests and tests with no obvious assertion or error check.
use polint::sdk::prelude::*;

pub(crate) struct GoTestQuality;

impl Rule for GoTestQuality {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "local/go-test-quality".to_string(),
            description: "Heuristically flag oversized Go tests and tests with no assertions."
                .to_string(),
            severity: Severity::Warn,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().go_tests().test_suite_metrics()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> RuleResult {
        let max = ctx.options().max.unwrap_or(24);
        let rule_id = self.meta().id;
        let mut diagnostics = Vec::new();
        for test in ctx.go_tests() {
            let file = ctx.file_path(test.file);
            if !file_in_scope(ctx.options(), &file) {
                continue;
            }
            let score = 1 + (test.subtest_count * 4) + (test.table_rows * 2) + test.assertion_count;
            if score > max {
                diagnostics.push(
                    Diagnostic::warning(
                        rule_id.clone(),
                        file.clone(),
                        test.span.diagnostic_range(),
                        format!(
                            "Go test `{}` has heuristic maintainability score {}, max {}.",
                            test.name, score, max
                        ),
                    )
                    .with_evidence("score", score.to_string())
                    .with_evidence("subtests", test.subtest_count.to_string())
                    .with_evidence("table_rows", test.table_rows.to_string())
                    .with_evidence("assertions", test.assertion_count.to_string())
                    .with_help("Split this test into smaller behavior-focused tests. This rule is heuristic."),
                );
            }
            if test.assertion_count == 0 {
                diagnostics.push(
                    Diagnostic::warning(
                        rule_id.clone(),
                        file,
                        test.span.diagnostic_range(),
                        format!("Go test `{}` has no obvious assertion or error check.", test.name),
                    )
                    .with_evidence("test", test.name.clone())
                    .with_evidence("assertions", test.assertion_count.to_string())
                    .with_evidence("evidence_terms", test.evidence_terms.join(", "))
                    .with_help("Add an explicit assertion, error check, or failure path. This rule is heuristic."),
                );
            }
        }
        for diagnostic in diagnostics {
            ctx.report(diagnostic);
        }
        Ok(())
    }
}
