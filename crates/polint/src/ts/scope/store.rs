#![allow(
    dead_code,
    reason = "Phase 45 wires private TS scope stores into direct binding across sequential plans"
)]

use crate::analysis::ids::{TsBindingId, TsScopeId};
use crate::ts::scope::facts::{TsBindingFact, TsScopeFact};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TsScopeOutput {
    pub(crate) scopes: Vec<TsScopeFact>,
    pub(crate) bindings: Vec<TsBindingFact>,
}

impl TsScopeOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.scopes.sort_by(|left, right| {
            (
                left.stable_key.as_str(),
                left.span.start_byte,
                left.span.end_byte,
            )
                .cmp(&(
                    right.stable_key.as_str(),
                    right.span.start_byte,
                    right.span.end_byte,
                ))
        });
        for (index, scope) in self.scopes.iter_mut().enumerate() {
            scope.id = TsScopeId(index as u64);
        }

        self.bindings.sort_by(|left, right| {
            (
                left.stable_key.as_str(),
                left.span.start_byte,
                left.span.end_byte,
            )
                .cmp(&(
                    right.stable_key.as_str(),
                    right.span.start_byte,
                    right.span.end_byte,
                ))
        });
        for (index, binding) in self.bindings.iter_mut().enumerate() {
            binding.id = TsBindingId(index as u64);
        }

        self
    }
}
