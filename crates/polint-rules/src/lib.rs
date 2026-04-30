use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use polint_core::{Capabilities, Rule, RuleCtx, RuleMeta, RuleOptions};
use polint_diagnostics::{Diagnostic, Severity, fingerprint};
use std::sync::Arc;

pub fn built_in_rules() -> Vec<Arc<dyn Rule>> {
    vec![
        Arc::new(GoCyclomaticComplexity),
        Arc::new(TsCyclomaticComplexity),
        Arc::new(GoImportBoundaries),
        Arc::new(TsNoRawColors),
        Arc::new(GoBranchObligations),
        Arc::new(GoTestSuiteSize),
        Arc::new(GoAssertionAfterAction),
        Arc::new(ConfigQueryNoLiteral),
    ]
}

struct GoCyclomaticComplexity;
struct TsCyclomaticComplexity;
struct GoImportBoundaries;
struct TsNoRawColors;
struct GoBranchObligations;
struct GoTestSuiteSize;
struct GoAssertionAfterAction;
struct ConfigQueryNoLiteral;

impl Rule for GoCyclomaticComplexity {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "examples/go-cyclomatic-complexity".to_string(),
            description: "Warn when a Go function's cyclomatic complexity is high.".to_string(),
            severity: Severity::Warn,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().syntax()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let max = ctx.options().max.unwrap_or(12);
        let functions: Vec<_> = ctx
            .functions()
            .iter()
            .filter(|function| function.language == polint_core::Language::Go)
            .cloned()
            .collect();
        for function in functions {
            if function.cyclomatic_complexity > max
                && file_selected(ctx.options(), &ctx.file_path(function.file))
            {
                ctx.report(
                    Diagnostic::warning(
                        self.meta().id,
                        ctx.file_path(function.file),
                        function.span.diagnostic_range(),
                        format!(
                            "Go function `{}` has cyclomatic complexity {}, max {}.",
                            function.name, function.cyclomatic_complexity, max
                        ),
                    )
                    .with_evidence("function", function.name.clone())
                    .with_evidence("complexity", function.cyclomatic_complexity.to_string())
                    .with_help("Split deeply branched behavior into smaller focused functions or table-driven helpers."),
                );
            }
        }
        Ok(())
    }
}

impl Rule for TsCyclomaticComplexity {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "examples/ts-cyclomatic-complexity".to_string(),
            description: "Warn when a TS/JS function's cyclomatic complexity is high.".to_string(),
            severity: Severity::Warn,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().syntax()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let max = ctx.options().max.unwrap_or(12);
        let functions: Vec<_> = ctx
            .functions()
            .iter()
            .filter(|function| function.language.is_ts_family())
            .cloned()
            .collect();
        for function in functions {
            if function.cyclomatic_complexity > max
                && file_selected(ctx.options(), &ctx.file_path(function.file))
            {
                ctx.report(
                    Diagnostic::warning(
                        self.meta().id,
                        ctx.file_path(function.file),
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
        Ok(())
    }
}

impl Rule for GoImportBoundaries {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "examples/go-import-boundaries".to_string(),
            description: "Enforce configured Go import boundaries.".to_string(),
            severity: Severity::Error,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().imports()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let imports: Vec<_> = ctx
            .imports()
            .iter()
            .filter(|import| import.language == polint_core::Language::Go)
            .cloned()
            .collect();
        let forbidden_imports = ctx.options().forbidden_imports.clone();
        for import in imports {
            let file = ctx.file_path(import.file);
            for (from_glob, forbidden) in &forbidden_imports {
                if glob_matches(from_glob, &file)
                    && forbidden.iter().any(|pattern| {
                        glob_matches(pattern, &import.path) || import.path.contains(pattern)
                    })
                {
                    ctx.report(
                        Diagnostic::error(
                            self.meta().id,
                            file.clone(),
                            import.span.diagnostic_range(),
                            format!("Go import `{}` violates configured import boundary.", import.path),
                        )
                        .with_evidence("import", import.path.clone())
                        .with_help("Move the dependency behind an allowed interface or update the boundary config if this is intentional."),
                    );
                }
            }
        }
        Ok(())
    }
}

impl Rule for TsNoRawColors {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "examples/ts-no-raw-colors".to_string(),
            description: "Detect raw color literals in TS/TSX/JS/JSX.".to_string(),
            severity: Severity::Error,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().string_literals().jsx_attributes()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let literals: Vec<_> = ctx.string_literals().to_vec();
        for literal in literals {
            let file = ctx.file_path(literal.file);
            if !file_selected(ctx.options(), &file) || file_allowed(ctx.options(), &file) {
                continue;
            }
            if is_raw_color(&literal.value) {
                ctx.report(
                    Diagnostic::error(
                        self.meta().id,
                        file,
                        literal.span.diagnostic_range(),
                        format!("Raw color literal `{}` should use a design token.", literal.value),
                    )
                    .with_help("Move this value to a theme/design-token file or use an existing token/CSS variable."),
                );
            }
        }
        Ok(())
    }
}

impl Rule for GoBranchObligations {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "examples/go-branch-obligations".to_string(),
            description: "Heuristically detect Go branches with no nearby test evidence."
                .to_string(),
            severity: Severity::Warn,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().branch_obligations().go_tests()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let branches: Vec<_> = ctx.db().branches().to_vec();
        for branch in branches {
            let file = ctx.file_path(branch.file);
            if !file_selected(ctx.options(), &file) {
                continue;
            }
            if branch.is_error_path
                && !has_nearby_test_evidence(ctx, branch.file, &branch.condition_text)
            {
                ctx.report(
                    Diagnostic::warning(
                        self.meta().id,
                        file,
                        branch.decision_span.diagnostic_range(),
                        format!(
                            "No nearby test evidence found for Go branch `{}`.",
                            branch.condition_text
                        ),
                    )
                    .with_evidence("edge", branch.edge_label.clone())
                    .with_help("Add a test case that exercises this branch. This rule is heuristic and does not prove exact coverage."),
                );
            }
        }
        Ok(())
    }
}

