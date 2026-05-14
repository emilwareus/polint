use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-sensitive-balance-writes",
    description = "Restrict direct writes to sensitive account fields.",
    severity = "error"
)]
pub(crate) fn no_sensitive_balance_writes(
    ctx: &mut RuleCtx<'_>,
    symbols: Symbols<'_>,
    references: References<'_>,
) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    let denied = ctx.options().deny.clone();
    let mut diagnostics = Vec::new();

    for field_name in &denied {
        for symbol in symbols
            .by_name(field_name)
            .filter(|symbol| symbol.language == Language::Go && symbol.kind == SymbolKind::Field)
        {
            for reference in references.to(symbol.id).filter(|reference| {
                matches!(
                    reference.kind,
                    ReferenceKind::Write | ReferenceKind::ReadWrite
                )
            }) {
                push_write_diagnostic(
                    ctx,
                    &rule_id,
                    &mut diagnostics,
                    field_name,
                    symbol,
                    reference,
                );
            }
        }
    }

    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}

fn push_write_diagnostic(
    ctx: &RuleCtx<'_>,
    rule_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
    field_name: &str,
    symbol: &SymbolFact,
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

    diagnostics.push(
        Diagnostic::error(
            rule_id.to_string(),
            file_path,
            range,
            format!("Direct write to sensitive field `{field_name}` is not allowed here."),
        )
        .with_evidence("field", field_name.to_string())
        .with_evidence("symbol_id", symbol.id.0.to_string())
        .with_evidence("reference_id", reference.id.0.to_string())
        .with_evidence("reference_kind", format!("{:?}", reference.kind))
        .with_evidence("precision", format!("{:?}", reference.precision))
        .with_evidence("status", format!("{:?}", reference.status))
        .with_help("Move the mutation into an approved maintenance path or expose a reviewed domain method."),
    );
}
