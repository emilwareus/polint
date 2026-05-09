use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-raw-colors",
    description = "Require design tokens instead of raw TSX color literals.",
    severity = "error"
)]
pub(crate) fn no_raw_colors(
    ctx: &mut RuleCtx<'_>,
    literals: StringLiterals<'_>,
    jsx: JsxAttributes<'_>,
) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    let mut diagnostics = Vec::new();
    for literal in literals
        .iter()
        .filter(|literal| literal.language.is_ts_family())
    {
        let file = ctx.file_path(literal.file);
        if file_in_scope(ctx.options(), &file) && is_raw_color(&literal.value) {
            diagnostics.push(raw_color_diagnostic(
                &rule_id,
                file,
                &literal.span,
                &literal.value,
                "string-literal",
            ));
        }
    }
    for attribute in jsx.iter() {
        let Some(value) = &attribute.value else {
            continue;
        };
        let file = ctx.file_path(attribute.file);
        if file_in_scope(ctx.options(), &file) && is_raw_color(value) {
            diagnostics.push(raw_color_diagnostic(
                &rule_id,
                file,
                &attribute.span,
                value,
                "jsx-attribute",
            ));
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}

fn raw_color_diagnostic(
    rule_id: &str,
    file: String,
    span: &Span,
    value: &str,
    source: &'static str,
) -> Diagnostic {
    Diagnostic::error(
        rule_id.to_string(),
        file,
        span.diagnostic_range(),
        format!("Raw color literal `{value}` should use a design token."),
    )
    .with_evidence("literal", value.to_string())
    .with_evidence("source", source)
    .with_help("Move this value to a theme/design-token file or use an existing token.")
}

fn is_raw_color(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with('#')
        && matches!(lower.len(), 4 | 5 | 7 | 9)
        && lower[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}
