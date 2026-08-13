pub mod direct;
pub mod facts;
pub mod store;

#[cfg(test)]
mod direct_local {
    use std::path::PathBuf;

    use crate::ts::binding::direct::resolve_direct_bindings;
    use crate::ts::binding::facts::{
        TsDirectBindingFact, TsDirectBindingReason, TsDirectBindingStatus,
    };
    use crate::ts::inventory::extract::extract_ts_inventory;
    use crate::ts::inventory::store::TsInventoryOutput;
    use crate::ts::local_db::LocalFactDb;
    use crate::ts::scope::extract::extract_ts_scope;

    #[test]
    fn resolves_same_file_function_alias_and_static_member_calls() {
        let interner = crate::internal_core::StableKeyInterner::default();
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

        let output = resolve_direct_bindings(
            &interner,
            &extract_ts_inventory(&interner, file),
            &extract_ts_scope(&interner, file),
        );
        let resolved = output
            .bindings
            .iter()
            .filter(|binding| binding.status == TsDirectBindingStatus::Resolved)
            .collect::<Vec<_>>();

        assert!(
            resolved
                .iter()
                .any(|binding| interner.resolve(binding.callsite_stable_key).contains("f"))
        );
        assert!(resolved.iter().any(|binding| {
            interner
                .resolve(binding.callsite_stable_key)
                .contains("alias")
        }));
        assert!(resolved.iter().any(|binding| {
            interner
                .resolve(binding.callsite_stable_key)
                .contains("ns.f")
        }));
    }

    #[test]
    fn arbitrary_static_member_does_not_bind_to_same_named_function() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let file = fixture_file(
            r#"
function f() {}
function run(obj) {
  obj.f();
}
"#,
        );

        let inventory = extract_ts_inventory(&interner, file);
        let output =
            resolve_direct_bindings(&interner, &inventory, &extract_ts_scope(&interner, file));
        let binding = binding_for_display(&output, &inventory, "obj.f");

