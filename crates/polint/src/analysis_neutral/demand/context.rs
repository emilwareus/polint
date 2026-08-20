use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::query::{QueryBudget, QueryKind, QueryStatus};
use super::trace::QueryTraceEntry;
use crate::analysis_api::{Digest, DigestKind, LayerKind};

// ---------------------------------------------------------------------------
// DependencyRead — a recorded read from the query context
// ---------------------------------------------------------------------------

/// Records a single dependency read made during query execution.
///
/// The query context tracks these reads so the invalidation planner can
/// determine which cached query results need recomputation when upstream
/// inputs change.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DependencyRead {
    pub kind: DependencyReadKind,
    pub key: String,
    pub digest: Digest,
}

/// The kind of upstream dependency that was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DependencyReadKind {
    /// Read a layer cache entry.
    Layer(LayerKind),
    /// Read a demand query result.
    Query(QueryKind),
    /// Read a summary result for a callable.
    Summary,
    /// Read an extension input.
    Extension,
}

// ---------------------------------------------------------------------------
// QueryContext — dependency-tracking context for demand queries
// ---------------------------------------------------------------------------

/// Context passed to demand query execution that tracks dependency reads
/// and enforces budget limits.
///
/// This is the central mechanism for demand-driven invalidation: when a
/// cached query result is checked, the recorded dependencies are compared
/// against current input digests to determine if the result is still valid.
#[derive(Debug, Clone)]
pub struct QueryContext {
    /// The query kind being executed.
    query_kind: QueryKind,
    /// Active resource budget.
    budget: QueryBudget,
    /// Current recursion depth.
    depth: u32,
    /// Reads recorded during execution.
    reads: Vec<DependencyRead>,
    /// Trace entries for debug output.
    trace_entries: Vec<QueryTraceEntry>,
    /// Whether trace recording is enabled.
    trace_enabled: bool,
    /// Per-query memoization table for in-run deduplication.
    memo_table: BTreeMap<MemoKey, MemoEntry>,
}

/// An in-run memoized query result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoEntry {
    pub output_digest: Digest,
    pub status: QueryStatus,
}

/// Key for in-run memoized results.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoKey {
    pub query_kind: QueryKind,
    pub key: String,
}

impl MemoKey {
    pub fn new(query_kind: QueryKind, key: impl Into<String>) -> Self {
        Self {
            query_kind,
            key: key.into(),
        }
    }
}

impl QueryContext {
    /// Creates a new query context for a demand query.
    pub fn new(query_kind: QueryKind, budget: QueryBudget, trace_enabled: bool) -> Self {
        Self {
            query_kind,
            budget,
            depth: 0,
            reads: Vec::new(),
            trace_entries: Vec::new(),
            trace_enabled,
            memo_table: BTreeMap::new(),
        }
    }

    /// Records a layer dependency read.
    pub fn read_layer(&mut self, layer_kind: LayerKind, key: &str, digest: Digest) {
        self.reads.push(DependencyRead {
            kind: DependencyReadKind::Layer(layer_kind),
            key: key.to_string(),
            digest,
        });

        if self.trace_enabled {
            self.trace_entries.push(QueryTraceEntry::DependencyRead {
                kind: format!("layer:{layer_kind:?}"),
                key: key.to_string(),
            });
        }
    }

    /// Records a demand query dependency read.
    pub fn read_query(&mut self, query_kind: QueryKind, key: &str, digest: Digest) {
        self.reads.push(DependencyRead {
            kind: DependencyReadKind::Query(query_kind),
            key: key.to_string(),
            digest,
        });

        if self.trace_enabled {
            self.trace_entries.push(QueryTraceEntry::DependencyRead {
                kind: format!("query:{}", query_kind.as_str()),
                key: key.to_string(),
            });
        }
    }

    /// Records a summary dependency read.
    pub fn read_summary(&mut self, callable_key: &str, digest: Digest) {
        self.reads.push(DependencyRead {
            kind: DependencyReadKind::Summary,
            key: callable_key.to_string(),
            digest,
        });

        if self.trace_enabled {
            self.trace_entries.push(QueryTraceEntry::DependencyRead {
                kind: "summary".to_string(),
                key: callable_key.to_string(),
            });
        }
    }

    /// Records an extension input dependency read.
    pub fn read_extension(&mut self, extension_key: &str, digest: Digest) {
        self.reads.push(DependencyRead {
            kind: DependencyReadKind::Extension,
            key: extension_key.to_string(),
            digest,
        });

        if self.trace_enabled {
            self.trace_entries.push(QueryTraceEntry::DependencyRead {
                kind: "extension".to_string(),
                key: extension_key.to_string(),
            });
        }
    }

