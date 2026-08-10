//! Stable-key helpers for Go semantic lowering.

use polint_analysis_api::{FactFamily, stable_key_text_from_parts};
use polint_core::StableKeyInterner;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct StableFactKey(String);

impl StableFactKey {
    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

pub(crate) fn semantic_stable_key(
    interner: &StableKeyInterner,
    family: FactFamily,
    parts: &[(&str, String)],
) -> StableFactKey {
    StableFactKey(stable_key_text_from_parts(interner, family, parts))
}
