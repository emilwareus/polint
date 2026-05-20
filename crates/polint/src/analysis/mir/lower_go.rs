use crate::analysis::mir::body::MirOutput;
use crate::core::AnalysisDb;

pub(crate) fn lower_go_mir(_db: &AnalysisDb) -> MirOutput {
    MirOutput {
        bodies: Vec::new(),
        places: Vec::new(),
        operations: Vec::new(),
        unsupported: Vec::new(),
    }
}

#[cfg(test)]
mod places {
    use super::*;
    use crate::analysis::places::{PlaceProjection, PlaceRoot};
    use crate::core::{AnalysisDb, FunctionId, Language};
    use std::path::PathBuf;

    fn lower(source: &str) -> MirOutput {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("auth.go"),
            "auth.go".to_string(),
            source.to_string(),
        );
        let diagnostics = crate::go::analyze(&mut db);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        lower_go_mir(&db)
    }

    #[test]
    fn go_function_places_include_parameters_locals_globals_and_projections() {
        let first = lower(
            r#"
package auth

type User struct { Tokens []string }

func authorize(user User, index int) bool {
    token := user.Tokens[index]
    global = token
    return token != ""
}
"#,
        );
        let second = lower(
            r#"
package auth

type User struct { Tokens []string }

func authorize(user User, index int) bool {
    token := user.Tokens[index]
    global = token
    return token != ""
}
"#,
        );

        assert_eq!(first.bodies.len(), 1);
        assert!(first.bodies[0].stable_key.contains("authorize"));
        assert_eq!(
            first
                .places
                .iter()
                .map(|place| place.stable_key.as_str())
                .collect::<Vec<_>>(),
            second
                .places
                .iter()
                .map(|place| place.stable_key.as_str())
                .collect::<Vec<_>>()
        );

        assert!(first.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                index: 0,
                name: Some(name),
                ..
            } if name == "user"
        )));
        assert!(first.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                index: 1,
                name: Some(name),
                ..
            } if name == "index"
        )));
        assert!(first.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Local { name, .. } if name == "token"
        )));
        assert!(first.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Global { name, .. } if name == "global"
        )));
        assert!(first.places.iter().any(|place| {
            matches!(&place.root, PlaceRoot::Parameter { name: Some(name), .. } if name == "user")
                && place
                    .projections
                    .contains(&PlaceProjection::Field("Tokens".to_string()))
                && place.projections.iter().any(|projection| {
                    matches!(projection, PlaceProjection::IndexUnknown { evidence } if evidence == "index")
                })
        }));
    }

    #[test]
    fn go_method_receiver_is_parameter_zero_and_function_name_contract_is_preserved() {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("service.go"),
            "service.go".to_string(),
            r#"
package auth

type Service struct { cache map[string]string }

func (svc *Service) authorize(user User) bool {
    token := svc.cache[user.Name]
    return token != ""
}
"#
            .to_string(),
        );
        let diagnostics = crate::go::analyze(&mut db);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let function = db
            .functions()
            .iter()
            .find(|function| function.name == "Service.authorize")
            .expect("method fact should retain existing receiver-qualified name");
        assert_eq!(function.id, FunctionId(0));
        assert_eq!(function.language, Language::Go);

        let output = lower_go_mir(&db);
        assert!(output.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                function,
                index: 0,
                name: Some(name),
            } if *function == FunctionId(0) && name == "svc"
        )));
        assert!(output.bodies[0].owner_stable_key.contains("Service.authorize"));
    }

    #[test]
    fn go_mir_place_rows_do_not_carry_parser_node_debug_evidence() {
        let output = lower(
            r#"
package auth

func authorize(user User) bool {
    token := user.Token
    return token != ""
}
"#,
        );
        let debug = format!("{output:#?}");

        assert!(!debug.contains("tree_sitter::Node"));
        assert!(!debug.contains("Node<'_"));
        assert!(!debug.contains("function_declaration"));
        assert!(!debug.contains("method_declaration"));
    }
}
