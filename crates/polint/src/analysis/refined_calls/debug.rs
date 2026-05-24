use serde_json::json;

use crate::core::AnalysisDb;

#[cfg(test)]
#[allow(
    dead_code,
    reason = "Refined-call debug snapshots are wired into provider debug output in a later Phase 37 slice."
)]
pub(crate) fn refined_calls_debug_json_for_test(db: &AnalysisDb) -> serde_json::Value {
    json!({
        "edge_count": db.refined_call_edges().len(),
    })
}