        assert_eq!(binding.status, TsDirectBindingStatus::Unresolved);
        assert!(binding.target_function.is_none());
    }

    #[test]
    fn block_scoped_alias_does_not_escape_into_other_function() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let file = fixture_file(
            r#"
function target() {}
if (ready) {
  const alias = target;
}
function run() {
  alias();
}
"#,
        );

        let inventory = extract_ts_inventory(&interner, file);
        let output =
            resolve_direct_bindings(&interner, &inventory, &extract_ts_scope(&interner, file));
        let binding = binding_for_display(&output, &inventory, "alias");

        assert_eq!(binding.status, TsDirectBindingStatus::Unresolved);
        assert!(binding.target_function.is_none());
    }

    #[test]
    fn non_function_local_alias_blocks_outer_function_resolution() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let file = fixture_file(
            r#"
function target() {}
function run() {
  const target = maybeFunction;
  target();
}
"#,
        );

        let inventory = extract_ts_inventory(&interner, file);
        let output =
            resolve_direct_bindings(&interner, &inventory, &extract_ts_scope(&interner, file));
        let binding = binding_for_display(&output, &inventory, "target");

        assert_eq!(binding.status, TsDirectBindingStatus::Unresolved);
        assert!(binding.target_function.is_none());
    }

    #[test]
    fn alias_target_resolution_respects_shadowing_scope() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let file = fixture_file(
            r#"
function f() {}
function run(f) {
  const alias = f;
  alias();
}
"#,
        );

        let inventory = extract_ts_inventory(&interner, file);
        let output =
            resolve_direct_bindings(&interner, &inventory, &extract_ts_scope(&interner, file));
        let binding = binding_for_display(&output, &inventory, "alias");

        assert_eq!(binding.status, TsDirectBindingStatus::Unresolved);
        assert!(binding.target_function.is_none());
    }

    #[test]
    fn parameter_callback_boundary_is_scope_aware() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let file = fixture_file(
            r#"
function cb() {}
function run() {
  cb();
}
function invoke(cb) {
  cb();
}
"#,
        );

        let inventory = extract_ts_inventory(&interner, file);
        let output =
            resolve_direct_bindings(&interner, &inventory, &extract_ts_scope(&interner, file));
        let cb_bindings = bindings_for_display(&output, &inventory, "cb");

        assert!(
            cb_bindings
                .iter()
                .any(|binding| binding.status == TsDirectBindingStatus::Resolved)
        );
        assert!(cb_bindings.iter().any(|binding| {
            binding.status == TsDirectBindingStatus::Unresolved
                && binding.reason == Some(TsDirectBindingReason::TokenFlowRequired)
        }));
    }

    #[test]
    fn local_function_expression_binding_resolves_by_lexical_binding() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let file = fixture_file(
            r#"
const f = function() {};
function run() {
  f();
}
"#,
        );

        let inventory = extract_ts_inventory(&interner, file);
        let output =
            resolve_direct_bindings(&interner, &inventory, &extract_ts_scope(&interner, file));
        let binding = binding_for_display(&output, &inventory, "f");

        assert_eq!(binding.status, TsDirectBindingStatus::Resolved);
        assert!(binding.target_function.is_some());
    }

    #[test]
    fn resolves_object_literal_destructuring_alias_call() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let file = fixture_file(
            r#"
function localTarget() {}
const { destructured } = { destructured: localTarget };
function run() {
  destructured();
}
"#,
        );

        let output = resolve_direct_bindings(
            &interner,
            &extract_ts_inventory(&interner, file),
            &extract_ts_scope(&interner, file),
        );
        let binding = output
            .bindings
            .iter()
            .find(|binding| {
                interner
                    .resolve(binding.callsite_stable_key)
                    .contains("destructured")
            })
            .expect("destructured call binding");

        assert_eq!(binding.status, TsDirectBindingStatus::Resolved);
    }

    #[test]
    fn computed_property_and_parameter_callback_remain_unresolved() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let file = fixture_file(
            r#"
function run(cb, obj, key) {
  cb();
  obj[key]();
}
"#,
        );

        let output = resolve_direct_bindings(
            &interner,
            &extract_ts_inventory(&interner, file),
            &extract_ts_scope(&interner, file),
        );
        let reasons = output
            .bindings
            .iter()
            .filter_map(|binding| binding.reason)
            .collect::<Vec<_>>();

        assert!(reasons.contains(&TsDirectBindingReason::ComputedProperty));
        assert!(reasons.contains(&TsDirectBindingReason::TokenFlowRequired));
    }

    fn fixture_file(source: &str) -> &'static crate::analysis_api::SourceFile {
        let mut db = Box::new(LocalFactDb::new());
        let file_id = db.add_file(
            PathBuf::from("src/direct.ts"),
            "src/direct.ts".to_string(),
            source.to_string(),
        );
        let leaked = Box::leak(db);
        leaked.file(file_id).expect("fixture file")
    }

    fn binding_for_display<'a>(
        output: &'a crate::ts::binding::store::TsDirectBindingOutput,
        inventory: &TsInventoryOutput,
        display_name: &str,
    ) -> &'a TsDirectBindingFact {
        let callsite = inventory
            .callsites
            .iter()
            .find(|callsite| callsite.display_name.as_deref() == Some(display_name))
            .unwrap_or_else(|| panic!("missing callsite {display_name}"));
        output
            .bindings
            .iter()
            .find(|binding| binding.callsite == callsite.id)
            .unwrap_or_else(|| panic!("missing binding for callsite {display_name}"))
    }

    fn bindings_for_display<'a>(
        output: &'a crate::ts::binding::store::TsDirectBindingOutput,
        inventory: &TsInventoryOutput,
        display_name: &str,
    ) -> Vec<&'a TsDirectBindingFact> {
        let callsite_ids = inventory
            .callsites
            .iter()
            .filter(|callsite| callsite.display_name.as_deref() == Some(display_name))
            .map(|callsite| callsite.id)
            .collect::<Vec<_>>();
        output
            .bindings
            .iter()
            .filter(|binding| callsite_ids.contains(&binding.callsite))
            .collect()
    }
}

#[cfg(test)]
mod direct_modules {
    use std::path::PathBuf;

