//! TypeScript / JavaScript (Oxc) adapter: discover TS/JS files, parse in parallel,
//! merge facts and parser diagnostics into [`crate::core::AnalysisDb`], with
//! optional disk cache.

mod adapter;
pub(crate) mod binding;
pub(crate) mod inventory;
pub(crate) mod object_model;
pub(crate) mod parse;
pub(crate) mod scope;

use std::cell::RefCell;

use std::sync::Arc;

use crate::core::{StableKeyId, StableKeyInterner};

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

pub(crate) fn with_frontend_stable_keys<T>(
    interner: &StableKeyInterner,
    operation: impl FnOnce() -> T,
) -> T {
    let previous = FRONTEND_STABLE_KEYS.with(|slot| slot.replace(Some(interner.clone())));
    let _guard = FrontendStableKeysGuard { previous };
    operation()
}

pub(crate) fn intern_frontend_stable_key(key: String) -> StableKeyId {
    FRONTEND_STABLE_KEYS.with(|slot| {
        slot.borrow()
            .as_ref()
            .expect("TS frontend extraction requires a stable-key interner")
            .intern(key)
    })
}

pub(crate) fn resolve_frontend_stable_key(key: StableKeyId) -> Arc<str> {
    FRONTEND_STABLE_KEYS.with(|slot| {
        slot.borrow()
            .as_ref()
            .expect("TS frontend extraction requires a stable-key interner")
            .resolve(key)
    })
}
pub(crate) mod spans;
#[cfg(test)]
mod tests;

pub(crate) use parse::{PARSER_RECOVERY_CONSTRUCT, parse_ts_file};

/// Re-export for `polint::_bench::ts`; production callers use the crate-internal plan-aware entrypoint.
#[allow(unreachable_pub, unused_imports)]
pub use adapter::analyze_with_options;
#[allow(unused_imports)]
pub(crate) use adapter::{
    DYNAMIC_IMPORT_SPECIFIER, analyze_files_with_plan_options_and_cache_stats,
    analyze_with_plan_options, analyze_with_plan_options_and_cache_stats, anonymous_callable_name,
    class_callable_name, is_anonymous_callable_name,
};
#[cfg(test)]
pub(crate) use adapter::{analyze, analyze_with_cache};
