// This is the whole policy for the custom-rule-go example repo.
// It registers one local rule, local/require-error-branch-tests, which looks
// for Go error branches and asks for nearby test evidence. The check is
// intentionally heuristic; it demonstrates repo-local policy code, not exact
// coverage analysis.
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/require-error-branch-tests",
    description = "Require local test evidence for Go error branches.",
    severity = "warn"
)]
pub(crate) fn require_error_branch_tests(
    ctx: &mut RuleCtx<'_>,
    branches: BranchObligations<'_>,
    tests: GoTests<'_>,
) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    let mut diagnostics = Vec::new();
    for branch in branches.iter() {
        let file = ctx.file_path(branch.file);
        if branch.is_error_path
            && file_in_scope(ctx.options(), &file)
            && tests.related_for_file(branch.file).is_empty()
        {
            diagnostics.push(
                Diagnostic::warning(
                    rule_id.clone(),
                    file,
                    branch.decision_span.diagnostic_range(),
                    "Add a companion test for this Go error branch.",
                )
                .with_evidence("condition", branch.condition_text.clone())
                .with_evidence("branch_fingerprint", branch.stable_fingerprint.clone())
                .with_help("This is a repo-local heuristic: it checks nearby extracted test facts, not exact coverage."),
            );
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}
