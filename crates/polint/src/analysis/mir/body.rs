use serde::{Deserialize, Serialize};

use crate::analysis::ids::{
    MirBodyId, MirOpId, MirPredicateId, MirStatementId, MirTerminatorId, UnsupportedId,
};
use crate::analysis::mir::op::{MirOperation, UnsupportedSemanticFact};
use crate::analysis::places::PlaceFact;
use crate::core::{FileId, FunctionId, Language, ModuleNodeId, PackageId, Span};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MirBody {
    pub(crate) id: MirBodyId,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) function: FunctionId,
    pub(crate) package: Option<PackageId>,
    pub(crate) module: Option<ModuleNodeId>,
    pub(crate) owner_stable_key: String,
    pub(crate) span: Span,
    pub(crate) stable_key: String,
    pub(crate) status: MirStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[expect(
    dead_code,
    reason = "Statement rows are part of the private MIR contract before lowering populates them."
)]
pub(crate) struct MirStatement {
    pub(crate) id: MirStatementId,
    pub(crate) body: MirBodyId,
    pub(crate) ordinal: u32,
    pub(crate) operation: MirOpId,
    pub(crate) stable_key: String,
    pub(crate) status: MirStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[expect(
    dead_code,
    reason = "Terminator rows are part of the private MIR contract before CFG lowering consumes them."
)]
pub(crate) struct MirTerminator {
    pub(crate) id: MirTerminatorId,
    pub(crate) body: MirBodyId,
    pub(crate) ordinal: u32,
    pub(crate) kind: MirTerminatorKind,
    pub(crate) stable_key: String,
    pub(crate) status: MirStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MirTerminatorKind {
    Branch { predicate: MirPredicateId },
    Return,
    Unsupported { unsupported: UnsupportedId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MirOutput {
    pub(crate) bodies: Vec<MirBody>,
    pub(crate) places: Vec<PlaceFact>,
    pub(crate) operations: Vec<MirOperation>,
    pub(crate) unsupported: Vec<UnsupportedSemanticFact>,
}

impl MirOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.bodies
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.places
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.operations.sort_by(|left, right| {
            (left.body, left.ordinal, left.stable_key.as_str()).cmp(&(
                right.body,
                right.ordinal,
                right.stable_key.as_str(),
            ))
        });
        self.unsupported
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum MirStatus {
    Resolved,
    Partial,
    Unknown,
    Unsupported,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId, UnsupportedId};
    use crate::analysis::mir::op::{
        AssignMode, ConservativeAction, MirOperation, MirOperationKind, MirValue,
        UnsupportedDomain, UnsupportedPrecision, UnsupportedSemanticFact,
    };
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::core::{FileId, FunctionId, Language, ModuleNodeId, PackageId, Span};

    fn span() -> Span {
        Span {
            file: FileId(1),
            start_byte: 1,
            end_byte: 2,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 2,
        }
    }

    fn body(id: u64, stable_key: &str) -> MirBody {
        MirBody {
            id: MirBodyId(id),
            language: Language::Go,
            file: FileId(1),
            function: FunctionId(id),
            package: Some(PackageId(1)),
            module: Some(ModuleNodeId(1)),
            owner_stable_key: format!("owner:{id}"),
            span: span(),
            stable_key: stable_key.to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn place(id: u64, stable_key: &str) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::Go,
            file: Some(FileId(1)),
            function: Some(FunctionId(1)),
            root: PlaceRoot::Local {
                function: FunctionId(1),
                name: stable_key.to_string(),
            },
            projections: Vec::new(),
            stable_key: stable_key.to_string(),
            status: PlaceStatus::Resolved,
        }
    }

    fn operation(body: u64, ordinal: u32, stable_key: &str) -> MirOperation {
        MirOperation {
            id: MirOpId(u64::from(ordinal)),
            body: MirBodyId(body),
            ordinal,
            span: span(),
            kind: MirOperationKind::Assign {
                place: PlaceId(1),
                value: MirValue::Place(PlaceId(2)),
                mode: AssignMode::Overwrite,
            },
            stable_key: stable_key.to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn unsupported(stable_key: &str) -> UnsupportedSemanticFact {
        UnsupportedSemanticFact {
            id: UnsupportedId(1),
            body: Some(MirBodyId(1)),
            operation: Some(MirOpId(1)),
            language: Language::Go,
            file: FileId(1),
            span: span(),
            construct: "go-reflect".to_string(),
            source_evidence: "reflect.ValueOf(x)".to_string(),
            affected_places: vec![PlaceId(1)],
            affected_domains: vec![UnsupportedDomain::Mir, UnsupportedDomain::DataFlow],
            conservative_action: ConservativeAction::HavocAffectedPlaces,
            precision: UnsupportedPrecision::Unsupported,
            status: MirStatus::Unsupported,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn mir_output_normalized_sorts_all_fact_rows_deterministically() {
        let output = MirOutput {
            bodies: vec![body(2, "body:z"), body(1, "body:a")],
            places: vec![place(2, "place:z"), place(1, "place:a")],
            operations: vec![
                operation(2, 2, "op:z"),
                operation(1, 2, "op:b"),
                operation(1, 1, "op:z"),
                operation(1, 1, "op:a"),
            ],
            unsupported: vec![unsupported("unsupported:z"), unsupported("unsupported:a")],
        }
        .normalized();

        assert_eq!(
            output.bodies.iter().map(|body| body.stable_key.as_str()).collect::<Vec<_>>(),
            vec!["body:a", "body:z"]
        );
        assert_eq!(
            output.places.iter().map(|place| place.stable_key.as_str()).collect::<Vec<_>>(),
            vec!["place:a", "place:z"]
        );
        assert_eq!(
            output.operations.iter().map(|op| (op.body, op.ordinal, op.stable_key.as_str())).collect::<Vec<_>>(),
            vec![
                (MirBodyId(1), 1, "op:a"),
                (MirBodyId(1), 1, "op:z"),
                (MirBodyId(1), 2, "op:b"),
                (MirBodyId(2), 2, "op:z"),
            ]
        );
        assert_eq!(
            output.unsupported.iter().map(|row| row.stable_key.as_str()).collect::<Vec<_>>(),
            vec!["unsupported:a", "unsupported:z"]
        );
    }

    #[test]
    fn mir_contract_source_does_not_store_parser_ast_objects() {
        let source = [
            include_str!("body.rs"),
            include_str!("op.rs"),
            include_str!("../places.rs"),
        ]
        .join("\n");

        let forbidden = [
            concat!("tree_sitter", "::Node"),
            concat!("oxc", "_ast"),
            concat!("oxc", "_span::Span"),
            concat!("Node", "<'_"),
            concat!("Program", "<'_"),
        ];

        for forbidden in forbidden {
            assert!(!source.contains(forbidden), "forbidden parser type leaked: {forbidden}");
        }
    }

    #[test]
    fn unsupported_semantic_rows_require_evidence_and_conservative_labels() {
        assert!(unsupported("unsupported:complete").is_complete());

        let incomplete = UnsupportedSemanticFact {
            construct: String::new(),
            source_evidence: String::new(),
            affected_domains: Vec::new(),
            ..unsupported("unsupported:missing")
        };

        assert!(!incomplete.is_complete());
    }

    #[test]
    fn mir_operation_kind_covers_call_shaped_operations_without_targets() {
        let operation = MirOperationKind::Call {
            site: CallSiteId(1),
            callee: MirValue::Unknown {
                evidence: "dynamic callee".to_string(),
            },
            arguments: vec![PlaceId(2)],
            return_place: PlaceId(3),
        };

        assert!(matches!(operation, MirOperationKind::Call { .. }));
    }
}
