//! polint: multi-language, repo-local static-analysis rules.
//!
//! Rule authors primarily use [`sdk`] and [`runner`]. Other modules are internal to this crate.

extern crate self as polint;

pub mod runner;
pub mod sdk;

pub use polint_macros::rule;

/// CLI entry used by the `polint` binary (`src/main.rs`).
pub fn run_main() -> anyhow::Result<u8> {
    cli::run()
}

pub(crate) mod analysis_kernel;
pub(crate) mod analysis_plan;
pub(crate) mod baseline;
pub(crate) mod cache;
pub(crate) mod cli;
pub(crate) mod config;
pub(crate) mod core;
pub(crate) mod diagnostics;
pub(crate) mod eval;
pub(crate) mod fs;
pub(crate) mod go;
#[cfg(test)]
pub(crate) mod graph;
pub(crate) mod ignores;
pub(crate) mod metrics;
pub(crate) mod module_graph;
pub(crate) mod path_context;
pub(crate) mod rule_error;
pub(crate) mod symbol_graph;
pub(crate) mod ts;

/// Internal surfaces for `polint-bench` (`feature = "bench"`). Not part of the supported API.
#[cfg(feature = "bench")]
pub mod _bench {
    /// Narrow surface for `polint-bench`: not a general `pub use crate::cache::*`.
    pub mod cache {
        pub use crate::cache::{Cache, stable_hash};
    }

    pub mod config {
        pub use crate::config::{LoadedConfig, load_config};
    }

    pub mod core {
        pub use crate::core::{AnalysisDb, Rule, RuleOptions, run_rules};
    }

    pub mod fs {
        pub use crate::fs::{LoadSourcesTimings, load_analysis_files_with_timings};
    }

    pub mod go {
        pub use crate::go::analyze_with_options;
    }

    pub mod ts {
        pub use crate::ts::analyze_with_options;
    }
    #[doc(hidden)]
    pub mod analysis_keys {
        use crate::config::LoadedConfig;
        use crate::core::{Rule, RuleOptions};
        use std::collections::BTreeMap;
        use std::collections::BTreeSet;

        #[inline]
        pub fn config_hash(config: &LoadedConfig) -> String {
            crate::cache::keys::config_hash(config)
        }

        #[inline]
        pub fn rule_hash(
            rules: &[Rule],
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
