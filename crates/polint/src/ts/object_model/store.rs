#![allow(
    dead_code,
    reason = "Phase 50 introduces the private TS object-model store before all solver consumers land"
)]

use std::collections::BTreeMap;

use crate::core::FileId;
use crate::ts::object_model::facts::{
    TsObjectAllocationFact, TsObjectAllocationId, TsPropertyReadFact, TsPropertyReadId,
    TsPropertyWriteFact, TsPropertyWriteId, TsPrototypeLinkFact, TsPrototypeLinkId,
    TsReceiverBindingFact, TsReceiverBindingId,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TsObjectModelOutput {
    pub(crate) allocations: Vec<TsObjectAllocationFact>,
    pub(crate) property_writes: Vec<TsPropertyWriteFact>,
    pub(crate) property_reads: Vec<TsPropertyReadFact>,
    pub(crate) receiver_bindings: Vec<TsReceiverBindingFact>,
    pub(crate) prototype_links: Vec<TsPrototypeLinkFact>,
}

impl TsObjectModelOutput {
    pub(crate) fn normalized(mut self) -> Self {
        normalize_rows(&mut self.allocations, |allocation| {
            (
                allocation.stable_key.as_str(),
                allocation.span.start_byte,
                allocation.span.end_byte,
            )
        });
        for (index, allocation) in self.allocations.iter_mut().enumerate() {
            allocation.id = TsObjectAllocationId(index as u64);
        }

        normalize_rows(&mut self.property_writes, |write| {
            (
                write.stable_key.as_str(),
                write.span.start_byte,
                write.span.end_byte,
            )
        });
        for (index, write) in self.property_writes.iter_mut().enumerate() {
            write.id = TsPropertyWriteId(index as u64);
        }

        normalize_rows(&mut self.property_reads, |read| {
            (
                read.stable_key.as_str(),
                read.span.start_byte,
                read.span.end_byte,
            )
        });
        for (index, read) in self.property_reads.iter_mut().enumerate() {
            read.id = TsPropertyReadId(index as u64);
        }

        normalize_rows(&mut self.receiver_bindings, |binding| {
            (
                binding.stable_key.as_str(),
                binding.span.start_byte,
                binding.span.end_byte,
            )
        });
        for (index, binding) in self.receiver_bindings.iter_mut().enumerate() {
            binding.id = TsReceiverBindingId(index as u64);
        }

        normalize_rows(&mut self.prototype_links, |link| {
            (
                link.stable_key.as_str(),
                link.span.start_byte,
                link.span.end_byte,
            )
        });
        for (index, link) in self.prototype_links.iter_mut().enumerate() {
            link.id = TsPrototypeLinkId(index as u64);
        }

        self
    }
}

fn normalize_rows<T>(rows: &mut Vec<T>, key: impl Fn(&T) -> (&str, u32, u32)) {
    rows.sort_by(|left, right| key(left).cmp(&key(right)));
    rows.dedup_by(|left, right| key(left).0 == key(right).0);
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TsObjectModelStore {
    output: TsObjectModelOutput,
    allocations_by_file: BTreeMap<FileId, Vec<usize>>,
    allocations_by_stable_key: BTreeMap<String, usize>,
    property_writes_by_base: BTreeMap<String, Vec<usize>>,
    property_reads_by_base: BTreeMap<String, Vec<usize>>,
    receiver_bindings_by_callsite: BTreeMap<String, Vec<usize>>,
    prototype_links_by_object: BTreeMap<String, Vec<usize>>,
    prototype_links_by_prototype: BTreeMap<String, Vec<usize>>,
}

impl TsObjectModelStore {
    pub(crate) fn from_output(output: TsObjectModelOutput) -> Self {
        let output = output.normalized();
        let mut store = Self {
            output,
            ..Self::default()
        };

        for (index, allocation) in store.output.allocations.iter().enumerate() {
            store
                .allocations_by_file
                .entry(allocation.file)
                .or_default()
                .push(index);
            store
                .allocations_by_stable_key
                .insert(allocation.stable_key.clone(), index);
        }

        for (index, write) in store.output.property_writes.iter().enumerate() {
            store
                .property_writes_by_base
                .entry(write.base_object_stable_key.clone())
                .or_default()
                .push(index);
        }

        for (index, read) in store.output.property_reads.iter().enumerate() {
            store
                .property_reads_by_base
                .entry(read.base_object_stable_key.clone())
                .or_default()
                .push(index);
        }

        for (index, binding) in store.output.receiver_bindings.iter().enumerate() {
            if let Some(callsite) = binding.callsite_stable_key.clone() {
                store
                    .receiver_bindings_by_callsite
                    .entry(callsite)
                    .or_default()
                    .push(index);
            }
        }

        for (index, link) in store.output.prototype_links.iter().enumerate() {
            store
                .prototype_links_by_object
                .entry(link.object_stable_key.clone())
                .or_default()
                .push(index);
            store
                .prototype_links_by_prototype
                .entry(link.prototype_stable_key.clone())
                .or_default()
                .push(index);
        }

        store
    }

    pub(crate) fn allocations(&self) -> &[TsObjectAllocationFact] {
        &self.output.allocations
    }

    pub(crate) fn property_writes(&self) -> &[TsPropertyWriteFact] {
        &self.output.property_writes
    }

    pub(crate) fn property_reads(&self) -> &[TsPropertyReadFact] {
        &self.output.property_reads
    }

    pub(crate) fn receiver_bindings(&self) -> &[TsReceiverBindingFact] {
        &self.output.receiver_bindings
    }

    pub(crate) fn prototype_links(&self) -> &[TsPrototypeLinkFact] {
        &self.output.prototype_links
    }

    pub(crate) fn allocations_for_file(&self, file: FileId) -> Vec<&TsObjectAllocationFact> {
        self.allocation_refs(self.allocations_by_file.get(&file))
    }

    pub(crate) fn allocation_by_stable_key(
        &self,
        stable_key: &str,
    ) -> Option<&TsObjectAllocationFact> {
        self.allocations_by_stable_key
            .get(stable_key)
            .map(|index| &self.output.allocations[*index])
    }

    pub(crate) fn property_writes_for_base(
        &self,
        base_object_stable_key: &str,
    ) -> Vec<&TsPropertyWriteFact> {
        self.write_refs(self.property_writes_by_base.get(base_object_stable_key))
    }

    pub(crate) fn property_reads_for_base(
        &self,
        base_object_stable_key: &str,
    ) -> Vec<&TsPropertyReadFact> {
        self.read_refs(self.property_reads_by_base.get(base_object_stable_key))
    }

    pub(crate) fn receiver_bindings_for_callsite(
        &self,
        callsite_stable_key: &str,
    ) -> Vec<&TsReceiverBindingFact> {
        self.receiver_refs(self.receiver_bindings_by_callsite.get(callsite_stable_key))
    }

    pub(crate) fn prototype_links_for_object(
        &self,
        object_stable_key: &str,
    ) -> Vec<&TsPrototypeLinkFact> {
        self.prototype_refs(self.prototype_links_by_object.get(object_stable_key))
    }

    pub(crate) fn prototype_links_for_prototype(
        &self,
        prototype_stable_key: &str,
    ) -> Vec<&TsPrototypeLinkFact> {
        self.prototype_refs(self.prototype_links_by_prototype.get(prototype_stable_key))
    }

    fn allocation_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&TsObjectAllocationFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.allocations[index])
                .collect()
        })
    }

    fn write_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&TsPropertyWriteFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.property_writes[index])
                .collect()
        })
    }

    fn read_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&TsPropertyReadFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.property_reads[index])
                .collect()
        })
    }

    fn receiver_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&TsReceiverBindingFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.receiver_bindings[index])
                .collect()
        })
    }

    fn prototype_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&TsPrototypeLinkFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.prototype_links[index])
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{FileId, Span};
    use crate::ts::object_model::facts::{
        TsObjectAllocationKind, TsObjectModelStatus, TsPropertyKey, TsPropertyKeyKind,
    };

    use super::*;

    #[test]
    fn normalized_sorts_deduplicates_and_assigns_dense_ids() {
        let output = TsObjectModelOutput {
            allocations: vec![
                allocation("object:b", 9, FileId(2)),
                allocation("object:a", 8, FileId(1)),
                allocation("object:a", 7, FileId(1)),
            ],
            property_writes: vec![
                property_write("write:b", 4, "object:b"),
                property_write("write:a", 3, "object:a"),
                property_write("write:a", 2, "object:a"),
            ],
            property_reads: vec![
                property_read("read:b", 4, "object:b"),
                property_read("read:a", 2, "object:a"),
            ],
            receiver_bindings: Vec::new(),
            prototype_links: Vec::new(),
        }
        .normalized();

        assert_eq!(
            output
                .allocations
                .iter()
                .map(|allocation| (allocation.stable_key.as_str(), allocation.id.0))
                .collect::<Vec<_>>(),
            vec![("object:a", 0), ("object:b", 1)]
        );
        assert_eq!(
            output
                .property_writes
                .iter()
                .map(|write| (write.stable_key.as_str(), write.id.0))
                .collect::<Vec<_>>(),
            vec![("write:a", 0), ("write:b", 1)]
        );
        assert_eq!(
            output
                .property_reads
                .iter()
                .map(|read| (read.stable_key.as_str(), read.id.0))
                .collect::<Vec<_>>(),
            vec![("read:a", 0), ("read:b", 1)]
        );
    }

    #[test]
    fn store_indexes_allocations_and_property_operations() {
        let store = TsObjectModelStore::from_output(TsObjectModelOutput {
            allocations: vec![
                allocation("object:a", 9, FileId(1)),
                allocation("object:b", 8, FileId(2)),
            ],
            property_writes: vec![property_write("write:a", 3, "object:a")],
            property_reads: vec![property_read("read:a", 2, "object:a")],
            receiver_bindings: Vec::new(),
            prototype_links: Vec::new(),
        });

        assert_eq!(store.allocations().len(), 2);
        assert_eq!(store.property_writes().len(), 1);
        assert_eq!(store.property_reads().len(), 1);
        assert_eq!(store.allocations_for_file(FileId(1)).len(), 1);
        assert_eq!(
            store
                .allocation_by_stable_key("object:b")
                .map(|allocation| allocation.file),
            Some(FileId(2))
        );
        assert_eq!(store.property_writes_for_base("object:a").len(), 1);
        assert_eq!(store.property_reads_for_base("object:a").len(), 1);
    }

    fn allocation(stable_key: &str, original_id: u64, file: FileId) -> TsObjectAllocationFact {
        TsObjectAllocationFact {
            id: TsObjectAllocationId(original_id),
            file,
            span: Span::point(file, original_id as u32, 1),
            stable_key: stable_key.to_string(),
            lexical_parent_key: Some("scope:module".to_string()),
            inventory_function: None,
            inventory_function_stable_key: None,
            inventory_callsite: None,
            inventory_callsite_stable_key: None,
            kind: TsObjectAllocationKind::ObjectLiteral,
            status: TsObjectModelStatus::resolved(),
        }
    }

    fn property_write(
        stable_key: &str,
        original_id: u64,
        base_object_stable_key: &str,
    ) -> TsPropertyWriteFact {
        TsPropertyWriteFact {
            id: TsPropertyWriteId(original_id),
            file: FileId(1),
            span: Span::point(FileId(1), original_id as u32, 1),
            stable_key: stable_key.to_string(),
            base_object_stable_key: base_object_stable_key.to_string(),
            property_key: property_key(),
            value_function: None,
            value_function_stable_key: Some("function:target".to_string()),
            value_object_stable_key: None,
            status: TsObjectModelStatus::resolved(),
        }
    }

    fn property_read(
        stable_key: &str,
        original_id: u64,
        base_object_stable_key: &str,
    ) -> TsPropertyReadFact {
        TsPropertyReadFact {
            id: TsPropertyReadId(original_id),
            file: FileId(1),
            span: Span::point(FileId(1), original_id as u32, 1),
            stable_key: stable_key.to_string(),
            base_object_stable_key: base_object_stable_key.to_string(),
            property_key: property_key(),
            destination_stable_key: Some("place:callee".to_string()),
            callsite: None,
            callsite_stable_key: Some("callsite:holder.target".to_string()),
            status: TsObjectModelStatus::resolved(),
        }
    }

    fn property_key() -> TsPropertyKey {
        TsPropertyKey {
            kind: TsPropertyKeyKind::Static,
            value: Some("target".to_string()),
        }
    }
}
