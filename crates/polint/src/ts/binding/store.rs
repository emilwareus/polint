#![allow(
    dead_code,
    reason = "Phase 45 expands private TS direct binding store indexes across sequential plans"
)]

use crate::analysis::ids::TsDirectBindingId;
use crate::ts::binding::facts::TsDirectBindingFact;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TsDirectBindingOutput {
    pub(crate) bindings: Vec<TsDirectBindingFact>,
}

impl TsDirectBindingOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.bindings.sort_by(|left, right| {
            (left.stable_key.as_str(), left.callsite_stable_key.as_str()).cmp(&(
                right.stable_key.as_str(),
                right.callsite_stable_key.as_str(),
            ))
        });
        for (index, binding) in self.bindings.iter_mut().enumerate() {
            binding.id = TsDirectBindingId(index as u64);
        }
        self
    }
}