    /// Checks if a memoized result exists for the given key.
    pub fn memo_lookup(&self, key: &str) -> Option<&MemoEntry> {
        self.memo_lookup_for(self.query_kind, key)
    }

    /// Checks if a memoized result exists for a specific query family and key.
    pub fn memo_lookup_for(&self, query_kind: QueryKind, key: &str) -> Option<&MemoEntry> {
        self.memo_table.get(&MemoKey::new(query_kind, key))
    }

    /// Records a memoized result for the given key.
    pub fn memo_store(&mut self, key: String, entry: MemoEntry) {
        self.memo_store_for(self.query_kind, key, entry);
    }

    /// Records a memoized result for a specific query family and key.
    pub fn memo_store_for(&mut self, query_kind: QueryKind, key: String, entry: MemoEntry) {
        self.memo_table.insert(MemoKey::new(query_kind, key), entry);
    }

    /// Returns the current recursion depth.
    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// Enters a deeper recursion level, returning `None` if the budget
    /// max_depth would be exceeded.
    pub fn enter_depth(&mut self) -> Option<u32> {
        if self.depth >= self.budget.max_depth {
            if self.trace_enabled {
                self.trace_entries.push(QueryTraceEntry::BudgetExceeded {
                    resource: "depth".to_string(),
                    limit: self.budget.max_depth,
                    actual: self.depth + 1,
                });
            }
            return None;
        }
        self.depth += 1;
        Some(self.depth)
    }

    /// Exits a recursion level.
    pub fn exit_depth(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Returns the active budget.
    pub fn budget(&self) -> &QueryBudget {
        &self.budget
    }

    /// Returns the query kind.
    pub fn query_kind(&self) -> QueryKind {
        self.query_kind
    }

    /// Returns all recorded dependency reads.
    pub fn reads(&self) -> &[DependencyRead] {
        &self.reads
    }

    /// Returns the trace entries (empty if tracing is disabled).
    pub fn trace_entries(&self) -> &[QueryTraceEntry] {
        &self.trace_entries
    }

    /// Consumes the context and returns all collected dependency reads.
    pub fn into_reads(self) -> Vec<DependencyRead> {
        self.reads
    }

    /// Returns a combined dependency digest from all recorded reads.
    ///
    /// This digest can be used for cache identity: if it matches a stored
    /// result's dependency digest, the result can be reused.
    pub fn dependency_digest(&self) -> Digest {
        if self.reads.is_empty() {
            return Digest::absent(DigestKind::DependencyLayer, "no_reads");
        }

        let mut parts: Vec<String> = self
            .reads
            .iter()
            .map(|read| format!("{:?}:{}:{}", read.kind, read.key, read.digest.value))
            .collect();
        parts.sort();

        let part_refs: Vec<&str> = parts.iter().map(String::as_str).collect();
        Digest::from_parts(
            DigestKind::DependencyLayer,
            "query_dependencies",
            &part_refs,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_digest(label: &str) -> Digest {
        Digest::from_parts(DigestKind::ProviderOutput, label, &["test"])
    }

    #[test]
    fn context_tracks_layer_reads() {
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, QueryBudget::default(), false);

        ctx.read_layer(LayerKind::Calls, "polint.calls", test_digest("calls"));
        ctx.read_layer(LayerKind::Cfg, "polint.cfg", test_digest("cfg"));

        assert_eq!(ctx.reads().len(), 2);
        assert_eq!(
            ctx.reads()[0].kind,
            DependencyReadKind::Layer(LayerKind::Calls)
        );
        assert_eq!(
            ctx.reads()[1].kind,
            DependencyReadKind::Layer(LayerKind::Cfg)
        );
    }

    #[test]
    fn context_tracks_query_reads() {
        let mut ctx =
            QueryContext::new(QueryKind::SummarySccFixpoint, QueryBudget::default(), false);

        ctx.read_query(QueryKind::FunctionSummary, "func::a", test_digest("sum_a"));

        assert_eq!(ctx.reads().len(), 1);
        assert_eq!(
            ctx.reads()[0].kind,
            DependencyReadKind::Query(QueryKind::FunctionSummary)
        );
        assert_eq!(ctx.reads()[0].key, "func::a");
    }

    #[test]
    fn context_tracks_summary_reads() {
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, QueryBudget::default(), false);

        ctx.read_summary("callable::helper", test_digest("helper"));

        assert_eq!(ctx.reads().len(), 1);
        assert_eq!(ctx.reads()[0].kind, DependencyReadKind::Summary);
        assert_eq!(ctx.reads()[0].key, "callable::helper");
    }

    #[test]
    fn context_tracks_extension_reads() {
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, QueryBudget::default(), false);

        ctx.read_extension("ext::custom_model", test_digest("ext"));

        assert_eq!(ctx.reads().len(), 1);
        assert_eq!(ctx.reads()[0].kind, DependencyReadKind::Extension);
    }

