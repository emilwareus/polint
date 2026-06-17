//! Whole-program reachable-set traversal over the v1.2 direct-call edge set.
//!
//! [`mark_call_reachability`] runs a BFS from every reachability root's
//! `target_function` over DIRECT-CALL RESOLVED-target edges only
//! (`CallTargetFact` with `target_function = Some(..)` and a resolved
//! `CallTargetStatus`) — the only pre-solver edge set (D-18). Every call site is
//! then emitted as a [`CallReachabilityFact`] keyed by the call-site stable key,
//! with `in_reachable_graph` recording whether the site's caller function is in
//! the reachable-from-roots set.
//!
//! Determinism: roots and edges are visited in a SORTED, ID-independent order
//! (sorted by stable key / a locked tuple before traversal) so the marking output
//! is byte-identical regardless of insertion order (D-20/D-21). The marking is a
//! SEPARATE fact family — `analysis::calls` (and `analysis::entrypoints`) are
//! never mutated (composition over mutation, D-18).
//!
//! Forward-compatibility (D-18): Phases 47/48 will swap this direct-call edge set
//! for solver-derived edges behind this SAME marking contract — the
//! `CallReachabilityFact` shape, the call-site-stable-key keying, and the
//! reachable-from-roots semantics stay fixed; only the adjacency source changes.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::analysis::calls::facts::CallTargetStatus;
use crate::analysis::reachability::facts::{
    CallReachabilityFact, CallReachabilityReason, ReachabilityRootFact,
};
use crate::analysis::reachability::store::REACHABILITY_PROVIDER_ID;
use crate::core::{AnalysisDb, FunctionId};

/// The set of functions reachable from the reachability roots over resolved
/// direct-call edges, together with the seed (root) functions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ReachableSet {
    reachable: BTreeSet<FunctionId>,
    roots: BTreeSet<FunctionId>,
}

impl ReachableSet {
    pub(crate) fn contains(&self, function: FunctionId) -> bool {
        self.reachable.contains(&function)
    }

    pub(crate) fn is_root(&self, function: FunctionId) -> bool {
        self.roots.contains(&function)
    }
}

/// Computes the reachable-from-roots function set by BFS over resolved
/// direct-call edges.
///
/// Adjacency is built from `db.call_targets()`: an edge `caller -> target` exists
/// iff the target row has `target_function = Some(target)` and a resolved
/// `CallTargetStatus`. Unresolved / ambiguous / setup-missing edges do NOT extend
/// the frontier. Both the seed roots and each node's neighbours are visited in
/// sorted order so the traversal is insertion-order independent.
///
/// Roots are supplied explicitly (the provider passes the discovered roots before
/// they are stored, so this never depends on `db.reachability_roots()` being
/// populated yet).
pub(crate) fn compute_reachable_set(
    db: &AnalysisDb,
    roots: &[ReachabilityRootFact],
) -> ReachableSet {
    // Sorted adjacency: caller -> sorted, de-duplicated resolved targets. Using
    // BTreeMap/BTreeSet keeps both the key iteration and neighbour iteration in a
    // deterministic, ID-independent order regardless of fact insertion order.
    let mut adjacency: BTreeMap<FunctionId, BTreeSet<FunctionId>> = BTreeMap::new();
    for target in db.call_targets() {
        if !is_resolved_direct_edge(target.status) {
            continue;
        }
        if let Some(callee) = target.target_function {
            adjacency.entry(target.caller).or_default().insert(callee);
        }
    }

    // Seed the frontier with every root target_function, in sorted order.
    let roots: BTreeSet<FunctionId> = roots.iter().map(|root| root.target_function).collect();

    let mut reachable: BTreeSet<FunctionId> = BTreeSet::new();
    let mut queue: VecDeque<FunctionId> = VecDeque::new();
    for &root in &roots {
        if reachable.insert(root) {
            queue.push_back(root);
        }
    }

    while let Some(function) = queue.pop_front() {
        if let Some(callees) = adjacency.get(&function) {
            for &callee in callees {
                if reachable.insert(callee) {
                    queue.push_back(callee);
                }
            }
        }
    }

    ReachableSet { reachable, roots }
}

/// Marks every call site as in / out of the reachable graph (D-18).
///
/// Returns one [`CallReachabilityFact`] per call site, keyed by the site's stable
/// key, with `in_reachable_graph = reachable.contains(site.caller)`. The returned
/// vec is sorted by the mark stable key so the output is byte-identical regardless
/// of call-site insertion order; the store's `normalized()` re-applies the same
/// sort defensively.
pub(crate) fn mark_call_reachability(
    db: &AnalysisDb,
    roots: &[ReachabilityRootFact],
) -> Vec<CallReachabilityFact> {
    let reachable = compute_reachable_set(db, roots);

    let mut marks: Vec<CallReachabilityFact> = db
        .call_sites()
        .iter()
        .map(|site| {
            let in_graph = reachable.contains(site.caller);
            let reason = if !in_graph {
                CallReachabilityReason::OutsideReachableGraph
            } else if reachable.is_root(site.caller) {
                CallReachabilityReason::RootSeed
            } else {
                CallReachabilityReason::ReachableFromRoot
            };
            CallReachabilityFact::new(&site.stable_key, in_graph, reason, REACHABILITY_PROVIDER_ID)
        })
        .collect();

    marks.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    marks
}

