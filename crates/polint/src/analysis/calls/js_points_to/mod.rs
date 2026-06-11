//! JS/TS value-token heap: an Andersen-style points-to fixpoint that resolves
//! call targets the per-syntax-form recognizers in [`super::ts_value_flows`]
//! cannot — value-flow depth, dynamic (constant-key) property flow on
//! function-objects and instances, closure parameter capture, object-return
//! chains. See `performance/2026-06-11-js-points-to-heap-plan.md`.
//!
//! Three layers:
//! - [`solver`] — the constraint vocabulary + worklist fixpoint (decoupled from
//!   the AST; unit-tested on hand-built constraints).
//! - [`harvest`] — walks the oxc AST per file into the solver's [`solver::Constraint`]
//!   vocabulary, reusing the span/identity conventions of the recognizers.
//! - [`provider`] — the db entry point: harvest → solve → map resolved edges back
//!   to [`crate::analysis::calls::facts::CallTargetFact`]s, emitted into
//!   `call_targets` alongside the recognizers (additive; deduped by stable key).

pub(crate) mod harvest;
pub(crate) mod provider;
pub(crate) mod solver;

pub(crate) use provider::resolve_js_points_to_targets;
