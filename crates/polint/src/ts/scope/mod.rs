#[cfg(feature = "lang-typescript")]
pub mod extract;
pub mod facts;
pub mod store;

#[cfg(all(test, feature = "lang-typescript"))]
mod direct_binding_boundary {
    use std::path::PathBuf;

    use crate::ts::local_db::LocalFactDb;
    use crate::ts::scope::extract::extract_ts_scope;
    use crate::ts::scope::facts::{TsBindingKind, TsBindingStatus};

    #[test]
    fn dynamic_direct_binding_cases_remain_unsupported() {
        let file = fixture_file(
            r#"
function run(cb, obj, key) {
  cb();
  obj[key]();
}
class Box {
  method() {
    this.run();
  }
}
Box.prototype.extra();
"#,
        );

        let interner = crate::internal_core::StableKeyInterner::default();
        let output = extract_ts_scope(&interner, file);
        let unsupported = output
            .bindings
            .iter()
            .filter(|binding| binding.binding_kind == TsBindingKind::UnsupportedDynamic)
            .collect::<Vec<_>>();
        let reasons = unsupported
            .iter()
            .filter_map(|binding| match &binding.status {
                TsBindingStatus::UnsupportedDynamic { reason } => Some(reason.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(reasons.contains(&"parameter callback requires function-token analysis"));
        assert!(reasons.contains(&"computed property requires property-flow analysis"));
        assert!(reasons.contains(&"prototype dispatch requires prototype analysis"));
        assert!(reasons.contains(&"this-dependent call requires receiver modeling"));
    }

    #[test]
    fn boundary_rows_do_not_resolve_to_present_bindings() {
        let file = fixture_file(
            r#"
function invoke(cb) {
  return cb();
}
"#,
        );

        let interner = crate::internal_core::StableKeyInterner::default();
        let output = extract_ts_scope(&interner, file);
        let callback = output
            .bindings
            .iter()
            .find(|binding| {
                binding.name == "cb" && binding.binding_kind == TsBindingKind::UnsupportedDynamic
            })
            .expect("callback boundary row");

        assert!(matches!(
            callback.status,
            TsBindingStatus::UnsupportedDynamic { .. }
        ));
    }

    fn fixture_file(source: &str) -> &'static crate::analysis_api::SourceFile {
        let mut db = Box::new(LocalFactDb::new());
        let file_id = db.add_file(
            PathBuf::from("src/boundary.ts"),
            "src/boundary.ts".to_string(),
            source.to_string(),
        );
        let leaked = Box::leak(db);
        leaked.file(file_id).expect("fixture file")
    }
}
