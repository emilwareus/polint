//! TypeScript / JavaScript (Oxc) adapter: discover TS/JS files, parse in parallel,
//! merge facts and parser diagnostics into [`crate::core::AnalysisDb`], with
//! optional disk cache.

mod adapter;
#[cfg(test)]
mod tests;

pub use adapter::*;
