use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::error::AnalysisError;
use crate::analysis::identity::dedup::record_sort_key;
use crate::analysis::identity::facts::{IdentityKind, IdentityRecord, LanguageTag};
use crate::analysis::ids::{CallSiteId, CallTargetId};
use crate::core::{FileId, StableKeyInterner};

/// Provider output for `polint.identity` — the normalized identity record set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct IdentityProviderOutput {
    pub(crate) records: Vec<IdentityRecord>,
}

impl IdentityProviderOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// Sorts records by the locked identity sort key
    /// `(language, package_or_module, container_path, file_id, span, kind)`.
    pub(crate) fn normalized(mut self, _interner: &StableKeyInterner) -> Self {
        self.records.sort_by_key(record_sort_key);
        self
    }
}

/// Typed identity store with the three indexes consumers read (Pattern I).
#[derive(Debug, Clone, Default)]
pub(crate) struct IdentityStore {
    pub(crate) records: Vec<IdentityRecord>,
    by_file: BTreeMap<FileId, Vec<usize>>,
    by_language: BTreeMap<LanguageTag, Vec<usize>>,
    by_kind: BTreeMap<IdentityKind, Vec<usize>>,
}

impl IdentityStore {
    /// Builds the store, validating that every originating call-site/target
    /// reference resolves against the supplied valid ID sets (Pattern I, D-04).
    pub(crate) fn from_output(
        output: IdentityProviderOutput,
        interner: &StableKeyInterner,
        valid_call_site_ids: &BTreeSet<CallSiteId>,
        valid_call_target_ids: &BTreeSet<CallTargetId>,
    ) -> Result<Self, AnalysisError> {
        let output = output.normalized(interner);

        for record in &output.records {
            if let Some(site) = record.originating_call_site_id
                && !valid_call_site_ids.contains(&site)
            {
                return Err(AnalysisError::InvalidFact {
                    provider: "polint.identity",
                    reason: format!(
                        "dangling originating call site {:?} for identity `{}`",
                        site,
                        interner.resolve(record.stable_key)
                    ),
                });
            }
            if let Some(target) = record.originating_call_target_id
                && !valid_call_target_ids.contains(&target)
            {
                return Err(AnalysisError::InvalidFact {
                    provider: "polint.identity",
                    reason: format!(
                        "dangling originating call target {:?} for identity `{}`",
                        target,
                        interner.resolve(record.stable_key)
                    ),
                });
            }
        }

        let mut store = Self {
            records: output.records,
            ..Self::default()
        };

        for (index, record) in store.records.iter().enumerate() {
            store.by_file.entry(record.file_id).or_default().push(index);
            store
                .by_language
                .entry(record.language)
                .or_default()
                .push(index);
            store.by_kind.entry(record.kind).or_default().push(index);
        }

        Ok(store)
    }

    pub(crate) fn records(&self) -> &[IdentityRecord] {
        &self.records
    }

    pub(crate) fn records_for_file(&self, file: FileId) -> &[usize] {
        self.by_file.get(&file).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn records_for_language(&self, language: LanguageTag) -> &[usize] {
        self.by_language.get(&language).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn records_for_kind(&self, kind: IdentityKind) -> &[usize] {
        self.by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::identity::facts::{
        IdentityRecordId, LanguageTag, compute_identity_stable_key, compute_signature_digest,
    };
    use crate::core::Span;
    use std::sync::Arc;

    fn record(id: u64, file: u32, kind: IdentityKind, site: Option<u64>) -> IdentityRecord {
        let language = LanguageTag::Go;
        let span = Span::point(FileId(file), 1, 1);
        IdentityRecord {
            id: IdentityRecordId(id),
            kind,
            file_id: FileId(file),
            span: span.clone(),
            language,
            package_or_module: Arc::from("pkg"),
            container_path: Arc::from(format!("pkg.T{id}")),
            display_name: Arc::from("name"),
            signature_digest: compute_signature_digest(
                language,
                "pkg",
                &format!("pkg.T{id}"),
                "name",
                None,
                None,
            ),
            multiplicity: 1,
            stable_key: crate::core::stable_key_for_test(&compute_identity_stable_key(
                kind,
                language,
                "pkg",
                &format!("pkg.T{id}"),
                FileId(file),
                &span,
            )),
            originating_call_site_id: site.map(CallSiteId),
            originating_call_target_id: None,
        }
    }

    #[test]
    fn from_output_builds_deterministic_indexes() {
        let interner = crate::core::test_stable_key_interner();
        let output = IdentityProviderOutput {
            records: vec![
                record(1, 0, IdentityKind::Function, None),
                record(2, 1, IdentityKind::Callsite, None),
            ],
        };
        let store =
            IdentityStore::from_output(output, &interner, &BTreeSet::new(), &BTreeSet::new())
                .expect("store");
        assert_eq!(store.records().len(), 2);
        assert_eq!(store.records_for_file(FileId(0)).len(), 1);
        assert_eq!(store.records_for_language(LanguageTag::Go).len(), 2);
        assert_eq!(store.records_for_kind(IdentityKind::Callsite).len(), 1);
        assert!(store.records_for_kind(IdentityKind::Function).len() == 1);
        assert!(
            store
                .records_for_language(LanguageTag::TypeScript)
                .is_empty()
        );
    }

    #[test]
    fn from_output_rejects_dangling_call_site_reference() {
        let interner = crate::core::test_stable_key_interner();
        let output = IdentityProviderOutput {
            records: vec![record(1, 0, IdentityKind::Callsite, Some(99))],
        };
        let error =
            IdentityStore::from_output(output, &interner, &BTreeSet::new(), &BTreeSet::new())
                .expect_err("dangling site rejected");
        assert!(
            error
                .to_string()
                .contains("dangling originating call site CallSiteId(99)")
        );
        assert!(error.to_string().contains("polint.identity"));
    }

    #[test]
    fn from_output_accepts_valid_call_site_reference() {
        let interner = crate::core::test_stable_key_interner();
        let mut sites = BTreeSet::new();
        sites.insert(CallSiteId(7));
        let output = IdentityProviderOutput {
            records: vec![record(1, 0, IdentityKind::Callsite, Some(7))],
        };
        let store = IdentityStore::from_output(output, &interner, &sites, &BTreeSet::new())
            .expect("valid reference");
        assert_eq!(store.records().len(), 1);
    }

    #[test]
    fn empty_output_builds_empty_store() {
        let interner = crate::core::test_stable_key_interner();
        let store = IdentityStore::from_output(
            IdentityProviderOutput::empty(),
            &interner,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .expect("empty store");
        assert!(store.records().is_empty());
    }
}
