use polint::sdk::prelude::*;

pub(crate) struct GoImportBoundaries;

impl Rule for GoImportBoundaries {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "local/go-import-boundaries".to_string(),
            description: "Enforce configured Go import boundaries.".to_string(),
            severity: Severity::Error,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().imports()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let rule_id = self.meta().id;
        let mut diagnostics = Vec::new();
        for import in ctx
            .imports()
            .iter()
            .filter(|import| import.language == Language::Go)
        {
            let file = ctx.file_path(import.file);
            if !file_in_scope(ctx.options(), &file) {
                continue;
            }
            for (from_glob, forbidden) in &ctx.options().forbidden_imports {
                if glob_matches(from_glob, &file)
                    && forbidden.iter().any(|pattern| {
                        glob_matches(pattern, &import.path) || import.path.contains(pattern)
                    })
                {
                    diagnostics.push(
                        Diagnostic::error(
                            rule_id.clone(),
                            file.clone(),
                            import.span.diagnostic_range(),
                            format!(
                                "Go import `{}` violates the local import boundary.",
                                import.path
                            ),
                        )
                        .with_evidence("import", import.path.clone())
                        .with_help(
                            "Move this dependency behind an allowed interface or update the local boundary config.",
                        ),
                    );
                }
            }
        }
        for diagnostic in diagnostics {
            ctx.report(diagnostic);
        }
        Ok(())
    }
}
