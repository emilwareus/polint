use std::collections::{BTreeMap, BTreeSet};

use super::facts::{
    EvidenceBundleFact, EvidenceEdgeFact, EvidenceEdgeKind, EvidenceNodeFact, EvidenceNodeKind,
    EvidenceOmittedRegionFact, EvidencePathFact, EvidenceProvenance, EvidenceQueryMode,
    EvidenceReplayKeyFact, EvidenceSliceFact, EvidenceStatus, EvidenceUnknownFact,
};
use crate::analysis::error::AnalysisError;
use crate::analysis::ids::{
    EvidenceBundleId, EvidenceEdgeId, EvidenceNodeId, EvidenceOmittedRegionId, EvidencePathId,
    EvidenceSliceId,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EvidenceOutput {
    pub(crate) nodes: Vec<EvidenceNodeFact>,
    pub(crate) edges: Vec<EvidenceEdgeFact>,
    pub(crate) bundles: Vec<EvidenceBundleFact>,
    pub(crate) paths: Vec<EvidencePathFact>,
    pub(crate) slices: Vec<EvidenceSliceFact>,
    pub(crate) unknowns: Vec<EvidenceUnknownFact>,
    pub(crate) omitted_regions: Vec<EvidenceOmittedRegionFact>,
    pub(crate) replay_keys: Vec<EvidenceReplayKeyFact>,
}

impl EvidenceOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.nodes = normalize_nodes(self.nodes);
        self.omitted_regions = normalize_omitted_regions(self.omitted_regions);
        self.bundles = normalize_bundles(self.bundles);
        self.paths = self
            .paths
            .into_iter()
            .map(EvidencePathFact::normalized)
            .collect();
        self.slices = self
            .slices
            .into_iter()
            .map(EvidenceSliceFact::normalized)
            .collect();
        self.unknowns = self
            .unknowns
            .into_iter()
            .map(EvidenceUnknownFact::normalized)
            .collect();
        self.replay_keys = self
            .replay_keys
            .into_iter()
            .map(EvidenceReplayKeyFact::normalized)
            .collect();

        let node_remap = self
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, EvidenceNodeId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        let omitted_remap = self
            .omitted_regions
            .iter()
            .enumerate()
            .map(|(index, omitted)| (omitted.id, EvidenceOmittedRegionId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        let bundle_remap = self
            .bundles
            .iter()
            .enumerate()
            .map(|(index, bundle)| (bundle.id, EvidenceBundleId(index as u64)))
            .collect::<BTreeMap<_, _>>();

        for (index, node) in self.nodes.iter_mut().enumerate() {
            node.id = EvidenceNodeId(index as u64);
        }
        for (index, omitted) in self.omitted_regions.iter_mut().enumerate() {
            omitted.id = EvidenceOmittedRegionId(index as u64);
            remap_option(&mut omitted.bundle, &bundle_remap);
        }
        for (index, bundle) in self.bundles.iter_mut().enumerate() {
            bundle.id = EvidenceBundleId(index as u64);
            remap_option(&mut bundle.entry_node, &node_remap);
        }

        self.edges = self
            .edges
            .into_iter()
            .map(EvidenceEdgeFact::normalized)
            .map(|mut edge| {
                remap_required(&mut edge.from, &node_remap);
                remap_required(&mut edge.to, &node_remap);
                edge
            })
            .collect();
        self.edges.sort_by(|left, right| {
            (
                left.stable_key.as_str(),
                left.from,
                left.to,
                left.kind,
                left.query_mode,
                left.id,
            )
                .cmp(&(
                    right.stable_key.as_str(),
                    right.from,
                    right.to,
                    right.kind,
                    right.query_mode,
                    right.id,
                ))
        });
        let edge_remap = self
            .edges
            .iter()
            .enumerate()
            .map(|(index, edge)| (edge.id, EvidenceEdgeId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        for (index, edge) in self.edges.iter_mut().enumerate() {
            edge.id = EvidenceEdgeId(index as u64);
        }

        self.paths = self
            .paths
            .into_iter()
            .map(|mut path| {
                remap_option(&mut path.bundle, &bundle_remap);
                remap_vec(&mut path.nodes, &node_remap);
                remap_vec(&mut path.edges, &edge_remap);
                remap_vec(&mut path.omitted_regions, &omitted_remap);
                path
            })
            .collect();
        self.paths.sort_by(|left, right| {
            (left.stable_key.as_str(), left.rank, left.id).cmp(&(
                right.stable_key.as_str(),
                right.rank,
                right.id,
            ))
        });
        let path_remap = self
            .paths
            .iter()
            .enumerate()
            .map(|(index, path)| (path.id, EvidencePathId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        for (index, path) in self.paths.iter_mut().enumerate() {
            path.id = EvidencePathId(index as u64);
        }

        self.slices = self
            .slices
            .into_iter()
            .map(|mut slice| {
                remap_option(&mut slice.bundle, &bundle_remap);
                remap_vec(&mut slice.root_nodes, &node_remap);
                remap_vec(&mut slice.nodes, &node_remap);
                remap_vec(&mut slice.edges, &edge_remap);
                remap_vec(&mut slice.omitted_regions, &omitted_remap);
                slice
            })
            .collect();
        self.slices.sort_by(|left, right| {
            (left.stable_key.as_str(), left.query_mode, left.id).cmp(&(
                right.stable_key.as_str(),
                right.query_mode,
                right.id,
            ))
        });
        let slice_remap = self
            .slices
            .iter()
            .enumerate()
            .map(|(index, slice)| (slice.id, EvidenceSliceId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        for (index, slice) in self.slices.iter_mut().enumerate() {
            slice.id = EvidenceSliceId(index as u64);
        }

        for bundle in &mut self.bundles {
            remap_vec(&mut bundle.selected_paths, &path_remap);
            remap_vec(&mut bundle.selected_slices, &slice_remap);
        }
        for omitted in &mut self.omitted_regions {
            remap_option(&mut omitted.path, &path_remap);
            remap_option(&mut omitted.slice, &slice_remap);
        }
        for unknown in &mut self.unknowns {
            remap_option(&mut unknown.bundle, &bundle_remap);
            remap_option(&mut unknown.path, &path_remap);
            remap_option(&mut unknown.slice, &slice_remap);
            remap_option(&mut unknown.edge, &edge_remap);
        }
        for replay in &mut self.replay_keys {
            remap_required(&mut replay.bundle, &bundle_remap);
        }

        self.unknowns.sort_by(|left, right| {
            (left.stable_key.as_str(), left.reason).cmp(&(right.stable_key.as_str(), right.reason))
        });
        self.replay_keys.sort_by(|left, right| {
            (left.stable_key.as_str(), left.query_mode)
                .cmp(&(right.stable_key.as_str(), right.query_mode))
        });

        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EvidenceStore {
    output: EvidenceOutput,
    by_node_kind: BTreeMap<EvidenceNodeKind, Vec<usize>>,
    by_edge_kind: BTreeMap<EvidenceEdgeKind, Vec<usize>>,
    by_bundle: BTreeMap<EvidenceBundleId, Vec<usize>>,
    by_diagnostic_stable_key: BTreeMap<String, Vec<usize>>,
    by_source_fact_stable_key: BTreeMap<String, Vec<usize>>,
    by_query_mode: BTreeMap<EvidenceQueryMode, Vec<usize>>,
    by_status: BTreeMap<EvidenceStatus, Vec<usize>>,
    by_provenance: BTreeMap<EvidenceProvenance, Vec<usize>>,
    by_replay_key: BTreeMap<String, Vec<usize>>,
    outgoing: BTreeMap<EvidenceNodeId, Vec<usize>>,
    incoming: BTreeMap<EvidenceNodeId, Vec<usize>>,
}

impl EvidenceStore {
    pub(crate) fn from_output(output: EvidenceOutput) -> Result<Self, AnalysisError> {
        validate_references(&output)?;
        let output = output.normalized();
        validate_references(&output)?;
        reject_duplicates(
            output.nodes.iter().map(|node| node.stable_key.as_str()),
            "evidence node stable key",
        )?;
        reject_duplicates(
            output.edges.iter().map(|edge| edge.stable_key.as_str()),
            "evidence edge stable key",
        )?;
        reject_duplicates(
            output
                .bundles
                .iter()
                .map(|bundle| bundle.stable_key.as_str()),
            "evidence bundle stable key",
        )?;
        reject_duplicates(
            output.paths.iter().map(|path| path.stable_key.as_str()),
            "evidence path stable key",
        )?;
        reject_duplicates(
            output.slices.iter().map(|slice| slice.stable_key.as_str()),
            "evidence slice stable key",
        )?;
        reject_duplicates(
            output
                .omitted_regions
                .iter()
                .map(|omitted| omitted.stable_key.as_str()),
            "evidence omitted-region stable key",
        )?;
        reject_duplicates(
            output
                .unknowns
                .iter()
                .map(|unknown| unknown.stable_key.as_str()),
            "evidence unknown stable key",
        )?;
        reject_duplicates(
            output
                .replay_keys
                .iter()
                .map(|replay| replay.stable_key.as_str()),
            "evidence replay key stable key",
        )?;

        let mut store = Self {
            output,
            ..Self::default()
        };
        for (index, node) in store.output.nodes.iter().enumerate() {
            store.by_node_kind.entry(node.kind).or_default().push(index);
            store.by_status.entry(node.status).or_default().push(index);
            store
                .by_provenance
                .entry(node.provenance)
                .or_default()
                .push(index);
            for source_key in &node.source_fact_stable_keys {
                store
                    .by_source_fact_stable_key
                    .entry(source_key.clone())
                    .or_default()
                    .push(index);
            }
        }
        for (index, edge) in store.output.edges.iter().enumerate() {
            store.by_edge_kind.entry(edge.kind).or_default().push(index);
            store
                .by_query_mode
                .entry(edge.query_mode)
                .or_default()
                .push(index);
            store.by_status.entry(edge.status).or_default().push(index);
            store
                .by_provenance
                .entry(edge.provenance)
                .or_default()
                .push(index);
            for source_key in &edge.source_fact_stable_keys {
                store
                    .by_source_fact_stable_key
                    .entry(source_key.clone())
                    .or_default()
                    .push(index);
            }
            store.outgoing.entry(edge.from).or_default().push(index);
            store.incoming.entry(edge.to).or_default().push(index);
        }
        for (index, bundle) in store.output.bundles.iter().enumerate() {
            store
                .by_diagnostic_stable_key
                .entry(bundle.diagnostic_stable_key.clone())
                .or_default()
                .push(index);
            store
                .by_query_mode
                .entry(bundle.query_mode)
                .or_default()
                .push(index);
            if let Some(replay_key) = &bundle.replay_key {
                store
                    .by_replay_key
                    .entry(replay_key.clone())
                    .or_default()
                    .push(index);
            }
        }
        for (index, path) in store.output.paths.iter().enumerate() {
            if let Some(bundle) = path.bundle {
                store.by_bundle.entry(bundle).or_default().push(index);
            }
        }
        Ok(store)
    }

    pub(crate) fn nodes(&self) -> &[EvidenceNodeFact] {
        &self.output.nodes
    }

    pub(crate) fn edges(&self) -> &[EvidenceEdgeFact] {
        &self.output.edges
    }

    pub(crate) fn bundles(&self) -> &[EvidenceBundleFact] {
        &self.output.bundles
    }

    pub(crate) fn paths(&self) -> &[EvidencePathFact] {
        &self.output.paths
    }

    pub(crate) fn slices(&self) -> &[EvidenceSliceFact] {
        &self.output.slices
    }

    pub(crate) fn unknowns(&self) -> &[EvidenceUnknownFact] {
        &self.output.unknowns
    }

    pub(crate) fn omitted_regions(&self) -> &[EvidenceOmittedRegionFact] {
        &self.output.omitted_regions
    }

    pub(crate) fn replay_keys(&self) -> &[EvidenceReplayKeyFact] {
        &self.output.replay_keys
    }

    #[allow(
        dead_code,
        reason = "Private evidence query helpers are consumed by subsequent Phase 39 path/rendering plans."
    )]
    pub(crate) fn node(&self, node: EvidenceNodeId) -> Option<&EvidenceNodeFact> {
        self.output.nodes.get(node.0 as usize)
    }

    #[allow(
        dead_code,
        reason = "Private evidence query helpers are consumed by subsequent Phase 39 path/rendering plans."
    )]
    pub(crate) fn edge(&self, edge: EvidenceEdgeId) -> Option<&EvidenceEdgeFact> {
        self.output.edges.get(edge.0 as usize)
    }

    pub(crate) fn incoming(&self, node: EvidenceNodeId) -> Vec<&EvidenceEdgeFact> {
        self.edge_refs(self.incoming.get(&node))
    }

    pub(crate) fn outgoing(&self, node: EvidenceNodeId) -> Vec<&EvidenceEdgeFact> {
        self.edge_refs(self.outgoing.get(&node))
    }

    #[allow(
        dead_code,
        reason = "Private evidence query helpers are consumed by subsequent Phase 39 path/rendering plans."
    )]
    pub(crate) fn by_edge_kind(&self, kind: EvidenceEdgeKind) -> Vec<&EvidenceEdgeFact> {
        self.edge_refs(self.by_edge_kind.get(&kind))
    }

    fn edge_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&EvidenceEdgeFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|index| &self.output.edges[*index])
                .collect()
        })
    }
}