    use crate::analysis_api::{
        ImportFact, ModuleNode, ModuleNodeKind, ResolutionPrecision, ResolutionStatus,
        ResolvedImportFact,
    };
    use crate::internal_core::{FileId, ImportId, Language, ModuleNodeId, Span};
    use crate::ts::binding::direct::{
        TsDirectBindingModuleFile, TsDirectBindingModuleInput, resolve_direct_bindings_with_modules,
    };
    use crate::ts::binding::facts::{
        TsDirectBindingKind, TsDirectBindingReason, TsDirectBindingStatus,
    };
    use crate::ts::binding::store::TsDirectBindingOutput;
    use crate::ts::inventory::extract::extract_ts_inventory;
    use crate::ts::inventory::store::TsInventoryOutput;
    use crate::ts::local_db::LocalFactDb;
    use crate::ts::scope::extract::extract_ts_scope;
    use crate::ts::scope::store::TsScopeOutput;

    #[test]
    fn resolves_esm_reexports_commonjs_and_path_aliases_from_module_graph_facts() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let mut db = Box::new(LocalFactDb::new());
        let app_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            r#"
import { f as g } from "./m";
import defaultFn from "./default";
import * as ns from "./ns";
import { h } from "@pkg/barrel";
import { f as aliasPath } from "@lib/m";
import { x } from "external-pkg";
const cjsMember = require("./cjs").f;
function run() {
  g();
  defaultFn();
  ns.f();
  h();
  aliasPath();
  cjsMember();
  x();
}
"#
            .to_string(),
        );
        let m_file = db.add_file(
            PathBuf::from("src/m.ts"),
            "src/m.ts".to_string(),
            "export function f() {}".to_string(),
        );
        let default_file = db.add_file(
            PathBuf::from("src/default.ts"),
            "src/default.ts".to_string(),
            "export default function defaultExport() {}".to_string(),
        );
        let ns_file = db.add_file(
            PathBuf::from("src/ns.ts"),
            "src/ns.ts".to_string(),
            "export function f() {}".to_string(),
        );
        let barrel_file = db.add_file(
            PathBuf::from("packages/pkg/barrel.ts"),
            "packages/pkg/barrel.ts".to_string(),
            r#"export { f as h } from "./m";"#.to_string(),
        );
        let cjs_file = db.add_file(
            PathBuf::from("src/cjs.ts"),
            "src/cjs.ts".to_string(),
            "exports.f = function f() {};".to_string(),
        );
        let db = Box::leak(db);

        let app_inventory = extract_ts_inventory(&interner, db.file(app_file).expect("app file"));
        let app_scope = extract_ts_scope(&interner, db.file(app_file).expect("app file"));
        let m_inventory = extract_ts_inventory(&interner, db.file(m_file).expect("m file"));
        let m_scope = extract_ts_scope(&interner, db.file(m_file).expect("m file"));
        let default_inventory =
            extract_ts_inventory(&interner, db.file(default_file).expect("default file"));
        let default_scope =
            extract_ts_scope(&interner, db.file(default_file).expect("default file"));
        let ns_inventory = extract_ts_inventory(&interner, db.file(ns_file).expect("ns file"));
        let ns_scope = extract_ts_scope(&interner, db.file(ns_file).expect("ns file"));
        let barrel_inventory =
            extract_ts_inventory(&interner, db.file(barrel_file).expect("barrel file"));
        let barrel_scope = extract_ts_scope(&interner, db.file(barrel_file).expect("barrel file"));
        let cjs_inventory = extract_ts_inventory(&interner, db.file(cjs_file).expect("cjs file"));
        let cjs_scope = extract_ts_scope(&interner, db.file(cjs_file).expect("cjs file"));

        let app_node = ModuleNodeId::from_raw(0);
        let m_node = ModuleNodeId::from_raw(1);
        let default_node = ModuleNodeId::from_raw(2);
        let ns_node = ModuleNodeId::from_raw(3);
        let barrel_node = ModuleNodeId::from_raw(4);
        let cjs_node = ModuleNodeId::from_raw(5);
        let external_node = ModuleNodeId::from_raw(6);
        let imports = vec![
            import(0, app_file, "./m"),
            import(1, app_file, "./default"),
            import(2, app_file, "./ns"),
            import(3, app_file, "@pkg/barrel"),
            import(4, app_file, "@lib/m"),
            import(5, app_file, "external-pkg"),
            import(6, app_file, "./cjs"),
            import(7, barrel_file, "./m"),
        ];
        let resolved = vec![
            resolved(0, 0, app_file, Some(m_node), ResolutionStatus::Resolved),
            resolved(
                1,
                1,
                app_file,
                Some(default_node),
                ResolutionStatus::Resolved,
            ),
            resolved(2, 2, app_file, Some(ns_node), ResolutionStatus::Resolved),
            resolved(
                3,
                3,
                app_file,
                Some(barrel_node),
                ResolutionStatus::Resolved,
            ),
            resolved(4, 4, app_file, Some(m_node), ResolutionStatus::Resolved),
            resolved(
                5,
                5,
                app_file,
                Some(external_node),
                ResolutionStatus::External,
            ),
            resolved(6, 6, app_file, Some(cjs_node), ResolutionStatus::Resolved),
            resolved(7, 7, barrel_file, Some(m_node), ResolutionStatus::Resolved),
        ];
        let nodes = vec![
            file_node(app_node, "src/app.ts", app_file),
            file_node(m_node, "src/m.ts", m_file),
            file_node(default_node, "src/default.ts", default_file),
            file_node(ns_node, "src/ns.ts", ns_file),
            file_node(barrel_node, "packages/pkg/barrel.ts", barrel_file),
            file_node(cjs_node, "src/cjs.ts", cjs_file),
            ModuleNode::new(
                external_node,
                ModuleNodeKind::External,
                "external-pkg".to_string(),
                None,
                None,
                Some(Language::TypeScript),
            ),
        ];
        let module_files = vec![
            module_file(m_node, &m_inventory, &m_scope),
            module_file(default_node, &default_inventory, &default_scope),
            module_file(ns_node, &ns_inventory, &ns_scope),
            module_file(barrel_node, &barrel_inventory, &barrel_scope),
            module_file(cjs_node, &cjs_inventory, &cjs_scope),
        ];
        let module_input = TsDirectBindingModuleInput {
            imports: &imports,
            resolved_imports: &resolved,
            module_nodes: &nodes,
            module_files: &module_files,
        };

        let output = resolve_direct_bindings_with_modules(
            &interner,
            &app_inventory,
            &app_scope,
            &module_input,
        );

        assert_call(
            &output,
            &app_inventory,
            "g",
            TsDirectBindingStatus::Resolved,
            TsDirectBindingKind::ImportedNamed,
            None,
        );
        assert_call(
            &output,
            &app_inventory,
            "defaultFn",
            TsDirectBindingStatus::Resolved,
            TsDirectBindingKind::ImportedDefault,
            None,
        );
        assert_call(
            &output,
            &app_inventory,
            "ns.f",
            TsDirectBindingStatus::Resolved,
            TsDirectBindingKind::ImportedNamespaceMember,
            None,
        );
        assert_call(
            &output,
            &app_inventory,
            "h",
            TsDirectBindingStatus::Resolved,
            TsDirectBindingKind::ReExportedAlias,
            None,
        );
        assert_call(
            &output,
            &app_inventory,
            "aliasPath",
            TsDirectBindingStatus::Resolved,
            TsDirectBindingKind::ImportedNamed,
            None,
        );
        assert_call(
            &output,
            &app_inventory,
            "cjsMember",
            TsDirectBindingStatus::Resolved,
            TsDirectBindingKind::CommonJsRequireMember,
            None,
        );
        assert_call(
            &output,
            &app_inventory,
            "x",
            TsDirectBindingStatus::External,
            TsDirectBindingKind::ImportedNamed,
            Some(TsDirectBindingReason::ExternalPackageUnresolved),
        );
    }

    #[test]
    fn local_parameter_shadowing_import_requires_token_flow() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let mut db = Box::new(LocalFactDb::new());
        let app_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            r#"
