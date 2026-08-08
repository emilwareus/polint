//! Evaluation harness package for polint.
//!
//! Harness sources live under `src/harness/` and are compiled into `polint` as
//! `crate::eval` under `cfg(test)` via a `#[path]` include from `polint`'s
//! `lib.rs`. Release builds of `polint` do not compile them.
//!
//! This crate's library target is intentionally empty so the harness is never
//! linked as a standalone product dependency.