fn normalize_nodes(mut nodes: Vec<EvidenceNodeFact>) -> Vec<EvidenceNodeFact> {
    nodes = nodes
        .into_iter()
        .map(EvidenceNodeFact::normalized)
        .collect();
    nodes.sort_by(|left, right| {
        (left.stable_key.as_str(), left.kind, left.id).cmp(&(
            right.stable_key.as_str(),
            right.kind,
            right.id,
        ))
    });
    nodes
}

fn normalize_omitted_regions(
    mut omitted_regions: Vec<EvidenceOmittedRegionFact>,
) -> Vec<EvidenceOmittedRegionFact> {
    omitted_regions.sort_by(|left, right| {
        (left.stable_key.as_str(), left.reason, left.id).cmp(&(
            right.stable_key.as_str(),
            right.reason,
            right.id,
        ))
    });
    omitted_regions
}

fn normalize_bundles(bundles: Vec<EvidenceBundleFact>) -> Vec<EvidenceBundleFact> {
    let mut bundles = bundles
        .into_iter()
        .map(EvidenceBundleFact::normalized)
        .collect::<Vec<_>>();
    bundles.sort_by(|left, right| {
        (left.stable_key.as_str(), left.query_mode, left.id).cmp(&(
            right.stable_key.as_str(),
            right.query_mode,
            right.id,
        ))
    });
    bundles
}

