#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Private semantic contracts are introduced before lowering/provider integration in later Phase 28 plans."
    )
)]

pub(crate) mod error;
pub(crate) mod ids;
pub(crate) mod mir;
pub(crate) mod places;
pub(crate) mod provider;
pub(crate) mod stable_key;
pub(crate) mod store;
