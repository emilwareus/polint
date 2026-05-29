pub(crate) mod cache_key;
pub(crate) mod dedup;
pub(crate) mod facts;
pub(crate) mod provider;
pub(crate) mod render;
// Placed after `render` per Plan 42-03 BLOCKER #3 sequencing (Wave 3 adds the
// categorize submodule after the Plan 02 render submodule).
pub(crate) mod categorize;
pub(crate) mod store;
pub(crate) mod validate;
