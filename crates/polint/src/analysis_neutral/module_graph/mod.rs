//! Language-neutral module graph models, queries, and topology facts.
//!
//! Frontend-specific discovery and resolution stays in the language crates; this
//! module owns the shared graph assembly contracts and fact transformations.

pub mod model;
pub mod query;
pub mod topology;
