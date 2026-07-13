use serde::{Deserialize, Serialize};

use crate::analysis_kernel::incremental::{
    Digest, DigestKind, PrecisionTier, QueryDependencyInputs, QueryKey,
};

// ---------------------------------------------------------------------------
// QueryKind — the set of demand query families
// ---------------------------------------------------------------------------

/// Identifies a demand query family.
///
/// Each variant represents a class of expensive computation that should be
/// demand-driven rather than eagerly materialized for the entire repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum QueryKind {
    /// One-function CFG and control-dependence view.
    FunctionCfg,
    /// One-function def-use and data-dependence view.
    FunctionDefUse,
    /// Direct call target resolution for a single call site.
    DirectCallTarget,
    /// One-function direct summary computation.
    FunctionSummary,
    /// Evidence/path query for a single diagnostic.
    DiagnosticEvidence,
    /// Bounded alias query for one place pair.
    BoundedAlias,
    /// Summary SCC fixpoint for a set of mutually recursive callables.
    SummarySccFixpoint,
}

impl QueryKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::FunctionCfg => "function_cfg",
            Self::FunctionDefUse => "function_def_use",
            Self::DirectCallTarget => "direct_call_target",
            Self::FunctionSummary => "function_summary",
            Self::DiagnosticEvidence => "diagnostic_evidence",
            Self::BoundedAlias => "bounded_alias",
            Self::SummarySccFixpoint => "summary_scc_fixpoint",
        }
    }

    pub(crate) fn version(self) -> &'static str {
        // All queries start at version 1.
        "1"
    }
}

// ---------------------------------------------------------------------------
// QueryBudget — resource limits for demand queries
// ---------------------------------------------------------------------------

/// Resource budget for a demand query execution.
///
/// Queries that exceed the budget produce conservative (top/unknown) results
/// with an explicit `BudgetExceeded` status rather than silently running
/// unbounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct QueryBudget {
    /// Maximum number of SCC iterations allowed for fixpoint queries.
    pub(crate) max_iterations: u32,
    /// Maximum number of nodes visited during graph traversal queries.
    pub(crate) max_nodes: u32,
    /// Maximum depth for recursive query evaluation.
    pub(crate) max_depth: u32,
}

impl QueryBudget {
    pub(crate) const DEFAULT_MAX_ITERATIONS: u32 = 100;
    pub(crate) const DEFAULT_MAX_NODES: u32 = 10_000;
    pub(crate) const DEFAULT_MAX_DEPTH: u32 = 64;

    pub(crate) fn default_budget() -> Self {
        Self {
            max_iterations: Self::DEFAULT_MAX_ITERATIONS,
            max_nodes: Self::DEFAULT_MAX_NODES,
            max_depth: Self::DEFAULT_MAX_DEPTH,
        }
    }

    /// Returns a digest of this budget for cache identity.
    pub(crate) fn digest(&self) -> Digest {
        Digest::from_parts(
            DigestKind::Budget,
            "query_budget",
            &[
                &self.max_iterations.to_string(),
                &self.max_nodes.to_string(),
                &self.max_depth.to_string(),
            ],
        )
    }
}

impl Default for QueryBudget {
    fn default() -> Self {
        Self::default_budget()
    }
}

// ---------------------------------------------------------------------------
// QueryStatus — outcome of a demand query
// ---------------------------------------------------------------------------

/// Outcome status of a demand query execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum QueryStatus {
    /// Query completed with full results.
    Complete,
    /// Query completed but results are partial due to missing inputs.
    Partial,
    /// Query exceeded its resource budget.
    BudgetExceeded,
    /// Query could not complete because required setup is missing.
    SetupMissing,
    /// Query could not complete because the subject is unsupported.
    Unsupported,
    /// Results were loaded from cache.
    Cached,
    /// Results were computed under extension quarantine.
    Quarantined,
}

impl QueryStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::BudgetExceeded => "budget_exceeded",
            Self::SetupMissing => "setup_missing",
            Self::Unsupported => "unsupported",
            Self::Cached => "cached",
            Self::Quarantined => "quarantined",
        }
    }
}

// ---------------------------------------------------------------------------
// QueryResult — typed wrapper for demand query output
// ---------------------------------------------------------------------------

/// Result of a demand query execution, pairing output with status and
/// metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryResult<T> {
    pub(crate) output: T,
    pub(crate) status: QueryStatus,
    pub(crate) precision: PrecisionTier,
    pub(crate) iterations: u32,
    pub(crate) nodes_visited: u32,
}

impl<T> QueryResult<T> {
    pub(crate) fn complete(output: T, precision: PrecisionTier) -> Self {
        Self {
            output,
            status: QueryStatus::Complete,
            precision,
            iterations: 0,
            nodes_visited: 0,
        }
    }

