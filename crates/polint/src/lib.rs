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
}
