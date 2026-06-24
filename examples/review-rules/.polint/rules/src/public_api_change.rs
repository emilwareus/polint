// The COMPLEX review rule: full engine + diff.
//
// This rule runs real symbol/reference analysis (the same machinery a `check`
// rule uses) but restricts it to changed files: for every exported symbol
// defined in a changed file that is referenced from *another* file, it flags a
// public-API impact for review. The diff (`ChangedFiles<'_>`) narrows the work;
// `Symbols<'_>` and `References<'_>` do the analysis.
use polint::sdk::prelude::*;

#[polint::rule(
    id = "review/public-api-change",
    description = "Changed exported symbol that other modules import (heuristic).",
    severity = "warn",
    kind = "review"
)]
pub(crate) fn public_api_change(
    ctx: &mut RuleCtx<'_>,
    changes: ChangedFiles<'_>,
    files: SourceFiles<'_>,
    symbols: Symbols<'_>,
    references: References<'_>,
) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    let mut diagnostics = Vec::new();
    for source in files.iter() {
        // Only consider files that actually changed in this review.
        if !changes.contains_path(&source.relative_path) {
            continue;
        }
        for symbol in symbols
            .for_file(source.id)
            .filter(|symbol| symbol.is_exported)
        {
            // Referenced from a DIFFERENT file than the one it is defined in?
            let used_elsewhere = references
                .to(symbol.id)
                .any(|reference| reference.file.is_some_and(|file| file != source.id));
            if !used_elsewhere {
                continue;
            }
            let Some(span) = &symbol.primary_span else {
                continue;
            };
            diagnostics.push(
                Diagnostic::warning(
                    rule_id.clone(),
                    source.relative_path.clone(),
                    span.diagnostic_range(),
                    format!(
                        "Exported `{}` changed and is imported by other modules — review the \
                         public-API impact.",
                        symbol.name
                    ),
                )
                .with_help(
                    "Repo-local review policy (heuristic): a changed exported symbol with \
                     cross-file references is a public-API surface; confirm callers still hold.",
                ),
            );
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}
