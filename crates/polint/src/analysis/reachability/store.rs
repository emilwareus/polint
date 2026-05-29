use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::error::AnalysisError;
use crate::analysis::ids::EntrypointId;
use crate::analysis::reachability::facts::{CallReachabilityFact, ReachabilityRootFact, RootKind};
use crate::core::{FunctionId, Language};

pub(crate) const REACHABILITY_PROVIDER_ID: &str = "polint.reachability";

/// Total-order projection of a root used for deterministic output ordering.
///
/// Extends the natural sort identity `(kind, language, target_function, file,
/// span bytes)` with the remaining distinguishing fields (`originating_entrypoint`,
/// `stable_key`) so two distinct roots can never tie — the byte-stable ordering
/// contract the determinism gate (Plan 03) inherits.
type RootSortKey = (
    RootKind,
    Language,
    FunctionId,
    u32, // file id
    u32, // span start_byte
    u32, // span end_byte
    Option<EntrypointId>,
    String, // stable_key
);

fn root_sort_key(root: &ReachabilityRootFact) -> RootSortKey {
    (
        root.kind,
        root.language,
        root.target_function,
        root.file.0,
        root.span.start_byte,
        root.span.end_byte,
        root.originating_entrypoint,
        root.stable_key.clone(),
    )
}

fn mark_sort_key(mark: &CallReachabilityFact) -> String {
    mark.stable_key.clone()
}

/// Provider output for `polint.reachability` — the normalized root set plus the
/// call-reachability marks (marks stay empty in this plan; Plan 02 populates
/// them).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ReachabilityProviderOutput {
    pub(crate) roots: Vec<ReachabilityRootFact>,
    pub(crate) marks: Vec<CallReachabilityFact>,
}

impl ReachabilityProviderOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// Sorts roots and marks by their locked total-order sort keys so the output
    /// is byte-identical regardless of insertion order (D-06/D-20).
    pub(crate) fn normalized(mut self) -> Self {
        self.roots.sort_by_key(root_sort_key);
        self.marks.sort_by_key(mark_sort_key);
        self
    }
}

/// Typed reachability store with the read indexes consumers use.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReachabilityStore {
    pub(crate) roots: Vec<ReachabilityRootFact>,
    pub(crate) marks: Vec<CallReachabilityFact>,
    by_kind: BTreeMap<RootKind, Vec<usize>>,
    by_language: BTreeMap<Language, Vec<usize>>,
    by_function: BTreeMap<FunctionId, Vec<usize>>,
}

impl ReachabilityStore {
    /// Builds the store, validating that every composed `target_function` and
    /// `originating_entrypoint` reference resolves against the supplied valid ID
    /// sets, returning [`AnalysisError::InvalidFact`] on a dangling ref (D-03).
    pub(crate) fn from_output(
        output: ReachabilityProviderOutput,
        valid_function_ids: &BTreeSet<FunctionId>,
        valid_entrypoint_ids: &BTreeSet<EntrypointId>,
    ) -> Result<Self, AnalysisError> {
        let output = output.normalized();

        for root in &output.roots {
            if !valid_function_ids.contains(&root.target_function) {
                return Err(AnalysisError::InvalidFact {
                    provider: REACHABILITY_PROVIDER_ID,
                    reason: format!(
                        "dangling target function {:?} for reachability root `{}`",
                        root.target_function, root.stable_key
                    ),
                });
            }
            if let Some(entrypoint) = root.originating_entrypoint
                && !valid_entrypoint_ids.contains(&entrypoint)
            {
                return Err(AnalysisError::InvalidFact {
                    provider: REACHABILITY_PROVIDER_ID,
                    reason: format!(
                        "dangling originating entrypoint {:?} for reachability root `{}`",
                        entrypoint, root.stable_key
                    ),
                });
            }
        }

        let mut store = Self {
            roots: output.roots,
            marks: output.marks,
            ..Self::default()
        };

        for (index, root) in store.roots.iter().enumerate() {
            store.by_kind.entry(root.kind).or_default().push(index);
            store
                .by_language
                .entry(root.language)
                .or_default()
                .push(index);
            store
                .by_function
                .entry(root.target_function)
                .or_default()
                .push(index);
        }

        Ok(store)
    }

    pub(crate) fn roots(&self) -> &[ReachabilityRootFact] {
        &self.roots
    }

    pub(crate) fn marks(&self) -> &[CallReachabilityFact] {
        &self.marks
    }

