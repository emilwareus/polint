//! Private repo-local adaptation model substrate.
//!
//! The public SDK does not expose this module. It loads, validates,
//! sort, budget, and digest source-evident model facts before later plans lower
//! accepted facts to semantic-graph `ModelEdge` constraints.

pub(crate) mod budget;
pub(crate) mod cache_key;
pub(crate) mod discovery;
pub(crate) mod facts;
pub(crate) mod loader;
pub(crate) mod store;
pub(crate) mod validate;

pub(crate) const ADAPTATION_MODEL_ALGORITHM: &str = "adaptation_model_v1";
