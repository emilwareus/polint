#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Extension-aware cache quarantine infrastructure is established before Phase 34 wires real extension providers."
    )
)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::dependency_index::CacheNode;
use super::digest::{Digest, DigestKind};
use super::invalidation::QuarantineReason;
use super::keys::LayerKey;

// ---------------------------------------------------------------------------
// QuarantineEntry
// ---------------------------------------------------------------------------

/// Records a single quarantined cache node.
///
/// When an extension's code or manifest digest changes, cache entries that
/// include that extension's digest are quarantined (not served as hits, not
/// deleted). If the extension later reverts to a previously seen digest,
/// quarantined entries can be reinstated.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct QuarantineEntry {
    /// The cache node that was quarantined.
    pub(crate) node: CacheNode,
    /// Reason for quarantine.
    pub(crate) reason: QuarantineReason,
    /// The extension digest that triggered the quarantine.
    pub(crate) extension_digest_at_quarantine: Digest,
    /// The run ID at which this entry was quarantined (for age tracking).
    pub(crate) quarantined_at_run: u64,
}

// ---------------------------------------------------------------------------
// QuarantinePolicy
// ---------------------------------------------------------------------------

/// Policy governing quarantine entry lifecycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct QuarantinePolicy {
    /// Entries older than this many runs are evicted on cleanup.
    pub(crate) max_quarantine_age_runs: u64,
}

