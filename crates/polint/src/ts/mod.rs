//! TypeScript / JavaScript (Oxc) adapter: discover TS/JS files, parse in parallel,
//! merge facts and parser diagnostics into [`crate::core::AnalysisDb`], with
//! optional disk cache.

mod adapter;
pub(crate) mod binding;
pub(crate) mod inventory;
pub(crate) mod object_model;
pub(crate) mod parse;
pub(crate) mod scope;
pub(crate) mod spans;
#[cfg(test)]
mod tests;

pub(crate) use parse::{PARSER_RECOVERY_CONSTRUCT, parse_ts_file};

/// Re-export for `polint::_bench::ts`; production callers use the crate-internal plan-aware entrypoint.
#[allow(unreachable_pub, unused_imports)]
pub use adapter::analyze_with_options;
#[allow(unused_imports)]
pub(crate) use adapter::{
    DYNAMIC_IMPORT_SPECIFIER, analyze_with_plan_options, analyze_with_plan_options_and_cache_stats,
    anonymous_callable_name, class_callable_name, is_anonymous_callable_name,
};
#[cfg(test)]
pub(crate) use adapter::{analyze, analyze_with_cache};
