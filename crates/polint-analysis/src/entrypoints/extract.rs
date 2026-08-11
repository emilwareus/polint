use crate::entrypoints::dispatch::derive_dispatch_edges;
use crate::entrypoints::recognizers_go::recognize_go_entrypoints;
use crate::entrypoints::recognizers_ts::recognize_ts_entrypoints;
use crate::entrypoints::store::EntrypointOutput;
use crate::entrypoints::trust_boundaries::derive_trust_boundaries;
use crate::entrypoints::unresolved::merge_unresolved;
use crate::AnalysisHost;

/// Orchestrate entrypoint extraction by calling Go and TS/JS recognizers,
/// then deriving trust boundaries, dispatch edges, and merging unresolved facts.
pub fn extract_entrypoints(db: &impl AnalysisHost) -> EntrypointOutput {
    // 1. Run language-specific recognizers
    let go_output = recognize_go_entrypoints(db);
    let ts_output = recognize_ts_entrypoints(db);

    // 2. Merge entrypoint vectors into one combined list
    let mut entrypoints = go_output.entrypoints;
    entrypoints.extend(ts_output.entrypoints);

    // 3. Derive trust boundaries from recognized entrypoints
    let trust_boundaries = derive_trust_boundaries(db, &entrypoints);

    // 4. Derive dispatch edges from recognized entrypoints
    let dispatch_edges = derive_dispatch_edges(db, &entrypoints);

    // 5. Merge unresolved facts from both recognizers
    let unresolved = merge_unresolved(
        &db.stable_key_interner(),
        go_output.unresolved,
        ts_output.unresolved,
    );

    // 6. Return combined output
    EntrypointOutput {
        entrypoints,
        trust_boundaries,
        dispatch_edges,
        unresolved,
    }
}

#[cfg(test)]
mod tests {
    use crate::LocalAnalysisDb;
    use super::*;

    #[test]
    fn extract_entrypoints_produces_empty_output_for_empty_db() {
        let db = LocalAnalysisDb::new();
        let output = extract_entrypoints(&db);

        assert!(output.entrypoints.is_empty());
        assert!(output.trust_boundaries.is_empty());
        assert!(output.dispatch_edges.is_empty());
        assert!(output.unresolved.is_empty());
    }
}