    pub(crate) fn roots_for_kind(&self, kind: RootKind) -> &[usize] {
        self.by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn roots_for_language(&self, language: Language) -> &[usize] {
        self.by_language.get(&language).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn roots_for_function(&self, function: FunctionId) -> &[usize] {
        self.by_function.get(&function).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::ReachabilityRootId;
    use crate::analysis::reachability::facts::{
        RootPrecision, RootProvenance, RootStatus, compute_reachability_root_stable_key,
    };
    use crate::core::{FileId, Span};

    fn span_bytes(file: FileId, start: u32, end: u32) -> Span {
        Span {
            file,
            start_byte: start,
            end_byte: end,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 1,
        }
    }

    fn root(function: u64, kind: RootKind, entrypoint: Option<u64>) -> ReachabilityRootFact {
        let span = span_bytes(FileId(1), function as u32, function as u32 + 1);
        ReachabilityRootFact {
            id: ReachabilityRootId(0),
            kind,
            language: Language::Go,
            target_function: FunctionId(function),
            target_symbol: None,
            originating_entrypoint: entrypoint.map(EntrypointId),
            file: FileId(1),
            span: span.clone(),
            precision: RootPrecision::ResolvedStatic,
            provenance: RootProvenance::NativeDiscovery,
            status: RootStatus::Resolved,
            provider_id: REACHABILITY_PROVIDER_ID.to_string(),
            stable_key: compute_reachability_root_stable_key(
                kind,
                Language::Go,
                &format!("pkg.F{function}"),
                FileId(1),
                &span,
            ),
        }
    }

    fn function_ids(ids: &[u64]) -> BTreeSet<FunctionId> {
        ids.iter().copied().map(FunctionId).collect()
    }

    #[test]
    fn normalized_is_insertion_order_independent() {
        let a = root(1, RootKind::Main, None);
        let b = root(2, RootKind::Init, None);
        let first = ReachabilityProviderOutput {
            roots: vec![a.clone(), b.clone()],
            marks: Vec::new(),
        }
        .normalized();
        let second = ReachabilityProviderOutput {
            roots: vec![b, a],
            marks: Vec::new(),
        }
        .normalized();
        assert_eq!(first, second);
    }

    #[test]
    fn from_output_builds_deterministic_indexes() {
        let output = ReachabilityProviderOutput {
            roots: vec![root(1, RootKind::Main, None), root(2, RootKind::Init, None)],
            marks: Vec::new(),
        };
        let store =
            ReachabilityStore::from_output(output, &function_ids(&[1, 2]), &BTreeSet::new())
                .expect("store");
        assert_eq!(store.roots().len(), 2);
        assert_eq!(store.roots_for_kind(RootKind::Main).len(), 1);
        assert_eq!(store.roots_for_kind(RootKind::Init).len(), 1);
        assert_eq!(store.roots_for_language(Language::Go).len(), 2);
        assert_eq!(store.roots_for_function(FunctionId(1)).len(), 1);
        assert!(store.roots_for_function(FunctionId(99)).is_empty());
    }

    #[test]
    fn from_output_rejects_dangling_target_function() {
        let output = ReachabilityProviderOutput {
            roots: vec![root(7, RootKind::Main, None)],
            marks: Vec::new(),
        };
        let error = ReachabilityStore::from_output(output, &BTreeSet::new(), &BTreeSet::new())
            .expect_err("dangling target function rejected");
        assert!(error.to_string().contains("dangling target function"));
        assert!(error.to_string().contains("polint.reachability"));
    }

    #[test]
    fn from_output_rejects_dangling_originating_entrypoint() {
        let output = ReachabilityProviderOutput {
            roots: vec![root(1, RootKind::Test, Some(42))],
            marks: Vec::new(),
        };
        let error = ReachabilityStore::from_output(output, &function_ids(&[1]), &BTreeSet::new())
            .expect_err("dangling entrypoint rejected");
        assert!(
            error
                .to_string()
                .contains("dangling originating entrypoint")
        );
    }

    #[test]
    fn from_output_accepts_valid_references() {
        let mut entrypoints = BTreeSet::new();
        entrypoints.insert(EntrypointId(42));
        let output = ReachabilityProviderOutput {
            roots: vec![root(1, RootKind::Test, Some(42))],
            marks: Vec::new(),
        };
        let store = ReachabilityStore::from_output(output, &function_ids(&[1]), &entrypoints)
            .expect("valid references");
        assert_eq!(store.roots().len(), 1);
    }

    #[test]
    fn empty_output_builds_empty_store() {
        let store = ReachabilityStore::from_output(
            ReachabilityProviderOutput::empty(),
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .expect("empty store");
        assert!(store.roots().is_empty());
        assert!(store.marks().is_empty());
    }
}
