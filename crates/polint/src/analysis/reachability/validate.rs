use std::collections::BTreeSet;

use crate::analysis::reachability::facts::{ReachabilityRootFact, RootStatus};
use crate::analysis::reachability::store::REACHABILITY_PROVIDER_ID;
use crate::analysis_kernel::FactPrecision;
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

/// Validates the stored reachability facts, emitting a diagnostic per problem:
/// duplicate stable keys, dangling composed references, and invalid spans. The
/// precision-ceiling check (D-19) is enforced separately via
/// [`reject_exact_precision`] because reachability over direct calls is
/// setup-aware/conservative and must never be reported as `FactPrecision::Exact`.
pub(crate) fn validate_reachability(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    let files = db.files().iter().map(|row| row.id).collect::<BTreeSet<_>>();
    let functions = db
        .functions()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let symbols = db
        .symbols()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let entrypoints = db
        .entrypoint_facts()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();

    check_duplicate_stable_keys(
        diagnostics,
        "ReachabilityRoot",
        db.reachability_roots()
            .iter()
            .map(|row| row.stable_key.as_str()),
    );

    for root in db.reachability_roots() {
        validate_root(
            diagnostics,
            root,
            &functions,
            &symbols,
            &files,
            &entrypoints,
        );
    }
}

fn validate_root(
    diagnostics: &mut Vec<Diagnostic>,
    root: &ReachabilityRootFact,
    functions: &BTreeSet<crate::core::FunctionId>,
    symbols: &BTreeSet<crate::core::SymbolId>,
    files: &BTreeSet<crate::core::FileId>,
    entrypoints: &BTreeSet<crate::analysis::ids::EntrypointId>,
) {
    // IN-04: an unresolved configured root carries sentinel target/file ids by
    // design (it resolved to no real function/file). Skip the referential dangling
    // checks for it — the sentinels are intentional, not dangling references.
    // (These roots are filtered out before storage today; this keeps validation
    // correct if a later stage chooses to store them.)
    let is_unresolved = root.status == RootStatus::Unresolved;
    if !is_unresolved && !functions.contains(&root.target_function) {
        push_diagnostic(
            diagnostics,
            "ReachabilityRoot",
            &root.stable_key,
            "target_function",
            "dangling reachability root target function reference",
        );
    }
    if let Some(symbol) = root.target_symbol
        && !symbols.contains(&symbol)
    {
        push_diagnostic(
            diagnostics,
            "ReachabilityRoot",
            &root.stable_key,
            "target_symbol",
            "dangling reachability root target symbol reference",
        );
    }
    if let Some(entrypoint) = root.originating_entrypoint
        && !entrypoints.contains(&entrypoint)
    {
        push_diagnostic(
            diagnostics,
            "ReachabilityRoot",
            &root.stable_key,
            "originating_entrypoint",
            "dangling reachability root originating entrypoint reference",
        );
    }
    if !is_unresolved && !files.contains(&root.file) {
        push_diagnostic(
            diagnostics,
            "ReachabilityRoot",
            &root.stable_key,
            "file",
            "dangling reachability root file reference",
        );
    }
    if root.span.start_byte > root.span.end_byte {
        push_diagnostic(
            diagnostics,
            "ReachabilityRoot",
            &root.stable_key,
            "span",
            "invalid span byte range",
        );
    }
}

/// Precision-ceiling check for the `polint.reachability` producer (D-19).
///
/// Reachability-from-roots is setup-aware/conservative — the producer must never
/// claim [`FactPrecision::Exact`]. Returns a diagnostic when the ceiling is
/// exceeded so the caller can record it.
pub(crate) fn reject_exact_precision(
    precision: FactPrecision,
    stable_key: &str,
) -> Option<Diagnostic> {
    if precision == FactPrecision::Exact {
        Some(precision_ceiling_diagnostic(stable_key))
    } else {
        None
    }
}

fn check_duplicate_stable_keys<'a>(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    keys: impl Iterator<Item = &'a str>,
) {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            push_diagnostic(
                diagnostics,
                family,
                key,
                "stable_key",
                "duplicate stable key",
            );
        }
    }
}

fn push_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    diagnostics.push(
        Diagnostic::error(
            "polint/internal",
            "<workspace>",
            TextRange::point(1, 1),
            format!("Reachability validation failed for {family} stable key."),
        )
        .with_evidence("family", family)
        .with_evidence("stable_key", stable_key.to_string())
        .with_evidence("field", field)
        .with_evidence("reason", reason),
    );
}

