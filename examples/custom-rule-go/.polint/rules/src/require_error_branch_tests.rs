// This is the whole policy for the custom-rule-go example repo.
// It registers one local rule, local/require-error-branch-tests, which looks
// for Go error branches and asks for nearby test evidence. The check is
// intentionally heuristic; it demonstrates repo-local policy code, not exact
// coverage analysis.
use globset::{Glob, GlobSet, GlobSetBuilder};
use polint::sdk::prelude::*;

pub struct RequireErrorBranchTests;

impl Rule for RequireErrorBranchTests {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "local/require-error-branch-tests".to_string(),
            description: "Require local test evidence for Go error branches.".to_string(),
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
                && ctx.go_tests_for_related_file(branch.file).is_empty()
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
