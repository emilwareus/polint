#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 22 plan 01 defines the internal eval schema before later harness plans consume it"
    )
)]

pub(crate) mod matcher;
pub(crate) mod model;
pub(crate) mod report;
