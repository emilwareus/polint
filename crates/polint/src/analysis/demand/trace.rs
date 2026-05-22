use serde::{Deserialize, Serialize};

use super::query::{QueryKind, QueryStatus};
use super::scc::SccFixpointStatus;

// ---------------------------------------------------------------------------
// Query trace types
// ---------------------------------------------------------------------------

/// A single entry in the query execution trace.
///
/// Trace entries record the internal execution flow of demand queries for
/// debug output. Traces are only collected when trace mode is enabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum QueryTraceEntry {
    /// A query execution started.
    QueryStarted {
        kind: QueryKind,
        key: String,
        depth: u32,
    },
    /// A query execution completed.
    QueryCompleted {
        kind: QueryKind,
        key: String,
        status: QueryStatus,
        depth: u32,
    },
    /// A dependency was read during query execution.
    DependencyRead { kind: String, key: String },
    /// A memoized result was reused.
    MemoHit { key: String },
    /// A budget limit was exceeded.
    BudgetExceeded {
        resource: String,
        limit: u32,
        actual: u32,
    },
    /// An SCC fixpoint iteration step.
    SccIteration {
        scc_id: u32,
        iteration: u32,
        changed_members: u32,
    },
    /// An SCC fixpoint completed.
    SccCompleted {
        scc_id: u32,
        status: SccFixpointStatus,
    },
    /// A quarantined result was encountered and skipped.
    QuarantineSkipped {
        key: String,
        extension: String,
        reason: String,
    },
    /// An extension result was used with provenance.
    ExtensionUsed {
        extension_key: String,
        fact_key: String,
    },
}

// ---------------------------------------------------------------------------
// QueryTrace — collected trace for a single demand query chain
// ---------------------------------------------------------------------------

/// Accumulated trace entries for a demand query execution.
///
/// The trace is crate-private and test-facing. It is collected only when
/// the internal trace mode is active and is used for debug snapshots and
/// eval fixture validation.
#[derive(Debug, Clone, Default)]
pub(crate) struct QueryTrace {
    entries: Vec<QueryTraceEntry>,
}

impl QueryTrace {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Adds a trace entry.
    pub(crate) fn push(&mut self, entry: QueryTraceEntry) {
        self.entries.push(entry);
    }

    /// Returns all trace entries.
    pub(crate) fn entries(&self) -> &[QueryTraceEntry] {
        &self.entries
    }

    /// Returns the number of trace entries.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the trace is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns entries matching a given query kind.
    pub(crate) fn entries_for_query(&self, kind: QueryKind) -> Vec<&QueryTraceEntry> {
        self.entries
            .iter()
            .filter(|entry| match entry {
                QueryTraceEntry::QueryStarted { kind: k, .. }
                | QueryTraceEntry::QueryCompleted { kind: k, .. } => *k == kind,
                _ => false,
            })
            .collect()
    }

    /// Returns the number of SCC iterations recorded.
    pub(crate) fn scc_iteration_count(&self) -> u32 {
        self.entries
            .iter()
            .filter(|entry| matches!(entry, QueryTraceEntry::SccIteration { .. }))
            .count() as u32
    }

    /// Returns the number of quarantine skip events.
    pub(crate) fn quarantine_skip_count(&self) -> u32 {
        self.entries
            .iter()
            .filter(|entry| matches!(entry, QueryTraceEntry::QuarantineSkipped { .. }))
            .count() as u32
    }

