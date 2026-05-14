use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{self, IsTerminal};
use std::sync::Arc;

/// Public URL of [`crate::diagnostics::PolintReport`] JSON Schema (v1); embedded in `--format json` when present.
pub const POLINT_REPORT_JSON_SCHEMA_V1_URL: &str =
    "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-report-v1.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

impl Severity {
    pub fn is_at_least(self, threshold: Severity) -> bool {
        self >= threshold
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => f.write_str("info"),
            Severity::Warn => f.write_str("warn"),
            Severity::Error => f.write_str("error"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl TextRange {
    pub fn point(line: u32, col: u32) -> Self {
        Self {
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Label {
    pub range: TextRange,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Suggestion {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Fix {
    pub message: String,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Evidence {
    pub label: String,
    pub value: String,
}

/// Construct diagnostics with `Diagnostic::new`, `Diagnostic::error`,
/// `Diagnostic::warning`, `Diagnostic::info`, and the fluent helpers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Diagnostic {
    pub rule_id: String,
    pub severity: Severity,
    pub file: String,
    pub range: TextRange,
    pub message: String,
    pub labels: Vec<Label>,
    pub help: Option<String>,
    pub evidence: Vec<Evidence>,
    pub suggestions: Vec<Suggestion>,
    pub fix: Option<Fix>,
    pub stable_fingerprint: String,
}

#[derive(Deserialize)]
struct DiagnosticWire {
    rule_id: String,
    severity: Severity,
    file: String,
    range: TextRange,
    message: String,
    #[serde(default)]
    labels: Vec<Label>,
    #[serde(default)]
    help: Option<String>,
    #[serde(default)]
    evidence: Vec<Evidence>,
    #[serde(default)]
    suggestions: Vec<Suggestion>,
    #[serde(default)]
    fix: Option<Fix>,
    #[serde(default)]
    stable_fingerprint: String,
}

impl<'de> Deserialize<'de> for Diagnostic {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DiagnosticWire::deserialize(deserializer)?;
        let stable_fingerprint = if wire.stable_fingerprint.is_empty() {
            diagnostic_fingerprint(&wire.rule_id, &wire.file, wire.range, &wire.message)
        } else {
            wire.stable_fingerprint
        };

        Ok(Self {
            rule_id: wire.rule_id,
            severity: wire.severity,
            file: wire.file,
            range: wire.range,
            message: wire.message,
            labels: wire.labels,
            help: wire.help,
            evidence: wire.evidence,
            suggestions: wire.suggestions,
            fix: wire.fix,
            stable_fingerprint,
        })
    }
}

impl Diagnostic {
    pub fn new(
        rule_id: impl Into<String>,
        severity: Severity,
        file: impl Into<String>,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        let rule_id = rule_id.into();
        let file = file.into();
        let message = message.into();
        let stable_fingerprint = diagnostic_fingerprint(&rule_id, &file, range, &message);
        Self {
            rule_id,
            severity,
            file,
            range,
            message,
            labels: Vec::new(),
            help: None,
            evidence: Vec::new(),
            suggestions: Vec::new(),
            fix: None,
            stable_fingerprint,
        }
    }

    pub fn error(
        rule_id: impl Into<String>,
        file: impl Into<String>,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        Self::new(rule_id, Severity::Error, file, range, message)
    }

    pub fn warning(
        rule_id: impl Into<String>,
        file: impl Into<String>,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        Self::new(rule_id, Severity::Warn, file, range, message)
    }

    pub fn info(
        rule_id: impl Into<String>,
        file: impl Into<String>,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        Self::new(rule_id, Severity::Info, file, range, message)
    }

    pub fn with_label(mut self, range: TextRange, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            range,
            message: message.into(),
        });
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_evidence(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.evidence.push(Evidence {
            label: label.into(),
            value: value.into(),
        });
        self
    }

    pub fn with_suggestion(mut self, message: impl Into<String>) -> Self {
        self.suggestions.push(Suggestion {
            message: message.into(),
        });
        self
    }

    pub fn with_fix(mut self, message: impl Into<String>, replacement: Option<String>) -> Self {
        self.fix = Some(Fix {
            message: message.into(),
            replacement,
        });
        self
    }

    pub fn with_fingerprint(mut self, stable_fingerprint: impl Into<String>) -> Self {
        self.stable_fingerprint = stable_fingerprint.into();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
    Sarif,
}

/// Metadata embedded in `--format json` output (`PolintReport.tool`).
#[derive(Debug, Clone, Copy)]
pub struct JsonReportMeta<'a> {
    pub tool_name: &'a str,
    pub tool_version: &'a str,
}

/// When to emit ANSI colors for `--format human` (honors `NO_COLOR` when [`ColorChoice::Auto`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    pub fn use_ansi_colors(self) -> bool {
        match self {
            ColorChoice::Never => false,
            ColorChoice::Always => true,
            ColorChoice::Auto => {
                io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RenderOpts<'a> {
    pub json: JsonReportMeta<'a>,
    pub color: ColorChoice,
    /// Indexed by `Diagnostic.file`-style relative paths for human code snippets.
    pub sources: Option<&'a BTreeMap<String, Arc<str>>>,
}

/// Tool identity in a [`PolintReport`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolintToolInfo {
    pub name: String,
    pub version: String,
}

/// Versioned JSON report for `--format json` and repo-local rule host subprocesses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolintReport {
    pub version: u32,
    pub tool: PolintToolInfo,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Serialize)]
struct PolintReportWire<'a> {
    version: u32,
    schema: &'static str,
    tool: PolintToolWire<'a>,
    diagnostics: &'a [Diagnostic],
}

#[derive(Serialize)]
struct PolintToolWire<'a> {
    name: &'a str,
    version: &'a str,
}

/// Parse stdout from a `polint-local-rules check --format json` process.
pub fn diagnostics_from_json_report(s: &str) -> Result<Vec<Diagnostic>, serde_json::Error> {
    let report: PolintReport = serde_json::from_str(s)?;
    Ok(report.diagnostics)
}

pub(crate) fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        (
            a.file.as_str(),
            a.range.start_line,
            a.range.start_col,
            diagnostic_sort_priority(a),
            a.rule_id.as_str(),
            a.message.as_str(),
            a.stable_fingerprint.as_str(),
        )
            .cmp(&(
                b.file.as_str(),
                b.range.start_line,
                b.range.start_col,
                diagnostic_sort_priority(b),
                b.rule_id.as_str(),
                b.message.as_str(),
                b.stable_fingerprint.as_str(),
            ))
    });
}

