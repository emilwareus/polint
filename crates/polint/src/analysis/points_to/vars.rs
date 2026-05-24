use crate::analysis::ids::{
    AbstractValueId, AccessPathId, AllocationTokenId, ObjectTokenId, PlaceId, PtVarId, ValueFactId,
};

const TAG_SHIFT: u64 = 56;
const PAYLOAD_MASK: u64 = (1_u64 << TAG_SHIFT) - 1;

const PLACE_VAR_TAG: u64 = 1;
const OPERATION_VAR_TAG: u64 = 2;
const ALLOCATION_VAR_TAG: u64 = 3;
const ACCESS_PATH_VAR_TAG: u64 = 4;
const DYNAMIC_VAR_TAG: u64 = 5;

const ALLOCATION_OBJECT_TAG: u64 = 1;
const ABSTRACT_VALUE_OBJECT_TAG: u64 = 2;
const VALUE_FACT_OBJECT_TAG: u64 = 3;

pub(crate) fn place_var(id: PlaceId) -> PtVarId {
    tagged_var(PLACE_VAR_TAG, id.0)
}

pub(crate) fn operation_var(id: crate::analysis::ids::MirOpId) -> PtVarId {
    tagged_var(OPERATION_VAR_TAG, id.0)
}

pub(crate) fn allocation_var(id: AllocationTokenId) -> PtVarId {
    tagged_var(ALLOCATION_VAR_TAG, id.0)
}

pub(crate) fn access_path_var(id: AccessPathId) -> PtVarId {
    tagged_var(ACCESS_PATH_VAR_TAG, id.0)
}

pub(crate) fn dynamic_var(slot_index: usize) -> PtVarId {
    tagged_var(DYNAMIC_VAR_TAG, slot_index as u64)
}

pub(crate) fn allocation_object(id: AllocationTokenId) -> ObjectTokenId {
    tagged_object(ALLOCATION_OBJECT_TAG, id.0)
}

pub(crate) fn abstract_value_object(id: AbstractValueId) -> ObjectTokenId {
    tagged_object(ABSTRACT_VALUE_OBJECT_TAG, id.0)
}

pub(crate) fn value_fact_object(id: ValueFactId) -> ObjectTokenId {
    tagged_object(VALUE_FACT_OBJECT_TAG, id.0)
}

fn tagged_var(tag: u64, payload: u64) -> PtVarId {
    PtVarId((tag << TAG_SHIFT) | (payload & PAYLOAD_MASK))
}

fn tagged_object(tag: u64, payload: u64) -> ObjectTokenId {
    ObjectTokenId((tag << TAG_SHIFT) | (payload & PAYLOAD_MASK))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{
        AbstractValueId, AccessPathId, AllocationTokenId, MirOpId, PlaceId, ValueFactId,
    };

    #[test]
    fn point_to_variable_ids_are_namespaced_by_source_kind() {
        let id = 42;

        let vars = [
            place_var(PlaceId(id)),
            operation_var(MirOpId(id)),
            allocation_var(AllocationTokenId(id)),
            access_path_var(AccessPathId(id)),
            dynamic_var(id as usize),
        ];

        for (left_index, left) in vars.iter().enumerate() {
            for right in vars.iter().skip(left_index + 1) {
                assert_ne!(left, right);
            }
        }
    }

    #[test]
    fn object_ids_are_namespaced_by_source_kind() {
        let id = 7;

        assert_ne!(
            allocation_object(AllocationTokenId(id)),
            abstract_value_object(AbstractValueId(id))
        );
        assert_ne!(
            allocation_object(AllocationTokenId(id)),
            value_fact_object(ValueFactId(id))
        );
        assert_ne!(
            abstract_value_object(AbstractValueId(id)),
            value_fact_object(ValueFactId(id))
        );
    }
}
