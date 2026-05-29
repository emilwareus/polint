//! Whole-program reachability-from-roots (provider `polint.reachability`).
//!
//! This module computes **whole-program** reachability: it discovers the set of
//! reachability *roots* (`main`/`init`, exported functions, the framework/test
//! entrypoint bridge, and configured `.polint.toml` roots) and — in later v1.3
//! plans — marks the call graph reachable from those roots. It is the input every
//! subsequent solver phase (44-54) consumes.
//!
//! D-02 naming-collision guard (MANDATORY): this whole-program module must NOT be
//! conflated with the **block-level** abstract domain `polint.domain.reachability`
//! at `crate::analysis::domains::core` (`ReachabilityDomain`:
//! `Reachable`/`Unreachable`/`Ambiguous` *inside one function body*). They are
//! different concepts at different granularities:
//!
//! - `polint.reachability` (here): whole-program reachability roots and the
//!   reachable function/call set derived from them.
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
pub(crate) mod traverse;
pub(crate) mod validate;