    #[test]
    fn depth_tracking_respects_budget() {
        let budget = QueryBudget {
            max_depth: 2,
            ..QueryBudget::default()
        };
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, budget, false);

        assert_eq!(ctx.depth(), 0);
        assert_eq!(ctx.enter_depth(), Some(1));
        assert_eq!(ctx.enter_depth(), Some(2));
        // Budget is max_depth=2, so depth 3 is refused
        assert_eq!(ctx.enter_depth(), None);

        ctx.exit_depth();
        assert_eq!(ctx.depth(), 1);
        ctx.exit_depth();
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn exit_depth_saturates_at_zero() {
        let mut ctx = QueryContext::new(QueryKind::FunctionCfg, QueryBudget::default(), false);
        ctx.exit_depth();
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn memo_table_stores_and_retrieves() {
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, QueryBudget::default(), false);

        assert!(ctx.memo_lookup("func::a").is_none());

        ctx.memo_store(
            "func::a".to_string(),
            MemoEntry {
                output_digest: test_digest("result_a"),
                status: QueryStatus::Complete,
            },
        );

        let entry = ctx.memo_lookup("func::a").expect("should be memoized");
        assert_eq!(entry.status, QueryStatus::Complete);
    }

    #[test]
    fn memo_table_separates_query_kinds_for_same_subject_key() {
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, QueryBudget::default(), false);

        ctx.memo_store_for(
            QueryKind::FunctionSummary,
            "func::a".to_string(),
            MemoEntry {
                output_digest: test_digest("summary"),
                status: QueryStatus::Complete,
            },
        );
        ctx.memo_store_for(
            QueryKind::FunctionCfg,
            "func::a".to_string(),
            MemoEntry {
                output_digest: test_digest("cfg"),
                status: QueryStatus::Partial,
            },
        );

        let summary = ctx
            .memo_lookup_for(QueryKind::FunctionSummary, "func::a")
            .expect("summary memo should exist");
        let cfg = ctx
            .memo_lookup_for(QueryKind::FunctionCfg, "func::a")
            .expect("cfg memo should exist");

        assert_ne!(summary.output_digest, cfg.output_digest);
        assert_eq!(summary.status, QueryStatus::Complete);
        assert_eq!(cfg.status, QueryStatus::Partial);
    }

    #[test]
    fn dependency_digest_is_deterministic() {
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, QueryBudget::default(), false);

        ctx.read_layer(LayerKind::Calls, "polint.calls", test_digest("calls"));
        ctx.read_summary("callable::a", test_digest("sum_a"));

        let d1 = ctx.dependency_digest();
        let d2 = ctx.dependency_digest();
        assert_eq!(d1, d2);
        assert!(!d1.value.is_empty());
    }

    #[test]
    fn empty_reads_produce_absent_dependency_digest() {
        let ctx = QueryContext::new(QueryKind::FunctionCfg, QueryBudget::default(), false);
        let digest = ctx.dependency_digest();
        assert!(!digest.value.is_empty());
    }

    #[test]
    fn trace_disabled_produces_no_entries() {
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, QueryBudget::default(), false);
        ctx.read_layer(LayerKind::Calls, "polint.calls", test_digest("calls"));

        assert!(ctx.trace_entries().is_empty());
    }

    #[test]
    fn trace_enabled_records_entries() {
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, QueryBudget::default(), true);
        ctx.read_layer(LayerKind::Calls, "polint.calls", test_digest("calls"));
        ctx.read_summary("callable::a", test_digest("sum_a"));

        assert_eq!(ctx.trace_entries().len(), 2);
    }

    #[test]
    fn into_reads_consumes_context() {
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, QueryBudget::default(), false);
        ctx.read_layer(LayerKind::Cfg, "polint.cfg", test_digest("cfg"));

        let reads = ctx.into_reads();
        assert_eq!(reads.len(), 1);
    }

    use super::QueryKind;

    #[test]
    fn trace_records_budget_exceeded_on_depth() {
        let budget = QueryBudget {
            max_depth: 1,
            ..QueryBudget::default()
        };
        let mut ctx = QueryContext::new(QueryKind::FunctionSummary, budget, true);
        assert_eq!(ctx.query_kind(), QueryKind::FunctionSummary);
        assert_eq!(ctx.budget().max_depth, 1);
        ctx.enter_depth(); // depth 1 (ok)
        ctx.enter_depth(); // depth 2 (exceeds max_depth=1)

        let trace = ctx.trace_entries();
        assert!(trace.iter().any(|entry| matches!(
            entry,
            QueryTraceEntry::BudgetExceeded { resource, limit, actual }
                if resource == "depth" && *limit == 1 && *actual == 2
        )));
    }
}
