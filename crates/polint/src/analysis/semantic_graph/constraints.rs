use serde::{Deserialize, Serialize};

use crate::analysis::ids::{SemanticConstraintId, SemanticNodeId, TypeFactId};
use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};

// ---------------------------------------------------------------------------
// ConstraintKind
// ---------------------------------------------------------------------------

/// The unified, closed constraint vocabulary the whole frontend graph emits into
/// (GRAPH-02). Exactly 7 roadmap-named variants in pinned declaration order so the
/// derived `Ord` and serde representation are declaration-driven and byte-stable,
/// matching the established `RootKind`/`PointsToConstraintKind` convention. No
/// `#[repr(u8)]` — byte-stability is achieved purely via pinned order + derived
/// `Ord` + serde rename + an `as_str()` label.
///
/// Because the variants carry payloads they are NOT `Copy`; every payload field is
/// an `Ord` type (`SemanticNodeId` / `TypeFactId` / `String`) so the derived `Ord`
/// that drives byte-stable ordering survives.
///
/// # D-09 conceptual map to `points_to::PointsToConstraintKind` (NO code coupling)
///
/// This enum is the unified frontend vocabulary that sits *above* the points-to
/// sub-domain's internal `crate::analysis::points_to::facts::PointsToConstraintKind`.
/// The two are distinct enums and Phase 44 does NOT merge, rename, import, or
/// otherwise couple them — folding the points-to constraint language into this
/// unified vocabulary is explicitly deferred to Phase 47. The conceptual
/// relationship (documented here for Phase 47's later folding) is:
///
/// - `ConstraintKind::CopyEdge`  <-> `PointsToConstraintKind::Copy`
/// - `ConstraintKind::Alloc`     <-> `PointsToConstraintKind::AddressOf`
/// - `ConstraintKind::FieldLoad` <-> `PointsToConstraintKind::FieldLoad`
/// - `ConstraintKind::FieldStore`<-> `PointsToConstraintKind::FieldStore`
///
/// Crucially, this module references `points_to::facts` ONLY for the shared
/// `PointsToStatus`/`PointsToPrecision` status/precision field types (D-10 mandates
/// `ConstraintFact` mirror `PointsToConstraintFact` exactly); it imports neither
/// `PointsToConstraintKind` nor any points-to solver type. The named variants here
/// reference semantic-graph `SemanticNodeId`s / existing fact IDs (`TypeFactId`),
/// never the points-to sub-domain's `PtVarId`/`ObjectTokenId` run-local IDs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConstraintKind {
    /// `dst <- src` value copy. Maps conceptually to points-to `Copy`.
    CopyEdge {
        dst: SemanticNodeId,
        src: SemanticNodeId,
    },
    /// `dst <- &object` allocation/address-of. Maps conceptually to points-to
    /// `AddressOf`.
    Alloc {
        dst: SemanticNodeId,
        object: SemanticNodeId,
    },
    /// `dst <- base.field` field/property load.
    FieldLoad {
        dst: SemanticNodeId,
        base: SemanticNodeId,
        field: String,
    },
    /// `base.field <- src` field/property store.
    FieldStore {
        base: SemanticNodeId,
        field: String,
        src: SemanticNodeId,
    },
    /// A call obligation anchored at a callsite node, consumed by the Phase 47
    /// unified solver to resolve dynamic dispatch / refine targets.
    CallConstraint { callsite: SemanticNodeId },
    /// Reserved adaptation-model edge. No producer exists until Phase 49
    /// (ADAPT-01); `build_semantic_graph` emits ZERO of these (honest emptiness,
    /// D-11). The variant is fieldless because no model-edge payload is defined yet.
    ModelEdge,
    /// A type obligation on a node, referencing an existing `TypeFactId` from the
    /// type substrate (never a run-local dense ID of another family).
    TypeConstraint {
        node: SemanticNodeId,
        type_fact: TypeFactId,
    },
}

impl ConstraintKind {
    /// Stable lowercase tag label used in stable keys and digest payloads.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::CopyEdge { .. } => "copy_edge",
            Self::Alloc { .. } => "alloc",
            Self::FieldLoad { .. } => "field_load",
            Self::FieldStore { .. } => "field_store",
            Self::CallConstraint { .. } => "call_constraint",
            Self::ModelEdge => "model_edge",
            Self::TypeConstraint { .. } => "type_constraint",
        }
    }
}

// ---------------------------------------------------------------------------
// ConstraintFact family
// ---------------------------------------------------------------------------

