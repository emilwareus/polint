use crate::config::IgnoreConfig;
use crate::core::{AnalysisDb, SourceFile, rule_id_matches};
use crate::diagnostics::{Diagnostic, TextRange};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const POLINT_IGNORES_JSON_SCHEMA_V1_URL: &str =
    "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-ignores-v1.json";

const DIRECTIVE_PREFIX: &str = "polint-ignore-";
const UNUSED_IGNORE_RULE_ID: &str = "polint/unused-ignore";
const MALFORMED_IGNORE_RULE_ID: &str = "polint/malformed-ignore";
const MISSING_REASON_RULE_ID: &str = "polint/ignore-missing-reason";

#[derive(Debug, Clone)]
pub(crate) struct IgnoreApplication {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) report: IgnoreReport,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct IgnoreReport {
    pub(crate) version: u32,
    pub(crate) schema: &'static str,
    pub(crate) summary: IgnoreSummary,
    pub(crate) directives: Vec<IgnoreDirectiveReport>,
    pub(crate) by_rule: Vec<IgnoreRuleSummary>,
    pub(crate) by_file: Vec<IgnoreFileSummary>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct IgnoreSummary {
    pub(crate) directives: usize,
    pub(crate) active: usize,
    pub(crate) unused: usize,
    pub(crate) malformed: usize,
    pub(crate) missing_reasons: usize,
    pub(crate) suppressed_diagnostics: usize,
    pub(crate) rules: usize,
    pub(crate) files: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct IgnoreDirectiveReport {
    pub(crate) file: String,
    pub(crate) line: u32,
    pub(crate) column: u32,
    pub(crate) kind: String,
    pub(crate) selectors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<String>,
    pub(crate) status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) problem: Option<String>,
    pub(crate) suppressed: Vec<IgnoredDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct IgnoredDiagnostic {
    pub(crate) rule_id: String,
    pub(crate) fingerprint: String,
    pub(crate) file: String,
    pub(crate) range: TextRange,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct IgnoreRuleSummary {
    pub(crate) rule_id: String,
    pub(crate) directives: usize,
    pub(crate) active_directives: usize,
    pub(crate) unused_directives: usize,
    pub(crate) suppressed_diagnostics: usize,
    pub(crate) files: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct IgnoreFileSummary {
    pub(crate) file: String,
    pub(crate) directives: usize,
    pub(crate) active: usize,
    pub(crate) unused: usize,
    pub(crate) malformed: usize,
    pub(crate) missing_reasons: usize,
    pub(crate) suppressed_diagnostics: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IgnoreKind {
    Line,
    NextLine,
    Start,
    End,
    File,
}

impl IgnoreKind {
    fn parse(suffix: &str) -> Option<Self> {
        match suffix {
            "line" => Some(Self::Line),
            "next-line" => Some(Self::NextLine),
            "start" => Some(Self::Start),
            "end" => Some(Self::End),
            "file" => Some(Self::File),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::NextLine => "next-line",
            Self::Start => "start",
            Self::End => "end",
            Self::File => "file",
        }
    }

    fn is_suppressing(self) -> bool {
        matches!(self, Self::Line | Self::NextLine | Self::Start | Self::File)
    }
}

#[derive(Debug, Clone)]
enum IgnoreTarget {
    Line(u32),
    File,
    Range { start: u32, end: u32 },
}

#[derive(Debug, Clone)]
struct IgnoreDirective {
    file: String,
    line: u32,
    column: u32,
    kind: IgnoreKind,
    selectors: Vec<String>,
    reason: Option<String>,
    target: Option<IgnoreTarget>,
    problem: Option<String>,
    missing_reason: bool,
    suppressed: Vec<IgnoredDiagnostic>,
}

impl IgnoreDirective {
    fn suppresses(&self, diagnostic: &Diagnostic) -> bool {
        if self.problem.is_some() || !self.kind.is_suppressing() {
            return false;
        }
        if diagnostic.file != self.file || !is_policy_diagnostic(&diagnostic.rule_id) {
            return false;
        }
        if !self
            .selectors
            .iter()
            .any(|selector| rule_id_matches(selector, &diagnostic.rule_id))
        {
            return false;
        }
        match self.target {
            Some(IgnoreTarget::Line(line)) => diagnostic.range.start_line == line,
            Some(IgnoreTarget::File) => true,
            Some(IgnoreTarget::Range { start, end }) => {
                start <= end
                    && diagnostic.range.start_line >= start
                    && diagnostic.range.start_line <= end
            }
            None => false,
        }
    }

    fn status(&self) -> Option<&'static str> {
        if self.problem.is_some() {
            return Some("malformed");
        }
        if self.kind == IgnoreKind::End {
            return None;
        }
        if self.missing_reason {
            return Some("missing_reason");
        }
        if self.suppressed.is_empty() {
            return Some("unused");
        }
        Some("active")
    }
}

struct RawDirective {
    file: String,
    line: u32,
    column: u32,
    kind: IgnoreKind,
    selectors: Vec<String>,
    reason: Option<String>,
    problem: Option<String>,
}

struct CommentSpan {
    line: u32,
    column: u32,
    text: String,
}

pub(crate) fn apply_ignores(
    db: &AnalysisDb,
    diagnostics: Vec<Diagnostic>,
    config: &IgnoreConfig,
) -> IgnoreApplication {
    let mut directives = collect_directives(db, config);
    let diagnostics = crate::diagnostics::dedupe_diagnostics(diagnostics);
    let mut kept = Vec::new();

    'diagnostics: for diagnostic in diagnostics {
        for directive in &mut directives {
            if directive.suppresses(&diagnostic) {
                directive.suppressed.push(IgnoredDiagnostic {
                    rule_id: diagnostic.rule_id.clone(),
                    fingerprint: diagnostic.stable_fingerprint.clone(),
                    file: diagnostic.file.clone(),
                    range: diagnostic.range,
                });
                continue 'diagnostics;
            }
        }
        kept.push(diagnostic);
    }

    kept.extend(ignore_health_diagnostics(&directives));
    let report = build_report(&directives);
    IgnoreApplication {
        diagnostics: kept,
        report,
    }
}

pub(crate) fn filter_report(report: &IgnoreReport, filters: &[String]) -> IgnoreReport {
    if filters.is_empty() {
        return report.clone();
    }
    let directives = report
        .directives
        .iter()
        .filter(|directive| directive_matches_filters(directive, filters))
        .map(|directive| {
            let mut directive = directive.clone();
            directive.suppressed.retain(|suppressed| {
                filters
                    .iter()
                    .any(|filter| rule_id_matches(filter, &suppressed.rule_id))
            });
            directive
        })
        .collect::<Vec<_>>();
    build_public_report(directives)
}

pub(crate) fn render_ignore_report_human(
    report: &IgnoreReport,
    stat: bool,
    shortstat: bool,
) -> String {
    let mut out = String::new();
    if shortstat {
        out.push_str(&format_shortstat(report));
        out.push('\n');
    }
    if stat {
        if !shortstat {
            out.push_str(&format_shortstat(report));
            out.push('\n');
        }
        out.push_str(&format_stat_tables(report));
        return out;
    }
    if shortstat {
        return out;
    }
    if report.directives.is_empty() {
        out.push_str("No polint ignore directives found.\n");
        return out;
    }
    for directive in &report.directives {
        let selectors = if directive.selectors.is_empty() {
            "<none>".to_string()
        } else {
            directive.selectors.join(",")
        };
        let reason = directive
            .reason
            .as_deref()
            .map(|reason| format!(" reason=\"{reason}\""))
            .unwrap_or_default();
        let problem = directive
            .problem
            .as_deref()
            .map(|problem| format!(" problem=\"{problem}\""))
            .unwrap_or_default();
        out.push_str(&format!(
            "{}:{}:{} {} {} {} suppressed={}{}{}\n",
            directive.file,
            directive.line,
            directive.column,
            directive.kind,
            selectors,
            directive.status,
            directive.suppressed.len(),
            reason,
            problem
        ));
    }
    out
}

fn format_shortstat(report: &IgnoreReport) -> String {
    format!(
        "{} directives, {} active, {} unused, {} malformed, {} missing reasons, {} suppressed diagnostics across {} files and {} rules",
        report.summary.directives,
        report.summary.active,
        report.summary.unused,
        report.summary.malformed,
        report.summary.missing_reasons,
        report.summary.suppressed_diagnostics,
        report.summary.files,
        report.summary.rules
    )
}

fn format_stat_tables(report: &IgnoreReport) -> String {
    let mut out = String::new();
    if !report.by_rule.is_empty() {
        out.push_str("\nBy rule\n");
        for rule in &report.by_rule {
            out.push_str(&format!(
                "  {}: directives={}, active={}, unused={}, suppressed={}, files={}\n",
                rule.rule_id,
                rule.directives,
                rule.active_directives,
                rule.unused_directives,
                rule.suppressed_diagnostics,
                rule.files.join(",")
            ));
        }
    }
    if !report.by_file.is_empty() {
        out.push_str("\nBy file\n");
        for file in &report.by_file {
            out.push_str(&format!(
                "  {}: directives={}, active={}, unused={}, malformed={}, missing_reasons={}, suppressed={}\n",
                file.file,
                file.directives,
                file.active,
                file.unused,
                file.malformed,
                file.missing_reasons,
                file.suppressed_diagnostics
            ));
        }
    }
    out
}

fn collect_directives(db: &AnalysisDb, config: &IgnoreConfig) -> Vec<IgnoreDirective> {
    let mut directives = Vec::new();
    for file in db.files() {
        directives.extend(collect_file_directives(file, config));
    }
    directives.sort_by(|a, b| {
        (a.file.as_str(), a.line, a.column).cmp(&(b.file.as_str(), b.line, b.column))
    });
    pair_block_directives(&mut directives);
    directives
}

fn collect_file_directives(file: &SourceFile, config: &IgnoreConfig) -> Vec<IgnoreDirective> {
    let first_code_line = first_code_line(&file.source);
    let mut directives = Vec::new();
    for comment in scan_comments(&file.source) {
        for raw in parse_comment_directives(file, &comment) {
            let mut directive = IgnoreDirective {
                file: raw.file,
                line: raw.line,
                column: raw.column,
                kind: raw.kind,
                selectors: raw.selectors,
                reason: raw.reason,
                target: None,
                problem: raw.problem,
                missing_reason: false,
                suppressed: Vec::new(),
            };
            if directive.problem.is_none()
                && config.require_reason
                && directive.kind.is_suppressing()
                && directive.reason.is_none()
            {
                directive.missing_reason = true;
            }
            if directive.problem.is_none() {
                directive.target = match directive.kind {
                    IgnoreKind::Line => Some(IgnoreTarget::Line(directive.line)),
                    IgnoreKind::NextLine => Some(IgnoreTarget::Line(directive.line + 1)),
                    IgnoreKind::File => {
                        if first_code_line
                            .map(|line| directive.line < line)
                            .unwrap_or(true)
                        {
                            Some(IgnoreTarget::File)
                        } else {
                            directive.problem = Some(
                                "file ignores must appear before the first code line".to_string(),
                            );
                            None
                        }
                    }
                    IgnoreKind::Start | IgnoreKind::End => None,
                };
            }
            directives.push(directive);
        }
    }
    directives
}

fn pair_block_directives(directives: &mut [IgnoreDirective]) {
    let mut stack: BTreeMap<(String, Vec<String>), Vec<usize>> = BTreeMap::new();
    for idx in 0..directives.len() {
        if directives[idx].problem.is_some() {
            continue;
        }
        let key = (
            directives[idx].file.clone(),
            directives[idx].selectors.clone(),
        );
        match directives[idx].kind {
            IgnoreKind::Start => stack.entry(key).or_default().push(idx),
            IgnoreKind::End => {
                if let Some(start_idx) = stack.get_mut(&key).and_then(Vec::pop) {
                    let start_line = directives[start_idx].line + 1;
                    let end_line = directives[idx].line.saturating_sub(1);
                    directives[start_idx].target = Some(IgnoreTarget::Range {
                        start: start_line,
                        end: end_line,
                    });
                } else {
                    directives[idx].problem = Some(
                        "ignore end has no matching ignore start with the same selectors"
                            .to_string(),
                    );
                }
            }
            IgnoreKind::Line | IgnoreKind::NextLine | IgnoreKind::File => {}
        }
    }

    for indices in stack.into_values() {
        for idx in indices {
            directives[idx].target = None;
            directives[idx].problem = Some("ignore start has no matching ignore end".to_string());
        }
    }
}

fn parse_comment_directives(file: &SourceFile, comment: &CommentSpan) -> Vec<RawDirective> {
    let mut directives = Vec::new();
    for (line_offset, line_text) in comment.text.lines().enumerate() {
        let mut search_start = 0usize;
        while let Some(relative_pos) = line_text[search_start..].find(DIRECTIVE_PREFIX) {
            let prefix_start = search_start + relative_pos;
            if prefix_start > 0
                && !line_text[..prefix_start]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_whitespace())
            {
                search_start = prefix_start + DIRECTIVE_PREFIX.len();
                continue;
            }
            let suffix_start = prefix_start + DIRECTIVE_PREFIX.len();
            let suffix = &line_text[suffix_start..];
            let Some((kind, suffix_len)) = parse_kind_prefix(suffix) else {
                search_start = suffix_start;
                continue;
            };
            let after_kind = &suffix[suffix_len..];
            if !after_kind.is_empty()
                && !after_kind
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_whitespace())
            {
                search_start = suffix_start;
                continue;
            }
            let directive_line = comment.line + line_offset as u32;
            let directive_col = if line_offset == 0 {
                comment.column + prefix_start as u32
            } else {
                prefix_start as u32 + 1
            };
            directives.push(parse_directive_tail(
                file.relative_path.clone(),
                directive_line,
                directive_col,
                kind,
                after_kind,
            ));
            search_start = suffix_start;
        }
    }
    directives
}

fn parse_kind_prefix(suffix: &str) -> Option<(IgnoreKind, usize)> {
    ["next-line", "line", "start", "end", "file"]
        .iter()
        .find_map(|name| {
            suffix
                .starts_with(name)
                .then(|| IgnoreKind::parse(name).map(|kind| (kind, name.len())))
                .flatten()
        })
}

fn parse_directive_tail(
    file: String,
    line: u32,
    column: u32,
    kind: IgnoreKind,
    tail: &str,
) -> RawDirective {
    let tail = tail.trim();
    let (selector_part, reason) = match tail.split_once("--") {
        Some((selectors, reason)) => {
            let reason = reason.trim();
            (
                selectors.trim(),
                (!reason.is_empty()).then(|| reason.to_string()),
            )
        }
        None => (tail, None),
    };
    let selectors = selector_part
        .split(',')
        .map(str::trim)
        .filter(|selector| !selector.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut selectors = selectors;
    selectors.sort();
    selectors.dedup();
    let problem = selectors.is_empty().then(|| {
        "ignore directives must name at least one rule selector, for example local/no-todo"
            .to_string()
    });
    RawDirective {
        file,
        line,
        column,
        kind,
        selectors,
        reason,
        problem,
    }
}

fn scan_comments(source: &str) -> Vec<CommentSpan> {
    let bytes = source.as_bytes();
    let mut comments = Vec::new();
    let mut i = 0usize;
    let mut line = 1u32;
    let mut col = 1u32;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                let start_line = line;
                let start_col = col + 2;
                i += 2;
                col += 2;
                let text_start = i;
                while i < bytes.len() && bytes[i] != b'\n' {
                    advance_char(source, &mut i, &mut line, &mut col);
                }
                comments.push(CommentSpan {
                    line: start_line,
                    column: start_col,
                    text: source[text_start..i].to_string(),
                });
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                let start_line = line;
                let start_col = col + 2;
                i += 2;
                col += 2;
                let text_start = i;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    advance_char(source, &mut i, &mut line, &mut col);
                }
                let text_end = i.min(bytes.len());
                if i + 1 < bytes.len() {
                    i += 2;
                    col += 2;
                }
                comments.push(CommentSpan {
                    line: start_line,
                    column: start_col,
                    text: source[text_start..text_end].to_string(),
                });
            }
            b'"' | b'\'' => {
                let quote = bytes[i];
                skip_quoted_string(source, &mut i, &mut line, &mut col, quote);
            }
            b'`' => skip_backtick_string(source, &mut i, &mut line, &mut col),
            _ => advance_char(source, &mut i, &mut line, &mut col),
        }
    }
    comments
}

