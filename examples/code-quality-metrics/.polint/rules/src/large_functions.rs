use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/large-function",
    description = "Warn when a function is over the configured line threshold.",
    severity = "warn"
)]
pub(crate) fn large_function(ctx: &mut RuleCtx<'_>, functions: FunctionMetrics<'_>) -> RuleResult {
    let max = ctx.options().max.unwrap_or(40);
    let rule_id = ctx.rule_id().to_string();
    for metric in functions.iter() {
        let file = ctx.file_path(metric.file);
        if file_in_scope(ctx.options(), &file) && metric.line_count > max {
            ctx.report(
                Diagnostic::warning(
                    rule_id.clone(),
                    file,
                    metric.span.diagnostic_range(),
                    format!(
                        "Function `{}` has {} lines, above the configured maximum of {}.",
                        metric.name, metric.line_count, max
                    ),
                )
                .with_evidence("function", metric.name.clone())
                .with_evidence("lines", metric.line_count.to_string()),
            );
        }
    }
    Ok(())
}
