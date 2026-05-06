//! polint: multi-language, repo-local static-analysis rules.
//!
//! Rule authors primarily use [`sdk`] and [`runner`]. Other modules are internal to this crate.

pub mod runner;
pub mod sdk;

/// CLI entry used by the `polint` binary (`src/main.rs`).
pub fn run_main() -> anyhow::Result<u8> {
    cli::run()
}

pub(crate) mod cache;
pub(crate) mod cli;
pub(crate) mod config;
pub(crate) mod core;
pub(crate) mod diagnostics;
pub(crate) mod fs;
pub(crate) mod go;
pub(crate) mod graph;
pub(crate) mod rule_error;
pub(crate) mod ts;

/// Internal surfaces for `polint-bench` (`feature = "bench"`). Not part of the supported API.
#[cfg(feature = "bench")]
pub mod _bench {
    pub mod cache {
        pub use crate::cache::*;
    }
    pub mod config {
        pub use crate::config::*;
    }
    pub mod core {
        pub use crate::core::*;
    }
    pub mod fs {
        pub use crate::fs::*;
    }
    pub mod go {
        pub use crate::go::*;
    }
    pub mod ts {
        pub use crate::ts::*;
    }
    #[doc(hidden)]
    pub mod analysis_keys {
        use crate::config::LoadedConfig;
        use crate::core::{Rule, RuleOptions};
        use std::collections::BTreeMap;
        use std::collections::BTreeSet;
        use std::sync::Arc;

        #[inline]
        pub fn config_hash(config: &LoadedConfig) -> String {
            crate::cache::keys::config_hash(config)
        }

        #[inline]
        pub fn rule_hash(
            rules: &[Arc<dyn Rule>],
            enabled: Option<&BTreeSet<String>>,
            options: &BTreeMap<String, RuleOptions>,
        ) -> String {
            crate::cache::keys::rule_hash(rules, enabled, options)
        }

        #[inline]
        pub fn deterministic_rule_options(options: &RuleOptions) -> String {
            crate::cache::keys::deterministic_rule_options(options)
        }
    }
}
