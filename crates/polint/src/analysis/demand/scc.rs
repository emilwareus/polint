use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::analysis_kernel::incremental::{Digest, DigestKind};

// ---------------------------------------------------------------------------
// SCC graph types for summary fixpoint scheduling
// ---------------------------------------------------------------------------

/// A strongly connected component of callables in the call graph.
///
/// When callables are mutually recursive, their summaries must be computed
/// together through fixpoint iteration. The SCC graph captures these groups
/// and the dependency edges between them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SccComponent {
    /// Unique SCC identifier assigned in topological order.
    pub(crate) id: SccId,
    /// The callable stable keys that belong to this SCC.
    pub(crate) members: BTreeSet<String>,
    /// Whether this SCC contains more than one callable (is truly recursive).
    pub(crate) is_recursive: bool,
    /// SCC IDs that this SCC depends on (callees in other SCCs).
    pub(crate) depends_on: BTreeSet<SccId>,
    /// SCC IDs that depend on this SCC (callers in other SCCs).
    pub(crate) depended_by: BTreeSet<SccId>,
}

/// Newtype for SCC identifiers. Assigned in topological order so lower IDs
/// have no dependencies on higher IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct SccId(pub(crate) u32);

// ---------------------------------------------------------------------------
// SCC graph
// ---------------------------------------------------------------------------

/// The SCC decomposition of the call graph for summary scheduling.
///
/// SCCs are ordered topologically: an SCC with a lower ID never depends on
/// an SCC with a higher ID. This allows summaries to be computed bottom-up,
/// with fixpoint iteration only needed within recursive SCCs.
#[derive(Debug, Clone, Default)]
pub(crate) struct SccGraph {
    /// Components in topological order (lowest ID first).
    pub(crate) components: Vec<SccComponent>,
    /// Map from callable stable key to the SCC it belongs to.
    pub(crate) callable_to_scc: BTreeMap<String, SccId>,
}

