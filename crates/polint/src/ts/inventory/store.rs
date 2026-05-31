#![allow(
    dead_code,
    reason = "Phase 45 wires private inventory stores into DB/graph consumers across sequential plans"
)]

use std::collections::BTreeMap;

use crate::analysis::ids::{TsInventoryCallsiteId, TsInventoryFunctionId};
use crate::core::FileId;
use crate::ts::inventory::facts::{
    TsCallsiteInventoryKind, TsFunctionInventoryKind, TsInventoryCallsiteFact,
    TsInventoryFunctionFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TsInventoryOutput {
    pub(crate) functions: Vec<TsInventoryFunctionFact>,
    pub(crate) callsites: Vec<TsInventoryCallsiteFact>,
}

impl TsInventoryOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.functions.sort_by(|left, right| {
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
        for (index, function) in self.functions.iter_mut().enumerate() {
            function.id = TsInventoryFunctionId(index as u64);
        }

        self.callsites.sort_by(|left, right| {
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
        for (index, callsite) in self.callsites.iter_mut().enumerate() {
            callsite.id = TsInventoryCallsiteId(index as u64);
        }

        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TsInventoryStore {
    output: TsInventoryOutput,
    functions_by_file: BTreeMap<FileId, Vec<usize>>,
    callsites_by_file: BTreeMap<FileId, Vec<usize>>,
    functions_by_stable_key: BTreeMap<String, usize>,
    callsites_by_stable_key: BTreeMap<String, usize>,
    functions_by_kind: BTreeMap<TsFunctionInventoryKind, Vec<usize>>,
    callsites_by_kind: BTreeMap<TsCallsiteInventoryKind, Vec<usize>>,
}

impl TsInventoryStore {
    pub(crate) fn from_output(output: TsInventoryOutput) -> Self {
        let output = output.normalized();
        let mut store = Self {
            output,
            ..Self::default()
        };

        for (index, function) in store.output.functions.iter().enumerate() {
            store
                .functions_by_file
                .entry(function.file)
                .or_default()
                .push(index);
            store
                .functions_by_stable_key
                .insert(function.stable_key.clone(), index);
            store
                .functions_by_kind
                .entry(function.kind)
                .or_default()
                .push(index);
        }

        for (index, callsite) in store.output.callsites.iter().enumerate() {
            store
                .callsites_by_file
                .entry(callsite.file)
                .or_default()
                .push(index);
            store
                .callsites_by_stable_key
                .insert(callsite.stable_key.clone(), index);
            store
                .callsites_by_kind
                .entry(callsite.kind)
                .or_default()
                .push(index);
        }

        store
    }

    pub(crate) fn functions(&self) -> &[TsInventoryFunctionFact] {
        &self.output.functions
    }

    pub(crate) fn callsites(&self) -> &[TsInventoryCallsiteFact] {
        &self.output.callsites
    }
}
