//! Whole-program reachability-from-roots (provider `polint.reachability`).
//!
//! This module discovers whole-program reachability roots (`main`/`init`, exported
//! functions, the framework/test entrypoint bridge, and configured
//! `.polint.toml` roots). Solvers consume these roots directly.
//!
//! D-02 naming-collision guard (MANDATORY): this whole-program module must NOT be
//! conflated with the **block-level** abstract domain `polint.domain.reachability`
//! at `crate::analysis::domains::core` (`ReachabilityDomain`:
//! `Reachable`/`Unreachable`/`Ambiguous` *inside one function body*). They are
//! different concepts at different granularities:
//!
//! - `polint.reachability` (here): whole-program reachability roots.
//! - `polint.domain.reachability` (`analysis::domains::core`): a local lattice
//!   describing whether a statement is reachable *within* a single CFG.
//!
//! Never reuse the `polint.domain.reachability` id or module for this work.

pub(crate) mod cache_key;
pub(crate) mod debug;
pub(crate) mod discover;
pub(crate) mod facts;
pub(crate) mod provider;
pub(crate) mod store;
pub(crate) mod validate;