impl Default for QuarantinePolicy {
    fn default() -> Self {
        Self {
            max_quarantine_age_runs: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// QuarantineStore
// ---------------------------------------------------------------------------

/// Maintains the set of quarantined cache nodes for extension-aware
/// cache invalidation.
///
/// When an extension's code digest changes, dependent cache entries are
/// quarantined rather than deleted. If the extension reverts to a previously
/// seen digest, quarantined entries are reinstated. Native facts are never
/// quarantined per D-09.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct QuarantineStore {
    entries: BTreeMap<CacheNode, QuarantineEntry>,
    #[serde(skip)]
    policy: QuarantinePolicy,
}

impl QuarantineStore {
    /// Creates a new empty quarantine store with the given policy.
    pub(crate) fn with_policy(policy: QuarantinePolicy) -> Self {
        Self {
            entries: BTreeMap::new(),
            policy,
        }
    }

    /// Quarantine a cache node. Returns `true` if the node was quarantined,
    /// `false` if rejected (native-only nodes are never quarantined per D-09).
    pub(crate) fn quarantine(
        &mut self,
        node: CacheNode,
        reason: QuarantineReason,
        extension_digest: Digest,
        run_id: u64,
    ) -> bool {
        if is_native_only_node(&node) {
            return false;
        }
        self.entries.insert(
            node.clone(),
            QuarantineEntry {
                node,
                reason,
                extension_digest_at_quarantine: extension_digest,
                quarantined_at_run: run_id,
            },
        );
        true
    }

    /// Returns whether a given cache node is currently quarantined.
    pub(crate) fn is_quarantined(&self, node: &CacheNode) -> bool {
        self.entries.contains_key(node)
    }

    /// Reinstates all quarantine entries whose `extension_digest_at_quarantine`
    /// matches the given digest (extension reverted to a known-good state per D-08).
    /// Returns the reinstated cache nodes.
    pub(crate) fn reinstate(&mut self, extension_digest: &Digest) -> Vec<CacheNode> {
        let to_reinstate: Vec<CacheNode> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.extension_digest_at_quarantine == *extension_digest)
            .map(|(node, _)| node.clone())
            .collect();

        for node in &to_reinstate {
            self.entries.remove(node);
        }

        to_reinstate
    }

    /// Evicts entries older than `max_quarantine_age_runs` based on the current run ID.
    pub(crate) fn cleanup(&mut self, current_run_id: u64) {
        let max_age = self.policy.max_quarantine_age_runs;
        self.entries
            .retain(|_, entry| current_run_id.saturating_sub(entry.quarantined_at_run) <= max_age);
    }

    /// Returns the number of quarantined cache nodes.
    pub(crate) fn quarantine_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns references to all quarantined cache nodes.
    pub(crate) fn quarantined_nodes(&self) -> Vec<&CacheNode> {
        self.entries.keys().collect()
    }
}

// ---------------------------------------------------------------------------
// Native-only node detection (D-09)
// ---------------------------------------------------------------------------

/// Returns `true` for cache nodes that represent native-only facts and must
/// never be quarantined per D-09.
///
/// Native-only nodes:
/// - `CacheNode::Input` -- raw source inputs, always native
/// - `CacheNode::ToolInvocation` -- tool lifecycle, always native
/// - `CacheNode::Layer` where all `extension_digests` are absent sentinels
///
/// Extension-influenced nodes (can be quarantined):
/// - `CacheNode::Extension` -- directly extension-produced
/// - `CacheNode::Query` -- may depend on extension data
/// - `CacheNode::Summary` -- may include extension digest
/// - `CacheNode::Diagnostic` -- may be extension-influenced
/// - `CacheNode::Layer` with at least one non-absent extension digest
fn is_native_only_node(node: &CacheNode) -> bool {
    match node {
        CacheNode::Input(_) | CacheNode::ToolInvocation(_) => true,
        CacheNode::Layer(key) => all_extension_digests_absent(key),
        CacheNode::Extension(_)
        | CacheNode::Query(_)
        | CacheNode::Summary(_)
        | CacheNode::Diagnostic(_) => false,
    }
}

/// The canonical absent extension digest used by all layer key constructors.
fn absent_extension_sentinel() -> Digest {
    Digest::absent(DigestKind::ExtensionCode, "extension_digest_absent")
}

/// Returns `true` if all extension digests in the layer key are absent sentinels.
fn all_extension_digests_absent(key: &LayerKey) -> bool {
    if key.extension_digests.is_empty() {
        return true;
    }
    let sentinel = absent_extension_sentinel();
    key.extension_digests.iter().all(|d| *d == sentinel)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{
        Digest, DigestKind, LayerKey, QueryKey, SummaryKey,
        keys::LayerKind,
        keys::PrecisionTier,
    };

    fn ext_digest(label: &str) -> Digest {
        Digest::from_parts(DigestKind::ExtensionCode, label, &[label])
    }

    fn absent_ext_digest() -> Digest {
        Digest::absent(DigestKind::ExtensionCode, "extension_digest_absent")
    }

    fn summary_node(callable: &str, ext_digest: Digest) -> CacheNode {
        CacheNode::Summary(SummaryKey::new(
            callable,
            "effects",
            "1",
            Digest::absent(DigestKind::SummaryBody, "none"),
            Vec::new(),
            ext_digest,
        ))
    }

    fn native_layer_node(provider_id: &str) -> CacheNode {
        CacheNode::Layer(LayerKey::new(
            LayerKind::TsSyntax,
            provider_id,
            "1",
            "ts-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::TsJsLifecycle, "none"),
            Digest::absent(DigestKind::Config, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![Digest::from_parts(DigestKind::SourceText, "a", &["a"])],
            Vec::new(),
            vec![absent_ext_digest()],
        ))
    }

    fn extension_layer_node(provider_id: &str, ext_label: &str) -> CacheNode {
        CacheNode::Layer(LayerKey::new(
            LayerKind::Extension,
            provider_id,
            "1",
            "ext-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::TsJsLifecycle, "none"),
            Digest::absent(DigestKind::Config, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![Digest::from_parts(DigestKind::SourceText, "a", &["a"])],
            Vec::new(),
            vec![ext_digest(ext_label)],
        ))
    }

    // (a) Quarantine a Summary node, verify is_quarantined returns true.
    #[test]
    fn quarantine_summary_node_is_quarantined() {
        let mut store = QuarantineStore::default();
        let node = summary_node("func_a", ext_digest("ext-v1"));

        let accepted = store.quarantine(
            node.clone(),
            QuarantineReason::ExtensionChanged,
            ext_digest("ext-v1"),
            1,
        );

        assert!(accepted);
        assert!(store.is_quarantined(&node));
        assert_eq!(store.quarantine_count(), 1);
    }

    // (b) Reinstate with digest "ext-v1", verify node is no longer quarantined.
    #[test]
    fn reinstate_removes_matching_entries() {
        let mut store = QuarantineStore::default();
        let node = summary_node("func_a", ext_digest("ext-v1"));

        store.quarantine(
            node.clone(),
            QuarantineReason::ExtensionChanged,
            ext_digest("ext-v1"),
            1,
        );

        let reinstated = store.reinstate(&ext_digest("ext-v1"));
        assert_eq!(reinstated.len(), 1);
        assert_eq!(reinstated[0], node);
        assert!(!store.is_quarantined(&node));
        assert_eq!(store.quarantine_count(), 0);
    }

    // (c) Native-only Layer node is rejected from quarantine.
    #[test]
    fn native_only_layer_node_rejected() {
        let mut store = QuarantineStore::default();
        let node = native_layer_node("polint.ts.syntax");

        let accepted = store.quarantine(
            node.clone(),
            QuarantineReason::ExtensionChanged,
            ext_digest("ext-v1"),
            1,
        );

        assert!(!accepted);
        assert!(!store.is_quarantined(&node));
        assert_eq!(store.quarantine_count(), 0);
    }

    // (d) Quarantine two nodes with different digests, reinstate only one.
    #[test]
    fn reinstate_only_matching_digest() {
        let mut store = QuarantineStore::default();
        let node_v1 = summary_node("func_a", ext_digest("ext-v1"));
        let node_v2 = summary_node("func_b", ext_digest("ext-v2"));

        store.quarantine(
            node_v1.clone(),
            QuarantineReason::ExtensionChanged,
            ext_digest("ext-v1"),
            1,
        );
        store.quarantine(
            node_v2.clone(),
            QuarantineReason::ExtensionChanged,
            ext_digest("ext-v2"),
            1,
        );

        assert_eq!(store.quarantine_count(), 2);

        let reinstated = store.reinstate(&ext_digest("ext-v1"));
        assert_eq!(reinstated.len(), 1);
        assert!(!store.is_quarantined(&node_v1));
        assert!(store.is_quarantined(&node_v2));
        assert_eq!(store.quarantine_count(), 1);
    }

    // (e) Cleanup evicts entries older than max_quarantine_age_runs.
    #[test]
    fn cleanup_evicts_old_entries() {
        let policy = QuarantinePolicy {
            max_quarantine_age_runs: 3,
        };
        let mut store = QuarantineStore::with_policy(policy);
        let old_node = summary_node("func_old", ext_digest("ext-v1"));
        let new_node = summary_node("func_new", ext_digest("ext-v1"));

        store.quarantine(
            old_node.clone(),
            QuarantineReason::ExtensionChanged,
            ext_digest("ext-v1"),
            1,
        );
        store.quarantine(
            new_node.clone(),
            QuarantineReason::ExtensionChanged,
            ext_digest("ext-v1"),
            5,
        );

        // At run 8: old_node age = 7 (> 3), new_node age = 3 (<= 3)
        store.cleanup(8);

        assert!(!store.is_quarantined(&old_node));
        assert!(store.is_quarantined(&new_node));
        assert_eq!(store.quarantine_count(), 1);
    }

    // Additional tests for native-only node detection.

    #[test]
    fn input_node_is_native_only() {
        let node = CacheNode::Input("src/main.ts".to_string());
        assert!(is_native_only_node(&node));
    }

    #[test]
    fn tool_invocation_node_is_native_only() {
        let node = CacheNode::ToolInvocation("go".to_string());
        assert!(is_native_only_node(&node));
    }

    #[test]
    fn extension_node_is_not_native_only() {
        let node = CacheNode::Extension("ext::custom_model".to_string());
        assert!(!is_native_only_node(&node));
    }

    #[test]
    fn query_node_is_not_native_only() {
        let node = CacheNode::Query(QueryKey::new(
            "call_graph",
            "1",
            Digest::absent(DigestKind::QueryParameters, "none"),
            Vec::new(),
            Digest::absent(DigestKind::Budget, "none"),
            PrecisionTier::Syntax,
        ));
        assert!(!is_native_only_node(&node));
    }

    #[test]
    fn layer_with_real_extension_digest_is_not_native() {
        let node = extension_layer_node("ext-provider", "ext-v1");
        assert!(!is_native_only_node(&node));
    }

    #[test]
    fn layer_with_empty_extension_digests_is_native() {
        let node = CacheNode::Layer(LayerKey::new(
            LayerKind::TsSyntax,
            "polint.ts.syntax",
            "1",
            "ts-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::TsJsLifecycle, "none"),
            Digest::absent(DigestKind::Config, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![Digest::from_parts(DigestKind::SourceText, "a", &["a"])],
            Vec::new(),
            Vec::new(), // empty extension_digests
        ));
        assert!(is_native_only_node(&node));
    }

    #[test]
    fn quarantined_nodes_returns_all_quarantined() {
        let mut store = QuarantineStore::default();
        let node_a = summary_node("func_a", ext_digest("ext-v1"));
        let node_b = summary_node("func_b", ext_digest("ext-v2"));

        store.quarantine(
            node_a.clone(),
            QuarantineReason::ExtensionChanged,
            ext_digest("ext-v1"),
            1,
        );
        store.quarantine(
            node_b.clone(),
            QuarantineReason::ExtensionChanged,
            ext_digest("ext-v2"),
            1,
        );

        let nodes = store.quarantined_nodes();
        assert_eq!(nodes.len(), 2);
    }
}
