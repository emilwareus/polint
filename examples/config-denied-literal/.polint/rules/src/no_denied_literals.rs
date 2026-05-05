// This is the whole policy for the config-denied-literal example repo.
// It registers one local rule, local/no-denied-literals, which reads deny-list
// values from .polint.toml and reports matching Go/TS/JS string literals.
//
// `RuleOptions::allow` here is wired to **literal-text** allowlisting via
// `literal_allowed`, so we use [`file_matches_globs`] for path scoping rather
// than [`file_in_scope`] (which would treat `allow` entries as paths).
use polint::sdk::prelude::*;

pub(crate) struct NoDeniedLiterals;

impl Rule for NoDeniedLiterals {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "local/no-denied-literals".to_string(),
            description: "Deny configured string or regex literal text.".to_string(),
            severity: Severity::Error,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().string_literals()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        if ctx.options().deny.is_empty() {
            return Ok(());
        }
        let rule_id = self.meta().id;
        let mut diagnostics = Vec::new();
        for literal in ctx
            .string_literals()
            .iter()
            .filter(|literal| literal.language.is_ts_family() || literal.language == Language::Go)
        {
            let file = ctx.file_path(literal.file);
            if !file_matches_globs(ctx.options(), &file)
                || literal_allowed(ctx.options(), &literal.value)
            {
                continue;
            }
            if let Some(matched) = ctx
                .options()
                .deny
                .iter()
                .find(|deny| literal.value.contains(deny.as_str()))
            {
                diagnostics.push(
                    Diagnostic::error(
                        rule_id.clone(),
                        file,
                        literal.span.diagnostic_range(),
                        format!("Configured denied literal `{}` found.", literal.value),
                    )
                    .with_evidence("literal", literal.value.clone())
                    .with_evidence("matched", matched.clone())
                    .with_help(
                        "Replace the literal with an allowed constant or local abstraction.",
                    ),
                );
            }
        }
        for diagnostic in diagnostics {
            ctx.report(diagnostic);
        }
        Ok(())
    }
}

fn literal_allowed(options: &RuleOptions, value: &str) -> bool {
    options.allow.iter().any(|allowed| allowed == value)
}
