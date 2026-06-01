//! Private Go semantic sidecar protocol and process client.
//!
//! Phase 46 keeps this module crate-private. The sidecar is implementation
//! infrastructure for semantic graph producers, not a public SDK surface.

#![allow(dead_code)]

pub(crate) mod client;
pub(crate) mod process;
pub(crate) mod protocol;

#[cfg(test)]
mod tests;
