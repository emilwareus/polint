//! Private unified semantic graph skeleton (provider `polint.semantic_graph`).
//!
//! This module holds the shared, byte-stable node/edge skeleton (GRAPH-01) that
//! v1.3's later solver phases write into and read from: the closed
//! [`facts::NodeKind`]/[`facts::EdgeKind`] taxonomies (composing the existing v1.2
//! identity newtypes by reference), the node/edge fact families, the run-local
//! dense `SemanticNodeId`/`SemanticEdgeId` handles, and the
//! [`store::SemanticGraphStore`] with deterministic indexes. Every type is
//! `pub(crate)`; nothing is promoted to the public SDK (the Phase 42 leak gate
//! must stay green).
//!
//! D-09 naming-collision guard (MANDATORY): this module's **unified frontend graph
//! vocabulary** — the `ConstraintKind` enum that lands in Plan 02 — is a distinct,
//! higher-level concept from the points-to sub-domain's internal
//! `crate::analysis::points_to::facts::PointsToConstraintKind`. The two are NOT the
//! same enum:
//!
//! - `semantic_graph::*::ConstraintKind` (Plan 02): the unified constraint
//!   vocabulary spanning the whole frontend graph (copy/alloc/field/call/model/type
//!   edges over `SemanticNodeId`s).
//! - `points_to::facts::PointsToConstraintKind`: the points-to solver's internal
//!   constraint shape over `PtVarId`/`ObjectTokenId`s.
//!
//! Phase 44 does **NOT** merge, rename, or delete the points-to enum. Folding the
//! points-to constraint vocabulary into the unified graph vocabulary is explicitly
//! deferred to Phase 47. Do not couple the two enums in code here; document the
//! conceptual map only.

pub(crate) mod build;
pub(crate) mod cache_key;
pub(crate) mod constraints;
#[cfg(test)]
pub(crate) mod debug;
pub(crate) mod facts;
pub(crate) mod provider;
pub(crate) mod store;
pub(crate) mod validate;
