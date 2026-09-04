//! Stable-key helpers for Go semantic lowering.

use crate::analysis_api::{FactFamily, stable_key_text_from_parts};
use crate::internal_core::StableKeyInterner;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct StableFactKey(String);

impl StableFactKey {
    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

pub(crate) fn semantic_stable_key(family: FactFamily, parts: &[(&str, String)]) -> StableFactKey {
    StableFactKey(stable_key_text_from_parts(family, parts))
}