fn validate_references(output: &EvidenceOutput) -> Result<(), AnalysisError> {
    reject_duplicate_ids(output.nodes.iter().map(|node| node.id), "evidence node id")?;
    reject_duplicate_ids(output.edges.iter().map(|edge| edge.id), "evidence edge id")?;
    reject_duplicate_ids(
        output.bundles.iter().map(|bundle| bundle.id),
        "evidence bundle id",
    )?;
    reject_duplicate_ids(output.paths.iter().map(|path| path.id), "evidence path id")?;
    reject_duplicate_ids(
        output.slices.iter().map(|slice| slice.id),
        "evidence slice id",
    )?;
    reject_duplicate_ids(
        output.omitted_regions.iter().map(|omitted| omitted.id),
        "evidence omitted-region id",
    )?;

    let nodes = output
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<BTreeSet<_>>();
    let edges = output
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<BTreeSet<_>>();
    let bundles = output
        .bundles
        .iter()
        .map(|bundle| bundle.id)
        .collect::<BTreeSet<_>>();
    let paths = output
        .paths
        .iter()
        .map(|path| path.id)
        .collect::<BTreeSet<_>>();
    let slices = output
        .slices
        .iter()
        .map(|slice| slice.id)
        .collect::<BTreeSet<_>>();
    let omitted = output
        .omitted_regions
        .iter()
        .map(|region| region.id)
        .collect::<BTreeSet<_>>();

    for edge in &output.edges {
        if !nodes.contains(&edge.from) || !nodes.contains(&edge.to) {
            return invalid(format!(
                "evidence edge `{}` references missing endpoint",
                edge.stable_key
            ));
        }
    }
    for bundle in &output.bundles {
        if bundle.entry_node.is_some_and(|node| !nodes.contains(&node)) {
            return invalid(format!(
                "evidence bundle `{}` references missing entry node",
                bundle.stable_key
            ));
        }
        require_all(
            &bundle.selected_paths,
            &paths,
            "path",
            &bundle.stable_key,
            "bundle",
        )?;
        require_all(
            &bundle.selected_slices,
            &slices,
            "slice",
            &bundle.stable_key,
            "bundle",
        )?;
    }
    for path in &output.paths {
        require_option(path.bundle, &bundles, "bundle", &path.stable_key, "path")?;
        require_all(&path.nodes, &nodes, "node", &path.stable_key, "path")?;
        require_all(&path.edges, &edges, "edge", &path.stable_key, "path")?;
        require_all(
            &path.omitted_regions,
            &omitted,
            "omitted region",
            &path.stable_key,
            "path",
        )?;
    }
    for slice in &output.slices {
        require_option(slice.bundle, &bundles, "bundle", &slice.stable_key, "slice")?;
        require_all(
            &slice.root_nodes,
            &nodes,
            "root node",
            &slice.stable_key,
            "slice",
        )?;
        require_all(&slice.nodes, &nodes, "node", &slice.stable_key, "slice")?;
        require_all(&slice.edges, &edges, "edge", &slice.stable_key, "slice")?;
        require_all(
            &slice.omitted_regions,
            &omitted,
            "omitted region",
            &slice.stable_key,
            "slice",
        )?;
    }
    for unknown in &output.unknowns {
        require_option(
            unknown.bundle,
            &bundles,
            "bundle",
            &unknown.stable_key,
            "unknown",
        )?;
        require_option(unknown.path, &paths, "path", &unknown.stable_key, "unknown")?;
        require_option(
            unknown.slice,
            &slices,
            "slice",
            &unknown.stable_key,
            "unknown",
        )?;
        require_option(unknown.edge, &edges, "edge", &unknown.stable_key, "unknown")?;
    }
    for region in &output.omitted_regions {
        require_option(
            region.bundle,
            &bundles,
            "bundle",
            &region.stable_key,
            "omitted",
        )?;
        require_option(region.path, &paths, "path", &region.stable_key, "omitted")?;
        require_option(
            region.slice,
            &slices,
            "slice",
            &region.stable_key,
            "omitted",
        )?;
    }
    for replay in &output.replay_keys {
        require_one(
            replay.bundle,
            &bundles,
            "bundle",
            &replay.stable_key,
            "replay",
        )?;
    }

    Ok(())
}

