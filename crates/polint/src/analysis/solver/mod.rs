//! Composition-root solver wiring.

pub(crate) use polint_analysis::solver::budget;
pub(crate) use polint_analysis::solver::cache_key;
pub(crate) use polint_analysis::solver::engine;
pub(crate) use polint_analysis::solver::facts;
pub(crate) use polint_analysis::solver::provenance;
pub(crate) use polint_analysis::solver::store;
pub(crate) mod go_rta;
pub(crate) mod policy;
pub(crate) mod provider;
pub(crate) use polint_analysis::solver::validate;
