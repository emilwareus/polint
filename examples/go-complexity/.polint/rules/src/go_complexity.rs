// This is the whole policy for the go-complexity example repo.
// It registers one local rule, local/go-cyclomatic-complexity, which warns when
// a Go function's extracted cyclomatic complexity exceeds the configured max.
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/go-cyclomatic-complexity",
    description = "Warn when a Go function's cyclomatic complexity is high.",
    severity = "warn"
)]
pub(crate) fn go_complexity(ctx: &mut RuleCtx<'_>, functions: Functions<'_>) -> RuleResult {
    let max = ctx.options().max.unwrap_or(12);
    let rule_id = ctx.rule_id().to_string();
    let mut diagnostics = Vec::new();
    for function in functions
        .iter()
        .filter(|function| function.language == Language::Go)
    {
        let file = ctx.file_path(function.file);
        if function.cyclomatic_complexity > max && file_in_scope(ctx.options(), &file) {
            diagnostics.push(
                Diagnostic::warning(
                    rule_id.clone(),
                    file,
                    function.span.diagnostic_range(),
                    format!(
                        "Go function `{}` has cyclomatic complexity {}, max {}.",
                        function.name, function.cyclomatic_complexity, max
                    ),
                )
                .with_evidence("function", function.name.clone())
                .with_evidence("complexity", function.cyclomatic_complexity.to_string())
                .with_help("Split deeply branched behavior into smaller focused functions."),
            );
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}
