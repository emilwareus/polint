// This is the whole policy for the go-branch-obligations example repo.
// It registers one local rule, local/go-branch-obligations, which reports
// important Go error branches when nearby tests do not appear to cover them.
// The matching is heuristic and uses extracted branch/test facts from polint.
use globset::{Glob, GlobSet, GlobSetBuilder};
use polint::sdk::prelude::*;

pub struct GoBranchObligations;

impl Rule for GoBranchObligations {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "local/go-branch-obligations".to_string(),
            description: "Heuristically require nearby tests for important Go branches."
                .to_string(),
            severity: Severity::Warn,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().branch_obligations().go_tests()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let rule_id = self.meta().id;
        let mut diagnostics = Vec::new();
        for branch in ctx.branches() {
            let file = ctx.file_path(branch.file);
            if branch.is_error_path
                && file_in_scope(ctx.options(), &file)
                && !has_nearby_test_evidence(ctx, branch.file, &branch.condition_text)
            {
                diagnostics.push(
                    Diagnostic::warning(
                        rule_id.clone(),
                        file,
                        branch.decision_span.diagnostic_range(),
                        format!("No nearby test evidence found for Go branch `{}`.", branch.condition_text),
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
}

fn has_nearby_test_evidence(ctx: &RuleCtx<'_>, file: FileId, condition: &str) -> bool {
    ctx.go_tests_for_related_file(file).iter().any(|test| {
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

fn file_in_scope(options: &RuleOptions, file: &str) -> bool {
    (options.files.is_empty()
        || options
            .files
            .iter()
            .any(|pattern| glob_matches(pattern, file)))
        && !options
            .allow_files
            .iter()
            .any(|pattern| glob_matches(pattern, file))
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    build_one(pattern)
        .map(|glob| glob.is_match(value) || glob.is_match(format!("./{value}")))
        .unwrap_or_else(|| value.contains(pattern.trim_matches('*')))
}

fn build_one(pattern: &str) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(pattern).ok()?);
    builder.build().ok()
}
