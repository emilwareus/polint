use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/code-quality-score",
    description = "Compose function-size and complexity signals into one heuristic.",
    severity = "warn"
)]
pub(crate) fn code_quality_score(
    ctx: &mut RuleCtx<'_>,
    functions: FunctionMetrics<'_>,
    complexity: ComplexityMetrics<'_>,
) -> RuleResult {
    let max_complexity = ctx.options().max.unwrap_or(8);
    let rule_id = ctx.rule_id().to_string();
    for function in functions.iter() {
        let Some(complexity_metric) = complexity.get(function.function) else {
            continue;
        };
        let file = ctx.file_path(function.file);
        if file_in_scope(ctx.options(), &file)
            && function.line_count > 6
            && complexity_metric.cyclomatic_complexity > max_complexity
        {
            ctx.report(
                Diagnostic::warning(
                    rule_id.clone(),
                    file,
                    function.span.diagnostic_range(),
                    format!(
                        "Function `{}` is both long and branch-heavy.",
                        function.name
                    ),
                )
                .with_evidence("lines", function.line_count.to_string())
                .with_evidence(
                    "complexity",
                    complexity_metric.cyclomatic_complexity.to_string(),
                )
                .with_help("Split the policy decision into smaller named branches."),
            );
        }
    }
    Ok(())
}
