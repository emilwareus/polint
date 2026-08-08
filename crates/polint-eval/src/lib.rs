//! Evaluation harness package for polint.
//!
//! Harness modules live under [`harness`](crate) paths in this package and are
//! compiled into `polint` as `crate::eval` under `cfg(test)` via a `#[path]`
//! include. Release builds of `polint` do not compile them. This crate's own
//! library target is intentionally empty so the harness is never linked as a
//! standalone product dependency.
