#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Private semantic contracts are introduced before lowering/provider integration in later plans."
    )
)]

pub(crate) mod access_paths;
pub(crate) mod adaptation;
pub(crate) mod aliases;
pub(crate) mod cache_key;
pub(crate) mod calls;
pub(crate) mod cfg;
pub(crate) mod data_flow;
pub(crate) mod demand;
pub(crate) mod domains;
pub(crate) mod entrypoints;
pub(crate) mod error;
pub(crate) mod evidence;
pub(crate) mod extensions;
pub(crate) mod identity;
pub(crate) mod ids;
pub(crate) mod ifds;
pub(crate) mod mir;
pub(crate) mod places;
pub(crate) mod points_to;
pub(crate) mod provider;
pub(crate) mod reachability;
pub(crate) mod refined_calls;
pub(crate) mod semantic_graph;
pub(crate) mod slicing;
pub(crate) mod solver;
pub(crate) mod stable_key;
pub(crate) mod store;
pub(crate) mod summaries;
pub(crate) mod types;
pub(crate) mod unknown_taxonomy;
pub(crate) mod validate;
pub(crate) mod values;