fn diagnostic_sort_priority(diagnostic: &Diagnostic) -> u8 {
    if diagnostic.rule_id == "polint/capability" {
        0
    } else {
        1
    }
}

pub(crate) fn dedupe_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut diagnostics = diagnostics;
    sort_diagnostics(&mut diagnostics);
    let mut seen = BTreeSet::new();
    diagnostics
        .into_iter()
        .filter(|diagnostic| seen.insert(diagnostic.stable_fingerprint.clone()))
        .collect()
}

/// Dedupe, optionally filter by `rule_id` pattern (see [`crate::core::rule_id_matches`]),
/// then sort for deterministic ordering.
pub(crate) fn apply_report_filters(
    diagnostics: Vec<Diagnostic>,
    only_rule: Option<&str>,
) -> Vec<Diagnostic> {
    let mut diagnostics = dedupe_diagnostics(diagnostics);
    if let Some(pattern) = only_rule {
        diagnostics.retain(|diagnostic| diagnostic_matches_rule_pattern(pattern, diagnostic));
    }
    sort_diagnostics(&mut diagnostics);
    diagnostics
}

fn diagnostic_matches_rule_pattern(pattern: &str, diagnostic: &Diagnostic) -> bool {
    crate::core::rule_id_matches(pattern, &diagnostic.rule_id)
        || (diagnostic.rule_id == "polint/capability"
            && diagnostic.evidence.iter().any(|evidence| {
                evidence.label == "rule" && crate::core::rule_id_matches(pattern, &evidence.value)
            }))
        || (diagnostic.rule_id.starts_with("polint/")
            && diagnostic.evidence.iter().any(|evidence| {
                evidence.label == "selectors"
                    && evidence.value.split(',').map(str::trim).any(|selector| {
                        !selector.is_empty()
                            && (crate::core::rule_id_matches(pattern, selector)
                                || crate::core::rule_id_matches(selector, pattern))
                    })
            }))
}

/// Truncate diagnostics for rendering only. Exit status must be computed before this cap.
pub(crate) fn limit_report_diagnostics(
    mut diagnostics: Vec<Diagnostic>,
    max_diagnostics: Option<usize>,
) -> Vec<Diagnostic> {
    if let Some(max) = max_diagnostics {
        diagnostics.truncate(max);
    }
    diagnostics
}

#[cfg(test)]
pub(crate) fn render(
    format: OutputFormat,
    diagnostics: &[Diagnostic],
    opts: RenderOpts<'_>,
) -> String {
    render_with_sarif_help(format, diagnostics, opts, None)
}

pub(crate) fn render_with_sarif_help(
    format: OutputFormat,
    diagnostics: &[Diagnostic],
    opts: RenderOpts<'_>,
    sarif_rule_help_uri: Option<&BTreeMap<String, String>>,
) -> String {
    match format {
        OutputFormat::Human => render_human(diagnostics, opts.color, opts.sources),
        OutputFormat::Json => render_json(diagnostics, opts.json),
        OutputFormat::Sarif => render_sarif(diagnostics, sarif_rule_help_uri),
    }
}

fn render_json(diagnostics: &[Diagnostic], json_meta: JsonReportMeta<'_>) -> String {
    let wire = PolintReportWire {
        version: 1,
        schema: POLINT_REPORT_JSON_SCHEMA_V1_URL,
        tool: PolintToolWire {
            name: json_meta.tool_name,
            version: json_meta.tool_version,
        },
        diagnostics,
    };
    serde_json::to_string_pretty(&wire).unwrap_or_else(|_| "{}".to_string())
}

struct HumanPalette {
    reset: &'static str,
    bold: &'static str,
    dim: &'static str,
    red: &'static str,
    yellow: &'static str,
    cyan: &'static str,
    green: &'static str,
    magenta: &'static str,
}

impl HumanPalette {
    fn new(color: bool) -> Self {
        if color {
            Self {
                reset: "\x1b[0m",
                bold: "\x1b[1m",
                dim: "\x1b[2m",
                red: "\x1b[1;31m",
                yellow: "\x1b[1;33m",
                cyan: "\x1b[1;36m",
                green: "\x1b[1;32m",
                magenta: "\x1b[1;35m",
            }
        } else {
            Self {
                reset: "",
                bold: "",
                dim: "",
                red: "",
                yellow: "",
                cyan: "",
                green: "",
                magenta: "",
            }
        }
    }

    fn severity_paint(&self, severity: Severity) -> &'static str {
        match severity {
            Severity::Error => self.red,
            Severity::Warn => self.yellow,
            Severity::Info => self.cyan,
        }
    }
}

fn lookup_source<'a>(sources: &'a BTreeMap<String, Arc<str>>, file: &str) -> Option<&'a str> {
    if let Some(s) = sources.get(file) {
        return Some(s.as_ref());
    }
    let trimmed = file.trim_start_matches("./");
    if let Some(s) = sources.get(trimmed) {
        return Some(s.as_ref());
    }
    let with_dot = format!("./{trimmed}");
    sources.get(&with_dot).map(|a| a.as_ref())
}

/// 1-based column: return byte index at that column (first column is 1).
fn byte_idx_for_one_based_col(line: &str, col_1based: u32) -> usize {
    if col_1based <= 1 {
        return 0;
    }
    let mut c = 1_u32;
    for (idx, _) in line.char_indices() {
        if c == col_1based {
            return idx;
        }
        c = c.saturating_add(1);
    }
    line.len()
}

fn gutter_width(last_line_no: u32) -> usize {
    let d = last_line_no.max(1).ilog10() as usize + 1;
    d.max(3)
}