/// One unified semantic-graph constraint. Mirrors `points_to::facts::
/// PointsToConstraintFact` exactly (D-10): `{ id, kind, status, precision,
/// stable_key }`, reusing the points-to `PointsToStatus`/`PointsToPrecision`
/// vocabulary as the status/precision field types rather than inventing a redundant
/// enum.
///
/// The dense `id` is a run-local post-normalization read concern only and MUST NOT
/// enter any serialized stable payload that feeds the output digest (D-06) —
/// `#[serde(skip)]` strips it; serde restores it via `SemanticConstraintId::default()`
/// (= 0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ConstraintFact {
    #[serde(skip)]
    pub(crate) id: SemanticConstraintId,
    pub(crate) kind: ConstraintKind,
    pub(crate) status: PointsToStatus,
    pub(crate) precision: PointsToPrecision,
    /// Built from the referenced existing identity (D-06), never run-local dense
    /// IDs. Populated by `build_semantic_graph`.
    pub(crate) stable_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constraint_kind_has_exactly_7_variants() {
        // Compile-time exhaustive match over every arm; the array length lock fails
        // to compile if a variant is added without updating this test.
        fn assert_all(kind: &ConstraintKind) -> &'static str {
            match kind {
                ConstraintKind::CopyEdge { .. } => "copy_edge",
                ConstraintKind::Alloc { .. } => "alloc",
                ConstraintKind::FieldLoad { .. } => "field_load",
                ConstraintKind::FieldStore { .. } => "field_store",
                ConstraintKind::CallConstraint { .. } => "call_constraint",
                ConstraintKind::ModelEdge => "model_edge",
                ConstraintKind::TypeConstraint { .. } => "type_constraint",
            }
        }
        let variants = [
            assert_all(&ConstraintKind::CopyEdge {
                dst: SemanticNodeId(0),
                src: SemanticNodeId(1),
            }),
            assert_all(&ConstraintKind::Alloc {
                dst: SemanticNodeId(0),
                object: SemanticNodeId(1),
            }),
            assert_all(&ConstraintKind::FieldLoad {
                dst: SemanticNodeId(0),
                base: SemanticNodeId(1),
                field: "f".to_string(),
            }),
            assert_all(&ConstraintKind::FieldStore {
                base: SemanticNodeId(0),
                field: "f".to_string(),
                src: SemanticNodeId(1),
            }),
            assert_all(&ConstraintKind::CallConstraint {
                callsite: SemanticNodeId(0),
            }),
            assert_all(&ConstraintKind::ModelEdge),
            assert_all(&ConstraintKind::TypeConstraint {
                node: SemanticNodeId(0),
                type_fact: TypeFactId(1),
            }),
        ];
        assert_eq!(variants.len(), 7);
    }

    #[test]
    fn constraint_kind_sorts_in_pinned_declaration_order() {
        // A permuted list sorts back to declaration order: the variant discriminant
        // ordering is declaration-driven, so kind ordering is stable regardless of
        // payload values.
        let mut kinds = [
            ConstraintKind::TypeConstraint {
                node: SemanticNodeId(0),
                type_fact: TypeFactId(0),
            },
            ConstraintKind::ModelEdge,
            ConstraintKind::CallConstraint {
                callsite: SemanticNodeId(0),
            },
            ConstraintKind::FieldStore {
                base: SemanticNodeId(0),
                field: "f".to_string(),
                src: SemanticNodeId(0),
            },
            ConstraintKind::FieldLoad {
                dst: SemanticNodeId(0),
                base: SemanticNodeId(0),
                field: "f".to_string(),
            },
            ConstraintKind::Alloc {
                dst: SemanticNodeId(0),
                object: SemanticNodeId(0),
            },
            ConstraintKind::CopyEdge {
                dst: SemanticNodeId(0),
                src: SemanticNodeId(0),
            },
        ];
        kinds.sort();
        let tags: Vec<&'static str> = kinds.iter().map(ConstraintKind::as_str).collect();
        assert_eq!(
            tags,
            vec![
                "copy_edge",
                "alloc",
                "field_load",
                "field_store",
                "call_constraint",
                "model_edge",
                "type_constraint",
            ]
        );
    }

    #[test]
    fn constraint_kind_labels_are_stable_snake_case() {
        assert_eq!(
            ConstraintKind::CopyEdge {
                dst: SemanticNodeId(0),
                src: SemanticNodeId(1)
            }
            .as_str(),
            "copy_edge"
        );
        assert_eq!(
            ConstraintKind::Alloc {
                dst: SemanticNodeId(0),
                object: SemanticNodeId(1)
            }
            .as_str(),
            "alloc"
        );
        assert_eq!(ConstraintKind::ModelEdge.as_str(), "model_edge");
        assert_eq!(
            ConstraintKind::TypeConstraint {
                node: SemanticNodeId(0),
                type_fact: TypeFactId(1)
            }
            .as_str(),
            "type_constraint"
        );
    }

    #[test]
    fn constraint_fact_mirrors_points_to_shape_and_skips_dense_id() {
        // D-10: the fact carries exactly { id, kind, status, precision, stable_key }
        // and reuses the points-to status/precision vocabulary.
        let fact = ConstraintFact {
            id: SemanticConstraintId(7),
            kind: ConstraintKind::CallConstraint {
                callsite: SemanticNodeId(3),
            },
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: "constraint|call_constraint|cs".to_string(),
        };
        let json = serde_json::to_string(&fact).expect("serialize");
        // The dense id is skipped, so the round-tripped value carries the default id
        // (0) while every other field is preserved.
        let restored: ConstraintFact = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, SemanticConstraintId(0));
        assert_eq!(restored.kind, fact.kind);
        assert_eq!(restored.status, fact.status);
        assert_eq!(restored.precision, fact.precision);
        assert_eq!(restored.stable_key, fact.stable_key);
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn type_constraint_references_existing_type_fact_id() {
        // The TypeConstraint variant references an existing TypeFactId from the type
        // substrate, not a run-local dense ID of another family.
        let kind = ConstraintKind::TypeConstraint {
            node: SemanticNodeId(2),
            type_fact: TypeFactId(99),
        };
        match kind {
            ConstraintKind::TypeConstraint { type_fact, .. } => {
                assert_eq!(type_fact, TypeFactId(99));
            }
            _ => panic!("expected type_constraint variant"),
        }
    }
}