/// A direct-call edge participates in the reachable-set frontier only when its
/// target is resolved. (Ambiguous targets are conservatively excluded — the
/// pre-solver edge set marks only edges it is confident about; later solver phases
/// widen this behind the same contract.)
fn is_resolved_direct_edge(status: CallTargetStatus) -> bool {
    matches!(status, CallTargetStatus::Resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
        CallSyntaxKind, CallTargetFact, CallTargetStatus,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId, ReachabilityRootId};
    use crate::analysis::reachability::facts::{
        ReachabilityRootFact, RootKind, RootPrecision, RootProvenance, RootStatus,
        compute_reachability_root_stable_key,
    };
    use crate::analysis::reachability::store::ReachabilityProviderOutput;
    use crate::core::{FileId, FunctionFact, FunctionId, Language, Span};
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

    fn add_function(db: &mut AnalysisDb, file: FileId, id: u64, name: &str) -> FunctionId {
        db.push_function(FunctionFact {
            id: FunctionId(id),
            file,
            name: name.to_string(),
            span: span(file, id as u32, id as u32 + 1),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        })
    }

    fn site(id: u64, caller: FunctionId, stable_key: &str) -> CallSiteFact {
        CallSiteFact {
            in_throw: false,
            id: CallSiteId(id),
            language: Language::Go,
            file: FileId(0),
            caller,
            owner_symbol: None,
            body: MirBodyId(caller.0),
            operation: MirOpId(id),
            span: Span::point(FileId(0), 1, 1),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: stable_key.to_string(),
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            stable_key: stable_key.to_string(),
        }
    }

    fn target(
        id: u64,
        site: u64,
        caller: FunctionId,
        callee: Option<FunctionId>,
        status: CallTargetStatus,
    ) -> CallTargetFact {
        CallTargetFact {
            id: CallTargetId(id),
            site: CallSiteId(site),
            caller,
            target_function: callee,
            target_symbol: None,
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status,
            reason: None,
            provenance: CallProvenance::NativeDirect,
            precision: CallPrecision::SetupAware,
            stable_key: format!("call-target:{id}"),
        }
    }

    fn root(function: FunctionId, file: FileId) -> ReachabilityRootFact {
        let s = span(file, function.0 as u32, function.0 as u32 + 1);
        ReachabilityRootFact {
            id: ReachabilityRootId(0),
            kind: RootKind::Main,
            language: Language::Go,
            target_function: function,
            target_symbol: None,
            originating_entrypoint: None,
            file,
            span: s.clone(),
            precision: RootPrecision::ResolvedStatic,
            provenance: RootProvenance::NativeDiscovery,
            status: RootStatus::Resolved,
            provider_id: REACHABILITY_PROVIDER_ID.to_string(),
            stable_key: compute_reachability_root_stable_key(
                RootKind::Main,
                Language::Go,
                &format!("pkg.F{}", function.0),
                file,
                &s,
            ),
        }
    }

    /// Builds a db with:
    ///   root A; resolved edge A->B (site sAB); disconnected C with edge C->D
    ///   (site sCD); chain edge B->E (site sBE).
    fn db_with_call_graph() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("pkg/app.go"),
            "pkg/app.go".to_string(),
            "package pkg\n".to_string(),
        );
        let a = add_function(&mut db, file, 1, "A");
        let b = add_function(&mut db, file, 2, "B");
        let c = add_function(&mut db, file, 3, "C");
        let d = add_function(&mut db, file, 4, "D");
        let e = add_function(&mut db, file, 5, "E");

        db.replace_call_facts(CallOutput {
            sites: vec![
                site(10, a, "site-A->B"),
                site(11, c, "site-C->D"),
                site(12, b, "site-B->E"),
            ],
            targets: vec![
                target(20, 10, a, Some(b), CallTargetStatus::Resolved),
                target(21, 11, c, Some(d), CallTargetStatus::Resolved),
                target(22, 12, b, Some(e), CallTargetStatus::Resolved),
            ],
            unresolved: Vec::new(),
        })
        .expect("call facts");

        db.replace_reachability_facts(ReachabilityProviderOutput {
            roots: vec![root(a, file)],
            marks: Vec::new(),
        })
        .expect("reachability roots");

        db
    }

    fn mark_for<'a>(
        marks: &'a [CallReachabilityFact],
        site_stable_key: &str,
    ) -> &'a CallReachabilityFact {
        marks
            .iter()
            .find(|mark| mark.call_site_stable_key == site_stable_key)
            .unwrap_or_else(|| panic!("mark for {site_stable_key} missing"))
    }

    #[test]
    fn reachable_and_unreachable_call_sites_are_marked_correctly() {
        let db = db_with_call_graph();
        let marks = mark_call_reachability(&db, db.reachability_roots());

        // A->B is reachable (A is a root); C->D is not (C is disconnected);
        // B->E is reachable transitively (root A -> B -> E).
        assert!(mark_for(&marks, "site-A->B").in_reachable_graph);
        assert!(!mark_for(&marks, "site-C->D").in_reachable_graph);
        assert!(mark_for(&marks, "site-B->E").in_reachable_graph);
    }

    #[test]
    fn every_call_site_gets_exactly_one_mark_with_unique_stable_keys() {
        let db = db_with_call_graph();
        let marks = mark_call_reachability(&db, db.reachability_roots());

        assert_eq!(marks.len(), db.call_sites().len());
        let mut keys: Vec<&str> = marks.iter().map(|m| m.stable_key.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), marks.len(), "no duplicate mark stable keys");
    }

    #[test]
    fn transitive_reachability_marks_the_chained_site_reachable() {
        let db = db_with_call_graph();
        let marks = mark_call_reachability(&db, db.reachability_roots());
        let chained = mark_for(&marks, "site-B->E");
        assert!(chained.in_reachable_graph);
        // B is reachable through A, not a root itself.
        assert_eq!(chained.reason, "reachable-from-root");
    }

    #[test]
    fn root_seeded_site_carries_root_seed_reason() {
        let db = db_with_call_graph();
        let marks = mark_call_reachability(&db, db.reachability_roots());
        // A is a root, so its outgoing site is marked root-seed.
        assert_eq!(mark_for(&marks, "site-A->B").reason, "root-seed");
        // C is outside the graph entirely.
        assert_eq!(
            mark_for(&marks, "site-C->D").reason,
            "outside-reachable-graph"
        );
    }

    #[test]
    fn only_resolved_target_edges_extend_the_frontier() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("pkg/app.go"),
            "pkg/app.go".to_string(),
            "package pkg\n".to_string(),
        );
        let a = add_function(&mut db, file, 1, "A");
        let b = add_function(&mut db, file, 2, "B");

        // The only A->B edge is UNRESOLVED, so it must not extend the frontier.
        db.replace_call_facts(CallOutput {
            sites: vec![site(10, a, "site-A"), site(11, b, "site-B")],
            targets: vec![target(20, 10, a, Some(b), CallTargetStatus::Unresolved)],
            unresolved: Vec::new(),
        })
        .expect("call facts");
        db.replace_reachability_facts(ReachabilityProviderOutput {
            roots: vec![root(a, file)],
            marks: Vec::new(),
        })
        .expect("roots");

        let marks = mark_call_reachability(&db, db.reachability_roots());
        // A is the root (reachable), but B is NOT reachable because the only edge
        // is unresolved.
        assert!(mark_for(&marks, "site-A").in_reachable_graph);
        assert!(!mark_for(&marks, "site-B").in_reachable_graph);
    }

    #[test]
    fn permuted_insertion_order_produces_byte_identical_marks() {
        // Build the same logical graph twice with permuted call-fact insertion
        // order; after the traverse sort the marks must be byte-identical.
        let first = {
            let db = db_with_call_graph();
            mark_call_reachability(&db, db.reachability_roots())
        };

        let second = {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("pkg/app.go"),
                "pkg/app.go".to_string(),
                "package pkg\n".to_string(),
            );
            let a = add_function(&mut db, file, 1, "A");
            let b = add_function(&mut db, file, 2, "B");
            let c = add_function(&mut db, file, 3, "C");
            let d = add_function(&mut db, file, 4, "D");
            let e = add_function(&mut db, file, 5, "E");
            // Permuted: sites and targets inserted in reverse order.
            db.replace_call_facts(CallOutput {
                sites: vec![
                    site(12, b, "site-B->E"),
                    site(11, c, "site-C->D"),
                    site(10, a, "site-A->B"),
                ],
                targets: vec![
                    target(22, 12, b, Some(e), CallTargetStatus::Resolved),
                    target(21, 11, c, Some(d), CallTargetStatus::Resolved),
                    target(20, 10, a, Some(b), CallTargetStatus::Resolved),
                ],
                unresolved: Vec::new(),
            })
            .expect("call facts");
            db.replace_reachability_facts(ReachabilityProviderOutput {
                roots: vec![root(a, file)],
                marks: Vec::new(),
            })
            .expect("roots");
            mark_call_reachability(&db, db.reachability_roots())
        };

        assert_eq!(first, second);
        // Serialized form is also byte-identical (the determinism-gate precursor).
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
    }

    #[test]
    fn marking_does_not_mutate_the_call_store() {
        let db = db_with_call_graph();
        let sites_before = db.call_sites().to_vec();
        let targets_before = db.call_targets().to_vec();
        let unresolved_before = db.unresolved_calls().to_vec();

        let _marks = mark_call_reachability(&db, db.reachability_roots());

        assert_eq!(db.call_sites(), sites_before.as_slice());
        assert_eq!(db.call_targets(), targets_before.as_slice());
        assert_eq!(db.unresolved_calls(), unresolved_before.as_slice());
    }
}