fn push_underline_row(
    out: &mut String,
    p: &HumanPalette,
    gutter_w: usize,
    start_col_1based: u32,
    line: &str,
    start_byte: usize,
    end_byte: usize,
) {
    out.push(' ');
    out.push_str(p.dim);
    for _ in 0..gutter_w {
        out.push(' ');
    }
    out.push_str(p.reset);
    out.push(' ');
    out.push_str(p.dim);
    out.push('|');
    out.push_str(p.reset);
    out.push(' ');
    out.push_str(p.red);
    let pad = (start_col_1based.saturating_sub(1)) as usize;
    for _ in 0..pad {
        out.push(' ');
    }
    let end_byte = end_byte.max(start_byte);
    let slice = line.get(start_byte..end_byte).unwrap_or("");
    let n = slice.chars().count().max(1);
    for _ in 0..n {
        out.push('^');
    }
    out.push_str(p.reset);
    out.push('\n');
}

fn push_code_snippet(out: &mut String, p: &HumanPalette, source: &str, range: TextRange) {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() || range.start_line < 1 {
        return;
    }
    let start_idx = (range.start_line - 1) as usize;
    if start_idx >= lines.len() {
        return;
    }
    let end_idx = (range.end_line.saturating_sub(1) as usize)
        .min(lines.len() - 1)
        .max(start_idx);
    let w = gutter_width((end_idx + 1) as u32);

    out.push_str("  ");
    out.push_str(p.dim);
    for _ in 0..w {
        out.push(' ');
    }
    out.push('|');
    out.push_str(p.reset);
    out.push('\n');

    for (rel_idx, content) in lines[start_idx..=end_idx].iter().enumerate() {
        let line_no = (start_idx + rel_idx + 1) as u32;
        out.push(' ');
        out.push_str(p.dim);
        out.push_str(&format!("{:>width$}", line_no, width = w));
        out.push_str(p.reset);
        out.push(' ');
        out.push_str(p.dim);
        out.push('|');
        out.push_str(p.reset);
        out.push(' ');
        out.push_str(content);
        out.push('\n');
    }

    if range.start_line == range.end_line {
        let line = lines[start_idx];
        let start_b = byte_idx_for_one_based_col(line, range.start_col);
        let end_b = if range.end_col > range.start_col {
            byte_idx_for_one_based_col(line, range.end_col)
        } else {
            (start_b
                + line[start_b..]
                    .chars()
                    .next()
                    .map(|ch| ch.len_utf8())
                    .unwrap_or(1))
            .min(line.len())
        };
        push_underline_row(out, p, w, range.start_col, line, start_b, end_b);
    } else {
        let first = lines[start_idx];
        let start_b = byte_idx_for_one_based_col(first, range.start_col);
        push_underline_row(out, p, w, range.start_col, first, start_b, first.len());
        if end_idx > start_idx {
            let last = lines[end_idx];
            let end_b = byte_idx_for_one_based_col(last, range.end_col.max(1));
            push_underline_row(out, p, w, 1, last, 0, end_b);
        }
    }
}

/// Header line for human output: counts per `rule_id`, grouped by severity (errors, then warnings, then infos). Rule ids sorted lexicographically within each group.
fn format_human_summary_line(diagnostics: &[Diagnostic], p: &HumanPalette) -> String {
    let mut errors: BTreeMap<String, u32> = BTreeMap::new();
    let mut warns: BTreeMap<String, u32> = BTreeMap::new();
    let mut infos: BTreeMap<String, u32> = BTreeMap::new();
    for d in diagnostics {
        match d.severity {
            Severity::Error => *errors.entry(d.rule_id.clone()).or_insert(0) += 1,
            Severity::Warn => *warns.entry(d.rule_id.clone()).or_insert(0) += 1,
            Severity::Info => *infos.entry(d.rule_id.clone()).or_insert(0) += 1,
        }
    }

    fn push_group(
        parts: &mut Vec<String>,
        p: &HumanPalette,
        sev: Severity,
        count: u32,
        rules: &BTreeMap<String, u32>,
    ) {
        if rules.is_empty() {
            return;
        }
        let label = if count == 1 {
            match sev {
                Severity::Error => "error",
                Severity::Warn => "warning",
                Severity::Info => "info",
            }
        } else {
            match sev {
                Severity::Error => "errors",
                Severity::Warn => "warnings",
                Severity::Info => "infos",
            }
        };
        let breakdown: Vec<String> = rules.iter().map(|(id, n)| format!("{id} ({n})")).collect();
        parts.push(format!(
            "{}{} {} — {}{}",
            p.severity_paint(sev),
            count,
            label,
            breakdown.join(", "),
            p.reset
        ));
    }

    let err_n: u32 = errors.values().sum();
    let warn_n: u32 = warns.values().sum();
    let info_n: u32 = infos.values().sum();

    let mut parts = Vec::new();
    push_group(&mut parts, p, Severity::Error, err_n, &errors);
    push_group(&mut parts, p, Severity::Warn, warn_n, &warns);
    push_group(&mut parts, p, Severity::Info, info_n, &infos);

    let mut out = String::new();
    out.push_str(&format!("{}Summary:{} ", p.bold, p.reset));
    out.push_str(&parts.join("; "));
    out.push('\n');
    out
}