    pub(crate) fn budget_exceeded(output: T, precision: PrecisionTier, iterations: u32) -> Self {
        Self {
            output,
            status: QueryStatus::BudgetExceeded,
            precision,
            iterations,
            nodes_visited: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Demand query key construction
// ---------------------------------------------------------------------------

/// Constructs a `QueryKey` for a demand query with the given parameters.
pub(crate) fn demand_query_key(
    kind: QueryKind,
    parameter_digest: Digest,
    dependency_inputs: QueryDependencyInputs,
    layer_digests: Vec<Digest>,
    budget: &QueryBudget,
    precision_tier: PrecisionTier,
) -> QueryKey {
    QueryKey::new(
        kind.as_str(),
        kind.version(),
        parameter_digest,
        dependency_inputs,
        layer_digests,
        budget.digest(),
        precision_tier,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_kind_as_str_covers_all_variants() {
        let kinds = [
            QueryKind::FunctionCfg,
            QueryKind::FunctionDefUse,
            QueryKind::DirectCallTarget,
            QueryKind::FunctionSummary,
            QueryKind::DiagnosticEvidence,
            QueryKind::BoundedAlias,
            QueryKind::SummarySccFixpoint,
        ];

        let strings: Vec<_> = kinds.iter().map(|k| k.as_str()).collect();
        assert_eq!(strings.len(), 7);
        // All unique
        let unique: std::collections::BTreeSet<_> = strings.iter().collect();
        assert_eq!(unique.len(), 7);
    }

    #[test]
    fn query_status_as_str_covers_all_variants() {
        let statuses = [
            QueryStatus::Complete,
            QueryStatus::Partial,
            QueryStatus::BudgetExceeded,
            QueryStatus::SetupMissing,
            QueryStatus::Unsupported,
            QueryStatus::Cached,
            QueryStatus::Quarantined,
        ];

        let strings: Vec<_> = statuses.iter().map(|s| s.as_str()).collect();
        assert_eq!(strings.len(), 7);
        let unique: std::collections::BTreeSet<_> = strings.iter().collect();
        assert_eq!(unique.len(), 7);
    }

    #[test]
    fn default_budget_matches_constants() {
        let budget = QueryBudget::default();
        assert_eq!(budget.max_iterations, QueryBudget::DEFAULT_MAX_ITERATIONS);
        assert_eq!(budget.max_nodes, QueryBudget::DEFAULT_MAX_NODES);
        assert_eq!(budget.max_depth, QueryBudget::DEFAULT_MAX_DEPTH);
    }

    #[test]
    fn budget_digest_is_deterministic() {
        let budget = QueryBudget::default_budget();
        let d1 = budget.digest();
        let d2 = budget.digest();
        assert_eq!(d1, d2);
        assert!(!d1.value.is_empty());
    }

    #[test]
    fn demand_query_key_builds_valid_key() {
        let param_digest = Digest::from_parts(
            DigestKind::QueryParameters,
            "test_params",
            &["callable:func_a"],
        );
        let layer_digests = vec![Digest::from_parts(
            DigestKind::ProviderOutput,
            "calls_output",
            &["digest:abc"],
        )];
        let budget = QueryBudget::default();

        let key = demand_query_key(
            QueryKind::FunctionSummary,
            param_digest,
            QueryDependencyInputs::new(Vec::new()),
            layer_digests,
            &budget,
            PrecisionTier::SetupAware,
        );

        assert_eq!(key.query_kind, "function_summary");
        assert_eq!(key.query_version, "1");
        assert_eq!(key.layer_digests.len(), 1);
        assert_eq!(key.precision_tier, PrecisionTier::SetupAware);
    }

    #[test]
    fn demand_query_key_canonicalizes_layer_digest_order() {
        let param_digest = Digest::from_parts(
            DigestKind::QueryParameters,
            "test_params",
            &["callable:func_a"],
        );
        let first = Digest::from_parts(DigestKind::ProviderOutput, "calls_output", &["a"]);
        let second = Digest::from_parts(DigestKind::ProviderOutput, "cfg_output", &["b"]);
        let budget = QueryBudget::default();

        let key_a = demand_query_key(
            QueryKind::FunctionSummary,
            param_digest.clone(),
            QueryDependencyInputs::new(Vec::new()),
            vec![first.clone(), second.clone()],
            &budget,
            PrecisionTier::SetupAware,
        );
        let key_b = demand_query_key(
            QueryKind::FunctionSummary,
            param_digest,
            QueryDependencyInputs::new(Vec::new()),
            vec![second, first],
            &budget,
            PrecisionTier::SetupAware,
        );

        assert_eq!(key_a, key_b);
    }

    #[test]
    fn query_result_complete_has_zero_iterations() {
        let result = QueryResult::complete(42_u64, PrecisionTier::SetupAware);
        assert_eq!(result.status, QueryStatus::Complete);
        assert_eq!(result.iterations, 0);
        assert_eq!(result.nodes_visited, 0);
        assert_eq!(result.output, 42);
    }

    #[test]
    fn query_result_budget_exceeded_records_iterations() {
        let result = QueryResult::budget_exceeded(0_u64, PrecisionTier::SetupAware, 100);
        assert_eq!(result.status, QueryStatus::BudgetExceeded);
        assert_eq!(result.iterations, 100);
    }
}
