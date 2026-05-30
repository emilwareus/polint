use crate::analysis_kernel::incremental::{Digest, DigestKind};

/// Schema label for the `polint.semantic_graph` provider manifest.
pub(crate) const SEMANTIC_GRAPH_SCHEMA_LABEL: &str = "semantic-graph-facts-1";

/// Provider parameter digest for `polint.semantic_graph`.
///
/// The algorithm-version strings are part of the parameter digest so any bump to a
/// node/edge/constraint emission algorithm deterministically invalidates the
/// semantic-graph cache. The locked test below is the intended trip-wire: adding or
/// bumping an algorithm version requires extending this list.
///
/// # SC3 dependency-index inputs — PRESENT-NOW vs DEFERRED-AND-WHY (D-17)
///
/// ROADMAP Phase 44 Success Criterion 3 enumerates the dependency-index inputs the
/// unified semantic graph will eventually digest. Phase 44 has producers for only a
/// subset; the rest have no producer yet and are documented here as RESERVED rather
/// than silently omitted, so the partial coverage reads as intentional. Each
/// deferred input enters this digest (and the manifest `inputs` slice) only when its
/// producer lands — invalidating the cache at that point by construction.
///
/// PRESENT-NOW (these have producers today; their output digests are folded into the
/// provider output digest in `provider.rs`, and the families they expose appear in
/// the manifest `inputs` slice):
/// - semantic index (`polint.symbol_graph`: scopes, symbols, references)
/// - module graph / topology (`polint.module_graph` / `polint.module_topology`)
/// - direct calls (`polint.calls`: call sites)
/// - types / value / alias (`polint.type_value_alias`)
/// - entrypoints (`polint.entrypoints`)
/// - extensions (`polint.extensions`)
/// - identity (`polint.identity`) and reachability (`polint.reachability`)
///
/// DEFERRED-AND-WHY (no producer exists yet; emitted/digested as ZERO until the named
/// phase lands a producer — NOT a silent omission):
/// - MIR — full unified MIR consumption is reserved for Phase 47 (unified call-graph
///   solver). (Local semantic MIR exists, but the SC3 dependency-index MIR input the
///   solver folds over is a Phase 47 concern.)
/// - CFG — reserved for Phase 47's budgeted solver consumption.
/// - summaries — reserved for Phase 47/50 (interprocedural summary folding).
/// - accepted adaptation models — reserved for Phase 49 (ADAPT-01); no model producer
///   exists, mirroring `ConstraintKind::ModelEdge`'s honest emptiness.
/// - solver budgets — reserved for Phase 51/53 (cache and solver budgets threaded
///   through the solver core).
///
/// This is a comment/doc addition only: ZERO deferred inputs are digested here, and
/// the runtime behavior is unchanged.
pub(crate) fn semantic_graph_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "semantic_graph_provider_parameters",
        &[
            SEMANTIC_GRAPH_SCHEMA_LABEL,
            "semantic_nodes",
            "semantic_edges",
            "semantic_constraints",
            "node_projection_v1",
            "edge_projection_v1",
            "constraint_projection_v1",
        ],
    )
}

#[cfg(test)]
mod tests {
    use crate::analysis_kernel::incremental::{Digest, DigestKind};

    #[test]
    fn semantic_graph_provider_parameter_digest_locks_parts_list() {
        assert_eq!(
            super::semantic_graph_provider_parameter_digest(),
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "semantic_graph_provider_parameters",
                &[
                    "semantic-graph-facts-1",
                    "semantic_nodes",
                    "semantic_edges",
                    "semantic_constraints",
                    "node_projection_v1",
                    "edge_projection_v1",
                    "constraint_projection_v1",
                ],
            )
        );
    }

    #[test]
    fn algorithm_version_bump_invalidates_the_pre_bump_digest() {
        // Bumping any frozen algorithm version must deterministically invalidate the
        // semantic-graph cache: the live digest differs from a pre-bump parts list,
        // so stale nodes/edges/constraints are not silently reused.
        let pre_bump = Digest::from_parts(
            DigestKind::ProviderParameters,
            "semantic_graph_provider_parameters",
            &[
                "semantic-graph-facts-1",
                "semantic_nodes",
                "semantic_edges",
                "semantic_constraints",
                "node_projection_v0",
                "edge_projection_v1",
                "constraint_projection_v1",
            ],
        );
        assert_ne!(super::semantic_graph_provider_parameter_digest(), pre_bump);
    }

    #[test]
    fn semantic_graph_schema_label_is_semantic_graph_facts_1() {
        assert_eq!(super::SEMANTIC_GRAPH_SCHEMA_LABEL, "semantic-graph-facts-1");
    }
}
