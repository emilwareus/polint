//! Frontend-neutral Rapid Type Analysis dispatch and fixpoint engine.
//!
//! Language frontends adapt their facts into [`RtaInputs`]. This module owns only
//! the deterministic snapshot, dispatch, and worklist algorithm.

mod dispatch;
mod fixpoint;
mod snapshot;

pub(crate) use fixpoint::solve_rta;
pub(crate) use snapshot::{RtaCallsite, RtaInputs, RtaMethod};
