use serde::{Deserialize, Serialize};

use crate::analysis::ids::{EntrypointId, ReachabilityRootId};
use crate::core::{FileId, FunctionId, Language, Span, SymbolId};

// ---------------------------------------------------------------------------
// ReachabilityRootFact
// ---------------------------------------------------------------------------

/// One whole-program reachability root (D-03).
///
/// Composes the v1.2 IDs by reference (`target_function`, `target_symbol`,
/// `originating_entrypoint`) — the entrypoint/call/identity facts are never
/// duplicated or mutated. `originating_entrypoint` is set only for the
/// `Test`/`FrameworkEntrypoint` bridge roots (D-12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReachabilityRootFact {
    pub(crate) id: ReachabilityRootId,
    pub(crate) kind: RootKind,
    pub(crate) language: Language,
    pub(crate) target_function: FunctionId,
    pub(crate) target_symbol: Option<SymbolId>,
    pub(crate) originating_entrypoint: Option<EntrypointId>,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) precision: RootPrecision,
    pub(crate) provenance: RootProvenance,
    pub(crate) status: RootStatus,
    pub(crate) provider_id: String,
    pub(crate) stable_key: String,
}

/// Closed taxonomy of reachability root kinds (D-04).
///
/// Pinned declaration order so the derived `Ord` and serde representation are
/// declaration-driven and byte-stable, matching the established
/// `EntrypointKind`/`IdentityCategory` convention. No explicit integer-ordinal
/// representation attribute is used — byte-stability is achieved purely by pinned
/// order + derived `Ord` + serde rename, exactly as the existing closed enums in
/// this codebase do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RootKind {
    Main,
    Init,
    Exported,
    Test,
    FrameworkEntrypoint,
    ConfiguredEntrypoint,
}

impl RootKind {
    /// Stable lowercase label used in stable keys and digest payloads.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Init => "init",
            Self::Exported => "exported",
            Self::Test => "test",
            Self::FrameworkEntrypoint => "framework_entrypoint",
            Self::ConfiguredEntrypoint => "configured_entrypoint",
        }
    }
}

/// Resolution status of a root (D-05). Mirrors `EntrypointStatus` loss-lessly so
/// the entrypoint bridge can inherit status without translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RootStatus {
    Resolved,
    Partial,
    Unresolved,
    SetupMissing,
    Unsupported,
}

impl RootStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Partial => "partial",
            Self::Unresolved => "unresolved",
            Self::SetupMissing => "setup_missing",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Precision tier of a root (D-05). Mirrors `EntrypointPrecision` loss-lessly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RootPrecision {
    ResolvedStatic,
    SetupAware,
    Heuristic,
    Conservative,
    Unknown,
}

impl RootPrecision {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ResolvedStatic => "resolved_static",
            Self::SetupAware => "setup_aware",
            Self::Heuristic => "heuristic",
            Self::Conservative => "conservative",
            Self::Unknown => "unknown",
        }
    }
}

/// Where a root came from (D-05). The three discovery sources map loss-lessly to
/// these variants: native Go/TS/JS discovery, the entrypoint bridge, and
/// configured `.polint.toml` roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RootProvenance {
    NativeDiscovery,
    EntrypointBridge,
    Configured,
}

impl RootProvenance {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NativeDiscovery => "native_discovery",
            Self::EntrypointBridge => "entrypoint_bridge",
            Self::Configured => "configured",
        }
    }
}

/// Stable lowercase label for the broader `core::Language` enum, used in stable
/// keys and digest payloads.
fn language_label(language: Language) -> &'static str {
    match language {
        Language::Go => "go",
        Language::TypeScript => "typescript",
        Language::Tsx => "tsx",
        Language::JavaScript => "javascript",
        Language::Jsx => "jsx",
        Language::Unknown => "unknown",
    }
}

/// Builds the deterministic, boundary-disambiguated stable key for a reachability
/// root. Mirrors the `analysis::identity` stable-key shape: single line, no
/// whitespace, explicit `|` separators, each field escaped so a value containing
/// `|` cannot forge a boundary.
///
/// Keyed on `(language, kind, function stable identity, file, span)` — never
/// run-local IDs (D-06). `function_identity` is a caller-supplied stable string
/// (e.g. the function's stable key or `package.Name`), not a dense `FunctionId`.
pub(crate) fn compute_reachability_root_stable_key(
    kind: RootKind,
    language: Language,
    function_identity: &str,
    file_id: FileId,
    span: &Span,
) -> String {
    format!(
        "reachability_root|{}|{}|{}|{}|{}..{}",
        kind.as_str(),
        language_label(language),
        escape_field(function_identity),
        file_id.0,
        span.start_byte,
        span.end_byte,
    )
}