fn precision_ceiling_diagnostic(stable_key: &str) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        format!("Reachability precision ceiling exceeded for {REACHABILITY_PROVIDER_ID}."),
    )
    .with_evidence("family", "ReachabilityRoot")
    .with_evidence("stable_key", stable_key.to_string())
    .with_evidence("field", "precision")
    .with_evidence(
        "reason",
        "precision ceiling exceeded: reachability facts must not claim Exact",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::ReachabilityRootId;
    use crate::analysis::reachability::facts::{
        RootKind, RootPrecision, RootProvenance, RootStatus, compute_reachability_root_stable_key,
    };
    use crate::analysis::reachability::store::ReachabilityProviderOutput;
    use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span};
    use std::path::PathBuf;

    fn span(file: FileId, start: u32, end: u32) -> Span {
        Span {
            file,
            start_byte: start,
            end_byte: end,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 1,
        }
    }

    fn root(target: FunctionId, file: FileId, span: Span) -> ReachabilityRootFact {
        ReachabilityRootFact {
            id: ReachabilityRootId(0),
            kind: RootKind::Main,
            language: Language::Go,
            target_function: target,
            target_symbol: None,
            originating_entrypoint: None,
            file,
            span: span.clone(),
            precision: RootPrecision::ResolvedStatic,
            provenance: RootProvenance::NativeDiscovery,
            status: RootStatus::Resolved,
            provider_id: REACHABILITY_PROVIDER_ID.to_string(),
            stable_key: compute_reachability_root_stable_key(
                RootKind::Main,
                Language::Go,
                "main.main",
                file,
                &span,
            ),
        }
    }

    fn db_with_function() -> (AnalysisDb, FileId, FunctionId) {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("cmd/app/main.go"),
            "cmd/app/main.go".to_string(),
            "package main\nfunc main() {}\n".to_string(),
        );
        let function = db.push_function(FunctionFact {
            id: FunctionId(1),
            file,
            name: "main".to_string(),
            span: span(file, 1, 2),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        (db, file, function)
    }

    #[test]
    fn valid_root_produces_no_diagnostics() {
        let (mut db, file, function) = db_with_function();
        db.replace_reachability_facts(ReachabilityProviderOutput {
            roots: vec![root(function, file, span(file, 1, 2))],
        })
        .expect("store");
        let mut diagnostics = Vec::new();
        validate_reachability(&db, &mut diagnostics);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn dangling_target_function_is_rejected() {
        let (mut db, file, _function) = db_with_function();
        // Bypass store referential validation by replacing via the unchecked path
        // is not available; instead point at a function id absent from db.
        let dangling = root(FunctionId(999), file, span(file, 1, 2));
        // The store would reject this, so validate against a hand-built db state:
        // push the root directly through the public replace path with a valid
        // function set, then assert validate flags the dangling reference by
        // pointing target at a function we never registered.
        let result = db.replace_reachability_facts(ReachabilityProviderOutput {
            roots: vec![dangling],
        });
        // from_output rejects the dangling target before storage.
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("dangling target function")
        );
    }

    #[test]
    fn invalid_span_is_flagged() {
        let (mut db, file, function) = db_with_function();
        // end_byte < start_byte
        let bad = root(function, file, span(file, 9, 1));
        db.replace_reachability_facts(ReachabilityProviderOutput { roots: vec![bad] })
            .expect("store accepts referential-valid root with bad span");
        let mut diagnostics = Vec::new();
        validate_reachability(&db, &mut diagnostics);
        assert!(
            diagnostics
                .iter()
                .any(|d| format!("{d:?}").contains("invalid span byte range"))
        );
    }

    #[test]
    fn precision_ceiling_rejects_exact() {
        let diagnostic = reject_exact_precision(
            FactPrecision::Exact,
            "reachability_root|main|go|main.main|0|1..2",
        );
        assert!(diagnostic.is_some());
        let rendered = format!("{:?}", diagnostic.unwrap());
        assert!(rendered.contains("must not claim Exact"));
    }

    #[test]
    fn precision_ceiling_allows_setup_aware() {
        assert!(reject_exact_precision(FactPrecision::SetupAware, "k").is_none());
        assert!(reject_exact_precision(FactPrecision::Heuristic, "k").is_none());
    }

    #[test]
    fn unresolved_configured_root_with_sentinel_ids_is_not_flagged_dangling() {
        // IN-04: an Unresolved configured root carries sentinel target/file ids by
        // design (FunctionId(u64::MAX) / FileId(u32::MAX)). validate_root must NOT
        // report those as dangling references — they are intentional, not real refs.
        // The SAME sentinel values on a Resolved root MUST still be flagged, proving
        // the skip is gated on RootStatus::Unresolved, not unconditional.
        let functions: BTreeSet<FunctionId> = BTreeSet::new();
        let symbols: BTreeSet<crate::core::SymbolId> = BTreeSet::new();
        let files: BTreeSet<FileId> = BTreeSet::new();
        let entrypoints: BTreeSet<crate::analysis::ids::EntrypointId> = BTreeSet::new();

        let sentinel_file = FileId(u32::MAX);
        let sentinel_span = span(sentinel_file, 0, 0);
        let mut root = ReachabilityRootFact {
            id: ReachabilityRootId(0),
            kind: RootKind::ConfiguredEntrypoint,
            language: Language::Unknown,
            target_function: FunctionId(u64::MAX),
            target_symbol: None,
            originating_entrypoint: None,
            file: sentinel_file,
            span: sentinel_span.clone(),
            precision: RootPrecision::Unknown,
            provenance: RootProvenance::Configured,
            status: RootStatus::Unresolved,
            provider_id: REACHABILITY_PROVIDER_ID.to_string(),
            stable_key: compute_reachability_root_stable_key(
                RootKind::ConfiguredEntrypoint,
                Language::Unknown,
                "configured:does/not.Resolve",
                sentinel_file,
                &sentinel_span,
            ),
        };

        let mut diagnostics = Vec::new();
        validate_root(
            &mut diagnostics,
            &root,
            &functions,
            &symbols,
            &files,
            &entrypoints,
        );
        assert!(
            diagnostics.is_empty(),
            "unresolved root sentinels must not be flagged: {diagnostics:?}"
        );

        root.status = RootStatus::Resolved;
        let mut diagnostics = Vec::new();
        validate_root(
            &mut diagnostics,
            &root,
            &functions,
            &symbols,
            &files,
            &entrypoints,
        );
        assert!(
            !diagnostics.is_empty(),
            "a Resolved root with sentinel target/file ids must still be flagged dangling"
        );
    }
}
