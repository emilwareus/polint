use crate::diagnostics::{Diagnostic, diagnostic_fingerprint, fingerprint};
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
    fn from_identity(identity: &DiagnosticIdentity) -> Self {
        Self {
            rule_id: identity.rule_id.clone(),
            fingerprint: identity.baseline_fingerprint.clone(),
            file: identity.file.clone(),
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
        let baseline = diagnostic_identities(diagnostics)
            .iter()
            .map(BaselineEntry::from_identity)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self {
            baseline,
            ignore: Vec::new(),
        }
    }

    pub(crate) fn updated_for_current_diagnostics(&self, diagnostics: &[Diagnostic]) -> Self {
        let identities = diagnostic_identities(diagnostics);
        let baseline_entries = active_baseline_entries(self);
        let ignore_matches =
            match_entries_to_diagnostics(&self.ignore, &identities, &BTreeSet::new(), false);
        let baseline_matches = match_entries_to_diagnostics(
            &baseline_entries,
            &identities,
            &ignore_matches.matched_diagnostics,
            true,
        );
        let baseline = baseline_matches
            .entry_to_diagnostic
            .values()
            .map(|diagnostic_index| BaselineEntry::from_identity(&identities[*diagnostic_index]))
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

#[derive(Debug, Clone)]
struct EntryMatches {
    matched_diagnostics: BTreeSet<usize>,
    entry_to_diagnostic: BTreeMap<usize, usize>,
    stale_path_entries: BTreeSet<BaselineEntry>,
}

impl EntryMatches {
    fn new() -> Self {
        Self {
            matched_diagnostics: BTreeSet::new(),
            entry_to_diagnostic: BTreeMap::new(),
            stale_path_entries: BTreeSet::new(),
        }
    }

    fn insert(&mut self, entry_index: usize, identity: &DiagnosticIdentity, entry: &BaselineEntry) {
        self.matched_diagnostics.insert(identity.index);
        self.entry_to_diagnostic.insert(entry_index, identity.index);
        if entry.file != identity.file {
            self.stale_path_entries.insert(entry.clone());
        }
    }
}

#[derive(Debug, Clone)]
struct DiagnosticIdentity {
    index: usize,
    rule_id: String,
    file: String,
    baseline_fingerprint: String,
    keys: BTreeSet<BaselineKey>,
}

impl DiagnosticIdentity {
    fn new(index: usize, diagnostic: &Diagnostic, baseline_fingerprint: String) -> Self {
        let mut keys = BTreeSet::new();
        keys.insert(BaselineKey::from_stable_diagnostic(diagnostic));
        keys.insert(BaselineKey {
            rule_id: diagnostic.rule_id.clone(),
            fingerprint: baseline_fingerprint.clone(),
        });
        Self {
            index,
            rule_id: diagnostic.rule_id.clone(),
            file: diagnostic.file.clone(),
            baseline_fingerprint,
            keys,
        }
    }

    fn matches_entry(&self, entry: &BaselineEntry) -> bool {
        self.keys.contains(&entry.key())
    }
}

#[derive(Debug, Clone)]
struct DiagnosticIdentitySeed {
    index: usize,
    rule_id: String,
    file: String,
    range: (u32, u32, u32, u32),
    stable_fingerprint: String,
    baseline_base: BaselineFingerprint,
}

#[derive(Debug, Clone)]
struct BaselineFingerprint {
    value: String,
    is_custom: bool,
}

fn active_baseline_entries(config: &BaselineConfig) -> Vec<BaselineEntry> {
    let ignored_entries = config.ignore.iter().cloned().collect::<BTreeSet<_>>();
    config
        .baseline
        .iter()
        .filter(|entry| !ignored_entries.contains(*entry))
        .cloned()
        .collect()
}

fn diagnostic_identities(diagnostics: &[Diagnostic]) -> Vec<DiagnosticIdentity> {
    let seeds = diagnostics
        .iter()
        .enumerate()
        .map(|(index, diagnostic)| DiagnosticIdentitySeed {
            index,
            rule_id: diagnostic.rule_id.clone(),
            file: diagnostic.file.clone(),
            range: (
                diagnostic.range.start_line,
                diagnostic.range.start_col,
                diagnostic.range.end_line,
                diagnostic.range.end_col,
            ),
            stable_fingerprint: diagnostic.stable_fingerprint.clone(),
            baseline_base: baseline_fingerprint_base(diagnostic),
        })
        .collect::<Vec<_>>();

    let mut baseline_fingerprints = vec![String::new(); seeds.len()];
    let mut ordered_seed_indices = (0..seeds.len()).collect::<Vec<_>>();
    ordered_seed_indices.sort_by(|left, right| {
        let left = &seeds[*left];
        let right = &seeds[*right];
        (
            left.rule_id.as_str(),
            left.file.as_str(),
            left.baseline_base.value.as_str(),
            left.range,
            left.stable_fingerprint.as_str(),
            left.index,
        )
            .cmp(&(
                right.rule_id.as_str(),
                right.file.as_str(),
                right.baseline_base.value.as_str(),
                right.range,
                right.stable_fingerprint.as_str(),
                right.index,
            ))
    });

    let mut next_occurrence_by_scope = BTreeMap::<(String, String, String), usize>::new();
    for seed_index in ordered_seed_indices {
        let seed = &seeds[seed_index];
        if seed.baseline_base.is_custom {
            baseline_fingerprints[seed_index] = seed.baseline_base.value.clone();
            continue;
        }
        let occurrence = next_occurrence_by_scope
            .entry((
                seed.rule_id.clone(),
                seed.file.clone(),
                seed.baseline_base.value.clone(),
            ))
            .or_default();
        baseline_fingerprints[seed_index] =
            baseline_fingerprint_for_occurrence(&seed.baseline_base.value, *occurrence);
        *occurrence += 1;
    }

    seeds
        .into_iter()
        .zip(baseline_fingerprints)
        .map(|(seed, baseline_fingerprint)| {
            DiagnosticIdentity::new(seed.index, &diagnostics[seed.index], baseline_fingerprint)
        })
        .collect()
}

fn match_entries_to_diagnostics(
    entries: &[BaselineEntry],
    identities: &[DiagnosticIdentity],
    excluded_diagnostics: &BTreeSet<usize>,
    allow_relocation: bool,
) -> EntryMatches {
    let mut matches = EntryMatches::new();

    for (entry_index, entry) in entries.iter().enumerate() {
        let Some(identity) = identities.iter().find(|identity| {
            !excluded_diagnostics.contains(&identity.index)
                && !matches.matched_diagnostics.contains(&identity.index)
                && identity.matches_entry(entry)
                && identity.file == entry.file
        }) else {
            continue;
        };
        matches.insert(entry_index, identity, entry);
    }

    if !allow_relocation {
        return matches;
    }

    let mut unmatched_entries_by_key = BTreeMap::<BaselineKey, Vec<usize>>::new();
    for (entry_index, entry) in entries.iter().enumerate() {
        if !matches.entry_to_diagnostic.contains_key(&entry_index) {
            unmatched_entries_by_key
                .entry(entry.key())
                .or_default()
                .push(entry_index);
        }
    }

    for (key, entry_indices) in unmatched_entries_by_key {
        let diagnostic_indices = identities
            .iter()
            .enumerate()
            .filter(|(_, identity)| {
                !excluded_diagnostics.contains(&identity.index)
                    && !matches.matched_diagnostics.contains(&identity.index)
                    && identity.keys.contains(&key)
            })
            .map(|(identity_index, _)| identity_index)
            .collect::<Vec<_>>();
        if entry_indices.len() == 1 && diagnostic_indices.len() == 1 {
            let entry_index = entry_indices[0];
            let identity_index = diagnostic_indices[0];
            matches.insert(
                entry_index,
                &identities[identity_index],
                &entries[entry_index],
            );
        }
    }

    matches
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
    fn from_stable_diagnostic(diagnostic: &Diagnostic) -> Self {
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
    let identities = diagnostic_identities(diagnostics);
    let baseline_entries = active_baseline_entries(config);
    let ignore_matches =
        match_entries_to_diagnostics(&config.ignore, &identities, &BTreeSet::new(), false);
    let baseline_matches = match_entries_to_diagnostics(
        &baseline_entries,
        &identities,
        &ignore_matches.matched_diagnostics,
        true,
    );
    let mut visible_diagnostics = Vec::new();
    let mut new_diagnostics = Vec::new();
    let mut summary = BaselineSummary::default();

    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if ignore_matches.matched_diagnostics.contains(&index) {
            continue;
        }
        if baseline_matches.matched_diagnostics.contains(&index) {
            visible_diagnostics.push(diagnostic.clone());
            continue;
        }
        summary.new += 1;
        visible_diagnostics.push(diagnostic.clone());
        new_diagnostics.push(diagnostic.clone());
    }

    summary.ignored = ignore_matches.matched_diagnostics.len();
    summary.existing = baseline_matches.matched_diagnostics.len();
    summary.fixed = baseline_entries.len() - baseline_matches.entry_to_diagnostic.len();
    summary.stale_path = baseline_matches.stale_path_entries.len();

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

fn baseline_fingerprint_base(diagnostic: &Diagnostic) -> BaselineFingerprint {
    let default_stable = diagnostic_fingerprint(
        &diagnostic.rule_id,
        &diagnostic.file,
        diagnostic.range,
        &diagnostic.message,
    );
    if diagnostic.stable_fingerprint != default_stable {
        return BaselineFingerprint {
            value: diagnostic.stable_fingerprint.clone(),
            is_custom: true,
        };
    }

    let severity = diagnostic.severity.to_string();
    let mut parts = vec![
        "baseline-v1",
        diagnostic.rule_id.as_str(),
        severity.as_str(),
        diagnostic.message.as_str(),
    ];
    for evidence in &diagnostic.evidence {
        parts.push("evidence");
        parts.push(evidence.label.as_str());
        parts.push(evidence.value.as_str());
    }
    for label in &diagnostic.labels {
        parts.push("label");
        parts.push(label.message.as_str());
    }
    if let Some(help) = &diagnostic.help {
        parts.push("help");
        parts.push(help.as_str());
    }
    for suggestion in &diagnostic.suggestions {
        parts.push("suggestion");
        parts.push(suggestion.message.as_str());
    }
    if let Some(fix) = &diagnostic.fix {
        parts.push("fix");
        parts.push(fix.message.as_str());
        if let Some(replacement) = &fix.replacement {
            parts.push(replacement.as_str());
        }
    }
    BaselineFingerprint {
        value: fingerprint(&parts),
        is_custom: false,
    }
}

fn baseline_fingerprint_for_occurrence(base: &str, occurrence: usize) -> String {
    if occurrence == 0 {
        return base.to_string();
    }
    let occurrence = occurrence.to_string();
    fingerprint(&["baseline-v1-occurrence", base, occurrence.as_str()])
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

    fn default_diagnostic(rule_id: &str, file: &str, line: u32, message: &str) -> Diagnostic {
        Diagnostic::new(
            rule_id,
            Severity::Warn,
            file,
            TextRange::point(line, 1),
            message,
        )
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

    #[test]
    fn classify_diagnostics_matches_unambiguous_default_fingerprint_relocations() {
        let original = default_diagnostic("local/moved", "src/old.ts", 3, "policy failed");
        let config = BaselineConfig::from_diagnostics(&[original]);
        let moved = default_diagnostic("local/moved", "src/new.ts", 10, "policy failed");

        let classification = classify_diagnostics(&[moved], &config);

        assert_eq!(classification.summary.new, 0);
        assert_eq!(classification.summary.existing, 1);
        assert_eq!(classification.summary.fixed, 0);
        assert_eq!(classification.summary.stale_path, 1);
        assert_eq!(classification.new_diagnostics.len(), 0);
    }

    #[test]
    fn classify_diagnostics_keeps_ambiguous_relocations_new() {
        let original = default_diagnostic("local/moved", "src/old.ts", 3, "policy failed");
        let config = BaselineConfig::from_diagnostics(&[original]);
        let diagnostics = vec![
            default_diagnostic("local/moved", "src/new.ts", 10, "policy failed"),
            default_diagnostic("local/moved", "src/other.ts", 12, "policy failed"),
        ];

        let classification = classify_diagnostics(&diagnostics, &config);

        assert_eq!(classification.summary.new, 2);
        assert_eq!(classification.summary.existing, 0);
        assert_eq!(classification.summary.fixed, 1);
        assert_eq!(classification.summary.stale_path, 0);
        assert_eq!(classification.new_diagnostics.len(), 2);
    }

    #[test]
    fn updated_for_current_diagnostics_refreshes_unambiguous_default_fingerprint_relocations() {
        let original = default_diagnostic("local/moved", "src/old.ts", 3, "policy failed");
        let config = BaselineConfig::from_diagnostics(&[original]);
        let moved = default_diagnostic("local/moved", "src/new.ts", 10, "policy failed");

        let updated = config.updated_for_current_diagnostics(&[moved]);

        assert_eq!(updated.baseline.len(), 1);
        assert_eq!(updated.baseline[0].file, "src/new.ts");
    }

    #[test]
    fn from_diagnostics_keeps_repeated_default_diagnostics_in_same_file_distinct() {
        let diagnostics = vec![
            default_diagnostic("local/repeated", "src/app.ts", 3, "policy failed"),
            default_diagnostic("local/repeated", "src/app.ts", 9, "policy failed"),
        ];

        let config = BaselineConfig::from_diagnostics(&diagnostics);

        assert_eq!(config.baseline.len(), 2);
        assert_eq!(config.baseline[0].file, "src/app.ts");
        assert_eq!(config.baseline[1].file, "src/app.ts");
        assert_ne!(
            config.baseline[0].fingerprint,
            config.baseline[1].fingerprint
        );
    }

    #[test]
    fn classify_diagnostics_matches_repeated_default_diagnostics_in_same_file() {
        let diagnostics = vec![
            default_diagnostic("local/repeated", "src/app.ts", 3, "policy failed"),
            default_diagnostic("local/repeated", "src/app.ts", 9, "policy failed"),
        ];
        let config = BaselineConfig::from_diagnostics(&diagnostics);

        let classification = classify_diagnostics(&diagnostics, &config);

        assert_eq!(classification.summary.new, 0);
        assert_eq!(classification.summary.existing, 2);
        assert_eq!(classification.summary.fixed, 0);
        assert_eq!(classification.new_diagnostics.len(), 0);
    }

    #[test]
    fn updated_for_current_diagnostics_refreshes_repeated_default_relocations() {
        let original = vec![
            default_diagnostic("local/repeated", "src/old.ts", 3, "policy failed"),
            default_diagnostic("local/repeated", "src/old.ts", 9, "policy failed"),
        ];
        let config = BaselineConfig::from_diagnostics(&original);
        let moved = vec![
            default_diagnostic("local/repeated", "src/new.ts", 10, "policy failed"),
            default_diagnostic("local/repeated", "src/new.ts", 20, "policy failed"),
        ];

        let classification = classify_diagnostics(&moved, &config);
        let updated = config.updated_for_current_diagnostics(&moved);

        assert_eq!(classification.summary.new, 0);
        assert_eq!(classification.summary.existing, 2);
        assert_eq!(classification.summary.stale_path, 2);
        assert_eq!(updated.baseline.len(), 2);
        assert!(
            updated
                .baseline
                .iter()
                .all(|entry| entry.file == "src/new.ts")
        );
    }

    #[test]
    fn baseline_ignore_entries_are_file_specific_for_default_fingerprints() {
        let active = default_diagnostic("local/repeated", "src/active.ts", 3, "policy failed");
        let ignored = default_diagnostic("local/repeated", "src/ignored.ts", 3, "policy failed");
        let active_entry = BaselineConfig::from_diagnostics(std::slice::from_ref(&active))
            .baseline
            .into_iter()
            .next()
            .unwrap();
        let ignored_entry = BaselineEntry {
            file: ignored.file.clone(),
            ..active_entry.clone()
        };
        let config = BaselineConfig {
            baseline: vec![active_entry],
            ignore: vec![ignored_entry],
        };

        let classification = classify_diagnostics(&[ignored.clone(), active.clone()], &config);
        let updated = config.updated_for_current_diagnostics(&[ignored, active]);

        assert_eq!(classification.summary.ignored, 1);
        assert_eq!(classification.summary.existing, 1);
        assert_eq!(classification.summary.fixed, 0);
        assert_eq!(classification.summary.new, 0);
        assert_eq!(updated.baseline.len(), 1);
        assert_eq!(updated.baseline[0].file, "src/active.ts");
    }
}