impl SccGraph {
    /// Creates an SCC graph from a call edge map.
    ///
    /// The `call_edges` map goes from caller callable_stable_key to the set
    /// of callee callable_stable_keys. The algorithm uses iterative Tarjan's
    /// SCC decomposition to produce a topologically ordered SCC list.
    pub(crate) fn from_call_edges(call_edges: &BTreeMap<String, BTreeSet<String>>) -> Self {
        // Collect all callable keys (both callers and callees)
        let mut all_callables = BTreeSet::new();
        for (caller, callees) in call_edges {
            all_callables.insert(caller.clone());
            for callee in callees {
                all_callables.insert(callee.clone());
            }
        }

        if all_callables.is_empty() {
            return Self::default();
        }

        // Assign dense indexes
        let index_map: BTreeMap<&str, usize> = all_callables
            .iter()
            .enumerate()
            .map(|(i, key)| (key.as_str(), i))
            .collect();
        let reverse_map: Vec<&str> = all_callables.iter().map(String::as_str).collect();
        let count = all_callables.len();

        // Build adjacency list
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); count];
        for (caller, callees) in call_edges {
            if let Some(&caller_idx) = index_map.get(caller.as_str()) {
                for callee in callees {
                    if let Some(&callee_idx) = index_map.get(callee.as_str()) {
                        adjacency[caller_idx].push(callee_idx);
                    }
                }
            }
        }

        // Tarjan's SCC algorithm (iterative version)
        let sccs = tarjan_scc(count, &adjacency);

        // Build SCC graph
        let mut callable_to_scc = BTreeMap::new();
        let mut components = Vec::with_capacity(sccs.len());

        for (scc_idx, scc_members) in sccs.iter().enumerate() {
            let scc_id = SccId(scc_idx as u32);
            let member_keys: BTreeSet<String> = scc_members
                .iter()
                .map(|&node_idx| reverse_map[node_idx].to_string())
                .collect();

            for key in &member_keys {
                callable_to_scc.insert(key.clone(), scc_id);
            }

            let is_recursive = member_keys.len() > 1
                || scc_members.iter().any(|&node_idx| {
                    adjacency[node_idx].contains(&node_idx)
                });

            components.push(SccComponent {
                id: scc_id,
                members: member_keys,
                is_recursive,
                depends_on: BTreeSet::new(),
                depended_by: BTreeSet::new(),
            });
        }

        // Compute inter-SCC edges
        for (caller, callees) in call_edges {
            if let Some(&caller_scc) = callable_to_scc.get(caller) {
                for callee in callees {
                    if let Some(&callee_scc) = callable_to_scc.get(callee)
                        && caller_scc != callee_scc
                    {
                        components[caller_scc.0 as usize]
                            .depends_on
                            .insert(callee_scc);
                        components[callee_scc.0 as usize]
                            .depended_by
                            .insert(caller_scc);
                    }
                }
            }
        }

        Self {
            components,
            callable_to_scc,
        }
    }

    /// Returns the SCC containing the given callable, if any.
    pub(crate) fn scc_of(&self, callable_key: &str) -> Option<&SccComponent> {
        self.callable_to_scc
            .get(callable_key)
            .map(|&scc_id| &self.components[scc_id.0 as usize])
    }

    /// Returns the number of SCCs.
    pub(crate) fn scc_count(&self) -> usize {
        self.components.len()
    }

    /// Returns the number of recursive SCCs.
    pub(crate) fn recursive_scc_count(&self) -> usize {
        self.components.iter().filter(|c| c.is_recursive).count()
    }

    /// Returns a topological iteration order for summary computation.
    /// Lower SCC IDs come first (they have no dependencies on higher IDs).
    pub(crate) fn topological_order(&self) -> impl Iterator<Item = &SccComponent> {
        self.components.iter()
    }

    /// Returns an identity digest for the SCC graph suitable for cache keys.
    pub(crate) fn digest(&self) -> Digest {
        if self.components.is_empty() {
            return Digest::absent(DigestKind::ProviderOutput, "scc_graph_empty");
        }

        let mut parts: Vec<String> = Vec::new();
        for component in &self.components {
            let members: Vec<&str> = component.members.iter().map(String::as_str).collect();
            parts.push(format!(
                "scc:{}:members={}:recursive={}",
                component.id.0,
                members.join(","),
                component.is_recursive
            ));
            for dep in &component.depends_on {
                parts.push(format!("scc:{}:dep={}", component.id.0, dep.0));
            }
        }
        parts.sort();

        let part_refs: Vec<&str> = parts.iter().map(String::as_str).collect();
        Digest::from_parts(DigestKind::ProviderOutput, "scc_graph", &part_refs)
    }
}

// ---------------------------------------------------------------------------
// SCC fixpoint status
// ---------------------------------------------------------------------------

/// Status of a summary SCC fixpoint computation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SccFixpointStatus {
    /// SCC is non-recursive; single-pass computation completed.
    SinglePass,
    /// Fixpoint converged within the iteration budget.
    Converged { iterations: u32 },
    /// Fixpoint did not converge; results are conservative approximations.
    BudgetExceeded { iterations: u32, limit: u32 },
}

impl SccFixpointStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::SinglePass => "single_pass",
            Self::Converged { .. } => "converged",
            Self::BudgetExceeded { .. } => "budget_exceeded",
        }
    }
}

// ---------------------------------------------------------------------------
// SCC summary cache entry
// ---------------------------------------------------------------------------

/// A cached SCC fixpoint result.
///
/// When summary results are cached, the SCC digest and member summary
/// digests are stored so that invalidation can be checked without
/// recomputing the fixpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SccCacheEntry {
    /// The SCC ID this entry belongs to.
    pub(crate) scc_id: SccId,
    /// Digest of the SCC structure (member set and dependency edges).
    pub(crate) scc_digest: Digest,
    /// Digests of member summaries at the time of computation.
    pub(crate) member_summary_digests: BTreeMap<String, Digest>,
    /// Digests of dependency SCC summaries at the time of computation.
    pub(crate) dependency_scc_digests: BTreeMap<SccId, Digest>,
    /// The fixpoint status from the original computation.
    pub(crate) fixpoint_status: SccFixpointStatus,
    /// Combined output digest for all summaries produced by this SCC.
    pub(crate) output_digest: Digest,
}

