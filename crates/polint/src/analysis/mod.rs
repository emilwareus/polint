#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Private semantic contracts are introduced before lowering/provider integration in later plans."
    )
)]

pub(crate) use polint_analysis::access_paths;
pub(crate) use polint_analysis::aliases;
pub(crate) mod cache_key;
pub(crate) use polint_analysis::error;
pub(crate) mod error_convert;
pub(crate) use polint_analysis::ids;
pub(crate) use polint_analysis::places;
pub(crate) use polint_analysis::points_to;
pub(crate) use polint_analysis::stable_key;
pub(crate) use polint_analysis::store;
pub(crate) use polint_analysis::values;

pub(crate) mod adaptation;
pub(crate) mod calls;
pub(crate) mod cfg;
pub(crate) mod data_flow;
pub(crate) mod domains;
pub(crate) mod entrypoints;
pub(crate) mod evidence;
pub(crate) mod extensions;
pub(crate) mod identity;
pub(crate) mod ifds;
pub(crate) mod mir;
pub(crate) mod provider;
pub(crate) mod reachability;
pub(crate) mod refined_calls;
pub(crate) mod semantic_graph;
pub(crate) mod solver;
pub(crate) mod summaries;
pub(crate) mod types;
pub(crate) mod unknown_taxonomy;
pub(crate) mod validate;
