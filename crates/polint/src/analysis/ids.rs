use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirBodyId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirOpId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirStatementId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirTerminatorId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirValueId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirPredicateId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct PlaceId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct UnsupportedId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CallSiteId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CallTargetId(pub(crate) u64);

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use std::collections::{BTreeSet, HashSet};
    use std::fmt::Debug;
    use std::hash::Hash;

    fn assert_small_id_contract<T>()
    where
        T: Debug
            + Clone
            + Copy
            + PartialEq
            + Eq
            + PartialOrd
            + Ord
            + Hash
            + Serialize
            + DeserializeOwned,
    {
    }

    #[test]
    fn semantic_id_newtypes_are_copy_ordered_hashable_and_serializable() {
        assert_small_id_contract::<MirBodyId>();
        assert_small_id_contract::<MirOpId>();
        assert_small_id_contract::<MirStatementId>();
        assert_small_id_contract::<MirTerminatorId>();
        assert_small_id_contract::<MirValueId>();
        assert_small_id_contract::<MirPredicateId>();
        assert_small_id_contract::<PlaceId>();
        assert_small_id_contract::<UnsupportedId>();
        assert_small_id_contract::<CallSiteId>();
        assert_small_id_contract::<CallTargetId>();
    }

    #[test]
    fn dense_ids_sort_and_hash_as_run_local_handles() {
        let mut ordered = BTreeSet::new();
        ordered.insert(PlaceId(2));
        ordered.insert(PlaceId(1));

        let mut hashed = HashSet::new();
        hashed.insert(PlaceId(1));

        assert_eq!(
            ordered.into_iter().collect::<Vec<_>>(),
            vec![PlaceId(1), PlaceId(2)]
        );
        assert!(hashed.contains(&PlaceId(1)));
    }
}
