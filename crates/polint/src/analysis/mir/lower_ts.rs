use crate::analysis::mir::body::MirOutput;
use crate::core::AnalysisDb;

pub(crate) fn lower_ts_mir(_db: &AnalysisDb) -> MirOutput {
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

    fn lower(path: &str, source: &str) -> MirOutput {
        let mut db = AnalysisDb::new();
        db.add_file(PathBuf::from(path), path.to_string(), source.to_string());
        let diagnostics = crate::ts::analyze(&mut db);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        lower_ts_mir(&db)
    }

    #[test]
    fn ts_function_places_include_parameters_locals_globals_properties_and_indexes() {
        let source = r#"
export function render(user, index) {
  const token = user.tokens[index];
  window.value = token;
  return token;
}
"#;
        let first = lower("src/render.ts", source);
        let second = lower("src/render.ts", source);

        assert_eq!(first.bodies.len(), 1);
        assert!(first.bodies[0].stable_key.contains("render"));
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
            PlaceRoot::Unknown { evidence } if evidence == "window"
        )));
        assert!(first.places.iter().any(|place| {
            matches!(&place.root, PlaceRoot::Parameter { name: Some(name), .. } if name == "user")
                && place
                    .projections
                    .contains(&PlaceProjection::Property("tokens".to_string()))
                && place.projections.iter().any(|projection| {
                    matches!(projection, PlaceProjection::IndexUnknown { evidence } if evidence == "index")
                })
        }));
    }

    #[test]
    fn ts_arrow_functions_and_class_methods_join_existing_function_facts() {
        let source = r#"
const render = (user) => user.name;

class View {
  render(user) {
    return user.name;
  }
}
"#;
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("src/view.tsx"),
            "src/view.tsx".to_string(),
            source.to_string(),
        );
        let diagnostics = crate::ts::analyze(&mut db);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let arrow = db
            .functions()
            .iter()
            .find(|function| function.name == "render")
            .expect("arrow function fact should use variable name");
        let method = db
            .functions()
            .iter()
            .find(|function| function.name == "View.render")
            .expect("method function fact should use class-qualified name");

        let output = lower_ts_mir(&db);
        assert!(output.bodies.iter().any(|body| {
            body.function == arrow.id
                && body.language == Language::Tsx
                && body.owner_stable_key.contains("render")
        }));
        assert!(output.bodies.iter().any(|body| {
            body.function == method.id
                && body.language == Language::Tsx
                && body.owner_stable_key.contains("View.render")
        }));
        assert!(output.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                function,
                index: 0,
                name: Some(name),
            } if *function == FunctionId(0) && name == "user"
        )));
        assert!(output.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                function,
                index: 0,
                name: Some(name),
            } if *function == FunctionId(1) && name == "user"
        )));
    }

    #[test]
    fn ts_mir_place_rows_do_not_carry_oxc_ast_debug_evidence() {
        let output = lower(
            "src/render.ts",
            r#"
export function render(user) {
  const token = user.token;
  return token;
}
"#,
        );
        let debug = format!("{output:#?}");

        assert!(!debug.contains("oxc_ast"));
        assert!(!debug.contains("oxc_span::Span"));
        assert!(!debug.contains("Program<'_"));
        assert!(!debug.contains("Expression<'_"));
        assert!(!debug.contains("Statement<'_"));
        assert!(!debug.contains("FunctionDeclaration"));
        assert!(!debug.contains("ArrowFunctionExpression"));
        assert!(!debug.contains("ClassElement"));
    }
}
