// This example intentionally pairs a normal policy rule with polint ignore
// comments. The rule itself reports every matching literal; suppression is
// handled by the polint engine after the rule returns diagnostics.
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-denied-literals",
    description = "Deny configured string literal text.",
    severity = "warn"
)]
pub(crate) fn no_denied_literals(
    ctx: &mut RuleCtx<'_>,
    literals: StringLiterals<'_>,
) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    let mut diagnostics = Vec::new();

    for literal in literals
        .iter()
        .filter(|literal| literal.language.is_ts_family())
    {
        let file = ctx.file_path(literal.file);
        if !file_matches_globs(ctx.options(), &file) {
            continue;
        }
        if let Some(matched) = ctx
            .options()
            .deny
            .iter()
            .find(|deny| literal.value.contains(deny.as_str()))
        {
            diagnostics.push(
                Diagnostic::warning(
                    rule_id.clone(),
                    file,
                    literal.span.diagnostic_range(),
                    format!("Configured denied literal `{}` found.", literal.value),
                )
                .with_evidence("literal", literal.value.clone())
                .with_evidence("matched", matched.clone())
                .with_help("Replace the literal or add a narrow, reasoned ignore."),
            );
        }
    }

    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}
