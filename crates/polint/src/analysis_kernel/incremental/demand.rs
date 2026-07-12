#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "demand query vocabulary is retained for private query consumers"
    )
)]

use std::collections::BTreeMap;
use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use super::digest::{Digest, DigestBuilder, DigestKind};
use super::keys::{PrecisionTier, QueryKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DemandCacheStatus {
    Computed,
    Hit,
    Miss,
}

impl DemandCacheStatus {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Computed => "computed",
            Self::Hit => "hit",
            Self::Miss => "miss",
        }
    }

    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownDemandCacheStatusLabel> {
        match label {
            "computed" => Ok(Self::Computed),
            "hit" => Ok(Self::Hit),
            "miss" => Ok(Self::Miss),
            _ => Err(UnknownDemandCacheStatusLabel {
                label: label.to_string(),
            }),
        }
    }
}

impl<'de> Deserialize<'de> for DemandCacheStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let label = String::deserialize(deserializer)?;
        Self::parse_label(&label).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnknownDemandCacheStatusLabel {
    label: String,
}

impl fmt::Display for UnknownDemandCacheStatusLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown demand cache status label `{}`",
            self.label
        )
    }
}

impl std::error::Error for UnknownDemandCacheStatusLabel {}

// ---------------------------------------------------------------------------
// DemandQueryResult — result of a single demand query execution
// ---------------------------------------------------------------------------

/// A memoized result of a demand query, keyed by `QueryKey`.
///
/// The engine stores one `DemandQueryResult` per unique `QueryKey` during a
/// kernel run. The result carries the output digest, precision tier,
/// provenance, and whether it was loaded from cache or computed fresh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct DemandQueryResult {
    pub(crate) query_key: QueryKey,
    pub(crate) output_digest: Digest,
    pub(crate) precision_tier: PrecisionTier,
    pub(crate) provenance: String,
    pub(crate) was_cached: bool,
}

// ---------------------------------------------------------------------------
// DemandQueryTraceEntry — one row of demand query debug trace
// ---------------------------------------------------------------------------

/// A single entry in the demand query trace for debug output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct DemandQueryTraceEntry {
    pub(crate) query_key: QueryKey,
    pub(crate) result_digest: Digest,
    pub(crate) precision_tier: PrecisionTier,
    pub(crate) provenance: String,
    pub(crate) cache_status: DemandCacheStatus,
    pub(crate) compute_duration_micros: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(crate) struct DemandQuerySemanticProjection<'a> {
    pub(crate) query_key: &'a QueryKey,
    pub(crate) result_digest: &'a Digest,
    pub(crate) precision_tier: PrecisionTier,
    pub(crate) provenance: &'a str,
}

// ---------------------------------------------------------------------------
// DemandQueryTrace — collected trace for a kernel run's demand queries
// ---------------------------------------------------------------------------

/// Accumulated trace of demand query executions during a single kernel run.
///
/// Wraps a `Vec<DemandQueryTraceEntry>` and keeps semantic query identity
/// separate from execution telemetry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct DemandQueryTrace {
    entries: Vec<DemandQueryTraceEntry>,
}

impl DemandQueryTrace {
    /// Records a trace entry.
    pub(crate) fn record_entry(&mut self, entry: DemandQueryTraceEntry) {
        self.entries.push(entry);
    }

    /// Returns all trace entries.
    pub(crate) fn entries(&self) -> &[DemandQueryTraceEntry] {
        &self.entries
    }

    pub(crate) fn semantic_projections(&self) -> Vec<DemandQuerySemanticProjection<'_>> {
        let mut projections = self
            .entries
            .iter()
            .map(|entry| DemandQuerySemanticProjection {
                query_key: &entry.query_key,
                result_digest: &entry.result_digest,
                precision_tier: entry.precision_tier,
                provenance: &entry.provenance,
            })
            .collect::<Vec<_>>();
        projections.sort();
        projections.dedup();
        projections
    }

    pub(crate) fn semantic_digest(&self) -> Digest {
        let mut builder = Digest::builder(DigestKind::Query, "demand_query_semantics");
        for projection in self.semantic_projections() {
            let key = projection.query_key;
            builder.labeled_part("query_kind", &key.query_kind);
            builder.labeled_part("query_version", &key.query_version);
            append_digest(
                &mut builder,
                "parameter_digest_kind",
                "parameter_digest_value",
                &key.parameter_digest,
            );
            for layer_digest in &key.layer_digests {
                append_digest(
                    &mut builder,
                    "layer_digest_kind",
                    "layer_digest_value",
                    layer_digest,
                );
            }
            append_digest(
                &mut builder,
                "budget_digest_kind",
                "budget_digest_value",
                &key.budget_digest,
            );
            builder.labeled_part("query_precision", key.precision_tier.label());
            append_digest(
                &mut builder,
                "result_digest_kind",
                "result_digest_value",
                projection.result_digest,
            );
            builder.labeled_part("result_precision", projection.precision_tier.label());
            builder.labeled_part("provenance", projection.provenance);
        }
        builder.finish()
    }

    /// Returns the number of entries.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the trace is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// DemandQueryEngine — in-run memoization and trace recording
