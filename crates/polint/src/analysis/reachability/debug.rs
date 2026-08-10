#![cfg(test)]

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::analysis::reachability::facts::{
    ReachabilityRootFact, RootKind, RootPrecision, RootProvenance, RootStatus,
};
use crate::core::{AnalysisDb, FileId};

/// Produces a debug JSON snapshot of all reachability facts in the db.
///
/// Rows use relative paths, stable keys, and label strings — never absolute
/// paths, raw source, or run-local dense IDs as identity. Used by the Plan 02
/// eval observation and the Plan 03 determinism gate.
pub(crate) fn metadata_debug_json_for_test(db: &AnalysisDb) -> Value {
    let report = ReachabilityDebugReport {
        counts: reachability_counts(db),
        roots: root_detail_rows(db),
    };
    serde_json::to_value(report).expect("reachability debug report should serialize")
}

#[derive(Serialize)]
struct ReachabilityDebugReport {
    counts: ReachabilityDebugCounts,
    roots: Vec<RootDetailRow>,
}

#[derive(Default, Serialize)]
struct ReachabilityDebugCounts {
    total_roots: usize,
    by_kind: BTreeMap<String, usize>,
    by_status: BTreeMap<String, usize>,
    by_precision: BTreeMap<String, usize>,
    by_provenance: BTreeMap<String, usize>,
}

#[derive(Serialize)]
struct RootDetailRow {
    relative_path: String,
    kind: String,
    status: String,
    precision: String,
    provenance: String,
    stable_key_text: String,
}

fn reachability_counts(db: &AnalysisDb) -> ReachabilityDebugCounts {
    let mut counts = ReachabilityDebugCounts {
        total_roots: db.reachability_roots().len(),
        ..Default::default()
    };
    for root in db.reachability_roots() {
        increment(&mut counts.by_kind, kind_label(root.kind));
        increment(&mut counts.by_status, status_label(root.status));
        increment(&mut counts.by_precision, precision_label(root.precision));
        increment(&mut counts.by_provenance, provenance_label(root.provenance));
    }
    counts
}

fn root_detail_rows(db: &AnalysisDb) -> Vec<RootDetailRow> {
    let mut rows: Vec<RootDetailRow> = db
        .reachability_roots()
        .iter()
        .map(|root: &ReachabilityRootFact| RootDetailRow {
            relative_path: relative_path_for(db, root.file),
            kind: kind_label(root.kind).to_string(),
            status: status_label(root.status).to_string(),
            precision: precision_label(root.precision).to_string(),
            provenance: provenance_label(root.provenance).to_string(),
            stable_key_text: db.resolve_stable_key(root.stable_key).to_string(),
        })
        .collect();
    rows.sort_by(|a, b| a.stable_key_text.cmp(&b.stable_key_text));
    rows
}

fn relative_path_for(db: &AnalysisDb, file: FileId) -> String {
    db.file(file)
        .map(|f| f.relative_path.clone())
        .unwrap_or_else(|| "<unknown-file>".to_string())
}

fn increment(counts: &mut BTreeMap<String, usize>, key: &str) {
    *counts.entry(key.to_string()).or_default() += 1;
}

fn kind_label(kind: RootKind) -> &'static str {
    kind.as_str()
}

fn status_label(status: RootStatus) -> &'static str {
    status.as_str()
}

fn precision_label(precision: RootPrecision) -> &'static str {
    precision.as_str()
}

fn provenance_label(provenance: RootProvenance) -> &'static str {
    provenance.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::ReachabilityRootId;
    use crate::analysis::reachability::facts::{
        RootKind, RootPrecision, RootProvenance, RootStatus, compute_reachability_root_stable_key,
    };
    use crate::analysis::reachability::store::{
        REACHABILITY_PROVIDER_ID, ReachabilityProviderOutput,
    };
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

    fn db_with_root() -> AnalysisDb {
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
        let root = ReachabilityRootFact {
            id: ReachabilityRootId(0),
            kind: RootKind::Main,
            language: Language::Go,
            target_function: function,
            target_symbol: None,
            originating_entrypoint: None,
            file,
            span: span(file, 1, 2),
            precision: RootPrecision::ResolvedStatic,
            provenance: RootProvenance::NativeDiscovery,
            status: RootStatus::Resolved,
            provider_id: REACHABILITY_PROVIDER_ID.to_string(),
            stable_key: crate::core::stable_key_for_test(&compute_reachability_root_stable_key(
                RootKind::Main,
                Language::Go,
                "main.main",
                file,
                &span(file, 1, 2),
            )),
        };
        db.replace_reachability_facts(ReachabilityProviderOutput { roots: vec![root] })
            .expect("store root");
        db
    }

    #[test]
    fn empty_db_produces_zero_counts() {
        let db = AnalysisDb::new();
        let report = metadata_debug_json_for_test(&db);
        assert_eq!(report["counts"]["total_roots"], 0);
        assert!(report["roots"].as_array().unwrap().is_empty());
    }

    #[test]
    fn populated_db_renders_relative_path_and_labels() {
        let db = db_with_root();
        let report = metadata_debug_json_for_test(&db);
        assert_eq!(report["counts"]["total_roots"], 1);
        assert_eq!(report["counts"]["by_kind"]["main"], 1);
        assert_eq!(report["counts"]["by_status"]["resolved"], 1);
        assert_eq!(report["counts"]["by_precision"]["resolved_static"], 1);
        assert_eq!(report["counts"]["by_provenance"]["native_discovery"], 1);

        let roots = report["roots"].as_array().unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0]["relative_path"], "cmd/app/main.go");
        assert_eq!(roots[0]["kind"], "main");
    }

    #[test]
    fn debug_output_avoids_absolute_paths_and_dense_ids() {
        let db = db_with_root();
        let report = metadata_debug_json_for_test(&db);
        let serialized = serde_json::to_string_pretty(&report).unwrap();
        assert!(!serialized.contains("/Users/"));
        assert!(!serialized.contains("\\Users\\"));
        assert!(!serialized.contains("\"id\":"));
    }

    #[test]
    fn debug_output_is_deterministic() {
        let db = db_with_root();
        assert_eq!(
            metadata_debug_json_for_test(&db),
            metadata_debug_json_for_test(&db)
        );
    }
}