fn first_code_line(source: &str) -> Option<u32> {
    let bytes = source.as_bytes();
    let mut i = 0usize;
    let mut line = 1u32;
    let mut col = 1u32;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                i += 2;
                col += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    advance_char(source, &mut i, &mut line, &mut col);
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                col += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    advance_char(source, &mut i, &mut line, &mut col);
                }
                if i + 1 < bytes.len() {
                    i += 2;
                    col += 2;
                }
            }
            byte if byte.is_ascii_whitespace() => advance_char(source, &mut i, &mut line, &mut col),
            _ => return Some(line),
        }
    }
    None
}

fn skip_quoted_string(source: &str, i: &mut usize, line: &mut u32, col: &mut u32, quote: u8) {
    advance_char(source, i, line, col);
    while *i < source.len() {
        let byte = source.as_bytes()[*i];
        if byte == b'\\' {
            advance_char(source, i, line, col);
            if *i < source.len() {
                advance_char(source, i, line, col);
            }
        } else if byte == quote {
            advance_char(source, i, line, col);
            break;
        } else {
            advance_char(source, i, line, col);
        }
    }
}

fn skip_backtick_string(source: &str, i: &mut usize, line: &mut u32, col: &mut u32) {
    advance_char(source, i, line, col);
    while *i < source.len() {
        let byte = source.as_bytes()[*i];
        advance_char(source, i, line, col);
        if byte == b'`' {
            break;
        }
    }
}

