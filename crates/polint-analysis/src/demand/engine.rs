use std::collections::BTreeMap;

use serde::Serialize;

use polint_analysis_api::{Digest, PrecisionTier, QueryKey};

// ---------------------------------------------------------------------------
// DemandQueryResult — result of a single demand query execution
// ---------------------------------------------------------------------------

/// A memoized result of a demand query, keyed by `QueryKey`.
///
/// The engine stores one `DemandQueryResult` per unique `QueryKey` during a
/// kernel run. The result carries the output digest, precision tier,
/// provenance, and whether it was loaded from cache vs computed fresh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DemandQueryResult {
    pub query_key: QueryKey,
    pub output_digest: Digest,
    pub precision_tier: PrecisionTier,
    pub provenance: String,
    pub was_cached: bool,
}

// ---------------------------------------------------------------------------
// DemandQueryTraceEntry — one row of demand query debug trace
// ---------------------------------------------------------------------------

/// A single entry in the demand query trace for debug output.
///
/// Per D-13, records query kind, precision tier, input layer digests, cache
/// hit/miss, compute time, and result digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DemandQueryTraceEntry {
    pub query_kind: String,
    pub query_version: String,
    pub parameter_digest: String,
    pub input_layer_digests: Vec<String>,
    pub cache_status: String,
    pub compute_duration_micros: u64,
    pub result_digest: String,
    pub precision_tier: String,
}

// ---------------------------------------------------------------------------
// DemandQueryTrace — collected trace for a kernel run's demand queries
// ---------------------------------------------------------------------------

/// Accumulated trace of demand query executions during a single kernel run.
///
/// Wraps a `Vec<DemandQueryTraceEntry>`. Crate-private and test-facing per
/// D-13.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct DemandQueryTrace {
    entries: Vec<DemandQueryTraceEntry>,
}

impl DemandQueryTrace {
    /// Records a trace entry.
    pub fn record_entry(&mut self, entry: DemandQueryTraceEntry) {
        self.entries.push(entry);
    }

