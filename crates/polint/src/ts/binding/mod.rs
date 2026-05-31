pub(crate) mod direct;
pub(crate) mod facts;
pub(crate) mod store;

#[cfg(test)]
mod direct_local {
    use std::path::PathBuf;

    use crate::core::AnalysisDb;
    use crate::ts::binding::direct::resolve_direct_bindings;
    use crate::ts::binding::facts::{TsDirectBindingReason, TsDirectBindingStatus};
    use crate::ts::inventory::extract::extract_ts_inventory;
    use crate::ts::scope::extract::extract_ts_scope;

    #[test]
    fn resolves_same_file_function_alias_and_static_member_calls() {
        let file = fixture_file(
            r#"
function f() {}
const alias = f;
const ns = { f };
function run() {
  f();
  alias();
  ns.f();
}
"#,
        );

        let output = resolve_direct_bindings(&extract_ts_inventory(file), &extract_ts_scope(file));
        let resolved = output
            .bindings
            .iter()
            .filter(|binding| binding.status == TsDirectBindingStatus::Resolved)
            .collect::<Vec<_>>();

        assert!(
            resolved
                .iter()
                .any(|binding| binding.callsite_stable_key.contains("f"))
        );
        assert!(
            resolved
                .iter()
                .any(|binding| binding.callsite_stable_key.contains("alias"))
        );
        assert!(
            resolved
                .iter()
                .any(|binding| binding.callsite_stable_key.contains("ns.f"))
        );
    }

    #[test]
    fn computed_property_and_parameter_callback_remain_unresolved() {
        let file = fixture_file(
            r#"
function run(cb, obj, key) {
  cb();
  obj[key]();
}
"#,
        );

        let output = resolve_direct_bindings(&extract_ts_inventory(file), &extract_ts_scope(file));
        let reasons = output
            .bindings
            .iter()
            .filter_map(|binding| binding.reason)
            .collect::<Vec<_>>();

        assert!(reasons.contains(&TsDirectBindingReason::ComputedProperty));
        assert!(reasons.contains(&TsDirectBindingReason::TokenFlowRequired));
    }

    fn fixture_file(source: &str) -> &'static crate::core::SourceFile {
        let mut db = Box::new(AnalysisDb::new());
        let file_id = db.add_file(
            PathBuf::from("src/direct.ts"),
            "src/direct.ts".to_string(),
            source.to_string(),
        );
        let leaked = Box::leak(db);
        leaked.file(file_id).expect("fixture file")
    }
}