fn require_all<T>(
    values: &[T],
    allowed: &BTreeSet<T>,
    label: &'static str,
    owner_key: &str,
    owner_kind: &'static str,
) -> Result<(), AnalysisError>
where
    T: Copy + Ord,
{
    for value in values {
        require_one(*value, allowed, label, owner_key, owner_kind)?;
    }
    Ok(())
}

fn require_option<T>(
    value: Option<T>,
    allowed: &BTreeSet<T>,
    label: &'static str,
    owner_key: &str,
    owner_kind: &'static str,
) -> Result<(), AnalysisError>
where
    T: Copy + Ord,
{
    if let Some(value) = value {
        require_one(value, allowed, label, owner_key, owner_kind)?;
    }
    Ok(())
}

fn require_one<T>(
    value: T,
    allowed: &BTreeSet<T>,
    label: &'static str,
    owner_key: &str,
    owner_kind: &'static str,
) -> Result<(), AnalysisError>
where
    T: Ord,
{
    if allowed.contains(&value) {
        Ok(())
    } else {
        invalid(format!(
            "evidence {owner_kind} `{owner_key}` references missing {label}"
        ))
    }
}

fn reject_duplicate_ids<T>(
    ids: impl Iterator<Item = T>,
    label: &'static str,
) -> Result<(), AnalysisError>
where
    T: Copy + Ord + std::fmt::Debug,
{
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return invalid(format!("duplicate {label} `{id:?}`"));
        }
    }
    Ok(())
}