pub(crate) fn render_human(
    diagnostics: &[Diagnostic],
    color: ColorChoice,
    sources: Option<&BTreeMap<String, Arc<str>>>,
) -> String {
    let p = HumanPalette::new(color.use_ansi_colors());
    if diagnostics.is_empty() {
        return format!("{}No diagnostics.{}\n", p.dim, p.reset);
    }

    let mut out = String::new();
    out.push_str(&format_human_summary_line(diagnostics, &p));
    out.push('\n');
    for diagnostic in diagnostics {
        let sev_word = match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warn => "warn",
            Severity::Info => "info",
        };
        out.push_str(&format!(
            "{}{}{}{}[{}{}{}]: {}{}\n  {}{}{}{} {}{}{}:{}\n",
            p.severity_paint(diagnostic.severity),
            sev_word,
            p.reset,
            p.bold,
            p.magenta,
            diagnostic.rule_id,
            p.reset,
            p.bold,
            diagnostic.message,
            p.reset,
            p.cyan,
            "-->",
            p.reset,
            p.bold,
            diagnostic.file,
            p.reset,
            format_range(diagnostic.range),
        ));
        if let Some(map) = sources
            && let Some(src) = lookup_source(map, &diagnostic.file)
        {
            push_code_snippet(&mut out, &p, src, diagnostic.range);
        }
        for label in &diagnostic.labels {
            out.push_str(&format!(
                "  {}label{} {}: {}\n",
                p.dim,
                p.reset,
                format_range(label.range),
                label.message
            ));
        }
        if !diagnostic.evidence.is_empty() {
            for evidence in &diagnostic.evidence {
                out.push_str(&format!(
                    "  {}evidence{} {}: {}\n",
                    p.dim, p.reset, evidence.label, evidence.value
                ));
            }
        }
        for suggestion in &diagnostic.suggestions {
            out.push_str(&format!(
                "  {}suggestion:{} {}\n",
                p.dim, p.reset, suggestion.message
            ));
        }
        if let Some(fix) = &diagnostic.fix {
            out.push_str(&format!("  {}fix:{} {}\n", p.yellow, p.reset, fix.message));
            if let Some(replacement) = &fix.replacement {
                out.push_str(&format!(
                    "    {}replacement:{} {}\n",
                    p.green, p.reset, replacement
                ));
            }
        }
        if let Some(help) = &diagnostic.help {
            out.push_str(&format!("  {}help:{} {}\n", p.cyan, p.reset, help));
        }
        out.push_str(&format!(
            "  {}fingerprint: {}{}\n\n",
            p.dim, diagnostic.stable_fingerprint, p.reset
        ));
    }
    out
}

fn format_range(range: TextRange) -> String {
    format!(
        "{}:{}-{}:{}",
        range.start_line, range.start_col, range.end_line, range.end_col
    )
}

pub(crate) fn render_sarif(
    diagnostics: &[Diagnostic],
    rule_help_uri: Option<&BTreeMap<String, String>>,
) -> String {
    #[derive(Serialize)]
    struct SarifLog {
        version: &'static str,
        #[serde(rename = "$schema")]
        schema: &'static str,
        runs: Vec<SarifRun>,
    }

    #[derive(Serialize)]
    struct SarifRun {
        tool: SarifTool,
        results: Vec<SarifResult>,
    }

    #[derive(Serialize)]
    struct SarifTool {
        driver: SarifDriver,
    }

    #[derive(Serialize)]
    struct SarifDriver {
        name: &'static str,
        #[serde(rename = "informationUri")]
        information_uri: &'static str,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        rules: Vec<SarifReportingRule>,
    }

    #[derive(Serialize)]
    struct SarifReportingRule {
        id: String,
        #[serde(rename = "shortDescription")]
        short_description: SarifMessage,
        #[serde(rename = "fullDescription", skip_serializing_if = "Option::is_none")]
        full_description: Option<SarifMessage>,
        #[serde(rename = "helpUri", skip_serializing_if = "Option::is_none")]
        help_uri: Option<String>,
        properties: SarifProperties,
    }

    #[derive(Serialize)]
    struct SarifProperties {
        tags: Vec<String>,
    }

    #[derive(Serialize)]
    struct SarifResult {
        #[serde(rename = "ruleId")]
        rule_id: String,
        level: &'static str,
        message: SarifMessage,
        fingerprints: SarifFingerprints,
        locations: Vec<SarifLocation>,
        #[serde(rename = "relatedLocations", skip_serializing_if = "Vec::is_empty")]
        related_locations: Vec<SarifRelatedLocation>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fixes: Option<Vec<SarifFix>>,
    }

    #[derive(Serialize)]
    struct SarifFingerprints {
        #[serde(rename = "polint/v1")]
        polint_v1: String,
    }

    #[derive(Serialize)]
    struct SarifMessage {
        text: String,
    }

    #[derive(Serialize)]
    struct SarifLocation {
        #[serde(rename = "physicalLocation")]
        physical_location: SarifPhysicalLocation,
    }

    #[derive(Serialize)]
    struct SarifRelatedLocation {
        #[serde(rename = "physicalLocation")]
        physical_location: SarifPhysicalLocation,
        message: SarifMessage,
    }

    #[derive(Serialize)]
    struct SarifPhysicalLocation {
        #[serde(rename = "artifactLocation")]
        artifact_location: SarifArtifactLocation,
        region: SarifRegion,
    }

    #[derive(Serialize)]
    struct SarifArtifactLocation {
        uri: String,
    }

    #[derive(Serialize)]
    struct SarifRegion {
        #[serde(rename = "startLine")]
        start_line: u32,
        #[serde(rename = "startColumn")]
        start_column: u32,
        #[serde(rename = "endLine")]
        end_line: u32,
        #[serde(rename = "endColumn")]
        end_column: u32,
    }

    #[derive(Serialize)]
    struct SarifFix {
        description: SarifMessage,
        #[serde(rename = "artifactChanges")]
        artifact_changes: Vec<SarifArtifactChange>,
    }

    #[derive(Serialize)]
    struct SarifArtifactChange {
        #[serde(rename = "artifactLocation")]
        artifact_location: SarifArtifactLocation,
        replacements: Vec<SarifReplacement>,
    }

    #[derive(Serialize)]
    struct SarifReplacement {
        #[serde(rename = "deletedRegion")]
        deleted_region: SarifRegion,
        #[serde(rename = "insertedContent")]
        inserted_content: SarifArtifactContent,
    }

    #[derive(Serialize)]
    struct SarifArtifactContent {
        text: String,
    }

    fn region_from(range: TextRange) -> SarifRegion {
        SarifRegion {
            start_line: range.start_line,
            start_column: range.start_col,
            end_line: range.end_line,
            end_column: range.end_col,
        }
    }

    fn physical(uri: &str, range: TextRange) -> SarifPhysicalLocation {
        SarifPhysicalLocation {
            artifact_location: SarifArtifactLocation {
                uri: uri.to_string(),
            },
            region: region_from(range),
        }
    }

    let mut rule_descriptions: BTreeMap<String, String> = BTreeMap::new();
    for diagnostic in diagnostics {
        rule_descriptions
            .entry(diagnostic.rule_id.clone())
            .or_insert_with(|| diagnostic.message.clone());
    }
    let rules: Vec<SarifReportingRule> = rule_descriptions
        .into_iter()
        .map(|(id, text)| {
            let help_uri = rule_help_uri.and_then(|m| m.get(&id).cloned());
            SarifReportingRule {
                id,
                short_description: SarifMessage { text: text.clone() },
                full_description: Some(SarifMessage { text }),
                help_uri,
                properties: SarifProperties {
                    tags: vec!["polint".to_string()],
                },
            }
        })
        .collect();

    let results: Vec<SarifResult> = diagnostics
        .iter()
        .map(|diagnostic| {
            let uri = diagnostic.file.as_str();
            let related_locations: Vec<SarifRelatedLocation> = diagnostic
                .labels
                .iter()
                .map(|label| SarifRelatedLocation {
                    physical_location: physical(uri, label.range),
                    message: SarifMessage {
                        text: label.message.clone(),
                    },
                })
                .collect();

            let fixes = diagnostic.fix.as_ref().and_then(|fix| {
                fix.replacement.as_ref().map(|replacement| {
                    vec![SarifFix {
                        description: SarifMessage {
                            text: fix.message.clone(),
                        },
                        artifact_changes: vec![SarifArtifactChange {
                            artifact_location: SarifArtifactLocation {
                                uri: uri.to_string(),
                            },
                            replacements: vec![SarifReplacement {
                                deleted_region: region_from(diagnostic.range),
                                inserted_content: SarifArtifactContent {
                                    text: replacement.clone(),
                                },
                            }],
                        }],
                    }]
                })
            });

            SarifResult {
                rule_id: diagnostic.rule_id.clone(),
                level: match diagnostic.severity {
                    Severity::Info => "note",
                    Severity::Warn => "warning",
                    Severity::Error => "error",
                },
                message: SarifMessage {
                    text: diagnostic.message.clone(),
                },
                fingerprints: SarifFingerprints {
                    polint_v1: diagnostic.stable_fingerprint.clone(),
                },
                locations: vec![SarifLocation {
                    physical_location: physical(uri, diagnostic.range),
                }],
                related_locations,
                fixes,
            }
        })
        .collect();

    let log = SarifLog {
        version: "2.1.0",
        schema: "https://json.schemastore.org/sarif-2.1.0.json",
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "polint",
                    information_uri: "https://github.com/emilwareus/polint",
                    rules,
                },
            },
            results,
        }],
    };

    serde_json::to_string_pretty(&log).unwrap_or_else(|_| "{}".to_string())
}