impl SccCacheEntry {
    /// Checks if this cache entry is still valid given current input digests.
    ///
    /// Backdating: if member summaries produce the same output digest as
    /// before, the entry is reusable even if upstream inputs changed.
    pub(crate) fn is_valid(
        &self,
        current_scc_digest: &Digest,
        current_dependency_digests: &BTreeMap<SccId, Digest>,
    ) -> bool {
        if &self.scc_digest != current_scc_digest {
            return false;
        }

        for (dep_scc, expected_digest) in &self.dependency_scc_digests {
            match current_dependency_digests.get(dep_scc) {
                Some(current) if current == expected_digest => {}
                _ => return false,
            }
        }

        true
    }
}

// ---------------------------------------------------------------------------
// Tarjan's SCC algorithm (iterative)
// ---------------------------------------------------------------------------

/// Computes SCCs using an iterative version of Tarjan's algorithm.
/// Returns SCCs in reverse topological order (leaves first), which is
/// the correct order for bottom-up summary computation.
fn tarjan_scc(count: usize, adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut index_counter: usize = 0;
    let mut stack: Vec<usize> = Vec::new();
    let mut on_stack = vec![false; count];
    let mut node_index = vec![usize::MAX; count];
    let mut lowlink = vec![0_usize; count];
    let mut result: Vec<Vec<usize>> = Vec::new();

    // Each frame tracks: (node, next_neighbor_to_process)
    // When next_neighbor == 0, this is a fresh entry (need initialization).
    // When next_neighbor > 0, we are resuming after a recursive call.
    struct Frame {
        node: usize,
        next_neighbor: usize,
    }

    for root in 0..count {
        if node_index[root] != usize::MAX {
            continue;
        }

        let mut call_stack: Vec<Frame> = vec![Frame {
            node: root,
            next_neighbor: 0,
        }];

        // Initialize root
        node_index[root] = index_counter;
        lowlink[root] = index_counter;
        index_counter += 1;
        stack.push(root);
        on_stack[root] = true;

        while let Some(frame) = call_stack.last_mut() {
            let node = frame.node;
            let neighbors = &adjacency[node];

            if frame.next_neighbor < neighbors.len() {
                let neighbor = neighbors[frame.next_neighbor];
                frame.next_neighbor += 1;

                if node_index[neighbor] == usize::MAX {
                    // Unvisited neighbor: initialize and push
                    node_index[neighbor] = index_counter;
                    lowlink[neighbor] = index_counter;
                    index_counter += 1;
                    stack.push(neighbor);
                    on_stack[neighbor] = true;

                    call_stack.push(Frame {
                        node: neighbor,
                        next_neighbor: 0,
                    });
                } else if on_stack[neighbor] {
                    lowlink[node] = lowlink[node].min(node_index[neighbor]);
                }
            } else {
                // All neighbors processed for this node
                let finished_node = node;
                let finished_lowlink = lowlink[finished_node];

                // Pop this frame
                call_stack.pop();

                // Propagate lowlink to parent
                if let Some(parent_frame) = call_stack.last() {
                    lowlink[parent_frame.node] =
                        lowlink[parent_frame.node].min(finished_lowlink);
                }

                // Check if this is an SCC root
                if finished_lowlink == node_index[finished_node] {
                    let mut component = Vec::new();
                    loop {
                        let top = stack.pop().expect("SCC stack invariant violated");
                        on_stack[top] = false;
                        component.push(top);
                        if top == finished_node {
                            break;
                        }
                    }
                    component.sort();
                    result.push(component);
                }
            }
        }
    }

    // Tarjan produces SCCs with leaves/sinks first — this is the
    // bottom-up order needed for summary computation (callees before callers).
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_call_edges_produce_empty_scc_graph() {
        let edges = BTreeMap::new();
        let graph = SccGraph::from_call_edges(&edges);

        assert_eq!(graph.scc_count(), 0);
        assert_eq!(graph.recursive_scc_count(), 0);
    }

    #[test]
    fn single_function_no_self_call_is_non_recursive() {
        let mut edges = BTreeMap::new();
        edges.insert(
            "func::a".to_string(),
            BTreeSet::from(["func::b".to_string()]),
        );

        let graph = SccGraph::from_call_edges(&edges);

        // func::a and func::b each in their own SCC
        assert_eq!(graph.scc_count(), 2);
        assert_eq!(graph.recursive_scc_count(), 0);

        let scc_a = graph.scc_of("func::a").expect("func::a should have SCC");
        assert!(!scc_a.is_recursive);
        assert!(scc_a.members.contains("func::a"));
        assert_eq!(scc_a.members.len(), 1);
    }

    #[test]
    fn self_recursive_function_is_recursive_scc() {
        let mut edges = BTreeMap::new();
        edges.insert(
            "func::a".to_string(),
            BTreeSet::from(["func::a".to_string()]),
        );

        let graph = SccGraph::from_call_edges(&edges);

        assert_eq!(graph.scc_count(), 1);
        assert_eq!(graph.recursive_scc_count(), 1);

        let scc = graph.scc_of("func::a").expect("should exist");
        assert!(scc.is_recursive);
    }

    #[test]
    fn mutual_recursion_forms_single_scc() {
        let mut edges = BTreeMap::new();
        edges.insert(
            "func::a".to_string(),
            BTreeSet::from(["func::b".to_string()]),
        );
        edges.insert(
            "func::b".to_string(),
            BTreeSet::from(["func::a".to_string()]),
        );

        let graph = SccGraph::from_call_edges(&edges);

        assert_eq!(graph.scc_count(), 1);
        assert_eq!(graph.recursive_scc_count(), 1);

        let scc = graph.scc_of("func::a").expect("should exist");
        assert!(scc.is_recursive);
        assert!(scc.members.contains("func::a"));
        assert!(scc.members.contains("func::b"));
        assert_eq!(scc.members.len(), 2);

        // Both in the same SCC
        assert_eq!(
            graph.callable_to_scc["func::a"],
            graph.callable_to_scc["func::b"]
        );
    }

    #[test]
    fn topological_order_callees_before_callers() {
        // func::a -> func::b -> func::c
        let mut edges = BTreeMap::new();
        edges.insert(
            "func::a".to_string(),
            BTreeSet::from(["func::b".to_string()]),
        );
        edges.insert(
            "func::b".to_string(),
            BTreeSet::from(["func::c".to_string()]),
        );

        let graph = SccGraph::from_call_edges(&edges);

        assert_eq!(graph.scc_count(), 3);
        assert_eq!(graph.recursive_scc_count(), 0);

        let order: Vec<String> = graph
            .topological_order()
            .flat_map(|scc| scc.members.iter().cloned())
            .collect();

        // c should appear before b, b before a
        let pos_a = order.iter().position(|k| k == "func::a").unwrap();
        let pos_b = order.iter().position(|k| k == "func::b").unwrap();
        let pos_c = order.iter().position(|k| k == "func::c").unwrap();

        assert!(pos_c < pos_b, "callee c should come before b");
        assert!(pos_b < pos_a, "callee b should come before a");
    }

    #[test]
    fn inter_scc_dependency_edges_are_recorded() {
        // func::a -> func::b (separate SCCs)
        let mut edges = BTreeMap::new();
        edges.insert(
            "func::a".to_string(),
            BTreeSet::from(["func::b".to_string()]),
        );

        let graph = SccGraph::from_call_edges(&edges);

        let scc_a = graph.scc_of("func::a").unwrap();
        let scc_b = graph.scc_of("func::b").unwrap();

        assert!(scc_a.depends_on.contains(&scc_b.id));
        assert!(scc_b.depended_by.contains(&scc_a.id));
    }

    #[test]
    fn scc_digest_is_deterministic() {
        let mut edges = BTreeMap::new();
        edges.insert(
            "func::a".to_string(),
            BTreeSet::from(["func::b".to_string()]),
        );
        edges.insert(
            "func::b".to_string(),
            BTreeSet::from(["func::a".to_string()]),
        );

        let graph1 = SccGraph::from_call_edges(&edges);
        let graph2 = SccGraph::from_call_edges(&edges);

        assert_eq!(graph1.digest(), graph2.digest());
        assert!(!graph1.digest().value.is_empty());
    }

    #[test]
    fn scc_cache_entry_validates_against_current_digests() {
        let scc_digest = Digest::from_parts(DigestKind::ProviderOutput, "scc", &["test"]);
        let dep_digest = Digest::from_parts(DigestKind::ProviderOutput, "dep", &["test"]);

        let entry = SccCacheEntry {
            scc_id: SccId(0),
            scc_digest: scc_digest.clone(),
            member_summary_digests: BTreeMap::new(),
            dependency_scc_digests: BTreeMap::from([(SccId(1), dep_digest.clone())]),
            fixpoint_status: SccFixpointStatus::Converged { iterations: 3 },
            output_digest: Digest::from_parts(DigestKind::ProviderOutput, "output", &["test"]),
        };

        // Valid when digests match
        let current_deps = BTreeMap::from([(SccId(1), dep_digest.clone())]);
        assert!(entry.is_valid(&scc_digest, &current_deps));

        // Invalid when SCC structure changed
        let changed_scc = Digest::from_parts(DigestKind::ProviderOutput, "scc", &["changed"]);
        assert!(!entry.is_valid(&changed_scc, &current_deps));

        // Invalid when dependency digest changed
        let changed_dep = Digest::from_parts(DigestKind::ProviderOutput, "dep", &["changed"]);
        let changed_deps = BTreeMap::from([(SccId(1), changed_dep)]);
        assert!(!entry.is_valid(&scc_digest, &changed_deps));

        // Invalid when dependency is missing
        let missing_deps: BTreeMap<SccId, Digest> = BTreeMap::new();
        assert!(!entry.is_valid(&scc_digest, &missing_deps));
    }

    #[test]
    fn fixpoint_status_as_str_covers_all_variants() {
        assert_eq!(SccFixpointStatus::SinglePass.as_str(), "single_pass");
        assert_eq!(
            SccFixpointStatus::Converged { iterations: 3 }.as_str(),
            "converged"
        );
        assert_eq!(
            SccFixpointStatus::BudgetExceeded {
                iterations: 10,
                limit: 10
            }
            .as_str(),
            "budget_exceeded"
        );
    }

    #[test]
    fn complex_call_graph_with_multiple_sccs() {
        // Graph:
        //   a -> b, c
        //   b -> c
        //   c -> d
        //   d -> c  (c-d are mutually recursive)
        //   e -> a
        let mut edges = BTreeMap::new();
        edges.insert(
            "func::a".to_string(),
            BTreeSet::from(["func::b".to_string(), "func::c".to_string()]),
        );
        edges.insert(
            "func::b".to_string(),
            BTreeSet::from(["func::c".to_string()]),
        );
        edges.insert(
            "func::c".to_string(),
            BTreeSet::from(["func::d".to_string()]),
        );
        edges.insert(
            "func::d".to_string(),
            BTreeSet::from(["func::c".to_string()]),
        );
        edges.insert(
            "func::e".to_string(),
            BTreeSet::from(["func::a".to_string()]),
        );

        let graph = SccGraph::from_call_edges(&edges);

        // c and d form a recursive SCC; a, b, e are each in their own SCC
        assert_eq!(graph.scc_count(), 4); // {c,d}, {b}, {a}, {e}
        assert_eq!(graph.recursive_scc_count(), 1);

        let scc_c = graph.scc_of("func::c").unwrap();
        let scc_d = graph.scc_of("func::d").unwrap();
        assert_eq!(scc_c.id, scc_d.id);
        assert!(scc_c.is_recursive);

        // In topological order, {c,d} should come before {b}, {a}, {e}
        let order: Vec<SccId> = graph.topological_order().map(|s| s.id).collect();
        let pos_cd = order.iter().position(|id| *id == scc_c.id).unwrap();
        let pos_a = order
            .iter()
            .position(|id| *id == graph.callable_to_scc["func::a"])
            .unwrap();
        let pos_e = order
            .iter()
            .position(|id| *id == graph.callable_to_scc["func::e"])
            .unwrap();

        assert!(pos_cd < pos_a, "recursive SCC should precede caller SCC");
        assert!(pos_a < pos_e, "a should precede its caller e");
    }
}