fn advance_char(source: &str, i: &mut usize, line: &mut u32, col: &mut u32) {
    let ch = source[*i..].chars().next().unwrap_or('\0');
    *i += ch.len_utf8();
    if ch == '\n' {
        *line += 1;
        *col = 1;
    } else {
        *col += 1;
    }
}

fn ignore_health_diagnostics(directives: &[IgnoreDirective]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for directive in directives {
        if let Some(problem) = &directive.problem {
            diagnostics.push(
                Diagnostic::error(
                    MALFORMED_IGNORE_RULE_ID,
                    directive.file.clone(),
                    TextRange::point(directive.line, directive.column),
                    format!("Malformed polint ignore directive: {problem}"),
                )
                .with_evidence("kind", directive.kind.as_str())
                .with_evidence("selectors", directive.selectors.join(",")),
            );
            continue;
        }
        if directive.missing_reason {
            diagnostics.push(
                Diagnostic::error(
                    MISSING_REASON_RULE_ID,
                    directive.file.clone(),
                    TextRange::point(directive.line, directive.column),
                    "polint ignore directive requires a reason after `--`",
                )
                .with_evidence("kind", directive.kind.as_str())
                .with_evidence("selectors", directive.selectors.join(",")),
            );
            continue;
        }
        if directive.kind.is_suppressing() && directive.suppressed.is_empty() {
            diagnostics.push(
                Diagnostic::warning(
                    UNUSED_IGNORE_RULE_ID,
                    directive.file.clone(),
                    TextRange::point(directive.line, directive.column),
                    "Unused polint ignore directive.",
                )
                .with_evidence("kind", directive.kind.as_str())
                .with_evidence("selectors", directive.selectors.join(",")),
            );
        }
    }
    diagnostics
}