// ---------------------------------------------------------------------------

/// Engine for demand query memoization within a single kernel run.
///
/// Maintains a `BTreeMap<QueryKey, DemandQueryResult>` for run-scoped dedup
/// and records trace entries for debug output. Cross-run persistence remains
/// a separate layer-cache concern.
#[derive(Debug, Default)]
pub(crate) struct DemandQueryEngine {
    memo: BTreeMap<QueryKey, DemandQueryResult>,
    trace: DemandQueryTrace,
}

impl DemandQueryEngine {
    /// Looks up a memoized result for the given query key.
    pub(crate) fn lookup(&self, key: &QueryKey) -> Option<&DemandQueryResult> {
        self.memo.get(key)
    }

    /// Stores a demand query result in the memo and records a trace entry
    /// with a computed cache status.
    pub(crate) fn insert(&mut self, result: DemandQueryResult) {
        let trace_entry = DemandQueryTraceEntry {
            query_key: result.query_key.clone(),
            result_digest: result.output_digest.clone(),
            precision_tier: result.precision_tier,
            provenance: result.provenance.clone(),
            cache_status: DemandCacheStatus::Computed,
            compute_duration_micros: 0,
        };
        self.trace.record_entry(trace_entry);
        self.memo.insert(result.query_key.clone(), result);
    }

    /// Records a trace entry for a cache hit without modifying the memo.
    pub(crate) fn record_cache_hit(
        &mut self,
        key: &QueryKey,
        result: &DemandQueryResult,
        duration_micros: u64,
    ) {
        let trace_entry = DemandQueryTraceEntry {
            query_key: key.clone(),
            result_digest: result.output_digest.clone(),
            precision_tier: result.precision_tier,
            provenance: result.provenance.clone(),
            cache_status: DemandCacheStatus::Hit,
            compute_duration_micros: duration_micros,
        };
        self.trace.record_entry(trace_entry);
    }

    /// Consumes the engine, returning the accumulated trace.
    pub(crate) fn into_trace(self) -> DemandQueryTrace {
        self.trace
    }

    /// Borrows the accumulated trace.
    pub(crate) fn trace(&self) -> &DemandQueryTrace {
        &self.trace
    }
}

fn append_digest(
    builder: &mut DigestBuilder,
    kind_label: &str,
    value_label: &str,
    digest: &Digest,
) {
    builder.labeled_part(kind_label, digest.kind.label());
    builder.labeled_part(value_label, &digest.value);
}

