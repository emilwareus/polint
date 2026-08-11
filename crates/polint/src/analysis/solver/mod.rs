//! Private unified solver core.

pub(crate) use polint_analysis::solver::budget;
pub(crate) use polint_analysis::solver::facts;
pub(crate) use polint_analysis::solver::provenance;
pub(crate) use polint_analysis::solver::store;
pub(crate) mod cache_key;
pub(crate) mod engine;
pub(crate) mod go_rta;
pub(crate) mod policy;
pub(crate) mod provider;
pub(crate) mod validate;

#[cfg(test)]
#[path = "provenance_d09.rs"]
mod provenance_d09;