fn build_report(directives: &[IgnoreDirective]) -> IgnoreReport {
    let public = directives
        .iter()
        .filter_map(|directive| {
            let status = directive.status()?;
            Some(IgnoreDirectiveReport {
                file: directive.file.clone(),
                line: directive.line,
                column: directive.column,
                kind: directive.kind.as_str().to_string(),
                selectors: directive.selectors.clone(),
                reason: directive.reason.clone(),
                status: status.to_string(),
                problem: directive.problem.clone(),
                suppressed: directive.suppressed.clone(),
            })
        })
        .collect::<Vec<_>>();
    build_public_report(public)
}

fn build_public_report(directives: Vec<IgnoreDirectiveReport>) -> IgnoreReport {
    let mut summary = IgnoreSummary {
        directives: directives.len(),
        ..IgnoreSummary::default()
    };
    let mut rules = BTreeSet::new();
    let mut files = BTreeSet::new();
    for directive in &directives {
        files.insert(directive.file.clone());
        match directive.status.as_str() {
            "active" => summary.active += 1,
            "unused" => summary.unused += 1,
            "malformed" => summary.malformed += 1,
            "missing_reason" => summary.missing_reasons += 1,
            _ => {}
        }
        summary.suppressed_diagnostics += directive.suppressed.len();
        for selector in &directive.selectors {
            rules.insert(selector.clone());
        }
        for suppressed in &directive.suppressed {
            rules.insert(suppressed.rule_id.clone());
        }
    }
    summary.rules = rules.len();
    summary.files = files.len();

    let by_rule = build_rule_summary(&directives);
    let by_file = build_file_summary(&directives);
    IgnoreReport {
        version: 1,
        schema: POLINT_IGNORES_JSON_SCHEMA_V1_URL,
        summary,
        directives,
        by_rule,
        by_file,
    }
}

