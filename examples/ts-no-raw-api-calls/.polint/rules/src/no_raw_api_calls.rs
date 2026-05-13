use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-raw-api-calls",
    description = "Require generated SDK clients instead of raw API calls.",
    severity = "error"
)]
pub(crate) fn no_raw_api_calls(
    ctx: &mut RuleCtx<'_>,
    symbols: Symbols<'_>,
    references: References<'_>,
) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    let denied = ctx.options().deny.clone();
    let mut diagnostics = Vec::new();

    for api in &denied {
        for symbol in symbols.by_name(api) {
            for reference in references
                .to(symbol.id)
                .filter(|reference| reference.kind == ReferenceKind::Call)
            {
                push_api_diagnostic(
                    ctx,
                    &rule_id,
                    &mut diagnostics,
                    api,
                    Some(symbol),
                    reference,
                );
            }
        }
    }

    for reference in references
        .unresolved()
        .filter(|reference| reference.kind == ReferenceKind::Call)
    {
        if denied.iter().any(|api| api == &reference.name) {
            push_api_diagnostic(
                ctx,
                &rule_id,
                &mut diagnostics,
                &reference.name,
                None,
                reference,
            );
        }
    }

    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}

fn push_api_diagnostic(
    ctx: &RuleCtx<'_>,
    rule_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
    api: &str,
    symbol: Option<&SymbolFact>,
    reference: &ReferenceFact,
) {
    let Some(file) = reference.file else {
        return;
    };
    let file_path = ctx.file_path(file);
    if !file_in_scope(ctx.options(), &file_path) {
        return;
    }
    let range = reference
        .primary_span
        .as_ref()
        .map(Span::diagnostic_range)
        .unwrap_or_else(|| DiagnosticRange::point(1, 1));
    let symbol_id = symbol
        .map(|symbol| symbol.id.0.to_string())
        .unwrap_or_else(|| "unresolved".to_string());

    diagnostics.push(
        Diagnostic::error(
            rule_id.to_string(),
            file_path,
            range,
            format!("Use the generated API SDK instead of calling `{api}` directly."),
        )
        .with_evidence("api", api.to_string())
        .with_evidence("symbol_id", symbol_id)
        .with_evidence("reference_id", reference.id.0.to_string())
        .with_evidence("reference_kind", format!("{:?}", reference.kind))
        .with_evidence("precision", format!("{:?}", reference.precision))
        .with_evidence("status", format!("{:?}", reference.status))
        .with_help("Move this call behind the generated SDK client for the service."),
    );
}
