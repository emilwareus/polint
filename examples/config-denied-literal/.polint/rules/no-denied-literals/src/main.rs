// This is the whole policy for the config-denied-literal example repo.
// It registers one local rule, local/no-denied-literals, which reads deny-list
// values from .polint.toml and reports matching Go/TS/JS string literals.
use globset::{Glob, GlobSet, GlobSetBuilder};
use polint_sdk::prelude::*;
use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint_runner::run_cli(vec![Arc::new(NoDeniedLiterals)])
}

struct NoDeniedLiterals;

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
            if !file_in_scope(ctx.options(), &file)
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

fn file_in_scope(options: &RuleOptions, file: &str) -> bool {
    (options.files.is_empty()
        || options
            .files
            .iter()
            .any(|pattern| glob_matches(pattern, file)))
        && !options
            .allow_files
            .iter()
            .any(|pattern| glob_matches(pattern, file))
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    build_one(pattern)
        .map(|glob| glob.is_match(value) || glob.is_match(format!("./{value}")))
        .unwrap_or_else(|| value.contains(pattern.trim_matches('*')))
}

fn build_one(pattern: &str) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(pattern).ok()?);
    builder.build().ok()
}
