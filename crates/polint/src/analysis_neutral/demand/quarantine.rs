use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::analysis_api::{Digest, DigestKind};

// ---------------------------------------------------------------------------
// Extension quarantine types
// ---------------------------------------------------------------------------

/// Reason an extension-produced or extension-influenced result was
/// quarantined rather than trusted.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum QuarantineReason {
    /// Extension code changed since the last validated run.
    ExtensionCodeChanged,
    /// Extension declared inputs do not match observed reads.
    UndeclaredRead {
        extension_key: String,
        observed_key: String,
    },
    /// Extension validation fixture is missing or failed.
    ValidationFailed { extension_key: String },
    /// Extension precision ceiling was downgraded.
    PrecisionCeilingChanged {
        extension_key: String,
        previous: String,
        current: String,
    },
    /// Extension was deactivated or removed between runs.
    ExtensionRemoved { extension_key: String },
    /// Extension produced facts that conflict with native provider facts.
    NativeConflict {
        extension_key: String,
        fact_key: String,
    },
}

impl QuarantineReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExtensionCodeChanged => "extension_code_changed",
            Self::UndeclaredRead { .. } => "undeclared_read",
            Self::ValidationFailed { .. } => "validation_failed",
            Self::PrecisionCeilingChanged { .. } => "precision_ceiling_changed",
            Self::ExtensionRemoved { .. } => "extension_removed",
            Self::NativeConflict { .. } => "native_conflict",
        }
    }

    fn digest_part(&self) -> String {
        match self {
            Self::ExtensionCodeChanged => self.as_str().to_string(),
            Self::UndeclaredRead {
                extension_key,
                observed_key,
            } => format!(
                "{}:extension={}:observed={}",
                self.as_str(),
                extension_key,
                observed_key
            ),
            Self::ValidationFailed { extension_key } => {
                format!("{}:extension={}", self.as_str(), extension_key)
            }
            Self::PrecisionCeilingChanged {
                extension_key,
                previous,
                current,
            } => format!(
                "{}:extension={}:previous={}:current={}",
                self.as_str(),
                extension_key,
                previous,
                current
            ),
            Self::ExtensionRemoved { extension_key } => {
                format!("{}:extension={}", self.as_str(), extension_key)
            }
            Self::NativeConflict {
                extension_key,
                fact_key,
            } => format!(
                "{}:extension={}:fact={}",
                self.as_str(),
                extension_key,
                fact_key
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// QuarantineEntry — a quarantined result record
// ---------------------------------------------------------------------------

/// Records a quarantined cache entry or query result.
///
/// When an extension's code, declared inputs, or validation status changes,
/// all facts influenced by that extension are quarantined until re-validated.
/// Quarantined results are not used for cache reuse or downstream
/// computation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantineEntry {
    /// The extension that caused the quarantine.
    pub extension_key: String,
    /// The cache node or query result that was quarantined.
    pub quarantined_key: String,
    /// Reason for quarantine.
    pub reason: QuarantineReason,
    /// Digest of the extension code at the time of quarantine.
    pub extension_digest: Digest,
    /// Digest of the quarantined result that should not be reused.
    pub quarantined_digest: Digest,
}

// ---------------------------------------------------------------------------
// QuarantineSet — the set of all active quarantine records
// ---------------------------------------------------------------------------

/// Maintains the set of quarantined results for a single analysis run.
///
/// Extension-aware cache quarantine ensures that when extension code,
/// declared read sets, validation fixtures, or precision ceilings change,
/// affected facts are recomputed rather than reused from stale cache.
#[derive(Debug, Clone, Default)]
pub struct QuarantineSet {
    entries: Vec<QuarantineEntry>,
    quarantined_keys: BTreeSet<String>,
}

impl QuarantineSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a quarantine entry.
    pub fn quarantine(&mut self, entry: QuarantineEntry) {
        self.quarantined_keys.insert(entry.quarantined_key.clone());
        self.entries.push(entry);
    }

    /// Returns whether a given key is quarantined.
    pub fn is_quarantined(&self, key: &str) -> bool {
        self.quarantined_keys.contains(key)
    }

    /// Returns all quarantine entries.
    pub fn entries(&self) -> &[QuarantineEntry] {
        &self.entries
    }

    /// Returns the number of quarantined results.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Returns quarantine entries for a specific extension.
    pub fn entries_for_extension(&self, extension_key: &str) -> Vec<&QuarantineEntry> {
        self.entries
            .iter()
            .filter(|e| e.extension_key == extension_key)
            .collect()
    }

    /// Returns a digest summarizing the quarantine state.
    pub fn digest(&self) -> Digest {
        if self.entries.is_empty() {
            return Digest::absent(DigestKind::ExtensionCode, "no_quarantine");
        }

        let mut parts: Vec<String> = self
            .entries
            .iter()
            .map(|e| {
                let reason = e.reason.digest_part();
                let extension_digest = e.extension_digest.to_string();
                let quarantined_digest = e.quarantined_digest.to_string();
                Digest::from_parts(
                    DigestKind::ExtensionCode,
                    "quarantine_entry",
                    &[
                        &e.extension_key,
                        &e.quarantined_key,
                        &reason,
                        &extension_digest,
                        &quarantined_digest,
                    ],
                )
                .to_string()
            })
            .collect();
        parts.sort();

        let part_refs: Vec<&str> = parts.iter().map(String::as_str).collect();
        Digest::from_parts(DigestKind::ExtensionCode, "quarantine_set", &part_refs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_digest(label: &str) -> Digest {
        Digest::from_parts(DigestKind::ExtensionCode, label, &["test"])
    }

    fn test_entry(ext_key: &str, q_key: &str, reason: QuarantineReason) -> QuarantineEntry {
        QuarantineEntry {
            extension_key: ext_key.to_string(),
            quarantined_key: q_key.to_string(),
            reason,
            extension_digest: test_digest(ext_key),
            quarantined_digest: test_digest(q_key),
        }
    }

    #[test]
    fn empty_quarantine_set() {
        let qs = QuarantineSet::new();
        assert_eq!(qs.count(), 0);
        assert!(!qs.is_quarantined("any_key"));
        assert!(qs.entries().is_empty());
    }

    #[test]
    fn quarantine_and_check() {
        let mut qs = QuarantineSet::new();

        qs.quarantine(test_entry(
            "ext::custom_model",
            "summary:func_a",
            QuarantineReason::ExtensionCodeChanged,
        ));

        assert_eq!(qs.count(), 1);
        assert!(qs.is_quarantined("summary:func_a"));
        assert!(!qs.is_quarantined("summary:func_b"));
    }

    #[test]
    fn entries_for_extension_filters() {
        let mut qs = QuarantineSet::new();

        qs.quarantine(test_entry(
            "ext::model_a",
            "summary:a",
            QuarantineReason::ExtensionCodeChanged,
        ));
        qs.quarantine(test_entry(
            "ext::model_b",
            "summary:b",
            QuarantineReason::ValidationFailed {
                extension_key: "ext::model_b".to_string(),
            },
        ));
        qs.quarantine(test_entry(
            "ext::model_a",
            "summary:c",
            QuarantineReason::ExtensionCodeChanged,
        ));

        let model_a = qs.entries_for_extension("ext::model_a");
        assert_eq!(model_a.len(), 2);

        let model_b = qs.entries_for_extension("ext::model_b");
        assert_eq!(model_b.len(), 1);

        let none = qs.entries_for_extension("ext::nonexistent");
        assert!(none.is_empty());
    }

    #[test]
    fn quarantine_reason_as_str_covers_all_variants() {
        let reasons = [
            QuarantineReason::ExtensionCodeChanged,
            QuarantineReason::UndeclaredRead {
                extension_key: "ext".to_string(),
                observed_key: "read".to_string(),
            },
            QuarantineReason::ValidationFailed {
                extension_key: "ext".to_string(),
            },
            QuarantineReason::PrecisionCeilingChanged {
                extension_key: "ext".to_string(),
                previous: "setup_aware".to_string(),
                current: "heuristic".to_string(),
            },
            QuarantineReason::ExtensionRemoved {
                extension_key: "ext".to_string(),
            },
            QuarantineReason::NativeConflict {
                extension_key: "ext".to_string(),
                fact_key: "fact".to_string(),
            },
        ];

        let strings: Vec<_> = reasons.iter().map(|r| r.as_str()).collect();
        assert_eq!(strings.len(), 6);
        let unique: std::collections::BTreeSet<_> = strings.iter().collect();
        assert_eq!(unique.len(), 6);
    }

    #[test]
    fn quarantine_digest_is_deterministic() {
        let mut qs1 = QuarantineSet::new();
        qs1.quarantine(test_entry(
            "ext::a",
            "key:1",
            QuarantineReason::ExtensionCodeChanged,
        ));
        qs1.quarantine(test_entry(
            "ext::b",
            "key:2",
            QuarantineReason::ExtensionCodeChanged,
        ));

        let mut qs2 = QuarantineSet::new();
        qs2.quarantine(test_entry(
            "ext::a",
            "key:1",
            QuarantineReason::ExtensionCodeChanged,
        ));
        qs2.quarantine(test_entry(
            "ext::b",
            "key:2",
            QuarantineReason::ExtensionCodeChanged,
        ));

        assert_eq!(qs1.digest(), qs2.digest());
        assert!(!qs1.digest().value.is_empty());
    }

    #[test]
    fn quarantine_digest_changes_when_reason_payload_changes() {
        let mut qs1 = QuarantineSet::new();
        qs1.quarantine(test_entry(
            "ext::a",
            "key:1",
            QuarantineReason::UndeclaredRead {
                extension_key: "ext::a".to_string(),
                observed_key: "read:a".to_string(),
            },
        ));

        let mut qs2 = QuarantineSet::new();
        qs2.quarantine(test_entry(
            "ext::a",
            "key:1",
            QuarantineReason::UndeclaredRead {
                extension_key: "ext::a".to_string(),
                observed_key: "read:b".to_string(),
            },
        ));

        assert_ne!(qs1.digest(), qs2.digest());
    }

    #[test]
    fn quarantine_digest_changes_when_entry_digest_changes() {
        let mut qs1 = QuarantineSet::new();
        qs1.quarantine(QuarantineEntry {
            extension_key: "ext::a".to_string(),
            quarantined_key: "key:1".to_string(),
            reason: QuarantineReason::ExtensionCodeChanged,
            extension_digest: test_digest("ext-a-v1"),
            quarantined_digest: test_digest("result-a"),
        });

        let mut qs2 = QuarantineSet::new();
        qs2.quarantine(QuarantineEntry {
            extension_key: "ext::a".to_string(),
            quarantined_key: "key:1".to_string(),
            reason: QuarantineReason::ExtensionCodeChanged,
            extension_digest: test_digest("ext-a-v2"),
            quarantined_digest: test_digest("result-a"),
        });

        assert_ne!(qs1.digest(), qs2.digest());
    }

    #[test]
    fn empty_quarantine_digest_is_absent() {
        let qs = QuarantineSet::new();
        let digest = qs.digest();
        assert!(!digest.value.is_empty()); // absent digest still has value
    }
}
