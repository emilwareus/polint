use std::collections::BTreeSet;

use crate::analysis_neutral::AnalysisHost;
use crate::analysis_neutral::identity::facts::SignatureDigest;
use crate::analysis_neutral::ids::{CallSiteId, CallTargetId};
use crate::internal_core::FileId;
use crate::internal_core::{Diagnostic, DiagnosticRange};

/// Validates identity records, pushing one diagnostic per malformed row
/// (Pattern J). Never panics.
pub fn validate_identity(db: &impl AnalysisHost, diagnostics: &mut Vec<Diagnostic>) {
    let files = db
        .files()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<FileId>>();
    let call_sites = db
        .call_sites()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<CallSiteId>>();
    let call_targets = db
        .call_targets()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<CallTargetId>>();

    check_duplicate_stable_keys(
        diagnostics,
        db.identity_records()
            .iter()
            .map(|row| db.resolve_stable_key(row.stable_key).to_string()),
    );

    for record in db.identity_records() {
        let stable_key = db.resolve_stable_key(record.stable_key);
        if !files.contains(&record.file_id) {
            push_diagnostic(
                diagnostics,
                &stable_key,
                "file_id",
                "dangling identity file reference",
            );
        }
        if let Some(site) = record.originating_call_site_id
            && !call_sites.contains(&site)
        {
            push_diagnostic(
                diagnostics,
                &stable_key,
                "originating_call_site_id",
                "dangling identity originating call site reference",
            );
        }
        if let Some(target) = record.originating_call_target_id
            && !call_targets.contains(&target)
        {
            push_diagnostic(
                diagnostics,
                &stable_key,
                "originating_call_target_id",
                "dangling identity originating call target reference",
            );
        }
        if record.span.start_byte > record.span.end_byte {
            push_diagnostic(diagnostics, &stable_key, "span", "invalid span byte range");
        }
        if record.signature_digest == SignatureDigest([0u8; 16]) {
            push_diagnostic(
                diagnostics,
                &stable_key,
                "signature_digest",
                "signature digest must not be all-zero",
            );
        }
    }
}

fn check_duplicate_stable_keys(
    diagnostics: &mut Vec<Diagnostic>,
    keys: impl Iterator<Item = String>,
) {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key.clone()) {
            push_diagnostic(diagnostics, &key, "stable_key", "duplicate stable key");
        }
    }
}