impl Rule for GoTestSuiteSize {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "examples/go-test-suite-size".to_string(),
            description: "Warn when a Go test suite appears overly large.".to_string(),
            severity: Severity::Warn,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().go_tests().test_suite_metrics()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let max = ctx.options().max.unwrap_or(120);
        let tests: Vec<_> = ctx.go_tests().to_vec();
        for test in tests {
            let weight = 1.0
                + f64::from(test.subtest_count) * 0.4
                + f64::from(test.table_rows) * 0.15
                + f64::from(test.assertion_count) * 0.1;
            if weight > f64::from(max) {
                ctx.report(
                    Diagnostic::warning(
                        self.meta().id,
                        ctx.file_path(test.file),
                        test.span.diagnostic_range(),
                        format!(
                            "Go test `{}` has suite weight {:.1}, max {}.",
                            test.name, weight, max
                        ),
                    )
                    .with_help("Split this test suite into smaller behavior-focused suites."),
                );
            }
        }
        Ok(())
    }
}

impl Rule for GoAssertionAfterAction {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "examples/go-assertion-after-action".to_string(),
            description: "Warn when a Go test appears to perform actions without assertions."
                .to_string(),
            severity: Severity::Warn,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().go_tests()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let tests: Vec<_> = ctx.go_tests().to_vec();
        for test in tests {
            if test.assertion_count == 0 {
                ctx.report(
                    Diagnostic::warning(
                        self.meta().id,
                        ctx.file_path(test.file),
                        test.span.diagnostic_range(),
                        format!("Go test `{}` has no obvious assertion or error check.", test.name),
                    )
                    .with_help("Add an explicit assertion, error check, or failure path. This rule is heuristic."),
                );
            }
        }
        Ok(())
    }
}

impl Rule for ConfigQueryNoLiteral {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "examples/config-query-no-literal".to_string(),
            description: "Deny configured string literals across supported languages.".to_string(),
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
        let literals: Vec<_> = ctx.string_literals().to_vec();
        for literal in literals {
            let file = ctx.file_path(literal.file);
            if !file_selected(ctx.options(), &file) || file_allowed(ctx.options(), &file) {
                continue;
            }
            if ctx
                .options()
                .deny
                .iter()
                .any(|deny| literal.value.contains(deny))
            {
                ctx.report(
                    Diagnostic::error(
                        self.meta().id,
                        file,
                        literal.span.diagnostic_range(),
                        format!("Denied literal `{}` found.", literal.value),
                    )
                    .with_help("Replace the literal with an allowed constant or project-specific abstraction."),
                );
            }
        }
        Ok(())
    }
}

fn is_raw_color(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    (lower.starts_with('#')
        && matches!(lower.len(), 4 | 5 | 7 | 9)
        && lower[1..].chars().all(|ch| ch.is_ascii_hexdigit()))
        || lower.starts_with("rgb(")
        || lower.starts_with("rgba(")
        || lower.starts_with("hsl(")
        || lower.starts_with("hsla(")
}

fn has_nearby_test_evidence(ctx: &RuleCtx<'_>, file: polint_core::FileId, condition: &str) -> bool {
    let condition_lower = condition.to_ascii_lowercase();
    ctx.go_tests().iter().any(|test| {
        test.file == file
            && test
                .evidence_terms
                .iter()
                .any(|term| condition_lower.contains(&term.to_ascii_lowercase()))
    })
}

fn file_selected(options: &RuleOptions, file: &str) -> bool {
    options.files.is_empty()
        || options
            .files
            .iter()
            .any(|pattern| glob_matches(pattern, file))
}

fn file_allowed(options: &RuleOptions, file: &str) -> bool {
    options
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

pub fn rule_options_from_config(config: Option<&polint_config::RuleConfig>) -> RuleOptions {
    let Some(config) = config else {
        return RuleOptions::default();
    };
    RuleOptions {
        severity: config.severity.as_deref().and_then(parse_severity),
        files: config.files.clone(),
        allow_files: config.allow_files.clone(),
        allow: config.allow.clone(),
        max: config.max,
        deny: config.deny.clone(),
        forbidden_imports: config.forbidden_imports.clone(),
    }
}

fn parse_severity(value: &str) -> Option<Severity> {
    match value {
        "info" => Some(Severity::Info),
        "warn" | "warning" => Some(Severity::Warn),
        "error" => Some(Severity::Error),
        _ => None,
    }
}

pub fn rule_fingerprint(id: &str) -> String {
    fingerprint(&["polint-rule", id])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_raw_colors() {
        assert!(is_raw_color("#fff"));
        assert!(is_raw_color("rgba(0,0,0,0.5)"));
        assert!(!is_raw_color("primary.500"));
    }

    #[test]
    fn rule_options_from_config_maps_literal_allow_list() {
        let config = polint_config::RuleConfig {
            id: "examples/ts-no-raw-colors".to_string(),
            severity: None,
            files: Vec::new(),
            allow_files: Vec::new(),
            allow: vec!["#fff".to_string(), "currentColor".to_string()],
            max: None,
            deny: Vec::new(),
            forbidden_imports: std::collections::BTreeMap::new(),
        };

        let options = rule_options_from_config(Some(&config));

        assert_eq!(options.allow, vec!["#fff", "currentColor"]);
    }
}
