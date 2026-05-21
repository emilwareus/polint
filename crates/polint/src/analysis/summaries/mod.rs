#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 32 introduces summary kernel contracts before store/builder/provider integration in later plans."
    )
)]

pub(crate) mod builder;
pub(crate) mod cache_key;
pub(crate) mod core;
pub(crate) mod domain;
pub(crate) mod facts;
pub(crate) mod store;
