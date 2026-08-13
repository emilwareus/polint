use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CfgFunctionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CfgNodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BasicBlockId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CfgEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReachabilityId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DominatorId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PostDominatorId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ControlDependenceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UnsupportedControlFlowId(pub u64);

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
    fn cfg_id_newtypes_are_copy_ordered_hashable_and_serializable() {
        assert_small_id_contract::<CfgFunctionId>();
        assert_small_id_contract::<CfgNodeId>();
        assert_small_id_contract::<BasicBlockId>();
        assert_small_id_contract::<CfgEdgeId>();
        assert_small_id_contract::<ReachabilityId>();
        assert_small_id_contract::<DominatorId>();
        assert_small_id_contract::<PostDominatorId>();
        assert_small_id_contract::<ControlDependenceId>();
        assert_small_id_contract::<UnsupportedControlFlowId>();
    }

    #[test]
    fn dense_cfg_ids_sort_and_hash_as_run_local_handles() {
        let mut ordered = BTreeSet::new();
        ordered.insert(CfgNodeId(2));
        ordered.insert(CfgNodeId(1));

        let mut hashed = HashSet::new();
        hashed.insert(CfgNodeId(1));

        assert_eq!(
            ordered.into_iter().collect::<Vec<_>>(),
            vec![CfgNodeId(1), CfgNodeId(2)]
        );
        assert!(hashed.contains(&CfgNodeId(1)));
    }
}
