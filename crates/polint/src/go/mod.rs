//! Go (tree-sitter) adapter: discover Go files, parse in parallel, merge facts and
//! parser diagnostics into [`crate::core::AnalysisDb`], with optional disk cache.

mod adapter;
#[cfg(test)]
mod tests;

/// Re-export for `polint::_bench::go`; production callers use the crate-internal plan-aware entrypoint.
#[allow(unreachable_pub, unused_imports)]
pub use adapter::analyze_with_options;
pub(crate) use adapter::analyze_with_plan_options;
#[cfg(test)]
pub(crate) use adapter::{analyze, analyze_with_cache};
