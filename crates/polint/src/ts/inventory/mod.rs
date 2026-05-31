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