fn reject_duplicates<'a>(
    keys: impl Iterator<Item = &'a str>,
    label: &'static str,
) -> Result<(), AnalysisError> {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key.to_string()) {
            return invalid(format!("duplicate {label} `{key}`"));
        }
    }
    Ok(())
}

fn invalid<T>(reason: String) -> Result<T, AnalysisError> {
    Err(AnalysisError::InvalidFact {
        provider: "polint.evidence",
        reason,
    })
}

fn remap_option<T>(value: &mut Option<T>, remap: &BTreeMap<T, T>)
where
    T: Copy + Ord,
{
    if let Some(remapped) = value.and_then(|id| remap.get(&id).copied()) {
        *value = Some(remapped);
    }
}

fn remap_required<T>(value: &mut T, remap: &BTreeMap<T, T>)
where
    T: Copy + Ord,
{
    if let Some(remapped) = remap.get(value).copied() {
        *value = remapped;
    }
}

fn remap_vec<T>(values: &mut Vec<T>, remap: &BTreeMap<T, T>)
where
    T: Copy + Ord,
{
    for value in values {
        remap_required(value, remap);
    }
}

#[allow(
    dead_code,
    reason = "Evidence construction helpers are consumed by subsequent Phase 39 slice/path plans."
)]
pub(crate) fn next_evidence_node_id(nodes: &[EvidenceNodeFact]) -> EvidenceNodeId {
    EvidenceNodeId(nodes.len() as u64)
}

