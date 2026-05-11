use crate::diagnostics::Diagnostic;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub(crate) const DEFAULT_BASELINE_PATH: &str = ".polint/baseline.yaml";
const BASELINE_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub(crate) enum BaselineError {
    #[error("failed to read baseline file {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to write baseline file {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("failed to create baseline directory {path}: {source}")]
    CreateDir { path: PathBuf, source: io::Error },
    #[error("failed to parse baseline YAML: {0}")]
    Yaml(#[from] serde_yml::Error),
    #[error("unsupported baseline version {0}; expected version 1")]
    UnsupportedVersion(u32),
    #[error("invalid {section} entry #{index}: {reason}; entry was `{entry}`")]
    InvalidEntry {
        section: &'static str,
        index: usize,
        entry: String,
        reason: &'static str,
    },
    #[error("baseline entry contains a newline and cannot be rendered compactly: `{0}`")]
    MultilineEntry(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BaselineEntry {
    pub(crate) rule_id: String,
    pub(crate) fingerprint: String,
    pub(crate) file: String,
}

impl BaselineEntry {
    fn from_diagnostic(diagnostic: &Diagnostic) -> Self {
        Self {
            rule_id: diagnostic.rule_id.clone(),
            fingerprint: diagnostic.stable_fingerprint.clone(),
            file: diagnostic.file.clone(),
        }
    }

    fn key(&self) -> BaselineKey {
        BaselineKey {
            rule_id: self.rule_id.clone(),
            fingerprint: self.fingerprint.clone(),
        }
    }

    fn as_protocol_string(&self) -> String {
        format!("{} {} {}", self.rule_id, self.fingerprint, self.file)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BaselineConfig {
    pub(crate) baseline: Vec<BaselineEntry>,
    pub(crate) ignore: Vec<BaselineEntry>,
}

impl BaselineConfig {
    pub(crate) fn from_diagnostics(diagnostics: &[Diagnostic]) -> Self {
        let baseline = diagnostics
            .iter()
            .map(BaselineEntry::from_diagnostic)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self {
            baseline,
            ignore: Vec::new(),
        }
    }

    pub(crate) fn updated_for_current_diagnostics(&self, diagnostics: &[Diagnostic]) -> Self {
        let current_by_key = diagnostics
            .iter()
            .map(|diagnostic| (BaselineKey::from_diagnostic(diagnostic), diagnostic))
            .collect::<BTreeMap<_, _>>();
        let ignore_keys = self
            .ignore
            .iter()
            .map(BaselineEntry::key)
            .collect::<BTreeSet<_>>();
        let baseline = self
            .baseline
            .iter()
            .filter_map(|entry| {
                let key = entry.key();
                if ignore_keys.contains(&key) {
                    return None;
                }
                current_by_key.get(&key).map(|diagnostic| BaselineEntry {
                    rule_id: entry.rule_id.clone(),
                    fingerprint: entry.fingerprint.clone(),
                    file: diagnostic.file.clone(),
                })
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let ignore = self
            .ignore
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self { baseline, ignore }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct BaselineSummary {
    pub(crate) new: usize,
    pub(crate) existing: usize,
    pub(crate) fixed: usize,
    pub(crate) ignored: usize,
    pub(crate) stale_path: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct BaselineClassification {
    pub(crate) visible_diagnostics: Vec<Diagnostic>,
    pub(crate) new_diagnostics: Vec<Diagnostic>,
    pub(crate) summary: BaselineSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BaselineKey {
    rule_id: String,
    fingerprint: String,
}

impl BaselineKey {
    fn from_diagnostic(diagnostic: &Diagnostic) -> Self {
        Self {
            rule_id: diagnostic.rule_id.clone(),
            fingerprint: diagnostic.stable_fingerprint.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BaselineWire {
    version: u32,
    #[serde(default)]
    baseline: Vec<String>,
    #[serde(default, rename = "ignore")]
    ignore: Vec<String>,
}

pub(crate) fn load_baseline(path: &Path) -> Result<BaselineConfig, BaselineError> {
    let raw = fs::read_to_string(path).map_err(|source| BaselineError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_baseline(&raw)
}

pub(crate) fn write_baseline(path: &Path, config: &BaselineConfig) -> Result<(), BaselineError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| BaselineError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let rendered = render_baseline(config)?;
    fs::write(path, rendered).map_err(|source| BaselineError::Write {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn parse_baseline(raw: &str) -> Result<BaselineConfig, BaselineError> {
    let wire: BaselineWire = serde_yml::from_str(raw)?;
    if wire.version != BASELINE_VERSION {
        return Err(BaselineError::UnsupportedVersion(wire.version));
    }
    let baseline = parse_entries("baseline", &wire.baseline)?;
    let ignore = parse_entries("ignore", &wire.ignore)?;
    Ok(BaselineConfig { baseline, ignore })
}

pub(crate) fn render_baseline(config: &BaselineConfig) -> Result<String, BaselineError> {
    let mut out = String::from("version: 1\n\n");
    push_section(&mut out, "baseline", &config.baseline)?;
    push_section(&mut out, "ignore", &config.ignore)?;
    Ok(out)
}

pub(crate) fn classify_diagnostics(
    diagnostics: &[Diagnostic],
    config: &BaselineConfig,
) -> BaselineClassification {
    let baseline_by_key = entries_by_key(&config.baseline);
    let ignore_by_key = entries_by_key(&config.ignore);
    let mut visible_diagnostics = Vec::new();
    let mut new_diagnostics = Vec::new();
    let mut current_keys = BTreeSet::new();
    let mut stale_path_entries = BTreeSet::new();
    let mut summary = BaselineSummary::default();

    for diagnostic in diagnostics {
        let key = BaselineKey::from_diagnostic(diagnostic);
        current_keys.insert(key.clone());
        if let Some(entries) = ignore_by_key.get(&key) {
            summary.ignored += 1;
            collect_stale_path_entries(entries, &diagnostic.file, &mut stale_path_entries);
            continue;
        }
        if let Some(entries) = baseline_by_key.get(&key) {
            summary.existing += 1;
            collect_stale_path_entries(entries, &diagnostic.file, &mut stale_path_entries);
            visible_diagnostics.push(diagnostic.clone());
            continue;
        }
        summary.new += 1;
        visible_diagnostics.push(diagnostic.clone());
        new_diagnostics.push(diagnostic.clone());
    }

    summary.fixed = config
        .baseline
        .iter()
        .map(BaselineEntry::key)
        .collect::<BTreeSet<_>>()
        .difference(&current_keys)
        .count();
    summary.stale_path = stale_path_entries.len();

    BaselineClassification {
        visible_diagnostics,
        new_diagnostics,
        summary,
    }
}

pub(crate) fn render_baseline_summary(summary: &BaselineSummary) -> String {
    let mut out = String::new();
    out.push_str("\nBaseline summary\n");
    out.push_str(&format!("  new: {}\n", summary.new));
    out.push_str(&format!("  existing: {}\n", summary.existing));
    out.push_str(&format!("  fixed: {}\n", summary.fixed));
    out.push_str(&format!("  ignored: {}\n", summary.ignored));
    if summary.stale_path > 0 {
        out.push_str(&format!("  stale_path: {}\n", summary.stale_path));
    }
    out
}

fn parse_entries(
    section: &'static str,
    raw_entries: &[String],
) -> Result<Vec<BaselineEntry>, BaselineError> {
    raw_entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_entry(section, index + 1, entry))
        .collect::<Result<BTreeSet<_>, _>>()
        .map(|entries| entries.into_iter().collect())
}

fn parse_entry(
    section: &'static str,
    index: usize,
    raw: &str,
) -> Result<BaselineEntry, BaselineError> {
    let entry = raw.trim();
    if entry.is_empty() {
        return invalid_entry(section, index, raw, "entry must not be empty");
    }
    if entry.contains(['\n', '\r']) {
        return invalid_entry(section, index, raw, "entry must fit on one line");
    }

    let (rule_id, rest) = next_token(entry)
        .ok_or_else(|| invalid_entry_error(section, index, raw, "missing fingerprint and file"))?;
    let (fingerprint, file) = next_token(rest.trim_start())
        .ok_or_else(|| invalid_entry_error(section, index, raw, "missing file"))?;
    let file = file.trim();
    if file.is_empty() {
        return invalid_entry(section, index, raw, "file must not be empty");
    }
    Ok(BaselineEntry {
        rule_id: rule_id.to_string(),
        fingerprint: fingerprint.to_string(),
        file: file.to_string(),
    })
}

fn next_token(s: &str) -> Option<(&str, &str)> {
    let trimmed = s.trim_start();
    let split_at = trimmed.find(char::is_whitespace)?;
    let token = &trimmed[..split_at];
    if token.is_empty() {
        return None;
    }
    Some((token, &trimmed[split_at..]))
}

fn invalid_entry<T>(
    section: &'static str,
    index: usize,
    entry: &str,
    reason: &'static str,
) -> Result<T, BaselineError> {
    Err(invalid_entry_error(section, index, entry, reason))
}

fn invalid_entry_error(
    section: &'static str,
    index: usize,
    entry: &str,
    reason: &'static str,
) -> BaselineError {
    BaselineError::InvalidEntry {
        section,
        index,
        entry: entry.to_string(),
        reason,
    }
}

fn push_section(
    out: &mut String,
    name: &'static str,
    entries: &[BaselineEntry],
) -> Result<(), BaselineError> {
    if entries.is_empty() {
        out.push_str(&format!("{name}: []\n"));
        return Ok(());
    }
    out.push_str(&format!("{name}:\n"));
    for entry in entries {
        let value = entry.as_protocol_string();
        if value.contains(['\n', '\r']) {
            return Err(BaselineError::MultilineEntry(value));
        }
        out.push_str("  - ");
        out.push_str(&quote_yaml_string(&value));
        out.push('\n');
    }
    Ok(())
}

fn quote_yaml_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn entries_by_key(entries: &[BaselineEntry]) -> BTreeMap<BaselineKey, Vec<&BaselineEntry>> {
    let mut by_key = BTreeMap::<BaselineKey, Vec<&BaselineEntry>>::new();
    for entry in entries {
        by_key.entry(entry.key()).or_default().push(entry);
    }
    by_key
}

fn collect_stale_path_entries(
    entries: &[&BaselineEntry],
    current_file: &str,
    stale_path_entries: &mut BTreeSet<BaselineEntry>,
) {
    stale_path_entries.extend(
        entries
            .iter()
            .copied()
            .filter(|entry| entry.file != current_file)
            .cloned(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{Severity, TextRange};

    fn diagnostic(rule_id: &str, fingerprint: &str, file: &str) -> Diagnostic {
        Diagnostic::new(
            rule_id,
            Severity::Warn,
            file,
            TextRange::point(1, 1),
            "message",
        )
        .with_fingerprint(fingerprint)
    }

    #[test]
    fn parse_baseline_accepts_compact_string_arrays() {
        let config = parse_baseline(
            r#"
version: 1
baseline:
  - "local/a fp-a src/a.go"
ignore:
  - "local/b fp-b src/b.go"
"#,
        )
        .unwrap();

        assert_eq!(config.baseline[0].rule_id, "local/a");
        assert_eq!(config.baseline[0].fingerprint, "fp-a");
        assert_eq!(config.baseline[0].file, "src/a.go");
        assert_eq!(config.ignore[0].rule_id, "local/b");
    }

    #[test]
    fn parse_entry_keeps_spaces_in_file_path() {
        let config = parse_baseline(
            r#"
version: 1
baseline:
  - "local/a fp-a src/path with spaces.go"
"#,
        )
        .unwrap();

        assert_eq!(config.baseline[0].file, "src/path with spaces.go");
    }

    #[test]
    fn parse_baseline_rejects_malformed_entry() {
        let error = parse_baseline(
            r#"
version: 1
baseline:
  - "local/a"
"#,
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("missing fingerprint and file"),
            "{error}"
        );
    }

    #[test]
    fn render_baseline_keeps_one_entry_per_yaml_line() {
        let config = BaselineConfig {
            baseline: vec![BaselineEntry {
                rule_id: "local/a".to_string(),
                fingerprint: "fp-a".to_string(),
                file: "src/a.go".to_string(),
            }],
            ignore: vec![BaselineEntry {
                rule_id: "local/b".to_string(),
                fingerprint: "fp-b".to_string(),
                file: "src/b.go".to_string(),
            }],
        };

        let rendered = render_baseline(&config).unwrap();

        assert!(rendered.contains("  - 'local/a fp-a src/a.go'\n"));
        assert!(rendered.contains("  - 'local/b fp-b src/b.go'\n"));
    }

    #[test]
    fn classify_diagnostics_separates_new_existing_ignored_and_fixed() {
        let config = BaselineConfig {
            baseline: vec![
                BaselineEntry {
                    rule_id: "local/existing".to_string(),
                    fingerprint: "fp-existing".to_string(),
                    file: "src/old.go".to_string(),
                },
                BaselineEntry {
                    rule_id: "local/fixed".to_string(),
                    fingerprint: "fp-fixed".to_string(),
                    file: "src/fixed.go".to_string(),
                },
            ],
            ignore: vec![BaselineEntry {
                rule_id: "local/ignored".to_string(),
                fingerprint: "fp-ignored".to_string(),
                file: "src/ignored.go".to_string(),
            }],
        };
        let diagnostics = vec![
            diagnostic("local/existing", "fp-existing", "src/new-path.go"),
            diagnostic("local/ignored", "fp-ignored", "src/ignored.go"),
            diagnostic("local/new", "fp-new", "src/new.go"),
        ];

        let classification = classify_diagnostics(&diagnostics, &config);

        assert_eq!(classification.summary.new, 1);
        assert_eq!(classification.summary.existing, 1);
        assert_eq!(classification.summary.ignored, 1);
        assert_eq!(classification.summary.fixed, 1);
        assert_eq!(classification.summary.stale_path, 1);
        assert_eq!(classification.visible_diagnostics.len(), 2);
        assert_eq!(classification.new_diagnostics.len(), 1);
    }

    #[test]
    fn updated_for_current_diagnostics_removes_fixed_and_refreshes_moved_paths() {
        let config = BaselineConfig {
            baseline: vec![
                BaselineEntry {
                    rule_id: "local/existing".to_string(),
                    fingerprint: "fp-existing".to_string(),
                    file: "src/old.go".to_string(),
                },
                BaselineEntry {
                    rule_id: "local/fixed".to_string(),
                    fingerprint: "fp-fixed".to_string(),
                    file: "src/fixed.go".to_string(),
                },
            ],
            ignore: Vec::new(),
        };
        let diagnostics = vec![diagnostic("local/existing", "fp-existing", "src/new.go")];

        let updated = config.updated_for_current_diagnostics(&diagnostics);

        assert_eq!(updated.baseline.len(), 1);
        assert_eq!(updated.baseline[0].file, "src/new.go");
    }
}
