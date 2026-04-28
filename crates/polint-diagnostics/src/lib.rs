use serde::{Deserialize, Serialize};
use std::fmt;

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
pub struct Label {
    pub range: TextRange,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fix {
    pub message: String,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        let stable_fingerprint =
            fingerprint(&[&rule_id, &file, &message, &range.start_line.to_string()]);
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

pub fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        (
            a.file.as_str(),
            a.range.start_line,
            a.range.start_col,
            a.rule_id.as_str(),
            a.message.as_str(),
            a.stable_fingerprint.as_str(),
        )
            .cmp(&(
                b.file.as_str(),
                b.range.start_line,
                b.range.start_col,
                b.rule_id.as_str(),
                b.message.as_str(),
                b.stable_fingerprint.as_str(),
            ))
    });
}

pub fn dedupe_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut diagnostics = diagnostics;
    sort_diagnostics(&mut diagnostics);
    diagnostics.dedup_by(|a, b| a.stable_fingerprint == b.stable_fingerprint);
    diagnostics
}

pub fn render(format: OutputFormat, diagnostics: &[Diagnostic]) -> String {
    match format {
        OutputFormat::Human => render_human(diagnostics),
        OutputFormat::Json => {
            serde_json::to_string_pretty(diagnostics).unwrap_or_else(|_| "[]".to_string())
        }
        OutputFormat::Sarif => render_sarif(diagnostics),
    }
}

pub fn render_human(diagnostics: &[Diagnostic]) -> String {
    if diagnostics.is_empty() {
        return "No diagnostics.\n".to_string();
    }

    let mut out = String::new();
    for diagnostic in diagnostics {
        out.push_str(&format!(
            "{}:{}:{} {} {}\n\n{}\n",
            diagnostic.file,
            diagnostic.range.start_line,
            diagnostic.range.start_col,
            diagnostic.severity,
            diagnostic.rule_id,
            diagnostic.message
        ));
        if !diagnostic.evidence.is_empty() {
            out.push_str("\nEvidence:\n");
            for evidence in &diagnostic.evidence {
                out.push_str(&format!("  {}: {}\n", evidence.label, evidence.value));
            }
        }
        if let Some(help) = &diagnostic.help {
            out.push_str("\nSuggested action:\n  ");
            out.push_str(help);
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

pub fn render_sarif(diagnostics: &[Diagnostic]) -> String {
    let results: Vec<serde_json::Value> = diagnostics
        .iter()
        .map(|diagnostic| {
            serde_json::json!({
                "ruleId": diagnostic.rule_id,
                "level": match diagnostic.severity {
                    Severity::Info => "note",
                    Severity::Warn => "warning",
                    Severity::Error => "error",
                },
                "message": { "text": diagnostic.message },
                "fingerprints": { "polint": diagnostic.stable_fingerprint },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": diagnostic.file },
                        "region": {
                            "startLine": diagnostic.range.start_line,
                            "startColumn": diagnostic.range.start_col,
                            "endLine": diagnostic.range.end_line,
                            "endColumn": diagnostic.range.end_col
                        }
                    }
                }]
            })
        })
        .collect();

    serde_json::to_string_pretty(&serde_json::json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "polint",
                    "informationUri": "https://github.com/emilwareus/exlint"
                }
            },
            "results": results
        }]
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

pub fn fingerprint(parts: &[&str]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
