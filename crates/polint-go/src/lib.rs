//! Go (tree-sitter) frontend: discover Go files, parse in parallel, merge facts and
//! parser diagnostics into a host [`polint_analysis_api::FactDatabase`], with optional
//! disk cache via [`polint_analysis_api::AnalysisCache`].

mod adapter;
pub mod error;
mod frontend;
mod hash;
pub mod lifecycle;
mod local_db;
mod mir;
pub mod module_graph;
#[doc(hidden)]
pub use mir::lower_go_mir;
#[allow(dead_code)]
mod repo_fs;
pub mod semantic;
mod stable_key;
mod syntax_store;

#[cfg(test)]
mod test_cache;
#[cfg(test)]
mod tests;

/// Re-export for `polint::_bench::go`; production callers use the plan-aware entrypoint.
#[allow(unreachable_pub, unused_imports)]
pub use adapter::analyze_with_options;
#[cfg(test)]
pub(crate) use adapter::{analyze, analyze_with_cache};
#[allow(unused_imports)]
pub(crate) use adapter::{
    analyze_files_with_plan_options_and_cache_stats, analyze_with_plan_options,
    analyze_with_plan_options_and_cache_stats,
};
pub use frontend::{FAMILY_GO, GO_FRONTEND_PROFILE, GoFrontend};
pub use lifecycle::{
    GoAnalysisConfig, GoLifecycleError, GoWorkspaceEnv, go_files, go_work_covers_module_roots,
    workspace_env,
};
pub use syntax_store::{GO_SYNTAX_STORE_FAMILY, GoSyntaxStore};
