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
            "{}:{} {} {}\n  {}\n",
            diagnostic.file,
            format_range(diagnostic.range),
            diagnostic.severity,
            diagnostic.rule_id,
            diagnostic.message
        ));
        for label in &diagnostic.labels {
            out.push_str(&format!(
                "  label {}: {}\n",
                format_range(label.range),
                label.message
            ));
        }
        if !diagnostic.evidence.is_empty() {
            for evidence in &diagnostic.evidence {
                out.push_str(&format!(
                    "  evidence {}: {}\n",
                    evidence.label, evidence.value
                ));
            }
        }
        for suggestion in &diagnostic.suggestions {
            out.push_str(&format!("  suggestion: {}\n", suggestion.message));
        }
        if let Some(fix) = &diagnostic.fix {
            out.push_str(&format!("  fix: {}\n", fix.message));
            if let Some(replacement) = &fix.replacement {
                out.push_str(&format!("    replacement: {replacement}\n"));
            }
        }
        if let Some(help) = &diagnostic.help {
            out.push_str(&format!("  help: {help}\n"));
        }
        out.push_str(&format!(
            "  fingerprint: {}\n\n",
            diagnostic.stable_fingerprint
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

fn diagnostic_fingerprint(rule_id: &str, file: &str, range: TextRange, message: &str) -> String {
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
    fn render_human_snapshot_includes_contract_fields() {
        insta::assert_snapshot!(render(OutputFormat::Human, &[contract_diagnostic()]), @r###"
        src/lib.rs:10:4-10:12 error project/rule
          policy failed
          label 11:2-11:8: related expression
          evidence symbol: unsafe_api
          suggestion: Prefer safe_api here
          fix: Replace unsafe_api
            replacement: safe_api()
          help: Use the safe wrapper before crossing this boundary.
          fingerprint: fingerprint-123

        "###);
    }

    #[test]
    fn render_json_snapshot_is_stable() {
        let rendered = render(OutputFormat::Json, &[contract_diagnostic()]);
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 1);

        insta::assert_snapshot!(rendered, @r###"
        [
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
        "###);
    }

    #[test]
    fn render_empty_human_output_is_stable() {
        assert_eq!(render(OutputFormat::Human, &[]), "No diagnostics.\n");
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
