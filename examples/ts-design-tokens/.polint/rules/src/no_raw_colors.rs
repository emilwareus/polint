// This is the whole policy for the ts-design-tokens example repo.
// It registers one local rule, local/no-raw-colors, which finds raw color
// literals in TSX code and asks contributors to use design tokens instead.
// The rule reads both string literals and JSX attributes, then dedupes overlaps.
use globset::{Glob, GlobSet, GlobSetBuilder};
use polint::sdk::prelude::*;

pub struct NoRawColors;

impl Rule for NoRawColors {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "local/no-raw-colors".to_string(),
            description: "Detect raw color literals in TSX UI code.".to_string(),
            severity: Severity::Error,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().string_literals().jsx_attributes()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        let rule_id = self.meta().id;
        let mut diagnostics = Vec::new();
        let mut seen = Vec::new();
        for literal in ctx
            .string_literals()
            .iter()
            .filter(|literal| literal.language.is_ts_family())
        {
            push_raw_color(
                ctx,
                &rule_id,
                &mut seen,
                &mut diagnostics,
                RawColorFinding {
                    file: literal.file,
                    span: &literal.span,
                    value: &literal.value,
                    source: "string-literal",
                },
            );
        }
        for attribute in ctx.jsx_attributes() {
            let Some(value) = &attribute.value else {
                continue;
            };
            let Some(source) = ctx.source_file(attribute.file) else {
                continue;
            };
            if source.language.is_ts_family() {
                push_raw_color(
                    ctx,
                    &rule_id,
                    &mut seen,
                    &mut diagnostics,
                    RawColorFinding {
                        file: attribute.file,
                        span: &attribute.span,
                        value,
                        source: "jsx-attribute",
                    },
                );
            }
        }
        for diagnostic in diagnostics {
            ctx.report(diagnostic);
        }
        Ok(())
    }
}

struct RawColorFinding<'a> {
    file: FileId,
    span: &'a Span,
    value: &'a str,
    source: &'static str,
}

fn push_raw_color(
    ctx: &RuleCtx<'_>,
    rule_id: &str,
    seen: &mut Vec<(FileId, u32, u32, String)>,
    diagnostics: &mut Vec<Diagnostic>,
    finding: RawColorFinding<'_>,
) {
    let file = ctx.file_path(finding.file);
    if !file_in_scope(ctx.options(), &file) || !is_raw_color(finding.value) {
        return;
    }
    if seen.iter().any(|seen| {
        seen.0 == finding.file
            && seen.3 == finding.value
            && seen.1 < finding.span.end_byte
            && finding.span.start_byte < seen.2
    }) {
        return;
    }
    seen.push((
        finding.file,
        finding.span.start_byte,
        finding.span.end_byte,
        finding.value.to_string(),
    ));
    diagnostics.push(
        Diagnostic::error(
            rule_id.to_string(),
            file,
            finding.span.diagnostic_range(),
            format!(
                "Raw color literal `{}` should use a design token.",
                finding.value
            ),
        )
        .with_evidence("literal", finding.value.to_string())
        .with_evidence("source", finding.source)
        .with_help("Use a theme token, CSS variable, or design-system color alias."),
    );
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
        && !options.allow.iter().any(|allowed| allowed == file)
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
