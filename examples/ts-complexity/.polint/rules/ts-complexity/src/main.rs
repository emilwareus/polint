// This is the whole policy for the ts-complexity example repo.
// It registers one local rule, local/ts-cyclomatic-complexity, which warns when
// a TS/JS function's extracted cyclomatic complexity exceeds the configured max.
use globset::{Glob, GlobSet, GlobSetBuilder};
use polint_sdk::prelude::*;
use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint_runner::run_cli(vec![Arc::new(TsComplexity)])
}

struct TsComplexity;

impl Rule for TsComplexity {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "local/ts-cyclomatic-complexity".to_string(),
            description: "Warn when a TS/JS function's cyclomatic complexity is high.".to_string(),
            severity: Severity::Warn,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().syntax()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let max = ctx.options().max.unwrap_or(12);
        let rule_id = self.meta().id;
        let mut diagnostics = Vec::new();
        for function in ctx
            .functions()
            .iter()
            .filter(|function| function.language.is_ts_family())
        {
            let file = ctx.file_path(function.file);
            if function.cyclomatic_complexity > max && file_in_scope(ctx.options(), &file) {
                diagnostics.push(
                    Diagnostic::warning(
                        rule_id.clone(),
                        file,
                        function.span.diagnostic_range(),
                        format!(
                            "TS/JS function `{}` has cyclomatic complexity {}, max {}.",
                            function.name, function.cyclomatic_complexity, max
                        ),
                    )
                    .with_evidence("function", function.name.clone())
                    .with_evidence("complexity", function.cyclomatic_complexity.to_string())
                    .with_help(
                        "Split condition-heavy UI or business logic into smaller named helpers.",
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
