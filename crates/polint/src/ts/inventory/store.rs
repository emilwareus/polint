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

    pub(crate) fn functions_for_file(&self, file: FileId) -> Vec<&TsInventoryFunctionFact> {
        self.function_refs(self.functions_by_file.get(&file))
    }

    pub(crate) fn callsites_for_file(&self, file: FileId) -> Vec<&TsInventoryCallsiteFact> {
        self.callsite_refs(self.callsites_by_file.get(&file))
    }

    pub(crate) fn function_by_stable_key(
        &self,
        stable_key: &str,
    ) -> Option<&TsInventoryFunctionFact> {
        self.functions_by_stable_key
            .get(stable_key)
            .map(|index| &self.output.functions[*index])
    }

    pub(crate) fn callsite_by_stable_key(
        &self,
        stable_key: &str,
    ) -> Option<&TsInventoryCallsiteFact> {
        self.callsites_by_stable_key
            .get(stable_key)
            .map(|index| &self.output.callsites[*index])
    }

    pub(crate) fn functions_by_kind(
        &self,
        kind: TsFunctionInventoryKind,
    ) -> Vec<&TsInventoryFunctionFact> {
        self.function_refs(self.functions_by_kind.get(&kind))
    }

    pub(crate) fn callsites_by_kind(
        &self,
        kind: TsCallsiteInventoryKind,
    ) -> Vec<&TsInventoryCallsiteFact> {
        self.callsite_refs(self.callsites_by_kind.get(&kind))
    }

    fn function_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&TsInventoryFunctionFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.functions[index])
                .collect()
        })
    }

    fn callsite_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&TsInventoryCallsiteFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.callsites[index])
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::ids::{TsInventoryCallsiteId, TsInventoryFunctionId};
    use crate::core::{FileId, Span};
    use crate::ts::inventory::facts::{
        TsCallsiteInventoryKind, TsFunctionInventoryKind, TsInventoryCallsiteFact,
        TsInventoryFunctionFact, TsInventoryStatus,
    };

    use super::*;

    #[test]
    fn normalized_assigns_dense_ids_after_stable_key_sort() {
        let output = TsInventoryOutput {
            functions: vec![
                function("function:b", TsInventoryFunctionId(40), FileId(1)),
                function("function:a", TsInventoryFunctionId(20), FileId(1)),
            ],
            callsites: vec![
                callsite("call:b", TsInventoryCallsiteId(8), FileId(1)),
                callsite("call:a", TsInventoryCallsiteId(4), FileId(1)),
            ],
        }
        .normalized();

        assert_eq!(
            output
                .functions
                .iter()
                .map(|function| (function.stable_key.as_str(), function.id.0))
                .collect::<Vec<_>>(),
            vec![("function:a", 0), ("function:b", 1)]
        );
        assert_eq!(
            output
                .callsites
                .iter()
                .map(|callsite| (callsite.stable_key.as_str(), callsite.id.0))
                .collect::<Vec<_>>(),
            vec![("call:a", 0), ("call:b", 1)]
        );
    }

    #[test]
    fn store_indexes_support_file_stable_key_and_kind_lookup() {
        let store = TsInventoryStore::from_output(TsInventoryOutput {
            functions: vec![
                function("function:a", TsInventoryFunctionId(10), FileId(1)),
                function("function:b", TsInventoryFunctionId(11), FileId(2)),
            ],
            callsites: vec![
                callsite("call:a", TsInventoryCallsiteId(10), FileId(1)),
                callsite("call:b", TsInventoryCallsiteId(11), FileId(2)),
            ],
        });

        assert_eq!(store.functions().len(), 2);
        assert_eq!(store.callsites().len(), 2);
        assert_eq!(store.functions_for_file(FileId(1)).len(), 1);
        assert_eq!(store.callsites_for_file(FileId(2)).len(), 1);
        assert_eq!(
            store
                .function_by_stable_key("function:a")
                .map(|function| function.file),
            Some(FileId(1))
        );
        assert_eq!(
            store
                .callsite_by_stable_key("call:b")
                .map(|callsite| callsite.file),
            Some(FileId(2))
        );
        assert_eq!(
            store
                .functions_by_kind(TsFunctionInventoryKind::Arrow)
                .len(),
            2
        );
        assert_eq!(
            store.callsites_by_kind(TsCallsiteInventoryKind::Call).len(),
            2
        );
    }

    fn function(
        stable_key: &str,
        id: TsInventoryFunctionId,
        file: FileId,
    ) -> TsInventoryFunctionFact {
        TsInventoryFunctionFact {
            id,
            file,
            span: span(file),
            stable_key: stable_key.to_string(),
            lexical_parent_key: Some("module".to_string()),
            display_name: Some(stable_key.to_string()),
            kind: TsFunctionInventoryKind::Arrow,
            status: TsInventoryStatus::Resolved,
        }
    }

    fn callsite(
        stable_key: &str,
        id: TsInventoryCallsiteId,
        file: FileId,
    ) -> TsInventoryCallsiteFact {
        TsInventoryCallsiteFact {
            id,
            file,
            span: span(file),
            stable_key: stable_key.to_string(),
            lexical_parent_key: Some("module".to_string()),
            display_name: Some(stable_key.to_string()),
            kind: TsCallsiteInventoryKind::Call,
            status: TsInventoryStatus::Resolved,
        }
    }

    fn span(file: FileId) -> Span {
        Span {
            file,
            start_byte: 1,
            end_byte: 5,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 5,
        }
    }
}
