#![allow(dead_code, reason = "kept for private internal consumers")]

use std::collections::{BTreeMap, BTreeSet};

use crate::error::AnalysisError;
use crate::object_model::facts::{
    TsObjectAllocationFact, TsObjectAllocationId, TsPropertyReadFact, TsPropertyReadId,
    TsPropertyWriteFact, TsPropertyWriteId, TsPrototypeLinkFact, TsPrototypeLinkId,
    TsReceiverBindingFact, TsReceiverBindingId,
};
use polint_analysis_api::{FactFamily, FactStore};
use polint_core::{FileId, StableKeyId, StableKeyInterner};
use std::any::Any;

pub const TS_OBJECT_MODEL_PROVIDER_ID: &str = "polint.ts.object_model";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TsObjectModelOutput {
    pub allocations: Vec<TsObjectAllocationFact>,
    pub property_writes: Vec<TsPropertyWriteFact>,
    pub property_reads: Vec<TsPropertyReadFact>,
    pub receiver_bindings: Vec<TsReceiverBindingFact>,
    pub prototype_links: Vec<TsPrototypeLinkFact>,
}

impl TsObjectModelOutput {
    pub fn normalized(mut self, interner: &StableKeyInterner) -> Self {
        normalize_rows(&mut self.allocations, |allocation| {
            (
                interner.resolve(allocation.stable_key),
                allocation.span.start_byte,
                allocation.span.end_byte,
            )
        });
        for (index, allocation) in self.allocations.iter_mut().enumerate() {
            allocation.id = TsObjectAllocationId(index as u64);
        }

        normalize_rows(&mut self.property_writes, |write| {
            (
                interner.resolve(write.stable_key),
                write.span.start_byte,
                write.span.end_byte,
            )
        });
        for (index, write) in self.property_writes.iter_mut().enumerate() {
            write.id = TsPropertyWriteId(index as u64);
        }

        normalize_rows(&mut self.property_reads, |read| {
            (
                interner.resolve(read.stable_key),
                read.span.start_byte,
                read.span.end_byte,
            )
        });
        for (index, read) in self.property_reads.iter_mut().enumerate() {
            read.id = TsPropertyReadId(index as u64);
        }

        normalize_rows(&mut self.receiver_bindings, |binding| {
            (
                interner.resolve(binding.stable_key),
                binding.span.start_byte,
                binding.span.end_byte,
            )
        });
        for (index, binding) in self.receiver_bindings.iter_mut().enumerate() {
            binding.id = TsReceiverBindingId(index as u64);
        }

        normalize_rows(&mut self.prototype_links, |link| {
            (
                interner.resolve(link.stable_key),
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

fn normalize_rows<T>(rows: &mut Vec<T>, key: impl Fn(&T) -> (std::sync::Arc<str>, u32, u32)) {
    rows.sort_by_cached_key(&key);
    rows.dedup_by(|left, right| key(left).0 == key(right).0);
}

#[derive(Debug, Clone, Default)]
pub struct TsObjectModelStore {
    output: TsObjectModelOutput,
    allocations_by_file: BTreeMap<FileId, Vec<usize>>,
    allocations_by_stable_key: BTreeMap<StableKeyId, usize>,
    property_writes_by_base: BTreeMap<StableKeyId, Vec<usize>>,
    property_reads_by_base: BTreeMap<StableKeyId, Vec<usize>>,
    receiver_bindings_by_callsite: BTreeMap<StableKeyId, Vec<usize>>,
    prototype_links_by_object: BTreeMap<StableKeyId, Vec<usize>>,
    prototype_links_by_prototype: BTreeMap<StableKeyId, Vec<usize>>,
}

impl TsObjectModelStore {
    pub fn from_output(output: TsObjectModelOutput, interner: &StableKeyInterner) -> Self {
        let output = output.normalized(interner);
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
                .insert(allocation.stable_key, index);
        }

        for (index, write) in store.output.property_writes.iter().enumerate() {
            store
                .property_writes_by_base
                .entry(write.base_object_stable_key)
                .or_default()
                .push(index);
        }

        for (index, read) in store.output.property_reads.iter().enumerate() {
            store
                .property_reads_by_base
                .entry(read.base_object_stable_key)
                .or_default()
                .push(index);
        }

        for (index, binding) in store.output.receiver_bindings.iter().enumerate() {
            if let Some(callsite) = binding.callsite_stable_key {
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
                .entry(link.object_stable_key)
                .or_default()
                .push(index);
            store
                .prototype_links_by_prototype
                .entry(link.prototype_stable_key)
                .or_default()
                .push(index);
        }

        store
    }

    pub fn try_from_output(
        output: TsObjectModelOutput,
        interner: &StableKeyInterner,
    ) -> Result<Self, AnalysisError> {
        validate_unique_stable_keys(
            "object allocation",
            output
                .allocations
                .iter()
                .map(|allocation| allocation.stable_key),
            interner,
        )?;
        validate_unique_stable_keys(
            "property write",
            output.property_writes.iter().map(|write| write.stable_key),
            interner,
        )?;
        validate_unique_stable_keys(
            "property read",
            output.property_reads.iter().map(|read| read.stable_key),
            interner,
        )?;
        validate_unique_stable_keys(
            "receiver binding",
            output
                .receiver_bindings
                .iter()
                .map(|binding| binding.stable_key),
            interner,
        )?;
        validate_unique_stable_keys(
            "prototype link",
            output.prototype_links.iter().map(|link| link.stable_key),
            interner,
        )?;

        Ok(Self::from_output(output, interner))
    }

    pub fn allocations(&self) -> &[TsObjectAllocationFact] {
        &self.output.allocations
    }

    pub fn property_writes(&self) -> &[TsPropertyWriteFact] {
        &self.output.property_writes
    }

    pub fn property_reads(&self) -> &[TsPropertyReadFact] {
        &self.output.property_reads
    }

    pub fn receiver_bindings(&self) -> &[TsReceiverBindingFact] {
        &self.output.receiver_bindings
    }

    pub fn prototype_links(&self) -> &[TsPrototypeLinkFact] {
        &self.output.prototype_links
    }

    pub fn allocations_for_file(&self, file: FileId) -> Vec<&TsObjectAllocationFact> {
        self.allocation_refs(self.allocations_by_file.get(&file))
    }

    pub fn allocation_by_stable_key(
        &self,
        stable_key: StableKeyId,
    ) -> Option<&TsObjectAllocationFact> {
        self.allocations_by_stable_key
            .get(&stable_key)
            .map(|index| &self.output.allocations[*index])
    }

    pub fn property_writes_for_base(
        &self,
        base_object_stable_key: StableKeyId,
    ) -> Vec<&TsPropertyWriteFact> {
        self.write_refs(self.property_writes_by_base.get(&base_object_stable_key))
    }

    pub fn property_reads_for_base(
        &self,
        base_object_stable_key: StableKeyId,
    ) -> Vec<&TsPropertyReadFact> {
        self.read_refs(self.property_reads_by_base.get(&base_object_stable_key))
    }

    pub fn receiver_bindings_for_callsite(
        &self,
        callsite_stable_key: StableKeyId,
    ) -> Vec<&TsReceiverBindingFact> {
        self.receiver_refs(self.receiver_bindings_by_callsite.get(&callsite_stable_key))
    }

    pub fn prototype_links_for_object(
        &self,
        object_stable_key: StableKeyId,
    ) -> Vec<&TsPrototypeLinkFact> {
        self.prototype_refs(self.prototype_links_by_object.get(&object_stable_key))
    }

    pub fn prototype_links_for_prototype(
        &self,
        prototype_stable_key: StableKeyId,
    ) -> Vec<&TsPrototypeLinkFact> {
        self.prototype_refs(self.prototype_links_by_prototype.get(&prototype_stable_key))
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

fn validate_unique_stable_keys(
    row_kind: &'static str,
    stable_keys: impl IntoIterator<Item = StableKeyId>,
    interner: &StableKeyInterner,
) -> Result<(), AnalysisError> {
    let mut seen = BTreeSet::new();
    for stable_key in stable_keys {
        if !seen.insert(stable_key) {
            return Err(AnalysisError::InvalidFact {
                provider: TS_OBJECT_MODEL_PROVIDER_ID,
                reason: format!(
                    "duplicate {row_kind} stable key `{}`",
                    interner.resolve(stable_key)
                ),
            });
        }
    }
    Ok(())
}

/// Registry key used for [`TsObjectModelStore`] in the host fact-store map.
pub const TS_OBJECT_MODEL_STORE_FAMILY: FactFamily = FactFamily::TsObjectModel;

impl FactStore for TsObjectModelStore {
    fn family(&self) -> FactFamily {
        FactFamily::TsObjectModel
    }

    fn clear(&mut self) {
        *self = TsObjectModelStore::default();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::object_model::facts::{
        TsObjectAllocationKind, TsObjectModelStatus, TsPropertyKey, TsPropertyKeyKind,
    };
    use polint_core::{FileId, Span};

    use super::*;

    #[test]
    fn normalized_sorts_deduplicates_and_assigns_dense_ids() {
        let interner = StableKeyInterner::default();
        let output = TsObjectModelOutput {
            allocations: vec![
                allocation(&interner, "object:b", 9, FileId::from_raw(2)),
                allocation(&interner, "object:a", 8, FileId::from_raw(1)),
                allocation(&interner, "object:a", 7, FileId::from_raw(1)),
            ],
            property_writes: vec![
                property_write(&interner, "write:b", 4, "object:b"),
                property_write(&interner, "write:a", 3, "object:a"),
                property_write(&interner, "write:a", 2, "object:a"),
            ],
            property_reads: vec![
                property_read(&interner, "read:b", 4, "object:b"),
                property_read(&interner, "read:a", 2, "object:a"),
            ],
            receiver_bindings: Vec::new(),
            prototype_links: Vec::new(),
        }
        .normalized(&interner);

        assert_eq!(
            output
                .allocations
                .iter()
                .map(|allocation| (interner.resolve(allocation.stable_key), allocation.id.0))
                .collect::<Vec<_>>(),
            vec![
                (std::sync::Arc::from("object:a"), 0),
                (std::sync::Arc::from("object:b"), 1)
            ]
        );
        assert_eq!(
            output
                .property_writes
                .iter()
                .map(|write| (interner.resolve(write.stable_key), write.id.0))
                .collect::<Vec<_>>(),
            vec![
                (std::sync::Arc::from("write:a"), 0),
                (std::sync::Arc::from("write:b"), 1)
            ]
        );
        assert_eq!(
            output
                .property_reads
                .iter()
                .map(|read| (interner.resolve(read.stable_key), read.id.0))
                .collect::<Vec<_>>(),
            vec![
                (std::sync::Arc::from("read:a"), 0),
                (std::sync::Arc::from("read:b"), 1)
            ]
        );
    }

    #[test]
    fn store_indexes_allocations_and_property_operations() {
        let interner = StableKeyInterner::default();
        let store = TsObjectModelStore::from_output(
            TsObjectModelOutput {
                allocations: vec![
                    allocation(&interner, "object:a", 9, FileId::from_raw(1)),
                    allocation(&interner, "object:b", 8, FileId::from_raw(2)),
                ],
                property_writes: vec![property_write(&interner, "write:a", 3, "object:a")],
                property_reads: vec![property_read(&interner, "read:a", 2, "object:a")],
                receiver_bindings: Vec::new(),
                prototype_links: Vec::new(),
            },
            &interner,
        );

        assert_eq!(store.allocations().len(), 2);
        assert_eq!(store.property_writes().len(), 1);
        assert_eq!(store.property_reads().len(), 1);
        assert_eq!(store.allocations_for_file(FileId::from_raw(1)).len(), 1);
        assert_eq!(
            store
                .allocation_by_stable_key(interner.intern("object:b"))
                .map(|allocation| allocation.file),
            Some(FileId::from_raw(2))
        );
        assert_eq!(
            store
                .property_writes_for_base(interner.intern("object:a"))
                .len(),
            1
        );
        assert_eq!(
            store
                .property_reads_for_base(interner.intern("object:a"))
                .len(),
            1
        );
    }

    #[test]
    fn try_from_output_rejects_duplicate_stable_keys() {
        let interner = StableKeyInterner::default();
        let error = TsObjectModelStore::try_from_output(
            TsObjectModelOutput {
                allocations: vec![
                    allocation(&interner, "object:a", 1, FileId::from_raw(1)),
                    allocation(&interner, "object:a", 2, FileId::from_raw(1)),
                ],
                property_writes: Vec::new(),
                property_reads: Vec::new(),
                receiver_bindings: Vec::new(),
                prototype_links: Vec::new(),
            },
            &interner,
        )
        .expect_err("duplicate allocation stable key should be invalid");

        assert_eq!(
            error.to_string(),
            "invalid fact from `polint.ts.object_model`: duplicate object allocation stable key `object:a`"
        );
    }

    fn allocation(
        interner: &StableKeyInterner,
        stable_key: &str,
        original_id: u64,
        file: FileId,
    ) -> TsObjectAllocationFact {
        TsObjectAllocationFact {
            id: TsObjectAllocationId(original_id),
            file,
            span: Span::point(file, original_id as u32, 1),
            stable_key: interner.intern(stable_key),
            lexical_parent_key: Some(interner.intern("scope:module")),
            inventory_function: None,
            inventory_function_stable_key: None,
            inventory_callsite: None,
            inventory_callsite_stable_key: None,
            kind: TsObjectAllocationKind::ObjectLiteral,
            status: TsObjectModelStatus::resolved(),
        }
    }

    fn property_write(
        interner: &StableKeyInterner,
        stable_key: &str,
        original_id: u64,
        base_object_stable_key: &str,
    ) -> TsPropertyWriteFact {
        TsPropertyWriteFact {
            id: TsPropertyWriteId(original_id),
            file: FileId::from_raw(1),
            span: Span::point(FileId::from_raw(1), original_id as u32, 1),
            stable_key: interner.intern(stable_key),
            base_object_stable_key: interner.intern(base_object_stable_key),
            property_key: property_key(),
            value_function: None,
            value_function_stable_key: Some(interner.intern("function:target")),
            value_object_stable_key: None,
            status: TsObjectModelStatus::resolved(),
        }
    }

    fn property_read(
        interner: &StableKeyInterner,
        stable_key: &str,
        original_id: u64,
        base_object_stable_key: &str,
    ) -> TsPropertyReadFact {
        TsPropertyReadFact {
            id: TsPropertyReadId(original_id),
            file: FileId::from_raw(1),
            span: Span::point(FileId::from_raw(1), original_id as u32, 1),
            stable_key: interner.intern(stable_key),
            base_object_stable_key: interner.intern(base_object_stable_key),
            property_key: property_key(),
            destination_stable_key: Some(interner.intern("place:callee")),
            callsite: None,
            callsite_stable_key: Some(interner.intern("callsite:holder.target")),
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
