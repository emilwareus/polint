// This is the whole policy for the custom-rule-ts example repo.
// It registers one local rule, local/no-product-hex-colors, which catches raw
// hex colors in product UI code so contributors use local design tokens.
use polint::sdk::prelude::*;

pub(crate) struct NoProductHexColors;

impl Rule for NoProductHexColors {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "local/no-product-hex-colors".to_string(),
            description: "Require product UI colors to come from design tokens.".to_string(),
            severity: Severity::Error,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().string_literals().jsx_attributes()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> RuleResult {
        let rule_id = self.meta().id;
        let mut diagnostics = Vec::new();
        for literal in ctx
            .string_literals()
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
        for attribute in ctx.jsx_attributes() {
            let Some(value) = &attribute.value else {
                continue;
            };
            let Some(source) = ctx.source_file(attribute.file) else {
                continue;
            };
            let file = ctx.file_path(attribute.file);
            if source.language.is_ts_family()
                && file_in_scope(ctx.options(), &file)
                && is_raw_color(value)
            {
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
        format!("Raw product color `{value}` should use a design token."),
    )
    .with_evidence("literal", value.to_string())
    .with_evidence("source", source)
    .with_help("Use a local design token instead of embedding raw product colors.")
}

fn is_raw_color(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with('#')
        && matches!(lower.len(), 4 | 5 | 7 | 9)
        && lower[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}
