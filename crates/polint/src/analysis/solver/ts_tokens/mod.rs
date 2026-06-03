//! Private JS/TS function-token solver driver (JS-04).
//!
//! This module models callable function values only. It intentionally does **not**
//! model object properties, prototypes, receivers/`this`, native callbacks, or
//! framework dispatch. Those families remain honest unresolved reasons until their
//! own object-model/adaptation phases land.

pub(crate) mod inputs;

pub(crate) use inputs::TsTokenInputs;