    /// Returns all trace entries.
    pub fn entries(&self) -> &[DemandQueryTraceEntry] {
        &self.entries
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the trace is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// DemandQueryEngine — in-run memoization and trace recording
// ---------------------------------------------------------------------------

/// Engine for demand query memoization within a single kernel run.
///
/// Maintains a `BTreeMap<QueryKey, DemandQueryResult>` for in-run dedup and
/// records trace entries for debug output. Per D-02, this is the in-run
/// memoization mode; cross-run persistence is handled via the layer cache
/// and will be wired by Plan 04 (SCC closure).
#[derive(Debug, Default)]
pub struct DemandQueryEngine {
    memo: BTreeMap<QueryKey, DemandQueryResult>,
    trace: DemandQueryTrace,
}

impl DemandQueryEngine {
    /// Looks up a memoized result for the given query key.
    pub fn lookup(&self, key: &QueryKey) -> Option<&DemandQueryResult> {
        self.memo.get(key)
    }

    /// Stores a demand query result in the memo and records a trace entry
    /// with `cache_status = "computed"`.
    pub fn insert(&mut self, result: DemandQueryResult) {
        let trace_entry = DemandQueryTraceEntry {
            query_kind: result.query_key.query_kind.clone(),
            query_version: result.query_key.query_version.clone(),
            parameter_digest: result.query_key.parameter_digest.value.clone(),
            input_layer_digests: result
                .query_key
                .layer_digests
                .iter()
                .map(|d| d.value.clone())
                .collect(),
            cache_status: "computed".to_string(),
            compute_duration_micros: 0,
            result_digest: result.output_digest.value.clone(),
            precision_tier: format!("{:?}", result.precision_tier),
        };
        self.trace.record_entry(trace_entry);
        self.memo.insert(result.query_key.clone(), result);
    }

    /// Records a trace entry for a cache hit without modifying the memo.
    pub fn record_cache_hit(
        &mut self,
        key: &QueryKey,
        result: &DemandQueryResult,
        duration_micros: u64,
    ) {
        let trace_entry = DemandQueryTraceEntry {
            query_kind: key.query_kind.clone(),
            query_version: key.query_version.clone(),
            parameter_digest: key.parameter_digest.value.clone(),
            input_layer_digests: key.layer_digests.iter().map(|d| d.value.clone()).collect(),
            cache_status: "hit".to_string(),
            compute_duration_micros: duration_micros,
            result_digest: result.output_digest.value.clone(),
            precision_tier: format!("{:?}", result.precision_tier),
        };
        self.trace.record_entry(trace_entry);
    }

    /// Consumes the engine, returning the accumulated trace.
    pub fn into_trace(self) -> DemandQueryTrace {
        self.trace
    }

    /// Borrows the accumulated trace.
    pub fn trace(&self) -> &DemandQueryTrace {
        &self.trace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polint_analysis_api::{Digest, DigestKind};

    fn test_query_key(kind: &str) -> QueryKey {
        QueryKey {
            query_kind: kind.to_string(),
            query_version: "1".to_string(),
            parameter_digest: Digest::from_parts(
                DigestKind::ProviderParameters,
                "test_params",
                &["param_a"],
            ),
            layer_digests: vec![Digest::from_parts(
                DigestKind::ProviderOutput,
                "layer",
                &["layer_a"],
            )],
            budget_digest: Digest::from_parts(
                DigestKind::ProviderParameters,
                "budget",
                &["default"],
            ),
            precision_tier: PrecisionTier::SetupAware,
        }
    }

    fn test_result(kind: &str, digest_label: &str) -> DemandQueryResult {
        DemandQueryResult {
            query_key: test_query_key(kind),
            output_digest: Digest::from_parts(
                DigestKind::ProviderOutput,
                "result",
                &[digest_label],
            ),
            precision_tier: PrecisionTier::SetupAware,
            provenance: "native".to_string(),
            was_cached: false,
        }
    }

    #[test]
    fn insert_then_lookup_returns_the_result() {
        let mut engine = DemandQueryEngine::default();
        let result = test_result("function_summary", "output_a");
        let key = result.query_key.clone();

        engine.insert(result.clone());

        let looked_up = engine.lookup(&key).expect("should find memoized result");
        assert_eq!(looked_up, &result);
    }

    #[test]
    fn lookup_for_absent_key_returns_none() {
        let engine = DemandQueryEngine::default();
        let key = test_query_key("function_cfg");

        assert!(engine.lookup(&key).is_none());
    }

    #[test]
    fn trace_records_entries_in_insertion_order() {
        let mut engine = DemandQueryEngine::default();

        engine.insert(test_result("function_summary", "out_1"));
        engine.insert(test_result("function_cfg", "out_2"));

        let entries = engine.trace().entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].query_kind, "function_summary");
        assert_eq!(entries[1].query_kind, "function_cfg");

        // Both should be "computed" since they were inserted, not cache hits
        assert_eq!(entries[0].cache_status, "computed");
        assert_eq!(entries[1].cache_status, "computed");
    }

    #[test]
    fn into_trace_is_not_empty_after_inserts() {
        let mut engine = DemandQueryEngine::default();

        engine.insert(test_result("function_summary", "out_a"));
        engine.insert(test_result("direct_call_target", "out_b"));

        let trace = engine.into_trace();
        assert!(!trace.is_empty());
        assert_eq!(trace.len(), 2);
    }

    #[test]
    fn record_cache_hit_adds_trace_entry_with_hit_status() {
        let mut engine = DemandQueryEngine::default();
        let result = test_result("function_summary", "out_cached");
        let key = result.query_key.clone();

        engine.record_cache_hit(&key, &result, 42);

        let entries = engine.trace().entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].cache_status, "hit");
        assert_eq!(entries[0].compute_duration_micros, 42);
    }

    #[test]
    fn default_engine_has_empty_trace() {
        let engine = DemandQueryEngine::default();
        assert!(engine.trace().is_empty());
        assert_eq!(engine.trace().len(), 0);
    }
}