import { f } from "./m";
function run(f) {
  f();
}
"#
            .to_string(),
        );
        let m_file = db.add_file(
            PathBuf::from("src/m.ts"),
            "src/m.ts".to_string(),
            "export function f() {}".to_string(),
        );
        let db = Box::leak(db);

        let app_inventory = extract_ts_inventory(&interner, db.file(app_file).expect("app file"));
        let app_scope = extract_ts_scope(&interner, db.file(app_file).expect("app file"));
        let m_inventory = extract_ts_inventory(&interner, db.file(m_file).expect("m file"));
        let m_scope = extract_ts_scope(&interner, db.file(m_file).expect("m file"));

        let m_node = ModuleNodeId::from_raw(1);
        let imports = vec![import(0, app_file, "./m")];
        let resolved = vec![resolved(
            0,
            0,
            app_file,
            Some(m_node),
            ResolutionStatus::Resolved,
        )];
        let nodes = vec![
            file_node(ModuleNodeId::from_raw(0), "src/app.ts", app_file),
            file_node(m_node, "src/m.ts", m_file),
        ];
        let module_files = vec![module_file(m_node, &m_inventory, &m_scope)];
        let module_input = TsDirectBindingModuleInput {
            imports: &imports,
            resolved_imports: &resolved,
            module_nodes: &nodes,
            module_files: &module_files,
        };

        let output = resolve_direct_bindings_with_modules(
            &interner,
            &app_inventory,
            &app_scope,
            &module_input,
        );

        assert_call(
            &output,
            &app_inventory,
            "f",
            TsDirectBindingStatus::Unresolved,
            TsDirectBindingKind::LocalFunction,
            Some(TsDirectBindingReason::TokenFlowRequired),
        );
    }

    #[test]
    fn module_imports_resolve_only_explicit_exports_and_local_export_aliases() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let mut db = Box::new(LocalFactDb::new());
        let app_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            r#"
