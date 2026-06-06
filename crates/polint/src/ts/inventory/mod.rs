pub(crate) mod extract;
pub(crate) mod facts;
pub(crate) mod store;

#[cfg(test)]
mod extract_function_forms {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use crate::core::AnalysisDb;
    use crate::ts::inventory::extract::extract_ts_inventory;
    use crate::ts::inventory::facts::TsFunctionInventoryKind;

    #[test]
    fn extracts_every_required_function_form() {
        let file = fixture_file(
            r#"
function declared() {}
const expr = function namedExpr() {};
const arrow = () => {};
class Box {
  constructor() {}
  method() {}
  get value() { return 1; }
  set value(next) {}
  static {
    declared();
  }
}
"#,
        );

        let output = extract_ts_inventory(file);
        let kinds = output
            .functions
            .iter()
            .map(|function| function.kind)
            .collect::<BTreeSet<_>>();

        for expected in [
            TsFunctionInventoryKind::Declaration,
            TsFunctionInventoryKind::FunctionExpression,
            TsFunctionInventoryKind::Arrow,
            TsFunctionInventoryKind::Method,
            TsFunctionInventoryKind::Constructor,
            TsFunctionInventoryKind::Accessor,
            TsFunctionInventoryKind::ClassStaticBlock,
        ] {
            assert!(kinds.contains(&expected), "missing {expected:?}");
        }
    }

    #[test]
    fn normalized_function_rows_sort_by_stable_key_before_dense_ids() {
        let file = fixture_file(
            r#"
const second = () => {};
function first() {}
"#,
        );
        let mut output = extract_ts_inventory(file);
        output.functions.reverse();

        let normalized = output.normalized();
        let stable_keys = normalized
            .functions
            .iter()
            .map(|function| function.stable_key.as_str())
            .collect::<Vec<_>>();
        let mut sorted_keys = stable_keys.clone();
        sorted_keys.sort();
        let dense_ids = normalized
            .functions
            .iter()
            .map(|function| function.id.0)
            .collect::<Vec<_>>();

        assert_eq!(stable_keys, sorted_keys);
        assert_eq!(dense_ids, vec![0, 1]);
    }

    #[test]
    fn stable_keys_include_file_span_parent_kind_and_display_name() {
        let file = fixture_file(
            r#"
function outer() {
  const inner = () => {};
}
"#,
        );
        let output = extract_ts_inventory(file);
        let inner = output
            .functions
            .iter()
            .find(|function| function.display_name.as_deref() == Some("inner"))
            .expect("inner arrow inventory row");

        assert!(inner.stable_key.contains("src/forms.ts"));
        assert!(inner.stable_key.contains("arrow"));
        assert!(inner.stable_key.contains("inner"));
        assert!(inner.stable_key.contains("outer"));
        assert!(inner.span.start_byte < inner.span.end_byte);
    }

    fn fixture_file(source: &str) -> &'static crate::core::SourceFile {
        let mut db = Box::new(AnalysisDb::new());
        let file_id = db.add_file(
            PathBuf::from("src/forms.ts"),
            "src/forms.ts".to_string(),
            source.to_string(),
        );
        let leaked = Box::leak(db);
        leaked.file(file_id).expect("fixture file")
    }
}

#[cfg(test)]
mod extract_callsite_forms {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use crate::core::AnalysisDb;
    use crate::ts::inventory::extract::extract_ts_inventory;
    use crate::ts::inventory::facts::{TsCallsiteInventoryKind, TsInventoryStatus};

    #[test]
    fn extracts_every_required_callsite_form() {
        let file = fixture_file(
            r#"
function invoke(dynamicSpecifier, maybe) {
  run();
  new Widget();
  tag`hello`;
  maybe?.();
  import("./static");
  import(dynamicSpecifier);
  require("pkg");
}
"#,
        );

        let output = extract_ts_inventory(file);
        let kinds = output
            .callsites
            .iter()
            .map(|callsite| callsite.kind)
            .collect::<BTreeSet<_>>();

        for expected in [
            TsCallsiteInventoryKind::Call,
            TsCallsiteInventoryKind::New,
            TsCallsiteInventoryKind::TaggedTemplate,
            TsCallsiteInventoryKind::OptionalCall,
            TsCallsiteInventoryKind::DynamicImport,
            TsCallsiteInventoryKind::Require,
        ] {
            assert!(kinds.contains(&expected), "missing {expected:?}");
        }
    }

    #[test]
    fn non_string_dynamic_import_is_explicitly_unresolved() {
        let file = fixture_file(
            r#"
function load(path) {
  return import(path);
}
"#,
        );

        let output = extract_ts_inventory(file);
        let dynamic_import = output
            .callsites
            .iter()
            .find(|callsite| callsite.kind == TsCallsiteInventoryKind::DynamicImport)
            .expect("dynamic import callsite");

        assert!(matches!(
            &dynamic_import.status,
            TsInventoryStatus::Unresolved { reason } if reason == "non-string dynamic import"
        ));
    }

    #[test]
    fn callsite_spans_are_valid_byte_spans() {
        let file = fixture_file(
            r#"
function invoke() {
  run();
}
"#,
        );

        let output = extract_ts_inventory(file);
        let call = output
            .callsites
            .iter()
            .find(|callsite| callsite.kind == TsCallsiteInventoryKind::Call)
            .expect("normal callsite");

        assert!(call.span.start_byte < call.span.end_byte);
        assert!(call.stable_key.contains("src/calls.ts"));
        assert!(call.stable_key.contains("call"));
    }

    #[test]
    fn callsite_spans_match_jelly_parenthesized_call_shapes() {
        let source = r#"
(function(){})();
((f))();
(f());
((new f()));
function f() {}
"#;
        let file = fixture_file(source);

        let output = extract_ts_inventory(file);
        let spans = output
            .callsites
            .iter()
            .map(|callsite| {
                (
                    callsite.span.start_byte,
                    callsite.span.end_byte,
                    &source[callsite.span.start_byte as usize..callsite.span.end_byte as usize],
                )
            })
            .collect::<Vec<_>>();
        let span_texts = output
            .callsites
            .iter()
            .map(|callsite| {
                &source[callsite.span.start_byte as usize..callsite.span.end_byte as usize]
            })
            .collect::<BTreeSet<_>>();

        for expected in ["function(){})()", "(f))()", "(f())", "(new f())"] {
            assert!(
                span_texts.contains(expected),
                "missing span {expected:?} in {span_texts:?}; spans: {spans:?}"
            );
        }
    }

    fn fixture_file(source: &str) -> &'static crate::core::SourceFile {
        let mut db = Box::new(AnalysisDb::new());
        let file_id = db.add_file(
            PathBuf::from("src/calls.ts"),
            "src/calls.ts".to_string(),
            source.to_string(),
        );
        let leaked = Box::leak(db);
        leaked.file(file_id).expect("fixture file")
    }
}