fn build_rule_summary(directives: &[IgnoreDirectiveReport]) -> Vec<IgnoreRuleSummary> {
    #[derive(Default)]
    struct Acc {
        directives: usize,
        active_directives: usize,
        unused_directives: usize,
        suppressed_diagnostics: usize,
        files: BTreeSet<String>,
    }
    let mut by_rule = BTreeMap::<String, Acc>::new();
    for directive in directives {
        for selector in &directive.selectors {
            let entry = by_rule.entry(selector.clone()).or_default();
            entry.directives += 1;
            if directive.status == "active" || directive.status == "missing_reason" {
                entry.active_directives += 1;
            }
            if directive.status == "unused" {
                entry.unused_directives += 1;
            }
            entry.files.insert(directive.file.clone());
        }
        for suppressed in &directive.suppressed {
            let entry = by_rule.entry(suppressed.rule_id.clone()).or_default();
            entry.suppressed_diagnostics += 1;
            entry.files.insert(suppressed.file.clone());
        }
    }
    by_rule
        .into_iter()
        .map(|(rule_id, acc)| IgnoreRuleSummary {
            rule_id,
            directives: acc.directives,
            active_directives: acc.active_directives,
            unused_directives: acc.unused_directives,
            suppressed_diagnostics: acc.suppressed_diagnostics,
            files: acc.files.into_iter().collect(),
        })
        .collect()
}

