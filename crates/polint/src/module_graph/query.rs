use crate::core::{ModuleEdge, ModuleNodeId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[allow(dead_code)]
pub(crate) trait EdgeLike {
    fn endpoints(&self) -> (ModuleNodeId, ModuleNodeId);
}

impl EdgeLike for ModuleEdge {
    fn endpoints(&self) -> (ModuleNodeId, ModuleNodeId) {
        (self.from, self.to)
    }
}

impl EdgeLike for &ModuleEdge {
    fn endpoints(&self) -> (ModuleNodeId, ModuleNodeId) {
        (self.from, self.to)
    }
}

impl EdgeLike for (ModuleNodeId, ModuleNodeId) {
    fn endpoints(&self) -> (ModuleNodeId, ModuleNodeId) {
        *self
    }
}

#[allow(dead_code)]
pub(crate) fn outgoing(
    edges: &[ModuleEdge],
    node: ModuleNodeId,
) -> impl Iterator<Item = &ModuleEdge> {
    edges.iter().filter(move |edge| edge.from == node)
}

#[allow(dead_code)]
pub(crate) fn incoming(
    edges: &[ModuleEdge],
    node: ModuleNodeId,
) -> impl Iterator<Item = &ModuleEdge> {
    edges.iter().filter(move |edge| edge.to == node)
}

#[allow(dead_code)]
pub(crate) fn reachable_from<E>(
    start: ModuleNodeId,
    edges: impl IntoIterator<Item = E>,
) -> Vec<ModuleNodeId>
where
    E: EdgeLike,
{
    let mut adjacency: BTreeMap<ModuleNodeId, BTreeSet<ModuleNodeId>> = BTreeMap::new();
    for edge in edges {
        let (from, to) = edge.endpoints();
        adjacency.entry(from).or_default().insert(to);
    }

    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    let mut result = Vec::new();

    if let Some(next_nodes) = adjacency.get(&start) {
        for next in next_nodes {
            queue.push_back(*next);
        }
    }

    while let Some(node) = queue.pop_front() {
        if !seen.insert(node) {
            continue;
        }
        result.push(node);
        if let Some(next_nodes) = adjacency.get(&node) {
            for next in next_nodes {
                if !seen.contains(next) {
                    queue.push_back(*next);
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::reachable_from;
    use crate::core::ModuleNodeId;

    #[test]
    fn module_graph_ts_determinism_reachable_from_uses_deterministic_id_order() {
        let unsorted_edges = vec![
            (ModuleNodeId(0), ModuleNodeId(3)),
            (ModuleNodeId(0), ModuleNodeId(1)),
            (ModuleNodeId(2), ModuleNodeId(4)),
            (ModuleNodeId(0), ModuleNodeId(2)),
            (ModuleNodeId(1), ModuleNodeId(4)),
        ];

        assert_eq!(
            reachable_from(ModuleNodeId(0), unsorted_edges),
            vec![
                ModuleNodeId(1),
                ModuleNodeId(2),
                ModuleNodeId(3),
                ModuleNodeId(4)
            ]
        );
    }
}
