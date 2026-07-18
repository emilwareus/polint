//! Private Go semantic sidecar protocol and process client.
//!
//! This module stays crate-private. The sidecar is implementation infrastructure
//! for semantic graph producers, not a public SDK surface.

#![allow(dead_code)]

pub(crate) mod cache_key;
pub(crate) mod client;
pub(crate) mod diagnostics;
pub(crate) mod facts;
pub(crate) mod local_fs;
pub(crate) mod lower;
pub(crate) mod process;
pub(crate) mod protocol;
pub(crate) mod provider;
pub(crate) mod store;
pub(crate) mod validate;
#[cfg(windows)]
mod windows;

#[cfg(test)]
mod tests;
