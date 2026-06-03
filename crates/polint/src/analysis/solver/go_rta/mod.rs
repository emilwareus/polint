//! Private Go Rapid Type Analysis (RTA) driver (GO-05, D-01, D-04, D-06..D-13).
//!
//! This module is the second real [`super::policy::SolverPolicy`] implementation
//! (after `PointsToPolicy`), replacing the Phase 47 `GoRtaPolicy` honest stub. It
//! runs a hand-rolled RTA fixpoint over a closed snapshot of the Go-frontend facts
//! (Plan 1's address-taken / instantiated-type / dynamic-dispatch facts plus the
//! Phase 46 method-sets, callsites, and reachability roots) and emits resolved Go
//! call edges as [`super::facts::DerivedEdgeFact`]s in the unified vocabulary
//! (D-04) — never a parallel Go edge family. Every type here is `pub(crate)`
//! (D-01/D-17); nothing reaches the public SDK surface.
//!
//! ## D-04 naming-collision guard (MANDATORY)
//!
//! Two distinct vocabularies meet in this module; do NOT conflate them:
//!
//! - **The unified solver's derived-edge vocabulary.** Resolved call edges are
//!   [`super::facts::DerivedEdgeFact`]s whose `source`/`target` are unified
//!   [`crate::analysis::ids::SemanticNodeId`]s, whose status/precision reuse the
//!   shared `points_to::facts::{PointsToStatus, PointsToPrecision}` enums, and whose
//!   provenance records the producing
//!   [`crate::analysis::semantic_graph::constraints::ConstraintKind::CallConstraint`].
//!   This is the vocabulary the engine, store, and downstream consumers speak.
//! - **The Go-frontend fact vocabulary.** The RTA *inputs* are
//!   `crate::go::semantic::facts::{GoSemanticMethodSetFact, GoSemanticCallsiteFact,
//!   GoSemanticAddressTakenFact, GoSemanticInstantiatedTypeFact,
//!   GoSemanticDynamicDispatchFact}` keyed on official `go/types`/`ssa.Function`
//!   string identities (`qualified` function names, `type_name`s, method names).
//!   These are consumed read-only and mapped INTO the derived-edge vocabulary; they
//!   are never re-emitted as a parallel edge family, and their string identities are
//!   never confused with run-local dense `SemanticNodeId`s.
//!
//! The mapping seam is the `qualified -> SemanticNodeId` function index built in
//! [`inputs`] from the already-built `polint.semantic_graph` function nodes.
//!
//! ## RTA model (D-06)
//!
//! RTA = CHA filtered by the instantiated runtime-type set, seeded from roots:
//! - the reachable function set is seeded from the Phase 43 reachability roots and
//!   expanded as dispatch is resolved (D-07);
//! - an interface invoke at a reachable callsite resolves to the callees whose
//!   receiver type is in the instantiated-type set AND whose method-set contains the
//!   invoked method (the instantiated-type filter is what makes it RTA, not coarse
//!   CHA);
//! - a func-value call resolves to address-taken functions whose signature matches;
//! - the loop iterates reachability ⊗ dispatch to a fixed point under a budget.
//!
//! Honesty (D-08/D-09): a callsite whose interface type or method has no method-set
//! match contributes NO edge (it stays an honest unresolved obligation), derived
//! edges never claim exact precision, and an edge justified by multiple contributing
//! facts carries the WEAKEST status/precision across that derivation.

pub(crate) mod dispatch;
pub(crate) mod fixpoint;
pub(crate) mod inputs;

pub(crate) use fixpoint::solve_go_rta;
pub(crate) use inputs::GoRtaInputs;
