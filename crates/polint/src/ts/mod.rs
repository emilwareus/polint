//! TypeScript / JavaScript (Oxc) adapter: discover TS/JS files, parse in parallel,
//! merge facts and parser diagnostics into [`crate::core::AnalysisDb`], with
//! optional disk cache.

mod adapter;
#[cfg(test)]
mod tests;

/// Re-export for `polint::_bench::ts`; use `crate::ts::analyze_with_options` in-tree.
#[allow(unreachable_pub)]
pub use adapter::analyze_with_options;
#[cfg(test)]
pub(crate) use adapter::{analyze, analyze_with_cache};
