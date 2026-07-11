//! Private JS/TS object/property solver driver (JS-05).
//!
//! This module owns the first object-model solver policy: allocation-site objects,
//! exact/computed property buckets, property read/write propagation, and
//! property-backed call dispatch. Prototype lookup and receiver binding are layered
//! on top in the next plan.

pub(crate) mod dispatch;
pub(crate) mod fixpoint;
pub(crate) mod inputs;
pub(crate) mod prototype;
pub(crate) mod receiver;

pub(crate) use fixpoint::solve_ts_object_model;
pub(crate) use inputs::TsObjectModelInputs;