fn build_file_summary(directives: &[IgnoreDirectiveReport]) -> Vec<IgnoreFileSummary> {
    let mut by_file = BTreeMap::<String, IgnoreFileSummary>::new();
    for directive in directives {
        let entry = by_file
            .entry(directive.file.clone())
            .or_insert_with(|| IgnoreFileSummary {
                file: directive.file.clone(),
                ..IgnoreFileSummary::default()
            });
        entry.directives += 1;
        match directive.status.as_str() {
            "active" => entry.active += 1,
            "unused" => entry.unused += 1,
            "malformed" => entry.malformed += 1,
            "missing_reason" => entry.missing_reasons += 1,
            _ => {}
        }
        entry.suppressed_diagnostics += directive.suppressed.len();
    }
    by_file.into_values().collect()
}

fn directive_matches_filters(directive: &IgnoreDirectiveReport, filters: &[String]) -> bool {
    directive.selectors.iter().any(|selector| {
        filters
            .iter()
            .any(|filter| selectors_overlap(selector, filter.as_str()))
    }) || directive.suppressed.iter().any(|suppressed| {
        filters
            .iter()
            .any(|filter| rule_id_matches(filter, &suppressed.rule_id))
    })
}

fn selectors_overlap(left: &str, right: &str) -> bool {
    left == "*" || right == "*" || rule_id_matches(left, right) || rule_id_matches(right, left)
}

