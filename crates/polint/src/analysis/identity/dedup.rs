use std::collections::BTreeMap;
use std::sync::Arc;

use crate::analysis::identity::facts::{
    IdentityKind, IdentityRecord, LanguageTag, SignatureDigest,
};
use crate::analysis::ids::{CallSiteId, CallTargetId};
use crate::core::{FileId, Span};

/// Comparable, hashable projection of a `Span` so it can participate in a
/// `BTreeMap` dedup key (`Span` itself is not `Ord`/`Hash`).
type SpanKey = (FileId, u32, u32, u32, u32, u32, u32);

/// Semantic dedup key (Pattern G, D-09, D-10).
///
/// `Some((file, span))` preserves in-file uniqueness; `None` collapses
/// cross-file aliases. The span portion is projected to an `Ord` tuple.
type DedupKey = (
    IdentityKind,
    LanguageTag,
    Arc<str>,
    Arc<str>,
    SignatureDigest,
    Option<(FileId, SpanKey)>,
);

/// Comparable projection used for deterministic output ordering: the locked
/// six-field sort key (`record_sort_key`).
type SortKey = (
    LanguageTag,
    Arc<str>,
    Arc<str>,
    FileId,
    SpanKey,
    IdentityKind,
);

/// Literal total-order projection: `SortKey` extended with the remaining record
/// fields (`originating_call_site_id`, `originating_call_target_id`,
/// `signature_digest`) so the comparison never ties on distinct records (CR-03).
type TotalOrderKey = (
    LanguageTag,
    Arc<str>,
    Arc<str>,
    FileId,
    SpanKey,
    IdentityKind,
    Option<CallSiteId>,
    Option<CallTargetId>,
    SignatureDigest,
);

fn span_key(span: &Span) -> SpanKey {
    (
        span.file,
        span.start_byte,
        span.end_byte,
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col,
    )
}

/// Comparable projection of an identity record used for deterministic output
/// ordering: `(language, package_or_module, container_path, file_id, span,
/// kind)` per the CONTEXT.md established determinism pattern and the locked
/// IdentityStore sort key.
pub(crate) fn record_sort_key(record: &IdentityRecord) -> SortKey {
    (
        record.language,
        Arc::clone(&record.package_or_module),
        Arc::clone(&record.container_path),
        record.file_id,
        span_key(&record.span),
        record.kind,
    )
}

/// Total-order projection of an identity record (CR-03 / IDENT-01).
///
/// `record_sort_key` is NOT a total order: two records can tie on all six of its
/// fields yet differ on `originating_call_site_id` / `originating_call_target_id`
/// / `signature_digest`. That left canonical selection and final ordering
/// input-order-dependent in same-span ties. This extends the key with the full
/// remaining tuple `(originating_call_site_id, originating_call_target_id,
/// signature_digest)` so the comparison is a literal total order — making both
/// canonical selection on collision and the final output sort byte-stable
/// regardless of input order (the contract Phase 43's determinism gate inherits).
///
/// `CallSiteId` / `CallTargetId` are `Ord`; `Option<T: Ord>` is `Ord`;
/// `SignatureDigest` is `Ord` — so the whole tuple is `Ord`.
pub(crate) fn record_total_order_key(record: &IdentityRecord) -> TotalOrderKey {
    let (language, package_or_module, container_path, file_id, span, kind) =
        record_sort_key(record);
    (
        language,
        package_or_module,
        container_path,
        file_id,
        span,
        kind,
        record.originating_call_site_id,
        record.originating_call_target_id,
        record.signature_digest,
    )
}

/// Deduplicates identity records by semantic identity, collapsing duplicates and
/// recording a `multiplicity` merge counter (D-09, D-10).
///
/// On a duplicate hit, `multiplicity` is the ONLY field changed — every other
/// field is preserved from the first-seen (canonical) record so dedup is
/// order-independent. The returned vector is sorted by the locked sort key so
/// the output is byte-stable across input shuffling.
pub(crate) fn dedup_identity_records(records: Vec<IdentityRecord>) -> Vec<IdentityRecord> {
    let mut collapsed: BTreeMap<DedupKey, IdentityRecord> = BTreeMap::new();
    for record in records {
        let key = dedup_key(&record);
        match collapsed.get_mut(&key) {
            Some(existing) => {
                existing.multiplicity = existing.multiplicity.saturating_add(1);
                // The canonical retained record must be order-independent (D-11,
                // CR-03): when a group collapses, keep the lexicographically-smallest
                // record by the TOTAL-ORDER key (record_sort_key extended with
                // originating_call_site_id / originating_call_target_id /
                // signature_digest) so the result is byte-stable even when records
                // tie on every record_sort_key field. Multiplicity is preserved
                // across the swap.
                if record_total_order_key(&record) < record_total_order_key(existing) {
                    let multiplicity = existing.multiplicity;
                    *existing = record.clone_with_multiplicity(multiplicity);
                }
            }
            None => {
                collapsed.insert(key, record.clone_with_multiplicity(1));
            }
        }
    }

    let mut output = collapsed.into_values().collect::<Vec<_>>();
    // Final ordering uses the same TOTAL-ORDER key as canonical selection so the
    // retained record and the output order agree and stay byte-stable (CR-03).
    output.sort_by_key(record_total_order_key);
    output
}

