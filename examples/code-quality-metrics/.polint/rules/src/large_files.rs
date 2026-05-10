use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/large-file",
    description = "Warn when a file is over the configured line threshold.",
    severity = "warn"
)]
pub(crate) fn large_file(ctx: &mut RuleCtx<'_>, files: FileMetrics<'_>) -> RuleResult {
    let max = ctx.options().max.unwrap_or(80);
    let rule_id = ctx.rule_id().to_string();
    for metric in files.iter() {
        let file = ctx.file_path(metric.file);
        if file_in_scope(ctx.options(), &file) && metric.line_count > max {
            ctx.report(
                Diagnostic::warning(
                    rule_id.clone(),
                    file,
                    DiagnosticRange::point(1, 1),
                    format!(
                        "File has {} lines, above the configured maximum of {}.",
                        metric.line_count, max
                    ),
                )
                .with_evidence("lines", metric.line_count.to_string())
                .with_evidence("functions", metric.function_count.to_string()),
            );
        }
    }
    Ok(())
}