fn is_policy_diagnostic(rule_id: &str) -> bool {
    !rule_id.starts_with("parser/")
        && !rule_id.starts_with("internal/")
        && !rule_id.starts_with("polint/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::IgnoreConfig;
    use crate::core::AnalysisDb;
    use crate::diagnostics::Severity;

    fn db_with_source(path: &str, source: &str) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        db.add_file(path.into(), path.to_string(), source.to_string());
        db
    }

    fn diagnostic(rule_id: &str, file: &str, line: u32) -> Diagnostic {
        Diagnostic::new(
            rule_id,
            Severity::Warn,
            file,
            TextRange::point(line, 1),
            "finding",
        )
    }

    #[test]
    fn next_line_ignore_suppresses_matching_policy_diagnostic() {
        let db = db_with_source(
            "src/a.ts",
            "// polint-ignore-next-line local/no-todo -- tracked cleanup\nconst x = 'TODO';\n",
        );
        let result = apply_ignores(
            &db,
            vec![diagnostic("local/no-todo", "src/a.ts", 2)],
            &IgnoreConfig::default(),
        );
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.report.summary.active, 1);
        assert_eq!(result.report.summary.suppressed_diagnostics, 1);
    }

    #[test]
    fn comments_inside_strings_do_not_create_ignores() {
        let db = db_with_source(
            "src/a.ts",
            r#"const s = "// polint-ignore-next-line local/no-todo";
const x = "TODO";
"#,
        );
        let result = apply_ignores(
            &db,
            vec![diagnostic("local/no-todo", "src/a.ts", 2)],
            &IgnoreConfig::default(),
        );
        assert_eq!(
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.rule_id == "local/no-todo")
                .count(),
            1
        );
        assert_eq!(result.report.summary.directives, 0);
    }

    #[test]
    fn directive_names_require_a_leading_boundary() {
        let db = db_with_source(
            "src/a.ts",
            "// notpolint-ignore-next-line local/no-todo\nconst x = \"TODO\";\n",
        );
        let result = apply_ignores(
            &db,
            vec![diagnostic("local/no-todo", "src/a.ts", 2)],
            &IgnoreConfig::default(),
        );
        assert_eq!(result.report.summary.directives, 0);
        assert_eq!(
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.rule_id == "local/no-todo")
                .count(),
            1
        );
    }

    #[test]
    fn inline_ignore_after_code_is_detected() {
        let db = db_with_source(
            "src/a.ts",
            "const x = \"TODO\"; // polint-ignore-line local/no-todo -- fixture\n",
        );
        let result = apply_ignores(
            &db,
            vec![diagnostic("local/no-todo", "src/a.ts", 1)],
            &IgnoreConfig::default(),
        );
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.report.summary.active, 1);
    }

    #[test]
    fn parser_and_polint_diagnostics_are_not_suppressed() {
        let db = db_with_source(
            "src/a.ts",
            "// polint-ignore-next-line * -- only policy diagnostics\nconst x =\n",
        );
        let result = apply_ignores(
            &db,
            vec![
                diagnostic("parser/ts", "src/a.ts", 2),
                diagnostic("polint/capability", "src/a.ts", 2),
                diagnostic("internal/local/rule", "src/a.ts", 2),
            ],
            &IgnoreConfig::default(),
        );
        assert_eq!(result.diagnostics.len(), 4);
        assert_eq!(result.report.summary.unused, 1);
    }

    #[test]
    fn require_reason_reports_missing_reason_but_still_suppresses() {
        let db = db_with_source(
            "src/a.ts",
            "// polint-ignore-next-line local/no-todo\nconst x = 'TODO';\n",
        );
        let result = apply_ignores(
            &db,
            vec![diagnostic("local/no-todo", "src/a.ts", 2)],
            &IgnoreConfig {
                require_reason: true,
            },
        );
        assert_eq!(
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.rule_id == MISSING_REASON_RULE_ID)
                .count(),
            1
        );
        assert_eq!(result.report.summary.missing_reasons, 1);
        assert_eq!(result.report.summary.suppressed_diagnostics, 1);
    }

    #[test]
    fn block_ignore_suppresses_only_between_matching_directives() {
        let db = db_with_source(
            "src/a.ts",
            "// polint-ignore-start local/no-todo -- generated block\nconst a = 'TODO';\n// polint-ignore-end local/no-todo\nconst b = 'TODO';\n",
        );
        let result = apply_ignores(
            &db,
            vec![
                diagnostic("local/no-todo", "src/a.ts", 2),
                diagnostic("local/no-todo", "src/a.ts", 4),
            ],
            &IgnoreConfig::default(),
        );
        assert_eq!(
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.rule_id == "local/no-todo")
                .count(),
            1
        );
        assert_eq!(result.report.summary.active, 1);
    }

    #[test]
    fn file_ignore_must_be_before_code() {
        let db = db_with_source(
            "src/a.ts",
            "const x = 1;\n// polint-ignore-file local/no-todo -- too late\n",
        );
        let result = apply_ignores(&db, Vec::new(), &IgnoreConfig::default());
        assert_eq!(result.report.summary.malformed, 1);
        assert_eq!(
            result.diagnostics[0].rule_id,
            "polint/malformed-ignore".to_string()
        );
    }

    #[test]
    fn filter_report_matches_broad_selectors() {
        let report = IgnoreReport {
            version: 1,
            schema: POLINT_IGNORES_JSON_SCHEMA_V1_URL,
            summary: IgnoreSummary::default(),
            directives: vec![IgnoreDirectiveReport {
                file: "src/a.ts".to_string(),
                line: 1,
                column: 1,
                kind: "next-line".to_string(),
                selectors: vec!["local/*".to_string()],
                reason: Some("legacy".to_string()),
                status: "unused".to_string(),
                problem: None,
                suppressed: Vec::new(),
            }],
            by_rule: Vec::new(),
            by_file: Vec::new(),
        };
        let filtered = filter_report(&report, &["local/no-todo".to_string()]);
        assert_eq!(filtered.summary.directives, 1);
    }

    #[test]
    fn filter_report_keeps_only_relevant_suppressed_diagnostics() {
        let report = IgnoreReport {
            version: 1,
            schema: POLINT_IGNORES_JSON_SCHEMA_V1_URL,
            summary: IgnoreSummary::default(),
            directives: vec![IgnoreDirectiveReport {
                file: "src/a.ts".to_string(),
                line: 1,
                column: 1,
                kind: "file".to_string(),
                selectors: vec!["local/*".to_string()],
                reason: Some("legacy".to_string()),
                status: "active".to_string(),
                problem: None,
                suppressed: vec![
                    IgnoredDiagnostic {
                        rule_id: "local/a".to_string(),
                        fingerprint: "a".to_string(),
                        file: "src/a.ts".to_string(),
                        range: TextRange::point(1, 1),
                    },
                    IgnoredDiagnostic {
                        rule_id: "local/b".to_string(),
                        fingerprint: "b".to_string(),
                        file: "src/a.ts".to_string(),
                        range: TextRange::point(2, 1),
                    },
                ],
            }],
            by_rule: Vec::new(),
            by_file: Vec::new(),
        };

        let filtered = filter_report(&report, &["local/a".to_string()]);
        assert_eq!(filtered.summary.suppressed_diagnostics, 1);
        assert_eq!(filtered.directives[0].suppressed[0].rule_id, "local/a");
    }

    #[test]
    fn stat_and_shortstat_human_output_prints_one_summary_line() {
        let report = IgnoreReport {
            version: 1,
            schema: POLINT_IGNORES_JSON_SCHEMA_V1_URL,
            summary: IgnoreSummary {
                directives: 1,
                active: 1,
                files: 1,
                rules: 1,
                ..IgnoreSummary::default()
            },
            directives: Vec::new(),
            by_rule: vec![IgnoreRuleSummary {
                rule_id: "local/a".to_string(),
                directives: 1,
                active_directives: 1,
                unused_directives: 0,
                suppressed_diagnostics: 0,
                files: vec!["src/a.ts".to_string()],
            }],
            by_file: Vec::new(),
        };

        let rendered = render_ignore_report_human(&report, true, true);
        assert_eq!(rendered.matches("1 directives, 1 active").count(), 1);
        assert!(rendered.contains("By rule"));
    }

    #[test]
    fn ignores_json_schema_file_tracks_repo() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/schemas/polint-ignores-v1.json");
        let raw = std::fs::read_to_string(&path).expect("polint ignores schema should exist");
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            value["$id"].as_str().unwrap(),
            POLINT_IGNORES_JSON_SCHEMA_V1_URL
        );
    }
}
