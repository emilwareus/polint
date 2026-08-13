//! Composition-root solver wiring.

pub(crate) use crate::analysis_neutral::solver::budget;
pub(crate) use crate::analysis_neutral::solver::cache_key;
pub(crate) use crate::analysis_neutral::solver::engine;
pub(crate) use crate::analysis_neutral::solver::facts;
#[cfg(test)]
pub(crate) use crate::analysis_neutral::solver::provenance;
pub(crate) use crate::analysis_neutral::solver::store;
pub(crate) mod policy;
pub(crate) mod provider;
pub(crate) use crate::analysis_neutral::solver::validate;
