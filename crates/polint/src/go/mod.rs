//! Go (tree-sitter) adapter: discover Go files, parse in parallel, merge facts and
//! parser diagnostics into [`crate::core::AnalysisDb`], with optional disk cache.

mod adapter;
#[cfg(test)]
mod tests;

/// Re-export for `polint::_bench::go`; use `crate::go::analyze_with_options` in-tree.
#[allow(unreachable_pub)]
pub use adapter::analyze_with_options;
#[cfg(test)]
pub(crate) use adapter::{analyze, analyze_with_cache};
