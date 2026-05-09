// This is the whole policy for the go-branch-obligations example repo.
// It registers one local rule, local/go-branch-obligations, which reports
// important Go error branches when nearby tests do not appear to cover them.
// The matching is heuristic and uses extracted branch/test facts from polint.
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/go-branch-obligations",
    description = "Heuristically require nearby tests for important Go branches.",
    severity = "warn"
)]
pub(crate) fn go_branch_obligations(
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
            && !has_nearby_test_evidence(tests, branch.file, &branch.condition_text)
        {
            diagnostics.push(
                Diagnostic::warning(
                    rule_id.clone(),
                    file,
                    branch.decision_span.diagnostic_range(),
                    format!(
                        "No nearby test evidence found for Go branch `{}`.",
                        branch.condition_text
                    ),
                )
                .with_evidence("condition", branch.condition_text.clone())
                .with_evidence("edge", branch.edge_label.clone())
                .with_evidence("branch_fingerprint", branch.stable_fingerprint.clone())
                .with_help("Add a test case that exercises this branch. This rule is heuristic and does not prove exact coverage."),
            );
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}

fn has_nearby_test_evidence(tests: GoTests<'_>, file: FileId, condition: &str) -> bool {
    tests.related_for_file(file).iter().any(|test| {
        test.evidence_terms
            .iter()
            .any(|term| evidence_matches_condition(condition, term))
    })
}

fn evidence_matches_condition(condition: &str, term: &str) -> bool {
    let condition_lower = condition.to_ascii_lowercase();
    let term_lower = term.to_ascii_lowercase();
    !term_lower.is_empty()
        && (condition_lower.contains(&term_lower) || term_lower.contains(&condition_lower))
}
