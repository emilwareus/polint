//! Benchmark curves, gates, and runners that consume crate-private measurement
//! primitives from [`crate::measure`].
//!
//! Peak RSS and cold/warm wall-clock live in `crate::measure` so the rules-host
//! golden-cost path can use them without compiling the eval harness. Nothing
//! here is public/SDK/CLI surface — everything stays `pub(crate)` under `eval`.

pub(crate) mod curve;
pub(crate) mod gate;
pub(crate) mod report;
pub(crate) mod retained_bytes;
pub(crate) mod runner;
pub(crate) mod scale_corpus;
pub(crate) mod sweep;
