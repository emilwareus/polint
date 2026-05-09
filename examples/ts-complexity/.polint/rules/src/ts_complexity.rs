// This is the whole policy for the ts-complexity example repo.
// It registers one local rule, local/ts-cyclomatic-complexity, which warns when
// a TS/JS function's extracted cyclomatic complexity exceeds the configured max.
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/ts-cyclomatic-complexity",
    description = "Warn when a TS/JS function's cyclomatic complexity is high.",
    severity = "warn"
)]
pub(crate) fn ts_complexity(ctx: &mut RuleCtx<'_>, functions: Functions<'_>) -> RuleResult {
    let max = ctx.options().max.unwrap_or(12);
    let rule_id = ctx.rule_id().to_string();
    let mut diagnostics = Vec::new();
    for function in functions
        .iter()
        .filter(|function| function.language.is_ts_family())
    {
        let file = ctx.file_path(function.file);
        if function.cyclomatic_complexity > max && file_in_scope(ctx.options(), &file) {
            diagnostics.push(
                Diagnostic::warning(
                    rule_id.clone(),
                    file,
                    function.span.diagnostic_range(),
                    format!(
                        "TS/JS function `{}` has cyclomatic complexity {}, max {}.",
                        function.name, function.cyclomatic_complexity, max
                    ),
                )
                .with_evidence("function", function.name.clone())
                .with_evidence("complexity", function.cyclomatic_complexity.to_string())
                .with_help(
                    "Split condition-heavy UI or business logic into smaller named helpers.",
                ),
            );
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}