    /// Returns a debug JSON representation of the trace.
    #[cfg(test)]
    pub(crate) fn to_debug_json(&self) -> serde_json::Value {
        serde_json::json!({
            "trace_entries": self.entries.len(),
            "scc_iterations": self.scc_iteration_count(),
            "quarantine_skips": self.quarantine_skip_count(),
            "entries": self.entries.iter().map(|entry| {
                match entry {
                    QueryTraceEntry::QueryStarted { kind, key, depth } => {
                        serde_json::json!({
                            "type": "query_started",
                            "kind": kind.as_str(),
                            "key": key,
                            "depth": depth,
                        })
                    }
                    QueryTraceEntry::QueryCompleted { kind, key, status, depth } => {
                        serde_json::json!({
                            "type": "query_completed",
                            "kind": kind.as_str(),
                            "key": key,
                            "status": status.as_str(),
                            "depth": depth,
                        })
                    }
                    QueryTraceEntry::DependencyRead { kind, key } => {
                        serde_json::json!({
                            "type": "dependency_read",
                            "kind": kind,
                            "key": key,
                        })
                    }
                    QueryTraceEntry::MemoHit { key } => {
                        serde_json::json!({
                            "type": "memo_hit",
                            "key": key,
                        })
                    }
                    QueryTraceEntry::BudgetExceeded { resource, limit, actual } => {
                        serde_json::json!({
                            "type": "budget_exceeded",
                            "resource": resource,
                            "limit": limit,
                            "actual": actual,
                        })
                    }
                    QueryTraceEntry::SccIteration { scc_id, iteration, changed_members } => {
                        serde_json::json!({
                            "type": "scc_iteration",
                            "scc_id": scc_id,
                            "iteration": iteration,
                            "changed_members": changed_members,
                        })
                    }
                    QueryTraceEntry::SccCompleted { scc_id, status } => {
                        serde_json::json!({
                            "type": "scc_completed",
                            "scc_id": scc_id,
                            "status": status.as_str(),
                        })
                    }
                    QueryTraceEntry::QuarantineSkipped { key, extension, reason } => {
                        serde_json::json!({
                            "type": "quarantine_skipped",
                            "key": key,
                            "extension": extension,
                            "reason": reason,
                        })
                    }
                    QueryTraceEntry::ExtensionUsed { extension_key, fact_key } => {
                        serde_json::json!({
                            "type": "extension_used",
                            "extension_key": extension_key,
                            "fact_key": fact_key,
                        })
                    }
                }
            }).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_trace() {
        let trace = QueryTrace::new();
        assert!(trace.is_empty());
        assert_eq!(trace.len(), 0);
        assert_eq!(trace.scc_iteration_count(), 0);
        assert_eq!(trace.quarantine_skip_count(), 0);
    }

    #[test]
    fn push_and_retrieve_entries() {
        let mut trace = QueryTrace::new();

        trace.push(QueryTraceEntry::QueryStarted {
            kind: QueryKind::FunctionSummary,
            key: "func::a".to_string(),
            depth: 0,
        });
        trace.push(QueryTraceEntry::DependencyRead {
            kind: "layer:Calls".to_string(),
            key: "polint.calls".to_string(),
        });
        trace.push(QueryTraceEntry::QueryCompleted {
            kind: QueryKind::FunctionSummary,
            key: "func::a".to_string(),
            status: QueryStatus::Complete,
            depth: 0,
        });

        assert_eq!(trace.len(), 3);
        assert!(!trace.is_empty());
    }

    #[test]
    fn entries_for_query_filters_by_kind() {
        let mut trace = QueryTrace::new();

        trace.push(QueryTraceEntry::QueryStarted {
            kind: QueryKind::FunctionSummary,
            key: "func::a".to_string(),
            depth: 0,
        });
        trace.push(QueryTraceEntry::QueryStarted {
            kind: QueryKind::FunctionCfg,
            key: "func::a".to_string(),
            depth: 1,
        });
        trace.push(QueryTraceEntry::QueryCompleted {
            kind: QueryKind::FunctionSummary,
            key: "func::a".to_string(),
            status: QueryStatus::Complete,
            depth: 0,
        });

        let summary_entries = trace.entries_for_query(QueryKind::FunctionSummary);
        assert_eq!(summary_entries.len(), 2);

        let cfg_entries = trace.entries_for_query(QueryKind::FunctionCfg);
        assert_eq!(cfg_entries.len(), 1);

        let alias_entries = trace.entries_for_query(QueryKind::BoundedAlias);
        assert!(alias_entries.is_empty());
    }

    #[test]
    fn scc_iteration_count_tracks_iterations() {
        let mut trace = QueryTrace::new();

        trace.push(QueryTraceEntry::SccIteration {
            scc_id: 0,
            iteration: 1,
            changed_members: 2,
        });
        trace.push(QueryTraceEntry::SccIteration {
            scc_id: 0,
            iteration: 2,
            changed_members: 1,
        });
        trace.push(QueryTraceEntry::SccCompleted {
            scc_id: 0,
            status: SccFixpointStatus::Converged { iterations: 2 },
        });

        assert_eq!(trace.scc_iteration_count(), 2);
    }

    #[test]
    fn quarantine_skip_count_tracks_skips() {
        let mut trace = QueryTrace::new();

        trace.push(QueryTraceEntry::QuarantineSkipped {
            key: "summary:func_a".to_string(),
            extension: "ext::model".to_string(),
            reason: "extension_code_changed".to_string(),
        });
        trace.push(QueryTraceEntry::QuarantineSkipped {
            key: "summary:func_b".to_string(),
            extension: "ext::model".to_string(),
            reason: "validation_failed".to_string(),
        });

        assert_eq!(trace.quarantine_skip_count(), 2);
    }

    #[test]
    fn debug_json_is_well_formed() {
        let mut trace = QueryTrace::new();

        trace.push(QueryTraceEntry::QueryStarted {
            kind: QueryKind::FunctionSummary,
            key: "func::a".to_string(),
            depth: 0,
        });
        trace.push(QueryTraceEntry::MemoHit {
            key: "func::b".to_string(),
        });
        trace.push(QueryTraceEntry::BudgetExceeded {
            resource: "iterations".to_string(),
            limit: 100,
            actual: 101,
        });
        trace.push(QueryTraceEntry::ExtensionUsed {
            extension_key: "ext::model".to_string(),
            fact_key: "fact::a".to_string(),
        });

        let json = trace.to_debug_json();
        assert_eq!(json["trace_entries"], 4);
        assert!(json["entries"].is_array());

        let entries = json["entries"].as_array().unwrap();
        assert_eq!(entries[0]["type"], "query_started");
        assert_eq!(entries[1]["type"], "memo_hit");
        assert_eq!(entries[2]["type"], "budget_exceeded");
        assert_eq!(entries[3]["type"], "extension_used");
    }
}
