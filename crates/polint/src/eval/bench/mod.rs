//! Benchmark measurement substrate (BENCH-01).
//!
//! Crate-private measurement primitives that later Phase 63 plans (curves,
//! report, baselines, regression gates) consume. This module measures real OS
//! peak RSS and cold/warm wall-clock, and defines the curve-point telemetry
//! types keyed by repo size and diff size. Nothing here is public/SDK/CLI
//! surface — everything stays `pub(crate)` under `eval`.

pub(crate) mod curve;
pub(crate) mod measure;