import { f, g } from "./m";
function run() {
  f();
  g();
}
"#
            .to_string(),
        );
        let m_file = db.add_file(
            PathBuf::from("src/m.ts"),
            "src/m.ts".to_string(),
            r#"
function f() {}
function real() {}
export { real as g };
"#
            .to_string(),
        );
        let db = Box::leak(db);

        let app_inventory = extract_ts_inventory(&interner, db.file(app_file).expect("app file"));
        let app_scope = extract_ts_scope(&interner, db.file(app_file).expect("app file"));
        let m_inventory = extract_ts_inventory(&interner, db.file(m_file).expect("m file"));
        let m_scope = extract_ts_scope(&interner, db.file(m_file).expect("m file"));

        let m_node = ModuleNodeId::from_raw(1);
        let imports = vec![import(0, app_file, "./m")];
        let resolved = vec![resolved(
            0,
            0,
            app_file,
            Some(m_node),
            ResolutionStatus::Resolved,
        )];
        let nodes = vec![
            file_node(ModuleNodeId::from_raw(0), "src/app.ts", app_file),
            file_node(m_node, "src/m.ts", m_file),
        ];
        let module_files = vec![module_file(m_node, &m_inventory, &m_scope)];
        let module_input = TsDirectBindingModuleInput {
            imports: &imports,
            resolved_imports: &resolved,
            module_nodes: &nodes,
            module_files: &module_files,
        };

        let output = resolve_direct_bindings_with_modules(
            &interner,
            &app_inventory,
            &app_scope,
            &module_input,
        );

        assert_call(
            &output,
            &app_inventory,
            "f",
            TsDirectBindingStatus::Unresolved,
            TsDirectBindingKind::ImportedNamed,
            Some(TsDirectBindingReason::ImportedBindingUnresolved),
        );
        assert_call(
            &output,
            &app_inventory,
            "g",
            TsDirectBindingStatus::Resolved,
            TsDirectBindingKind::ImportedNamed,
            None,
        );
    }

    #[test]
    fn local_export_alias_does_not_resolve_non_visible_nested_function() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let mut db = Box::new(LocalFactDb::new());
        let app_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            r#"