#[cfg(test)]
pub(crate) fn dependency_free_test_query_key(
    query_kind: impl Into<String>,
    query_version: impl Into<String>,
    parameter_digest: Digest,
    budget_digest: Digest,
    precision_tier: PrecisionTier,
) -> QueryKey {
    QueryKey::new(
        query_kind,
        query_version,
        parameter_digest,
        Vec::new(),
        budget_digest,
        precision_tier,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::dependency_free_test_query_key;
    use crate::analysis_kernel::incremental::digest::{Digest, DigestKind};

    fn test_query_key(kind: &str) -> QueryKey {
        dependency_free_test_query_key(
            kind,
            "1",
            Digest::from_parts(DigestKind::ProviderParameters, "test_params", &["param_a"]),
            Digest::from_parts(DigestKind::ProviderParameters, "budget", &["default"]),
            PrecisionTier::SetupAware,
        )
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
        let first = test_result("function_summary", "out_1");
        let first_key = first.query_key.clone();
        let first_digest = first.output_digest.clone();

        engine.insert(first);
        engine.insert(test_result("function_cfg", "out_2"));

        let entries = engine.trace().entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].query_key, first_key);
        assert_eq!(entries[0].result_digest, first_digest);
        assert_eq!(entries[0].precision_tier, PrecisionTier::SetupAware);
        assert_eq!(entries[0].provenance, "native");
        assert_eq!(entries[1].query_key.query_kind, "function_cfg");

        assert_eq!(entries[0].cache_status, DemandCacheStatus::Computed);
        assert_eq!(entries[1].cache_status, DemandCacheStatus::Computed);
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
        assert_eq!(entries[0].cache_status, DemandCacheStatus::Hit);
        assert_eq!(entries[0].compute_duration_micros, 42);
    }

    #[test]
    fn default_engine_has_empty_trace() {
        let engine = DemandQueryEngine::default();
        assert!(engine.trace().is_empty());
        assert_eq!(engine.trace().len(), 0);
    }

    #[test]
    fn demand_cache_status_labels_round_trip_and_reject_unknown_values() {
        for status in [
            DemandCacheStatus::Computed,
            DemandCacheStatus::Hit,
            DemandCacheStatus::Miss,
        ] {
            assert_eq!(DemandCacheStatus::parse_label(status.label()), Ok(status));
            assert_eq!(
                serde_json::to_string(&status).expect("status serializes"),
                format!("\"{}\"", status.label())
            );
        }

        assert!(DemandCacheStatus::parse_label("cached").is_err());
        assert!(serde_json::from_str::<DemandCacheStatus>("\"cached\"").is_err());
    }

    #[test]
    fn status_and_duration_mutations_preserve_query_semantics() {
        let mut trace = DemandQueryTrace::default();
        trace.record_entry(trace_entry("function_summary", "result"));
        let expected_rows = trace.semantic_projections();
        let expected_digest = trace.semantic_digest();

        for status in [
            DemandCacheStatus::Computed,
            DemandCacheStatus::Hit,
            DemandCacheStatus::Miss,
        ] {
            let mut changed = trace.clone();
            changed.entries[0].cache_status = status;
            changed.entries[0].compute_duration_micros += 1_000;
            assert_eq!(changed.semantic_projections(), expected_rows);
            assert_eq!(changed.semantic_digest(), expected_digest);
        }
    }

    #[test]
    fn result_key_precision_and_provenance_mutations_change_query_semantics() {
        let mut trace = DemandQueryTrace::default();
        trace.record_entry(trace_entry("function_summary", "result"));
        let expected_rows = trace.semantic_projections();
        let expected_digest = trace.semantic_digest();

        let mut result_changed = trace.clone();
        result_changed.entries[0].result_digest =
            Digest::from_parts(DigestKind::ProviderOutput, "result", &["changed"]);
        assert_ne!(result_changed.semantic_projections(), expected_rows);
        assert_ne!(result_changed.semantic_digest(), expected_digest);

        let mut key_changed = trace.clone();
        key_changed.entries[0].query_key = test_query_key("function_cfg");
        assert_ne!(key_changed.semantic_projections(), expected_rows);
        assert_ne!(key_changed.semantic_digest(), expected_digest);

        let mut precision_changed = trace.clone();
        precision_changed.entries[0].precision_tier = PrecisionTier::Exact;
        assert_ne!(precision_changed.semantic_projections(), expected_rows);
        assert_ne!(precision_changed.semantic_digest(), expected_digest);

        let mut provenance_changed = trace.clone();
        provenance_changed.entries[0].provenance = "extension".to_string();
        assert_ne!(provenance_changed.semantic_projections(), expected_rows);
        assert_ne!(provenance_changed.semantic_digest(), expected_digest);
    }

    #[test]
    fn query_semantic_projection_is_sorted_deduplicated_and_telemetry_free() {
        let first = trace_entry("function_summary", "one");
        let second = trace_entry("function_cfg", "two");
        let mut trace = DemandQueryTrace::default();
        trace.record_entry(first.clone());
        trace.record_entry(second);
        trace.record_entry(first);

        let rows = trace.semantic_projections();
        assert_eq!(rows.len(), 2);
        assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));

        let source = include_str!("demand.rs");
        let semantic_projection = source
            .split_once("pub(crate) fn semantic_projections")
            .expect("semantic query projection exists")
            .1
            .split_once("pub(crate) fn len")
            .expect("semantic query projection has a bounded source section")
            .0;
        for forbidden in [
            "cache_status",
            "was_cached",
            "compute_duration_micros",
            "duration",
            "timestamp",
            "hits",
            "misses",
            "writes",
            "bypasses",
        ] {
            assert!(
                !semantic_projection.contains(forbidden),
                "query semantic projection must exclude `{forbidden}`"
            );
        }
    }

    fn trace_entry(query_kind: &str, result_label: &str) -> DemandQueryTraceEntry {
        DemandQueryTraceEntry {
            query_key: test_query_key(query_kind),
            result_digest: Digest::from_parts(
                DigestKind::ProviderOutput,
                "result",
                &[result_label],
            ),
            precision_tier: PrecisionTier::SetupAware,
            provenance: "native".to_string(),
            cache_status: DemandCacheStatus::Computed,
            compute_duration_micros: 42,
        }
    }
}