pub(crate) fn fingerprint(parts: &[&str]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub(crate) fn diagnostic_fingerprint(
    rule_id: &str,
    file: &str,
    range: TextRange,
    message: &str,
) -> String {
    let start_line = range.start_line.to_string();
    let start_col = range.start_col.to_string();
    let end_line = range.end_line.to_string();
    let end_col = range.end_col.to_string();
    fingerprint(&[
        rule_id,
        file,
        &start_line,
        &start_col,
        &end_line,
        &end_col,
        message,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn test_opts() -> RenderOpts<'static> {
        RenderOpts {
            json: JsonReportMeta {
                tool_name: "polint",
                tool_version: env!("CARGO_PKG_VERSION"),
            },
            color: ColorChoice::Never,
            sources: None,
        }
    }

    #[test]
    fn sorting_is_deterministic() {
        let mut diagnostics = vec![
            Diagnostic::warning("b", "b.go", TextRange::point(2, 1), "b"),
            Diagnostic::warning("a", "a.go", TextRange::point(1, 1), "a"),
        ];

        sort_diagnostics(&mut diagnostics);

        assert_eq!(diagnostics[0].file, "a.go");
        assert_eq!(diagnostics[1].file, "b.go");
    }

    fn contract_diagnostic() -> Diagnostic {
        Diagnostic::error(
            "project/rule",
            "src/lib.rs",
            TextRange {
                start_line: 10,
                start_col: 4,
                end_line: 10,
                end_col: 12,
            },
            "policy failed",
        )
        .with_label(
            TextRange {
                start_line: 11,
                start_col: 2,
                end_line: 11,
                end_col: 8,
            },
            "related expression",
        )
        .with_evidence("symbol", "unsafe_api")
        .with_suggestion("Prefer safe_api here")
        .with_fix("Replace unsafe_api", Some("safe_api()".to_string()))
        .with_help("Use the safe wrapper before crossing this boundary.")
        .with_fingerprint("fingerprint-123")
    }

    #[test]
    fn render_human_summary_line_groups_counts_by_severity_and_rule_id() {
        let d1 = Diagnostic::error("local/a", "a.go", TextRange::point(1, 1), "m");
        let d2 = Diagnostic::error("local/a", "b.go", TextRange::point(1, 1), "m");
        let d3 = Diagnostic::error("parser/go", "c.go", TextRange::point(1, 1), "m");
        let d4 = Diagnostic::warning("local/b", "d.go", TextRange::point(1, 1), "m");
        let d5 = Diagnostic::info("local/c", "e.go", TextRange::point(1, 1), "m");
        let d6 = Diagnostic::info("local/c", "f.go", TextRange::point(1, 1), "m");
        let rendered = render_human(&[d1, d2, d3, d4, d5, d6], ColorChoice::Never, None);
        assert!(
            rendered.starts_with("Summary:"),
            "unexpected prefix: {rendered:?}"
        );
        assert!(rendered.contains("3 errors"));
        assert!(rendered.contains("local/a (2)"));
        assert!(rendered.contains("parser/go (1)"));
        assert!(rendered.contains("1 warning"));
        assert!(rendered.contains("local/b (1)"));
        assert!(rendered.contains("2 infos"));
        assert!(rendered.contains("local/c (2)"));
    }

    #[test]
    fn only_rule_keeps_polint_ignore_health_diagnostics_for_matching_selectors() {
        let diagnostics = vec![
            Diagnostic::warning(
                "polint/unused-ignore",
                "src/a.ts",
                TextRange::point(1, 1),
                "unused",
            )
            .with_evidence("selectors", "local/*"),
            Diagnostic::warning(
                "polint/unused-ignore",
                "src/b.ts",
                TextRange::point(1, 1),
                "unused",
            )
            .with_evidence("selectors", "other/rule"),
            Diagnostic::warning(
                "local/no-todo",
                "src/c.ts",
                TextRange::point(1, 1),
                "finding",
            ),
        ];

        let filtered = apply_report_filters(diagnostics, Some("local/no-todo"));
        let rule_ids = filtered
            .iter()
            .map(|diagnostic| diagnostic.rule_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(rule_ids, ["polint/unused-ignore", "local/no-todo"]);
    }

    #[test]
    fn render_human_always_color_includes_escape_sequences() {
        let diagnostic = Diagnostic::error("r", "f.go", TextRange::point(1, 1), "oops");
        let rendered = render_human(&[diagnostic], ColorChoice::Always, None);
        assert!(
            rendered.contains("\x1b["),
            "expected ANSI SGR sequences: {rendered:?}"
        );
    }

    #[test]
    fn render_human_snapshot_includes_contract_fields() {
        insta::assert_snapshot!(
            render(
                OutputFormat::Human,
                &[contract_diagnostic()],
                test_opts(),
            ),
            @r###"
        Summary: 1 error — project/rule (1)

        error[project/rule]: policy failed
          --> src/lib.rs:10:4-10:12
          label 11:2-11:8: related expression
          evidence symbol: unsafe_api
          suggestion: Prefer safe_api here
          fix: Replace unsafe_api
            replacement: safe_api()
          help: Use the safe wrapper before crossing this boundary.
          fingerprint: fingerprint-123

        "###,
        );
    }

    #[test]
    fn render_human_includes_snippet_when_sources_map_provided() {
        let source_text: String = (1..=12)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut sources = BTreeMap::new();
        sources.insert(
            "src/lib.rs".to_string(),
            Arc::from(source_text.into_boxed_str()),
        );
        let opts = RenderOpts {
            json: JsonReportMeta {
                tool_name: "polint",
                tool_version: env!("CARGO_PKG_VERSION"),
            },
            color: ColorChoice::Never,
            sources: Some(&sources),
        };
        insta::assert_snapshot!(
            render(OutputFormat::Human, &[contract_diagnostic()], opts),
            @r###"
        Summary: 1 error — project/rule (1)

        error[project/rule]: policy failed
          --> src/lib.rs:10:4-10:12
             |
          10 | line 10
             |    ^^^^
          label 11:2-11:8: related expression
          evidence symbol: unsafe_api
          suggestion: Prefer safe_api here
          fix: Replace unsafe_api
            replacement: safe_api()
          help: Use the safe wrapper before crossing this boundary.
          fingerprint: fingerprint-123

        "###,
        );
    }

    #[test]
    fn render_json_snapshot_is_stable() {
        let rendered = render(OutputFormat::Json, &[contract_diagnostic()], test_opts());
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed["version"], 1);
        assert_eq!(
            parsed["schema"].as_str().unwrap(),
            POLINT_REPORT_JSON_SCHEMA_V1_URL
        );
        assert_eq!(parsed["tool"]["name"], "polint");
        assert_eq!(parsed["tool"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 1);

        let normalized = rendered.replace(env!("CARGO_PKG_VERSION"), "<PKG_VERSION>");
        insta::assert_snapshot!(normalized, @r###"
        {
          "version": 1,
          "schema": "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-report-v1.json",
          "tool": {
            "name": "polint",
            "version": "<PKG_VERSION>"
          },
          "diagnostics": [
            {
              "rule_id": "project/rule",
              "severity": "error",
              "file": "src/lib.rs",
              "range": {
                "start_line": 10,
                "start_col": 4,
                "end_line": 10,
                "end_col": 12
              },
              "message": "policy failed",
              "labels": [
                {
                  "range": {
                    "start_line": 11,
                    "start_col": 2,
                    "end_line": 11,
                    "end_col": 8
                  },
                  "message": "related expression"
                }
              ],
              "help": "Use the safe wrapper before crossing this boundary.",
              "evidence": [
                {
                  "label": "symbol",
                  "value": "unsafe_api"
                }
              ],
              "suggestions": [
                {
                  "message": "Prefer safe_api here"
                }
              ],
              "fix": {
                "message": "Replace unsafe_api",
                "replacement": "safe_api()"
              },
              "stable_fingerprint": "fingerprint-123"
            }
          ]
        }
        "###);
    }

    #[test]
    fn diagnostics_from_json_report_round_trips() {
        let input = vec![contract_diagnostic()];
        let json = render(OutputFormat::Json, &input, test_opts());
        let output = diagnostics_from_json_report(&json).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn polint_report_json_schema_file_tracks_repo() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/schemas/polint-report-v1.json");
        let raw = std::fs::read_to_string(&path).expect("polint-report schema should exist");
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            value["$id"].as_str().unwrap(),
            POLINT_REPORT_JSON_SCHEMA_V1_URL
        );
    }

    #[test]
    fn render_sarif_includes_rule_help_uri_from_map() {
        let mut help = BTreeMap::new();
        help.insert(
            "project/rule".to_string(),
            "https://example.invalid/rules/project-rule".to_string(),
        );
        let rendered = render_with_sarif_help(
            OutputFormat::Sarif,
            &[contract_diagnostic()],
            RenderOpts {
                json: JsonReportMeta {
                    tool_name: "polint",
                    tool_version: env!("CARGO_PKG_VERSION"),
                },
                color: ColorChoice::Never,
                sources: None,
            },
            Some(&help),
        );
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(
            parsed
                .pointer("/runs/0/tool/driver/rules/0/helpUri")
                .unwrap(),
            "https://example.invalid/rules/project-rule"
        );
        assert_eq!(
            parsed
                .pointer("/runs/0/tool/driver/rules/0/properties/tags/0")
                .unwrap(),
            "polint"
        );
        assert!(parsed.pointer("/runs/0/tool/driver/rules/0/tags").is_none());
    }

    #[test]
    fn render_sarif_snapshot_includes_ci_fields() {
        let rendered = render(OutputFormat::Sarif, &[contract_diagnostic()], test_opts());
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(parsed.pointer("/version").unwrap(), "2.1.0");
        assert_eq!(
            parsed.pointer("/runs/0/tool/driver/name").unwrap(),
            "polint"
        );
        assert_eq!(
            parsed.pointer("/runs/0/tool/driver/rules/0/id").unwrap(),
            "project/rule"
        );
        assert_eq!(
            parsed.pointer("/runs/0/results/0/ruleId").unwrap(),
            "project/rule"
        );
        assert_eq!(parsed.pointer("/runs/0/results/0/level").unwrap(), "error");
        assert_eq!(
            parsed.pointer("/runs/0/results/0/message/text").unwrap(),
            "policy failed"
        );
        assert_eq!(
            parsed
                .pointer("/runs/0/results/0/fingerprints/polint~1v1")
                .unwrap(),
            "fingerprint-123"
        );
        assert_eq!(
            parsed
                .pointer("/runs/0/results/0/locations/0/physicalLocation/artifactLocation/uri")
                .unwrap(),
            "src/lib.rs"
        );
        assert_eq!(
            parsed
                .pointer("/runs/0/results/0/relatedLocations/0/physicalLocation/region/startLine",)
                .and_then(|v| v.as_u64()),
            Some(11)
        );
        assert!(parsed
            .pointer("/runs/0/results/0/fixes/0/artifactChanges/0/replacements/0/insertedContent/text")
            .is_some());

        insta::assert_snapshot!(rendered, @r###"
        {
          "version": "2.1.0",
          "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
          "runs": [
            {
              "tool": {
                "driver": {
                  "name": "polint",
                  "informationUri": "https://github.com/emilwareus/polint",
                  "rules": [
                    {
                      "id": "project/rule",
                      "shortDescription": {
                        "text": "policy failed"
                      },
                      "fullDescription": {
                        "text": "policy failed"
                      },
                      "properties": {
                        "tags": [
                          "polint"
                        ]
                      }
                    }
                  ]
                }
              },
              "results": [
                {
                  "ruleId": "project/rule",
                  "level": "error",
                  "message": {
                    "text": "policy failed"
                  },
                  "fingerprints": {
                    "polint/v1": "fingerprint-123"
                  },
                  "locations": [
                    {
                      "physicalLocation": {
                        "artifactLocation": {
                          "uri": "src/lib.rs"
                        },
                        "region": {
                          "startLine": 10,
                          "startColumn": 4,
                          "endLine": 10,
                          "endColumn": 12
                        }
                      }
                    }
                  ],
                  "relatedLocations": [
                    {
                      "physicalLocation": {
                        "artifactLocation": {
                          "uri": "src/lib.rs"
                        },
                        "region": {
                          "startLine": 11,
                          "startColumn": 2,
                          "endLine": 11,
                          "endColumn": 8
                        }
                      },
                      "message": {
                        "text": "related expression"
                      }
                    }
                  ],
                  "fixes": [
                    {
                      "description": {
                        "text": "Replace unsafe_api"
                      },
                      "artifactChanges": [
                        {
                          "artifactLocation": {
                            "uri": "src/lib.rs"
                          },
                          "replacements": [
                            {
                              "deletedRegion": {
                                "startLine": 10,
                                "startColumn": 4,
                                "endLine": 10,
                                "endColumn": 12
                              },
                              "insertedContent": {
                                "text": "safe_api()"
                              }
                            }
                          ]
                        }
                      ]
                    }
                  ]
                }
              ]
            }
          ]
        }
        "###);
    }

    #[test]
    fn diagnostic_deserializes_missing_phase3_fields_with_computed_fingerprint() {
        let diagnostic: Diagnostic = serde_json::from_str(
            r#"{
                "rule_id": "project/rule",
                "severity": "warn",
                "file": "src/lib.rs",
                "range": {
                    "start_line": 4,
                    "start_col": 2,
                    "end_line": 4,
                    "end_col": 9
                },
                "message": "policy failed"
            }"#,
        )
        .unwrap();

        let range = TextRange {
            start_line: 4,
            start_col: 2,
            end_line: 4,
            end_col: 9,
        };

        assert_eq!(diagnostic.rule_id, "project/rule");
        assert_eq!(diagnostic.severity, Severity::Warn);
        assert_eq!(diagnostic.file, "src/lib.rs");
        assert_eq!(diagnostic.range, range);
        assert_eq!(diagnostic.message, "policy failed");
        assert!(diagnostic.labels.is_empty());
        assert!(diagnostic.help.is_none());
        assert!(diagnostic.evidence.is_empty());
        assert!(diagnostic.suggestions.is_empty());
        assert!(diagnostic.fix.is_none());
        assert_eq!(
            diagnostic.stable_fingerprint,
            diagnostic_fingerprint("project/rule", "src/lib.rs", range, "policy failed")
        );
    }

    #[test]
    fn render_empty_human_output_is_stable() {
        assert_eq!(
            render(OutputFormat::Human, &[], test_opts()),
            "No diagnostics.\n"
        );
    }

    #[test]
    fn render_empty_json_report_is_stable() {
        let rendered = render(OutputFormat::Json, &[], test_opts());
        let parsed: PolintReport = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.tool.name, "polint");
        assert!(parsed.diagnostics.is_empty());
    }

    #[test]
    fn diagnostic_builders_cover_labels_suggestions_fixes_evidence_and_help() {
        let range = TextRange {
            start_line: 10,
            start_col: 4,
            end_line: 10,
            end_col: 12,
        };
        let label_range = TextRange {
            start_line: 11,
            start_col: 2,
            end_line: 11,
            end_col: 8,
        };

        let diagnostic = Diagnostic::error("project/rule", "src/lib.rs", range, "policy failed")
            .with_label(label_range, "related expression")
            .with_evidence("symbol", "unsafe_api")
            .with_suggestion("Prefer safe_api here")
            .with_fix("Replace unsafe_api", Some("safe_api".to_string()))
            .with_help("Use the safe wrapper before crossing this boundary.");

        assert_eq!(diagnostic.rule_id, "project/rule");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.file, "src/lib.rs");
        assert_eq!(diagnostic.range, range);
        assert_eq!(diagnostic.message, "policy failed");
        assert_eq!(
            diagnostic.labels,
            vec![Label {
                range: label_range,
                message: "related expression".to_string()
            }]
        );
        assert_eq!(
            diagnostic.evidence,
            vec![Evidence {
                label: "symbol".to_string(),
                value: "unsafe_api".to_string()
            }]
        );
        assert_eq!(
            diagnostic.suggestions,
            vec![Suggestion {
                message: "Prefer safe_api here".to_string()
            }]
        );
        assert_eq!(
            diagnostic.fix,
            Some(Fix {
                message: "Replace unsafe_api".to_string(),
                replacement: Some("safe_api".to_string())
            })
        );
        assert_eq!(
            diagnostic.help,
            Some("Use the safe wrapper before crossing this boundary.".to_string())
        );
        assert!(!diagnostic.stable_fingerprint.is_empty());
    }

    #[test]
    fn fingerprint_includes_rule_file_full_range_and_message() {
        let range = TextRange {
            start_line: 4,
            start_col: 2,
            end_line: 4,
            end_col: 9,
        };
        let baseline = Diagnostic::warning("project/rule", "src/lib.rs", range, "policy failed")
            .stable_fingerprint;

        let changed_start_line = Diagnostic::warning(
            "project/rule",
            "src/lib.rs",
            TextRange {
                start_line: 5,
                ..range
            },
            "policy failed",
        )
        .stable_fingerprint;
        let changed_start_col = Diagnostic::warning(
            "project/rule",
            "src/lib.rs",
            TextRange {
                start_col: 3,
                ..range
            },
            "policy failed",
        )
        .stable_fingerprint;
        let changed_end_line = Diagnostic::warning(
            "project/rule",
            "src/lib.rs",
            TextRange {
                end_line: 5,
                ..range
            },
            "policy failed",
        )
        .stable_fingerprint;
        let changed_end_col = Diagnostic::warning(
            "project/rule",
            "src/lib.rs",
            TextRange {
                end_col: 10,
                ..range
            },
            "policy failed",
        )
        .stable_fingerprint;
        let changed_file =
            Diagnostic::warning("project/rule", "src/main.rs", range, "policy failed")
                .stable_fingerprint;
        let changed_rule =
            Diagnostic::warning("project/other", "src/lib.rs", range, "policy failed")
                .stable_fingerprint;
        let changed_message =
            Diagnostic::warning("project/rule", "src/lib.rs", range, "different message")
                .stable_fingerprint;

        for changed in [
            changed_start_line,
            changed_start_col,
            changed_end_line,
            changed_end_col,
            changed_file,
            changed_rule,
            changed_message,
        ] {
            assert_ne!(baseline, changed);
        }

        let changed_non_identity_fields =
            Diagnostic::error("project/rule", "src/lib.rs", range, "policy failed")
                .with_label(range, "related")
                .with_evidence("name", "value")
                .with_suggestion("try another expression")
                .with_fix("replace it", Some("replacement".to_string()))
                .with_help("extra context");

        assert_eq!(baseline, changed_non_identity_fields.stable_fingerprint);
    }

    #[test]
    fn dedupe_diagnostics_collapses_same_fingerprint_after_sorting() {
        let duplicate = Diagnostic::warning("project/rule", "b.go", TextRange::point(2, 1), "b");
        let unique = Diagnostic::warning("project/rule", "a.go", TextRange::point(1, 1), "a");

        let diagnostics = dedupe_diagnostics(vec![duplicate.clone(), unique.clone(), duplicate]);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].stable_fingerprint, unique.stable_fingerprint);
        assert_eq!(diagnostics[1].file, "b.go");
    }

    #[test]
    fn dedupe_diagnostics_removes_non_adjacent_duplicate_fingerprints() {
        let first =
            Diagnostic::warning("rule/a", "a.go", TextRange::point(1, 1), "first duplicate")
                .with_fingerprint("same-fingerprint");
        let unique = Diagnostic::warning("rule/b", "b.go", TextRange::point(1, 1), "unique")
            .with_fingerprint("unique-fingerprint");
        let second =
            Diagnostic::warning("rule/c", "c.go", TextRange::point(1, 1), "second duplicate")
                .with_fingerprint("same-fingerprint");

        let diagnostics = dedupe_diagnostics(vec![second, unique.clone(), first.clone()]);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0], first);
        assert_eq!(diagnostics[1], unique);
    }

    fn diagnostic_from_index(index: usize) -> Diagnostic {
        let file = format!("src/{:02}.rs", index % 5);
        let rule_id = format!("rule/{:02}", index % 7);
        let line = (index % 11) as u32 + 1;
        let col = (index % 13) as u32 + 1;
        Diagnostic::warning(
            rule_id,
            file,
            TextRange {
                start_line: line,
                start_col: col,
                end_line: line + ((index % 3) as u32),
                end_col: col + ((index % 5) as u32),
            },
            format!("message {index:02}"),
        )
    }

    fn sorted_keys(
        diagnostics: &mut [Diagnostic],
    ) -> Vec<(String, u32, u32, String, String, String)> {
        sort_diagnostics(diagnostics);
        diagnostics
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.file.clone(),
                    diagnostic.range.start_line,
                    diagnostic.range.start_col,
                    diagnostic.rule_id.clone(),
                    diagnostic.message.clone(),
                    diagnostic.stable_fingerprint.clone(),
                )
            })
            .collect()
    }

    proptest! {
        #[test]
        fn sort_diagnostics_is_input_order_independent(order in proptest::collection::vec(0usize..30, 1..80)) {
            let mut first: Vec<_> = order.iter().copied().map(diagnostic_from_index).collect();
            let mut second: Vec<_> = order.iter().rev().copied().map(diagnostic_from_index).collect();

            prop_assert_eq!(sorted_keys(&mut first), sorted_keys(&mut second));
        }
    }
}
