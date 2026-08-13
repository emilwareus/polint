//! Frontend-neutral Rapid Type Analysis input snapshot.
//!
//! Frontends project their stored facts into [`RtaInputs`] once. The dispatch and
//! fixpoint engine then consume only this closed, deterministic snapshot and never
//! read a frontend database or language-specific fact family.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis_neutral::ids::SemanticNodeId;
use crate::internal_core::StableKeyId;

/// One concrete Go method, indexed by its receiver type, for interface-invoke
/// resolution. `qualified` is the method's official identity; `node` is the unified
/// semantic-graph node the resolved edge targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RtaMethod {
    pub(crate) method_name: String,
    pub(crate) qualified: String,
    pub(crate) node: SemanticNodeId,
}

/// One pre-resolved interface-invoke candidate callee in the inverted dispatch index,
/// containing the target `node` plus the contributing-fact keys that justify the edge
/// (`[method_set_keys[type], instantiated_keys[type]]`, each present only if recorded).
/// Built once by the frontend projection so per-callsite resolution is a single
/// `BTreeMap` lookup instead of an O(whole-instantiated-set) scan — see
/// [`RtaInputs::interface_candidate_index`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RtaInterfaceCandidate {
    pub(crate) node: SemanticNodeId,
    pub(crate) contributing_keys: Vec<StableKeyId>,
}

/// The closed dispatch obligation for one `UnresolvedDynamic` Go callsite that maps
/// to a `CallConstraint` node in the semantic graph, joined with its dispatch detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RtaCallsite {
    /// The caller function's official `qualified` identity (a key into
    /// [`RtaInputs::function_node`] / the reachable set).
    pub(crate) caller: String,
    /// The unified `CallConstraint` callsite node (the edge SOURCE for resolved
    /// edges is the caller function node, not this — see the fixpoint).
    pub(crate) callsite_node: SemanticNodeId,
    /// The contributing callsite fact's stable key (recorded in edge provenance so
    /// the deletion-invalidation property holds).
    pub(crate) callsite_stable_key: StableKeyId,
    /// Interface-invoke discriminant: the invoked interface method name, if any.
    pub(crate) interface_method: Option<String>,
    /// Func-value-call discriminant: the call signature, if any.
    pub(crate) signature: Option<String>,
    /// The dynamic-dispatch detail fact's stable key (a second contributing fact for
    /// the resolved edge's provenance).
    pub(crate) dispatch_stable_key: StableKeyId,
}

/// The closed RTA input snapshot. See the module docs for the determinism contract.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RtaInputs {
    /// Reachability-root function identities (`qualified`) that map to a Go function
    /// node — the RTA seed.
    pub(crate) roots: BTreeSet<String>,
    /// Every `UnresolvedDynamic` callsite that maps to a `CallConstraint` node, with
    /// its dispatch detail joined by `callsite_stable_key`.
    pub(crate) callsites: Vec<RtaCallsite>,
    /// `type_name -> method names` (the method-set input). Text-keyed for RTA
    /// graph algorithms; ordered by type-name string, never by StableKeyId.
    pub(crate) method_sets: BTreeMap<String, BTreeSet<String>>,
    /// Lookup-only map: `type_name -> method-set fact StableKeyId` for provenance
    /// membership. Not an ordering surface; contributing keys are sorted by
    /// resolved text in `DerivedEdgeProvenance::new`.
    pub(crate) method_set_keys: BTreeMap<String, StableKeyId>,
    /// The instantiated runtime-type set (the RTA rapid-type set).
    pub(crate) instantiated: BTreeSet<String>,
    /// Lookup-only map: `type_name -> instantiated-type fact StableKeyId` for
    /// provenance membership (not an ordering surface).
    pub(crate) instantiated_keys: BTreeMap<String, StableKeyId>,
    /// Address-taken function identities (`qualified`) — func-value candidates.
    pub(crate) address_taken: BTreeSet<String>,
    /// Lookup-only map: `qualified -> address-taken fact StableKeyId` for
    /// provenance membership (not an ordering surface).
    pub(crate) address_taken_keys: BTreeMap<String, StableKeyId>,
    /// `qualified -> the function's unified semantic node` (edge endpoints).
    pub(crate) function_node: BTreeMap<String, SemanticNodeId>,
    /// `qualified -> the function's signature` (func-value matching).
    pub(crate) function_signature: BTreeMap<String, String>,
    /// `receiver type_name -> its concrete methods` (interface-invoke resolution).
    pub(crate) methods_by_receiver: BTreeMap<String, Vec<RtaMethod>>,
    /// Inverted interface-dispatch index `invoked method name -> candidate callees`
    /// Built once from `instantiated` ⋈ `method_sets` ⋈
    /// `methods_by_receiver` so resolving one interface-invoke callsite is a single
    /// `BTreeMap` lookup rather than an O(whole-instantiated-set) scan per callsite.
    /// This keeps total interface resolution from growing as O(C_iface · T) outside the
    /// worklist-step budget. Each per-method `Vec` is built by iterating `instantiated` in its `BTreeSet` (sorted)
    /// order and, within a type, `methods_by_receiver[type]` in its existing order, so
    /// the candidate order — and therefore the resolved edge set, the per-callsite cap
    /// prefix, and the stable keys — is byte-identical to the whole-set reference scan.
    /// A type whose method-set lacks the method, or that is not instantiated, contributes
    /// no entry (the same exclusions the scan applied).
    pub(crate) interface_candidate_index: BTreeMap<String, Vec<RtaInterfaceCandidate>>,
    /// `caller qualified -> statically-called callee qualified`s — the resolved STATIC
    /// call graph restricted to Go functions in [`Self::function_node`].
    /// Standard RTA reachability is the fixpoint closure over BOTH static-call edges and
    /// resolved-dynamic-dispatch edges from roots: a function reached only via a direct
    /// (static) call that is not itself a root must still enter the worklist so dispatch
    /// inside it is resolved. Static edges GROW reachability only — they do NOT emit
    /// `DerivedEdgeFact`s (only dynamic-dispatch resolution emits edges). Built from the
    /// `ResolvedStatic` callsite facts whose `static_callee` resolves to a known function
    /// identity; an unresolvable static callee is skipped (honest, no fabrication).
    pub(crate) static_call_targets: BTreeMap<String, BTreeSet<String>>,
}

