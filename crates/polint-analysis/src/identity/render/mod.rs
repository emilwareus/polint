//! Per-benchmark identity renderers (D-05, D-06).
//!
//! Both renderers are pure `pub(crate)` functions over an [`crate::identity::facts::IdentityRecord`]
//! (plus, for Jelly, the borrowed [`polint_analysis_api::SourceFile`]). They are the
//! single source of truth for benchmark-matchable identity strings — eval
//! adapters call into these renderers rather than formatting names/spans
//! inline. No `pub use` lifting: call sites use the explicit
//! `render::go_relstring::render` / `render::jelly_span::render` paths so the
//! renderer being invoked is always unambiguous at the call site.

pub mod go_relstring;
pub mod jelly_span;
