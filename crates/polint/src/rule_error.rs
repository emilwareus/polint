//! Typed errors for [`crate::core::Rule`] implementations.
//!
//! This module is crate-private; use [`crate::sdk::prelude::RuleError`] and
//! [`crate::sdk::prelude::RuleResult`] from rule packs.
//!
//! Rules return [`RuleResult`] from [`crate::core::Rule::run`]. Use `?` with
//! operations that produce [`anyhow::Error`] (including [`anyhow::bail!`]) —
//! they convert through [`From`].

/// Error returned when a rule's [`crate::core::Rule::run`] fails.
///
/// Wraps [`anyhow::Error`] so rule authors keep context chains while the
/// public surface stays a single `thiserror`-derived type.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct RuleError(#[from] anyhow::Error);

/// Convenient [`Result`] alias for rule implementations.
pub type RuleResult<T = ()> = std::result::Result<T, RuleError>;
