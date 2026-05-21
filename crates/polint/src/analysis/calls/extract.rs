#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::analysis::calls::facts::{
        CallCallee, CallPrecision, CallSyntaxKind, CallTargetStatus,
    };
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{MirOperation, MirOperationKind, MirValue};
    use crate::analysis::places::{PlaceFact, PlaceProjection, PlaceRoot, PlaceStatus};
    use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span};

    fn span(file: FileId, line: u32, start_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte: start_byte + 4,
            start_line: line,
            start_col: 1,
            end_line: line,
            end_col: 5,
        }
    }

    fn add_file_and_function(db: &mut AnalysisDb, relative_path: &str) -> (FileId, FunctionId) {
        let file = db.add_file(
            PathBuf::from(relative_path),
            relative_path.to_string(),
            "function caller() {}".to_string(),
        );
        let function = db.push_function(FunctionFact {
            id: FunctionId(999),
            file,
            name: "caller".to_string(),
            span: span(file, 1, 0),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        (file, function)
    }

    fn body(file: FileId, function: FunctionId, language: Language) -> MirBody {
        MirBody {
            id: MirBodyId(1),
            language,
            file,
            function,
            package: None,
            module: None,
            owner_stable_key: "function:caller:stable".to_string(),
            span: span(file, 1, 0),
            stable_key: "mir-body:caller".to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn place(
        id: u64,
        file: FileId,
        function: FunctionId,
        root: PlaceRoot,
        projections: Vec<PlaceProjection>,
    ) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::TypeScript,
            file: Some(file),
            function: Some(function),
            root,
            projections,
            stable_key: format!("place:{id}"),
            status: PlaceStatus::Resolved,
        }
    }

    fn call_op(
        id: u64,
        ordinal: u32,
        file: FileId,
        site: u64,
        callee: MirValue,
        arguments: Vec<PlaceId>,
    ) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(1),
            ordinal,
            span: span(file, 2, 10),
            kind: MirOperationKind::Call {
                site: CallSiteId(site),
                callee,
                arguments,
                return_place: PlaceId(9),
            },
            stable_key: format!("mir-op:call:{id}"),
            status: MirStatus::Resolved,
        }
    }

    #[test]
    fn extract_call_sites_maps_mir_calls_to_complete_call_site_facts() {
        let mut db = AnalysisDb::new();
        let (file, function) = add_file_and_function(&mut db, "src/app.ts");
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body(file, function, Language::TypeScript)],
            places: vec![
                place(
                    1,
                    file,
                    function,
                    PlaceRoot::Global {
                        symbol: None,
                        name: "run".to_string(),
                    },
                    Vec::new(),
                ),
                place(
                    2,
                    file,
                    function,
                    PlaceRoot::Local {
                        function,
                        name: "arg".to_string(),
                    },
                    Vec::new(),
                ),
                place(
                    9,
                    file,
                    function,
                    PlaceRoot::CallReturn {
                        call: CallSiteId(10),
                    },
                    Vec::new(),
                ),
            ],
            operations: vec![call_op(
                1,
                0,
                file,
                10,
                MirValue::Place(PlaceId(1)),
                vec![PlaceId(2)],
            )],
            unsupported: Vec::new(),
        })
        .expect("semantic MIR should store");

        let sites = super::extract_call_sites(&db);

        assert_eq!(sites.len(), 1);
        let site = &sites[0];
        assert_eq!(site.id, CallSiteId(10));
        assert_eq!(site.language, Language::TypeScript);
        assert_eq!(site.file, file);
        assert_eq!(site.caller, function);
        assert_eq!(site.body, MirBodyId(1));
        assert_eq!(site.operation, MirOpId(1));
        assert_eq!(site.kind, CallSyntaxKind::Function);
        assert_eq!(
            site.callee,
            CallCallee::Identifier {
                reference: None,
                name: "run".to_string()
            }
        );
        assert_eq!(site.arguments, vec![PlaceId(2)]);
        assert_eq!(site.result, Some(PlaceId(9)));
        assert_eq!(site.status, CallTargetStatus::Unresolved);
        assert_eq!(site.precision, CallPrecision::Conservative);
    }

    #[test]
    fn extract_call_sites_stable_key_uses_required_stable_inputs() {
        let mut db = AnalysisDb::new();
        let (file, function) = add_file_and_function(&mut db, "src/app.ts");
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body(file, function, Language::TypeScript)],
            places: vec![place(
                1,
                file,
                function,
                PlaceRoot::Local {
                    function,
                    name: "callback".to_string(),
                },
                Vec::new(),
            )],
            operations: vec![call_op(
                1,
                0,
                file,
                10,
                MirValue::Place(PlaceId(1)),
                Vec::new(),
            )],
            unsupported: Vec::new(),
        })
        .expect("semantic MIR should store");

        let sites = super::extract_call_sites(&db);
        let stable_key = &sites[0].stable_key;

        assert!(stable_key.contains("8:CallSite"));
        assert!(stable_key.contains("8:language=10:TypeScript"));
        assert!(stable_key.contains("8:file_key="));
        assert!(stable_key.contains("10:caller_key="));
        assert!(stable_key.contains("4:span="));
        assert!(stable_key.contains("12:callee_shape=14:function_value"));
        assert!(stable_key.contains("13:operation_key=13:mir-op:call:1"));
        assert!(stable_key.contains("9:call_kind=13:FunctionValue"));
    }

    #[test]
    fn extract_call_sites_is_deterministic_for_different_operation_orders() {
        let mut first = AnalysisDb::new();
        let (file, function) = add_file_and_function(&mut first, "src/app.ts");
        let output = MirOutput {
            bodies: vec![body(file, function, Language::TypeScript)],
            places: vec![
                place(
                    1,
                    file,
                    function,
                    PlaceRoot::Global {
                        symbol: None,
                        name: "alpha".to_string(),
                    },
                    Vec::new(),
                ),
                place(
                    2,
                    file,
                    function,
                    PlaceRoot::Global {
                        symbol: None,
                        name: "beta".to_string(),
                    },
                    Vec::new(),
                ),
                place(
                    9,
                    file,
                    function,
                    PlaceRoot::CallReturn {
                        call: CallSiteId(10),
                    },
                    Vec::new(),
                ),
            ],
            operations: vec![
                call_op(2, 1, file, 20, MirValue::Place(PlaceId(2)), Vec::new()),
                call_op(1, 0, file, 10, MirValue::Place(PlaceId(1)), Vec::new()),
            ],
            unsupported: Vec::new(),
        };
        first
            .replace_semantic_mir(output.clone())
            .expect("semantic MIR should store");

        let mut second = AnalysisDb::new();
        let (second_file, second_function) = add_file_and_function(&mut second, "src/app.ts");
        let mut reordered = output;
        reordered.bodies = vec![body(second_file, second_function, Language::TypeScript)];
        reordered.operations.reverse();
        second
            .replace_semantic_mir(reordered)
            .expect("semantic MIR should store");

        let first_keys = super::extract_call_sites(&first)
            .into_iter()
            .map(|site| site.stable_key)
            .collect::<Vec<_>>();
        let second_keys = super::extract_call_sites(&second)
            .into_iter()
            .map(|site| site.stable_key)
            .collect::<Vec<_>>();

        assert_eq!(first_keys, second_keys);
    }
}
