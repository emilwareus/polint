//! TypeScript / JavaScript (Oxc) frontend: discover TS/JS files, parse in parallel,
//! merge facts and parser diagnostics into a host [`polint_analysis_api::FactDatabase`],
//! with optional disk cache via [`polint_analysis_api::AnalysisCache`].

mod adapter;
pub mod binding;
pub mod error;
mod frontend;
#[cfg(test)]
mod hash;
pub mod ids;
pub mod inventory;
mod local_db;
mod mir;
pub mod module_graph;
#[doc(hidden)]
pub use mir::lower_ts_mir;
pub mod object_model;
pub mod parse;
pub mod points_to;
#[allow(dead_code)]
mod repo_fs;
pub mod scope;
pub mod semantic_graph;
pub mod spans;
pub mod symbol_graph;
pub mod syntax_store;
pub mod token_flow;

use std::cell::RefCell;
use std::sync::Arc;

use polint_core::{StableKeyId, StableKeyInterner};

#[cfg(test)]
mod test_cache;
#[cfg(test)]
mod tests;

thread_local! {
    static FRONTEND_STABLE_KEYS: RefCell<Option<StableKeyInterner>> = const { RefCell::new(None) };
}

struct FrontendStableKeysGuard {
    previous: Option<StableKeyInterner>,
}

impl Drop for FrontendStableKeysGuard {
    fn drop(&mut self) {
        FRONTEND_STABLE_KEYS.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

pub fn with_frontend_stable_keys<T>(
    interner: &StableKeyInterner,
    operation: impl FnOnce() -> T,
) -> T {
    let previous = FRONTEND_STABLE_KEYS.with(|slot| slot.replace(Some(interner.clone())));
    let _guard = FrontendStableKeysGuard { previous };
    operation()
}

pub fn intern_frontend_stable_key(key: String) -> StableKeyId {
    FRONTEND_STABLE_KEYS.with(|slot| {
        slot.borrow()
            .as_ref()
            .expect("TS frontend extraction requires a stable-key interner")
            .intern(key)
    })
}

pub fn resolve_frontend_stable_key(key: StableKeyId) -> Arc<str> {
    FRONTEND_STABLE_KEYS.with(|slot| {
        slot.borrow()
            .as_ref()
            .expect("TS frontend extraction requires a stable-key interner")
            .resolve(key)
    })
}

pub use parse::{PARSER_RECOVERY_CONSTRUCT, parse_ts_file};

/// Re-export for `polint::_bench::ts`; production callers use the plan-aware entrypoint.
#[allow(unreachable_pub, unused_imports)]
pub use adapter::analyze_with_options;
#[allow(unused_imports)]
pub use adapter::{
    DYNAMIC_IMPORT_SPECIFIER, analyze_files_with_plan_options_and_cache_stats,
    analyze_with_plan_options, analyze_with_plan_options_and_cache_stats, class_callable_name,
};
#[cfg(test)]
pub(crate) use adapter::{analyze, analyze_with_cache};
pub use polint_analysis_api::{anonymous_callable_name, is_anonymous_callable_name};

pub use frontend::{FAMILY_TYPESCRIPT_JAVASCRIPT, TS_FRONTEND_PROFILE, TsJsFrontend};
pub use syntax_store::{TS_SYNTAX_STORE_FAMILY, TsSyntaxStore};