fn push_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    diagnostics.push(
        Diagnostic::error(
            "polint/internal",
            "<workspace>",
            DiagnosticRange::point(1, 1),
            "Identity validation failed for IdentityRecord stable key.".to_string(),
        )
        .with_evidence("family", "IdentityRecord")
        .with_evidence("stable_key", stable_key.to_string())
        .with_evidence("field", field)
        .with_evidence("reason", reason),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_api::FunctionFact;
    use crate::analysis_neutral::LocalAnalysisDb;
    use crate::analysis_neutral::identity::facts::{
        IdentityKind, IdentityRecord, IdentityRecordId, LanguageTag, compute_identity_stable_key,
        compute_signature_digest,
    };
    use crate::analysis_neutral::identity::store::IdentityProviderOutput;
    use crate::internal_core::{FunctionId, Language, Span};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn base_db() -> LocalAnalysisDb {
        let mut db = LocalAnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nfunc main() {}\n".to_string(),
        );
        db.push_function(FunctionFact::new(
            FunctionId::from_raw(0),
            file,
            "main".to_string(),
            Span::point(file, 2, 1),
            Language::Go,
            false,
            true,
            1,
            Vec::new(),
        ));
        db
    }

    fn record(file: u32, stable_key: &str, digest_zero: bool, bad_span: bool) -> IdentityRecord {
        let language = LanguageTag::Go;
        let span = if bad_span {
            Span::new(FileId::from_raw(file), 10, 1, 1, 11, 1, 2)
        } else {
            Span::point(FileId::from_raw(file), 1, 1)
        };
        let signature_digest = if digest_zero {
            SignatureDigest([0u8; 16])
        } else {
            compute_signature_digest(language, "pkg", "pkg.T", "T", None, None)
        };
        IdentityRecord {
            id: IdentityRecordId(0),
            kind: IdentityKind::Function,
            file_id: FileId::from_raw(file),
            span,
            language,
            package_or_module: Arc::from("pkg"),
            container_path: Arc::from("pkg.T"),
            display_name: Arc::from("T"),
            signature_digest,
            multiplicity: 1,
            stable_key: crate::internal_core::stable_key_for_test(stable_key),
            originating_call_site_id: None,
            originating_call_target_id: None,
        }
    }

    #[test]
    fn validate_identity_reports_malformed_rows_with_required_evidence() {
        let mut db = base_db();
        // Inject directly: these malformed rows are the defense-in-depth layer
        // and would be rejected at the store boundary, so they cannot arrive via
        // replace_identity_facts.
        db.set_identity_records_for_test(vec![
            record(0, "identity:dup", false, false),
            record(0, "identity:dup", false, false),
            record(99, "identity:dangling-file", false, false),
            record(0, "identity:bad-span", false, true),
            record(0, "identity:zero-digest", true, false),
        ]);

        let mut diagnostics = Vec::new();
        validate_identity(&db, &mut diagnostics);

        let reasons = diagnostics
            .iter()
            .flat_map(|diagnostic| diagnostic.evidence.iter())
            .filter(|evidence| evidence.label == "reason")
            .map(|evidence| evidence.value.clone())
            .collect::<Vec<_>>();

        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("duplicate stable key"))
        );
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("dangling identity file reference"))
        );
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("invalid span byte range"))
        );
        assert!(reasons.iter().any(|reason| reason.contains("all-zero")));
        assert!(diagnostics.iter().all(|diagnostic| {
            let labels = diagnostic
                .evidence
                .iter()
                .map(|evidence| evidence.label.as_str())
                .collect::<BTreeSet<_>>();
            labels.contains("family")
                && labels.contains("stable_key")
                && labels.contains("field")
                && labels.contains("reason")
        }));
    }

    #[test]
    fn validate_identity_reports_dangling_call_site_reference() {
        let mut db = base_db();
        let file = db.files()[0].id;
        let mut dangling = record(file.0, "identity:dangling-site", false, false);
        dangling.kind = IdentityKind::Callsite;
        dangling.originating_call_site_id = Some(crate::analysis_neutral::ids::CallSiteId(42));
        dangling.stable_key =
            crate::internal_core::stable_key_for_test(&compute_identity_stable_key(
                IdentityKind::Callsite,
                LanguageTag::Go,
                "pkg",
                "pkg.T",
                file,
                &dangling.span,
            ));
        db.set_identity_records_for_test(vec![dangling]);

        let mut diagnostics = Vec::new();
        validate_identity(&db, &mut diagnostics);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.evidence.iter().any(|evidence| {
                evidence.label == "reason"
                    && evidence
                        .value
                        .contains("dangling identity originating call site reference")
            })
        }));
    }

    #[test]
    fn validate_identity_is_clean_for_well_formed_rows() {
        let mut db = base_db();
        let file = db.files()[0].id;
        let span = Span::point(file, 2, 1);
        let stable_key = crate::internal_core::stable_key_for_test(&compute_identity_stable_key(
            IdentityKind::Function,
            LanguageTag::Go,
            "src/main.go",
            "main",
            file,
            &span,
        ));
        db.replace_identity_facts(IdentityProviderOutput {
            records: vec![IdentityRecord {
                id: IdentityRecordId(0),
                kind: IdentityKind::Function,
                file_id: file,
                span,
                language: LanguageTag::Go,
                package_or_module: Arc::from("src/main.go"),
                container_path: Arc::from("main"),
                display_name: Arc::from("main"),
                signature_digest: compute_signature_digest(
                    LanguageTag::Go,
                    "src/main.go",
                    "main",
                    "main",
                    None,
                    None,
                ),
                multiplicity: 1,
                stable_key,
                originating_call_site_id: None,
                originating_call_target_id: None,
            }],
        })
        .expect("identity rows should store");

        let mut diagnostics = Vec::new();
        validate_identity(&db, &mut diagnostics);
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }
}
