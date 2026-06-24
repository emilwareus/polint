use polint::sdk::prelude::*;

#[polint::rule(
    id = "review/gorm-model-read-indexes",
    description = "GORM model changes require read-index validation.",
    severity = "error",
    kind = "review"
)]
pub(crate) fn gorm_model_read_indexes(
    ctx: &mut RuleCtx<'_>,
    changes: ChangedFiles<'_>,
) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();

    for changed in changes.iter() {
        let is_gorm_model =
            changed.path().ends_with(".go") && changed.matches_glob("internal/**/models/**");

        if changed.is_deleted() || !is_gorm_model {
            continue;
        }

        let line = changed.lines().first().map(|&(lo, _)| lo).unwrap_or(1);
        ctx.report(
            Diagnostic::error(
                rule_id.clone(),
                changed.path().to_string(),
                DiagnosticRange::point(line, 1),
                "GORM model changed: validate the correct read indexes for this model.",
            )
            .with_help(
                "Check the read paths for this model and add or update composite indexes in \
                 GORM tags or migrations. If no index is needed, explain why in the PR.",
            ),
        );
    }

    Ok(())
}
