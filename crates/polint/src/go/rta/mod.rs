//! Go RTA composition facade.
//!
//! [`inputs`] projects Go facts into the frontend-neutral snapshot. Dispatch and
//! fixpoint execution remain owned by `analysis_neutral::solver::go_rta`.

pub(crate) mod inputs;

pub(crate) use crate::analysis_neutral::solver::go_rta::solve_rta as solve_go_rta;
pub(crate) use inputs::GoRtaInputs;