fn dedup_key(record: &IdentityRecord) -> DedupKey {
    // Cross-file alias collapse: callsite records that carry no originating call
    // site (i.e. cross-file aliases) drop the span so identical callees in
    // different files collapse. Function records and callsites with a concrete
    // originating callsite keep their span for in-file uniqueness (D-09).
    let span = if record.originating_call_site_id.is_none() && record.kind == IdentityKind::Callsite
    {
        None
    } else {
        Some((record.file_id, span_key(&record.span)))
    };
    (
        record.kind,
        record.language,
        Arc::clone(&record.package_or_module),
        Arc::clone(&record.container_path),
        record.signature_digest,
        span,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::identity::facts::{
        IdentityRecordId, compute_identity_stable_key, compute_signature_digest,
    };
    use crate::analysis::ids::CallSiteId;

    fn record(
        kind: IdentityKind,
        file: u32,
        start: u32,
        site: Option<u64>,
        package: &str,
        container: &str,
    ) -> IdentityRecord {
        let language = LanguageTag::Go;
        let span = Span {
            file: FileId(file),
            start_byte: start,
            end_byte: start + 4,
            start_line: 1,
            start_col: start + 1,
            end_line: 1,
            end_col: start + 5,
        };
        IdentityRecord {
            id: IdentityRecordId(0),
            kind,
            file_id: FileId(file),
            span: span.clone(),
            language,
            package_or_module: Arc::from(package),
            container_path: Arc::from(container),
            display_name: Arc::from("name"),
            signature_digest: compute_signature_digest(
                language, package, container, "name", None, None,
            ),
            multiplicity: 1,
            stable_key: compute_identity_stable_key(
                kind,
                language,
                package,
                container,
                FileId(file),
                &span,
            ),
            originating_call_site_id: site.map(CallSiteId),
            originating_call_target_id: None,
        }
    }

    #[test]
    fn two_identical_callsites_collapse_to_multiplicity_two() {
        // Same file + span + semantic fields, no originating site difference that
        // matters: identical in-file callsites collapse.
        let first = record(IdentityKind::Callsite, 0, 10, Some(1), "pkg", "pkg.Func");
        let second = record(IdentityKind::Callsite, 0, 10, Some(1), "pkg", "pkg.Func");
        let output = dedup_identity_records(vec![first, second]);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].multiplicity, 2);
    }

    #[test]
    fn cross_file_aliases_collapse_when_span_dropped() {
        // Cross-file aliases: no originating call site, same semantic fields,
        // different files. They collapse to one record with multiplicity 2.
        let first = record(IdentityKind::Callsite, 0, 10, None, "pkg", "pkg.Func");
        let second = record(IdentityKind::Callsite, 1, 20, None, "pkg", "pkg.Func");
        let output = dedup_identity_records(vec![first, second]);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].multiplicity, 2);
    }

    #[test]
    fn distinct_semantic_fields_do_not_collapse() {
        let first = record(IdentityKind::Callsite, 0, 10, Some(1), "pkg", "pkg.Func");
        let second = record(IdentityKind::Callsite, 0, 10, Some(2), "pkg", "pkg.Other");
        let output = dedup_identity_records(vec![first, second]);
        assert_eq!(output.len(), 2);
        assert!(output.iter().all(|record| record.multiplicity == 1));
    }

    #[test]
    fn dedup_only_changes_multiplicity_on_duplicate_hit() {
        let canonical = record(IdentityKind::Callsite, 0, 10, Some(1), "pkg", "pkg.Func");
        let duplicate = record(IdentityKind::Callsite, 0, 10, Some(1), "pkg", "pkg.Func");
        let output = dedup_identity_records(vec![canonical.clone(), duplicate]);
        assert_eq!(output.len(), 1);
        let expected = canonical.clone_with_multiplicity(2);
        assert_eq!(output[0], expected);
    }

    #[test]
    fn identity_dedup_fixture_manifest_asserts_multiplicity_two() {
        // Keeps the dedup snapshot fixture live: the manifest must parse and
        // assert multiplicity = 2 on the collapsed dedup group (D-10, D-11).
        let manifest = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/eval-fixtures/identity/dedup/expected.polint-eval.toml"
        ))
        .expect("dedup fixture manifest should exist");
        assert!(manifest.contains("multiplicity"));

        // The semantic-collapse contract: two semantically-identical records
        // (cross-file aliases with the span dropped) collapse to one record with
        // multiplicity = 2, byte-stable across input order (D-10, D-11).
        let first = record(
            IdentityKind::Callsite,
            0,
            100,
            None,
            "src/main.go",
            "helper",
        );
        let second = record(
            IdentityKind::Callsite,
            0,
            200,
            None,
            "src/main.go",
            "helper",
        );
        let forward = dedup_identity_records(vec![first.clone(), second.clone()]);
        let reverse = dedup_identity_records(vec![second, first]);
        assert_eq!(forward.len(), 1);
        assert_eq!(forward[0].multiplicity, 2);
        assert_eq!(forward, reverse);
    }

    #[test]
    fn identity_dedup_fixture_determinism_is_byte_stable_across_orders() {
        // Byte-stability across run order, file order, and provider order: the
        // serialized dedup output must be identical regardless of input order
        // (D-11). This is the determinism contract Phase 43's gate inherits.
        let records = vec![
            record(IdentityKind::Function, 1, 10, None, "src/main.go", "main"),
            record(IdentityKind::Function, 0, 20, None, "src/util.go", "helper"),
            record(IdentityKind::Callsite, 0, 30, None, "src/util.go", "helper"),
            record(IdentityKind::Callsite, 1, 40, None, "src/util.go", "helper"),
        ];
        let mut reversed = records.clone();
        reversed.reverse();

        let forward = dedup_identity_records(records);
        let backward = dedup_identity_records(reversed);
        let forward_json = serde_json::to_string(&forward).expect("serialize");
        let backward_json = serde_json::to_string(&backward).expect("serialize");
        assert_eq!(forward_json, backward_json);
    }

    #[test]
    fn dedup_canonical_selection_is_total_order_on_call_site_id_tie() {
        // CR-03: two records that tie on EVERY record_sort_key field (and on the
        // dedup_key, so they collapse) but carry a different originating_call_site_id
        // must dedup to byte-identical output regardless of input order.
        // record_sort_key alone is not a total order here (it omits
        // originating_call_site_id), so without a full-tuple tie-break the canonical
        // retained record would be whichever arrived first — its
        // originating_call_site_id would flip between input orders. The total-order
        // tie-break makes canonical selection (and the final sort) order-independent,
        // which is what Phase 43's byte-stability gate depends on.
        let a = record(IdentityKind::Callsite, 0, 10, Some(1), "pkg", "pkg.Func");
        let b = record(IdentityKind::Callsite, 0, 10, Some(2), "pkg", "pkg.Func");
        let forward = serde_json::to_string(&dedup_identity_records(vec![a.clone(), b.clone()]))
            .expect("serialize forward");
        let reverse =
            serde_json::to_string(&dedup_identity_records(vec![b, a])).expect("serialize reverse");
        assert_eq!(forward, reverse);
    }

    #[test]
    fn sorted_output_is_stable_across_input_shuffling() {
        let a = record(IdentityKind::Function, 0, 10, None, "a-pkg", "a.Type");
        let b = record(IdentityKind::Function, 1, 20, None, "b-pkg", "b.Type");
        let c = record(IdentityKind::Function, 2, 30, None, "c-pkg", "c.Type");
        let forward = dedup_identity_records(vec![a.clone(), b.clone(), c.clone()]);
        let reverse = dedup_identity_records(vec![c, b, a]);
        let forward_keys = forward
            .iter()
            .map(|record| record.stable_key.clone())
            .collect::<Vec<_>>();
        let reverse_keys = reverse
            .iter()
            .map(|record| record.stable_key.clone())
            .collect::<Vec<_>>();
        assert_eq!(forward_keys, reverse_keys);
    }
}