import { g } from "./m";
function run() {
  g();
}
"#
            .to_string(),
        );
        let m_file = db.add_file(
            PathBuf::from("src/m.ts"),
            "src/m.ts".to_string(),
            r#"
function wrapper() {
  function hidden() {}
}
export { hidden as g };
"#
            .to_string(),
        );
        let db = Box::leak(db);

        let app_inventory = extract_ts_inventory(&interner, db.file(app_file).expect("app file"));
        let app_scope = extract_ts_scope(&interner, db.file(app_file).expect("app file"));
        let m_inventory = extract_ts_inventory(&interner, db.file(m_file).expect("m file"));
        let m_scope = extract_ts_scope(&interner, db.file(m_file).expect("m file"));

        let m_node = ModuleNodeId::from_raw(1);
        let imports = vec![import(0, app_file, "./m")];
        let resolved = vec![resolved(
            0,
            0,
            app_file,
            Some(m_node),
            ResolutionStatus::Resolved,
        )];
        let nodes = vec![
            file_node(ModuleNodeId::from_raw(0), "src/app.ts", app_file),
            file_node(m_node, "src/m.ts", m_file),
        ];
        let module_files = vec![module_file(m_node, &m_inventory, &m_scope)];
        let module_input = TsDirectBindingModuleInput {
            imports: &imports,
            resolved_imports: &resolved,
            module_nodes: &nodes,
            module_files: &module_files,
        };

        let output = resolve_direct_bindings_with_modules(
            &interner,
            &app_inventory,
            &app_scope,
            &module_input,
        );

        assert_call(
            &output,
            &app_inventory,
            "g",
            TsDirectBindingStatus::Unresolved,
            TsDirectBindingKind::ImportedNamed,
            Some(TsDirectBindingReason::ImportedBindingUnresolved),
        );
    }

    fn assert_call(
        output: &TsDirectBindingOutput,
        inventory: &TsInventoryOutput,
        display_name: &str,
        status: TsDirectBindingStatus,
        kind: TsDirectBindingKind,
        reason: Option<TsDirectBindingReason>,
    ) {
        let callsite = inventory
            .callsites
            .iter()
            .find(|callsite| callsite.display_name.as_deref() == Some(display_name))
            .unwrap_or_else(|| panic!("missing callsite {display_name}"));
        let binding = output
            .bindings
            .iter()
            .find(|binding| binding.callsite == callsite.id)
            .unwrap_or_else(|| panic!("missing direct binding for {display_name}"));

        assert_eq!(binding.status, status);
        assert_eq!(binding.kind, kind);
        assert_eq!(binding.reason, reason);
        if status == TsDirectBindingStatus::Resolved {
            assert!(binding.target_function.is_some());
            assert!(binding.resolved_import.is_some());
            assert!(binding.module_node.is_some());
        }
    }

    fn import(id: u64, file: FileId, path: &str) -> ImportFact {
        ImportFact::new(
            ImportId::from_raw(id),
            file,
            None,
            path.to_string(),
            Span::point(file, 0, 0),
            Language::TypeScript,
        )
    }

    fn resolved(
        id: u64,
        import: u64,
        from_file: FileId,
        target_node: Option<ModuleNodeId>,
        status: ResolutionStatus,
    ) -> ResolvedImportFact {
        ResolvedImportFact::new(
            crate::internal_core::ResolvedImportId::from_raw(id),
            ImportId::from_raw(import),
            from_file,
            target_node,
            status,
            if status == ResolutionStatus::External {
                ResolutionPrecision::ExternalPackage
            } else {
                ResolutionPrecision::ExactFile
            },
            None,
        )
    }

    fn file_node(id: ModuleNodeId, label: &str, file: FileId) -> ModuleNode {
        ModuleNode::new(
            id,
            ModuleNodeKind::File,
            label.to_string(),
            Some(file),
            None,
            Some(Language::TypeScript),
        )
    }

    fn module_file<'a>(
        module_node: ModuleNodeId,
        inventory: &'a TsInventoryOutput,
        scope: &'a TsScopeOutput,
    ) -> TsDirectBindingModuleFile<'a> {
        TsDirectBindingModuleFile {
            module_node,
            inventory,
            scope,
        }
    }
}
