#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{CallSiteId, MirBodyId, PlaceId};
    use crate::core::{FileId, FunctionId, Language, SymbolId};

    fn single_place_key(root: PlaceRoot, projections: Vec<PlaceProjection>) -> String {
        let mut builder = PlaceTableBuilder::default();
        builder.insert(
            Language::TypeScript,
            Some(FileId(1)),
            Some(FunctionId(10)),
            root,
            projections,
            PlaceStatus::Resolved,
        );
        builder.finish().remove(0).stable_key
    }

    #[test]
    fn place_stable_keys_distinguish_supported_root_kinds() {
        let roots = [
            PlaceRoot::Local {
                function: FunctionId(10),
                name: "value".to_string(),
            },
            PlaceRoot::Parameter {
                function: FunctionId(10),
                index: 0,
                name: Some("value".to_string()),
            },
            PlaceRoot::Global {
                symbol: Some(SymbolId(3)),
                name: "value".to_string(),
            },
            PlaceRoot::Temporary {
                body: MirBodyId(4),
                ordinal: 0,
            },
            PlaceRoot::CallReturn {
                call: CallSiteId(5),
            },
            PlaceRoot::Unknown {
                evidence: "dynamic root".to_string(),
            },
        ];

        let mut keys = roots
            .into_iter()
            .map(|root| single_place_key(root, Vec::new()))
            .collect::<Vec<_>>();
        keys.sort();
        keys.dedup();

        assert_eq!(keys.len(), 6);
    }

    #[test]
    fn place_projection_stable_keys_preserve_projection_order() {
        let ordered = single_place_key(
            PlaceRoot::Local {
                function: FunctionId(10),
                name: "value".to_string(),
            },
            vec![
                PlaceProjection::Field("field".to_string()),
                PlaceProjection::Property("prop".to_string()),
                PlaceProjection::IndexKnown("0".to_string()),
                PlaceProjection::IndexUnknown {
                    evidence: "dynamic index".to_string(),
                },
                PlaceProjection::Deref,
                PlaceProjection::AwaitResult,
                PlaceProjection::CallReturn(CallSiteId(8)),
                PlaceProjection::Unknown {
                    evidence: "dynamic projection".to_string(),
                },
            ],
        );
        let reversed = single_place_key(
            PlaceRoot::Local {
                function: FunctionId(10),
                name: "value".to_string(),
            },
            vec![
                PlaceProjection::Unknown {
                    evidence: "dynamic projection".to_string(),
                },
                PlaceProjection::CallReturn(CallSiteId(8)),
                PlaceProjection::AwaitResult,
                PlaceProjection::Deref,
                PlaceProjection::IndexUnknown {
                    evidence: "dynamic index".to_string(),
                },
                PlaceProjection::IndexKnown("0".to_string()),
                PlaceProjection::Property("prop".to_string()),
                PlaceProjection::Field("field".to_string()),
            ],
        );

        assert_ne!(ordered, reversed);
        assert!(ordered.contains("projection_00"));
        assert!(ordered.contains("projection_07"));
    }

    #[test]
    fn place_table_builder_assigns_dense_ids_by_sorted_stable_key() {
        let mut builder = PlaceTableBuilder::default();
        builder.insert(
            Language::Go,
            Some(FileId(1)),
            Some(FunctionId(10)),
            PlaceRoot::Local {
                function: FunctionId(10),
                name: "zeta".to_string(),
            },
            Vec::new(),
            PlaceStatus::Resolved,
        );
        builder.insert(
            Language::Go,
            Some(FileId(1)),
            Some(FunctionId(10)),
            PlaceRoot::Local {
                function: FunctionId(10),
                name: "alpha".to_string(),
            },
            Vec::new(),
            PlaceStatus::Resolved,
        );

        let places = builder.finish();

        assert_eq!(places.iter().map(|place| place.id).collect::<Vec<_>>(), vec![PlaceId(0), PlaceId(1)]);
        assert!(places[0].stable_key < places[1].stable_key);
        assert!(!places[0].stable_key.contains("place_id"));
        assert!(!places[1].stable_key.contains("PlaceId"));
    }
}