#[allow(
    dead_code,
    reason = "Evidence construction helpers are consumed by subsequent Phase 39 slice/path plans."
)]
pub(crate) fn next_evidence_edge_id(edges: &[EvidenceEdgeFact]) -> EvidenceEdgeId {
    EvidenceEdgeId(edges.len() as u64)
}

#[allow(
    dead_code,
    reason = "Evidence construction helpers are consumed by subsequent Phase 39 diagnostic bundle plans."
)]
pub(crate) fn next_evidence_bundle_id(bundles: &[EvidenceBundleFact]) -> EvidenceBundleId {
    EvidenceBundleId(bundles.len() as u64)
}

#[allow(
    dead_code,
    reason = "Evidence construction helpers are consumed by subsequent Phase 39 path plans."
)]
pub(crate) fn next_evidence_path_id(paths: &[EvidencePathFact]) -> EvidencePathId {
    EvidencePathId(paths.len() as u64)
}

#[allow(
    dead_code,
    reason = "Evidence construction helpers are consumed by subsequent Phase 39 slice plans."
)]
pub(crate) fn next_evidence_slice_id(slices: &[EvidenceSliceFact]) -> EvidenceSliceId {
    EvidenceSliceId(slices.len() as u64)
}

#[allow(
    dead_code,
    reason = "Evidence construction helpers are consumed by subsequent Phase 39 bounded rendering plans."
)]
pub(crate) fn next_evidence_omitted_region_id(
    omitted_regions: &[EvidenceOmittedRegionFact],
) -> EvidenceOmittedRegionId {
    EvidenceOmittedRegionId(omitted_regions.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::evidence::facts::{
        EvidenceConfidence, EvidenceExpansion, EvidencePrecision, EvidenceRankScore,
        EvidenceUnknownReason, EvidenceValidation,
    };
    use crate::core::Language;

    #[test]
    fn output_sorts_and_reassigns_ids_with_edge_endpoint_remap() {
        let output = EvidenceOutput {
            nodes: vec![node(9, "node:z"), node(3, "node:a")],
            edges: vec![edge(7, 9, 3, "edge:z_to_a")],
            bundles: Vec::new(),
            paths: Vec::new(),
            slices: Vec::new(),
            unknowns: Vec::new(),
            omitted_regions: Vec::new(),
            replay_keys: Vec::new(),
        }
        .normalized();

        assert_eq!(output.nodes[0].stable_key, "node:a");
        assert_eq!(output.nodes[0].id, EvidenceNodeId(0));
        assert_eq!(output.edges[0].from, EvidenceNodeId(1));
        assert_eq!(output.edges[0].to, EvidenceNodeId(0));
        assert_eq!(output.edges[0].id, EvidenceEdgeId(0));
    }

    #[test]
    fn store_rejects_dangling_edge_endpoints() {
        let result = EvidenceStore::from_output(EvidenceOutput {
            nodes: vec![node(0, "node:a")],
            edges: vec![edge(0, 0, 4, "edge:dangling")],
            bundles: Vec::new(),
            paths: Vec::new(),
            slices: Vec::new(),
            unknowns: Vec::new(),
            omitted_regions: Vec::new(),
            replay_keys: Vec::new(),
        });

        assert!(result.is_err());
    }

    #[test]
    fn store_rejects_duplicate_node_stable_keys() {
        let result = EvidenceStore::from_output(EvidenceOutput {
            nodes: vec![node(0, "node:a"), node(1, "node:a")],
            edges: Vec::new(),
            bundles: Vec::new(),
            paths: Vec::new(),
            slices: Vec::new(),
            unknowns: Vec::new(),
            omitted_regions: Vec::new(),
            replay_keys: Vec::new(),
        });

        assert!(result.is_err());
    }

    #[test]
    fn store_rejects_duplicate_unknown_stable_keys() {
        let result = EvidenceStore::from_output(EvidenceOutput {
            nodes: Vec::new(),
            edges: Vec::new(),
            bundles: Vec::new(),
            paths: Vec::new(),
            slices: Vec::new(),
            unknowns: vec![
                EvidenceUnknownFact {
                    bundle: None,
                    path: None,
                    slice: None,
                    edge: None,
                    reason: EvidenceUnknownReason::OpaqueSummary,
                    message: "first".to_string(),
                    source_fact_stable_keys: Vec::new(),
                    stable_key: "unknown:dup".to_string(),
                },
                EvidenceUnknownFact {
                    bundle: None,
                    path: None,
                    slice: None,
                    edge: None,
                    reason: EvidenceUnknownReason::BudgetExceeded,
                    message: "second".to_string(),
                    source_fact_stable_keys: Vec::new(),
                    stable_key: "unknown:dup".to_string(),
                },
            ],
            omitted_regions: Vec::new(),
            replay_keys: Vec::new(),
        });

        assert!(result.is_err());
    }

    #[test]
    fn store_rejects_missing_bundle_path_slice_and_omitted_references() {
        let mut output = EvidenceOutput {
            nodes: vec![node(0, "node:a")],
            edges: Vec::new(),
            bundles: vec![EvidenceBundleFact {
                id: EvidenceBundleId(0),
                diagnostic_stable_key: "diagnostic:a".to_string(),
                query_mode: EvidenceQueryMode::ThinBackward,
                status: EvidenceStatus::Present,
                precision: EvidencePrecision::Syntax,
                provenance: EvidenceProvenance::Native,
                validation: EvidenceValidation::Native,
                confidence: EvidenceConfidence::High,
                entry_node: Some(EvidenceNodeId(0)),
                selected_paths: vec![EvidencePathId(9)],
                selected_slices: vec![EvidenceSliceId(9)],
                replay_key: None,
                stable_key: "bundle:a".to_string(),
            }],
            paths: vec![EvidencePathFact {
                id: EvidencePathId(0),
                bundle: Some(EvidenceBundleId(9)),
                query_mode: EvidenceQueryMode::Path,
                nodes: vec![EvidenceNodeId(0)],
                edges: Vec::new(),
                rank: 0,
                score: EvidenceRankScore::default(),
                status: EvidenceStatus::Present,
                hidden_node_count: 0,
                omitted_regions: vec![EvidenceOmittedRegionId(9)],
                stable_key: "path:a".to_string(),
            }],
            slices: Vec::new(),
            unknowns: Vec::new(),
            omitted_regions: Vec::new(),
            replay_keys: Vec::new(),
        };

        assert!(EvidenceStore::from_output(output.clone()).is_err());

        output.bundles[0].selected_paths.clear();
        output.bundles[0].selected_slices.clear();
        assert!(EvidenceStore::from_output(output).is_err());
    }

    fn node(id: u64, stable_key: &str) -> EvidenceNodeFact {
        EvidenceNodeFact {
            id: EvidenceNodeId(id),
            kind: EvidenceNodeKind::Operation,
            language: Language::Go,
            file: None,
            function: None,
            body: None,
            operation: None,
            cfg_node: None,
            place: None,
            symbol: None,
            reference: None,
            call_site: None,
            span: None,
            status: EvidenceStatus::Present,
            precision: EvidencePrecision::Syntax,
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::Native,
            confidence: EvidenceConfidence::High,
            compact_label: None,
            source_fact_stable_keys: Vec::new(),
            stable_key: stable_key.to_string(),
        }
    }

    fn edge(id: u64, from: u64, to: u64, stable_key: &str) -> EvidenceEdgeFact {
        EvidenceEdgeFact {
            id: EvidenceEdgeId(id),
            from: EvidenceNodeId(from),
            to: EvidenceNodeId(to),
            kind: EvidenceEdgeKind::DataValue,
            query_mode: EvidenceQueryMode::ThinBackward,
            status: EvidenceStatus::Present,
            precision: EvidencePrecision::Syntax,
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::Native,
            confidence: EvidenceConfidence::High,
            call_site: None,
            summary_stable_key: None,
            expansion: EvidenceExpansion::None,
            compact_label: None,
            source_fact_stable_keys: Vec::new(),
            stable_key: stable_key.to_string(),
        }
    }
}