/// Escapes the `|` separator so a value containing the separator cannot forge a
/// different field boundary.
fn escape_field(value: &str) -> String {
    value.replace('\\', "\\\\").replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;
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

    fn sample_root() -> ReachabilityRootFact {
        let span = span_bytes(FileId(3), 4, 9);
        ReachabilityRootFact {
            id: ReachabilityRootId(0),
            kind: RootKind::Main,
            language: Language::Go,
            target_function: FunctionId(10),
            target_symbol: Some(SymbolId(20)),
            originating_entrypoint: None,
            file: FileId(3),
            span: span.clone(),
            precision: RootPrecision::ResolvedStatic,
            provenance: RootProvenance::NativeDiscovery,
            status: RootStatus::Resolved,
            provider_id: "polint.reachability".to_string(),
            stable_key: compute_reachability_root_stable_key(
                RootKind::Main,
                Language::Go,
                "main.main",
                FileId(3),
                &span,
            ),
        }
    }

    #[test]
    fn reachability_root_fact_round_trips_through_serde_json() {
        let root = sample_root();
        let json = serde_json::to_string(&root).expect("serialize");
        let restored: ReachabilityRootFact = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(root, restored);
    }

    #[test]
    fn root_vocabulary_enums_are_copy_ordered_hashable_serializable() {
        assert_small_id_contract::<RootKind>();
        assert_small_id_contract::<RootStatus>();
        assert_small_id_contract::<RootPrecision>();
        assert_small_id_contract::<RootProvenance>();
    }

    #[test]
    fn vocabulary_labels_are_stable_snake_case() {
        assert_eq!(
            RootKind::FrameworkEntrypoint.as_str(),
            "framework_entrypoint"
        );
        assert_eq!(RootStatus::SetupMissing.as_str(), "setup_missing");
        assert_eq!(RootPrecision::ResolvedStatic.as_str(), "resolved_static");
        assert_eq!(
            RootProvenance::EntrypointBridge.as_str(),
            "entrypoint_bridge"
        );
    }

    #[test]
    fn root_kind_sorts_in_pinned_declaration_order() {
        // Sort a permuted list and assert it returns to declaration order.
        let mut kinds = vec![
            RootKind::ConfiguredEntrypoint,
            RootKind::Test,
            RootKind::Main,
            RootKind::FrameworkEntrypoint,
            RootKind::Exported,
            RootKind::Init,
        ];
        kinds.sort();
        assert_eq!(
            kinds,
            vec![
                RootKind::Main,
                RootKind::Init,
                RootKind::Exported,
                RootKind::Test,
                RootKind::FrameworkEntrypoint,
                RootKind::ConfiguredEntrypoint,
            ]
        );
    }

    #[test]
    fn root_kind_has_exactly_6_variants() {
        // Compile-time exhaustive match over every arm; the array length lock
        // fails to compile if a variant is added without updating this test.
        fn assert_all(kind: RootKind) -> RootKind {
            match kind {
                RootKind::Main
                | RootKind::Init
                | RootKind::Exported
                | RootKind::Test
                | RootKind::FrameworkEntrypoint
                | RootKind::ConfiguredEntrypoint => kind,
            }
        }
        let variants = [
            assert_all(RootKind::Main),
            assert_all(RootKind::Init),
            assert_all(RootKind::Exported),
            assert_all(RootKind::Test),
            assert_all(RootKind::FrameworkEntrypoint),
            assert_all(RootKind::ConfiguredEntrypoint),
        ];
        assert_eq!(variants.len(), 6);
    }

    #[test]
    fn stable_key_disambiguates_field_boundaries() {
        // Two roots whose joined fields differ only by where a `|` falls
        // (`a|b`,`c` vs `a`,`b|c`) must produce distinct stable keys.
        let span = span_bytes(FileId(1), 1, 1);
        let left = compute_reachability_root_stable_key(
            RootKind::Exported,
            Language::Go,
            "a|b",
            FileId(1),
            &span,
        );
        // A sibling whose function identity is "b|c" joined the same way proves
        // the escape protects the boundary instead of merging fields.
        let right_with_pipe = compute_reachability_root_stable_key(
            RootKind::Exported,
            Language::Go,
            "b|c",
            FileId(1),
            &span,
        );
        assert_ne!(left, right_with_pipe);
        // The escape must appear so a literal `|` in a value cannot forge a boundary.
        assert!(left.contains("a\\|b"));
        assert!(right_with_pipe.contains("b\\|c"));
    }

    #[test]
    fn stable_key_is_keyed_on_stable_identity_not_dense_ids() {
        let key = compute_reachability_root_stable_key(
            RootKind::Main,
            Language::Go,
            "main.main",
            FileId(3),
            &span_bytes(FileId(3), 4, 9),
        );
        assert_eq!(key, "reachability_root|main|go|main.main|3|4..9");
    }
}
