//! Shared naming helpers for anonymous callables.
//!
//! Kept language-neutral so analysis families can classify callee evidence without
//! depending on the TypeScript frontend crate.

/// Prefix used for synthetic names of anonymous functions/classes.
pub const ANONYMOUS_CALLABLE_PREFIX: &str = "<polint:anonymous:";

/// Build the synthetic name for an anonymous callable spanning `[start, end)`.
pub fn anonymous_callable_name(start: u32, end: u32) -> String {
    format!("{ANONYMOUS_CALLABLE_PREFIX}{start}:{end}>")
}

/// Whether `value` looks like a synthetic anonymous-callable name.
pub fn is_anonymous_callable_name(value: &str) -> bool {
    value.starts_with(ANONYMOUS_CALLABLE_PREFIX) && value.ends_with('>')
}