impl RtaInputs {
    /// Rebuild the derived dispatch indexes from the primary RTA fields and return self.
    /// This is the single chokepoint that populates
    /// [`Self::interface_candidate_index`] — frontend adapters call it after building the primary
    /// fields, and any hand-constructed `RtaInputs` (tests) calls it so the index stays
    /// consistent with `instantiated` / `method_sets` / `methods_by_receiver`. Idempotent:
    /// the index is rebuilt from scratch, so calling it twice yields the same result.
    pub(crate) fn finalize_indexes(mut self) -> Self {
        self.interface_candidate_index = build_interface_candidate_index(
            &self.instantiated,
            &self.method_sets,
            &self.method_set_keys,
            &self.instantiated_keys,
            &self.methods_by_receiver,
        );
        self
    }
}

/// Build the inverted interface-dispatch index: `invoked method name ->
/// candidate callees`, enabling a single `BTreeMap` lookup while preserving the
/// byte-identical candidate order of a whole-instantiated-set scan.
///
/// **Byte-identity contract.** The reference scan for a query method `M` iterates
/// `instantiated` in `BTreeSet` (sorted) order and, for each instantiated type whose
/// `method_sets[type]` contains `M`, iterates `methods_by_receiver[type]` in order,
/// pushing every concrete method named `M` with `contributing_keys =
/// [method_set_keys[type]?, instantiated_keys[type]?]`. This builder visits the SAME
/// (type, concrete-method) pairs in the SAME order and files each concrete method under
/// the key of its OWN bare name — but only when `method_sets[type]` contains that name —
/// so for any `M`, `index[M]` equals the scan's output for `M` element-for-element
/// (set, order, contributing keys, and therefore the per-callsite cap prefix). A type
/// not in `instantiated`, or whose method-set lacks the method, is simply never visited
/// / never filed (the scan's exact exclusions).
fn build_interface_candidate_index(
    instantiated: &BTreeSet<String>,
    method_sets: &BTreeMap<String, BTreeSet<String>>,
    method_set_keys: &BTreeMap<String, StableKeyId>,
    instantiated_keys: &BTreeMap<String, StableKeyId>,
    methods_by_receiver: &BTreeMap<String, Vec<RtaMethod>>,
) -> BTreeMap<String, Vec<RtaInterfaceCandidate>> {
    let mut index: BTreeMap<String, Vec<RtaInterfaceCandidate>> = BTreeMap::new();
    // Iterate the instantiated set in BTreeSet (sorted) order — the scan's outer loop.
    for type_name in instantiated {
        // The type must declare a method-set; otherwise it dispatches nothing (the scan
        // `continue`d on a missing method-set).
        let Some(methods) = method_sets.get(type_name) else {
            continue;
        };
        let Some(concrete_methods) = methods_by_receiver.get(type_name) else {
            continue;
        };
        // The contributing-fact keys are per-TYPE, identical for every candidate of this
        // type — compute them once (the scan rebuilt the same Vec per candidate).
        let mut contributing_keys = Vec::new();
        if let Some(key) = method_set_keys.get(type_name) {
            contributing_keys.push(*key);
        }
        if let Some(key) = instantiated_keys.get(type_name) {
            contributing_keys.push(*key);
        }
        // Within a type, iterate `methods_by_receiver[type]` in its existing order — the
        // scan's inner loop — filing each concrete method under its own bare name, but
        // only when the method-set contains that name (the scan's `methods.contains`
        // guard, here applied per concrete method so the `index[M]` lookup is exact).
        for concrete in concrete_methods {
            if !methods.contains(&concrete.method_name) {
                continue;
            }
            index
                .entry(concrete.method_name.clone())
                .or_default()
                .push(RtaInterfaceCandidate {
                    node: concrete.node,
                    contributing_keys: contributing_keys.clone(),
                });
        }
    }
    index
}
