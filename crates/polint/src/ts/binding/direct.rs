#![allow(
    dead_code,
    reason = "Phase 45 wires private TS direct binding analysis into semantic graph across sequential plans"
)]

use std::collections::BTreeMap;

use crate::analysis::ids::TsDirectBindingId;
use crate::core::{
    FileId, ImportFact, ModuleNode, ModuleNodeId, ModuleNodeKind, ResolutionStatus,
    ResolvedImportFact,
};
use crate::ts::binding::facts::{
    TsDirectBindingFact, TsDirectBindingKind, TsDirectBindingReason, TsDirectBindingStatus,
};
use crate::ts::binding::store::TsDirectBindingOutput;
use crate::ts::inventory::facts::{
    TsCallsiteInventoryKind, TsInventoryCallsiteFact, TsInventoryFunctionFact, TsInventoryStatus,
};
use crate::ts::inventory::store::TsInventoryOutput;
use crate::ts::scope::facts::{TsBindingFact, TsBindingKind, TsBindingStatus};
use crate::ts::scope::store::TsScopeOutput;

#[derive(Clone, Copy)]
pub(crate) struct TsDirectBindingModuleFile<'a> {
    pub(crate) module_node: ModuleNodeId,
    pub(crate) inventory: &'a TsInventoryOutput,
    pub(crate) scope: &'a TsScopeOutput,
}

#[derive(Clone, Copy)]
pub(crate) struct TsDirectBindingModuleInput<'a> {
    pub(crate) imports: &'a [ImportFact],
    pub(crate) resolved_imports: &'a [ResolvedImportFact],
    pub(crate) module_nodes: &'a [ModuleNode],
    pub(crate) module_files: &'a [TsDirectBindingModuleFile<'a>],
}

impl<'a> TsDirectBindingModuleInput<'a> {
    pub(crate) fn empty() -> Self {
        Self {
            imports: &[],
            resolved_imports: &[],
            module_nodes: &[],
            module_files: &[],
        }
    }
}

pub(crate) fn resolve_direct_bindings(
    inventory: &TsInventoryOutput,
    scope: &TsScopeOutput,
) -> TsDirectBindingOutput {
    resolve_direct_bindings_with_modules(inventory, scope, &TsDirectBindingModuleInput::empty())
}

pub(crate) fn resolve_direct_bindings_with_modules(
    inventory: &TsInventoryOutput,
    scope: &TsScopeOutput,
    module_input: &TsDirectBindingModuleInput<'_>,
) -> TsDirectBindingOutput {
    let index = DirectBindingIndex::new(inventory, scope, module_input);
    let mut rows = Vec::new();

    for callsite in &inventory.callsites {
        if let Some(reason) = unresolved_reason_for_callsite(callsite, &index) {
            rows.push(unresolved_row(callsite, reason));
            continue;
        }

        let Some(display_name) = callsite.display_name.as_deref() else {
            rows.push(unresolved_row(
                callsite,
                TsDirectBindingReason::LocalBindingMissing,
            ));
            continue;
        };
        let lookup = CalleeLookup::new(display_name);
        if let Some(target) = index.function_by_name.get(lookup.target_name()).copied() {
            rows.push(resolved_row(callsite, target, None, lookup.local_kind));
            continue;
        }
        if let Some(alias) = index.alias_by_name.get(lookup.target_name()).copied()
            && let Some(target_name) = alias.imported_name.as_deref()
            && let Some(target) = index.function_by_name.get(target_name).copied()
        {
            rows.push(resolved_row(
                callsite,
                target,
                Some(alias),
                TsDirectBindingKind::LocalAlias,
            ));
            continue;
        }
        if let Some(binding) = index.import_binding_for_lookup(lookup)
            && let Some(row) = index.module_row(callsite, binding, lookup)
        {
            rows.push(row);
            continue;
        }

        rows.push(unresolved_row(
            callsite,
            TsDirectBindingReason::LocalBindingMissing,
        ));
    }

    TsDirectBindingOutput { bindings: rows }.normalized()
}

struct DirectBindingIndex<'a> {
    function_by_name: BTreeMap<&'a str, &'a TsInventoryFunctionFact>,
    alias_by_name: BTreeMap<&'a str, &'a TsBindingFact>,
    import_by_name: BTreeMap<&'a str, &'a TsBindingFact>,
    unsupported_by_name: BTreeMap<&'a str, &'a TsBindingFact>,
    module_index: ModuleBindingIndex<'a>,
}

impl<'a> DirectBindingIndex<'a> {
    fn new(
        inventory: &'a TsInventoryOutput,
        scope: &'a TsScopeOutput,
        module_input: &'a TsDirectBindingModuleInput<'a>,
    ) -> Self {
        Self {
            function_by_name: inventory
                .functions
                .iter()
                .filter_map(|function| {
                    function
                        .display_name
                        .as_deref()
                        .map(|name| (name, function))
                })
                .collect(),
            alias_by_name: scope
                .bindings
                .iter()
                .filter(|binding| binding.binding_kind == TsBindingKind::Alias)
                .map(|binding| (binding.name.as_str(), binding))
                .collect(),
            import_by_name: scope
                .bindings
                .iter()
                .filter(|binding| binding.module_source.is_some())
                .filter(|binding| {
                    matches!(
                        binding.binding_kind,
                        TsBindingKind::Import
                            | TsBindingKind::NamedImport
                            | TsBindingKind::DefaultImport
                            | TsBindingKind::NamespaceImport
                            | TsBindingKind::ReExport
                    )
                })
                .map(|binding| (binding.name.as_str(), binding))
                .collect(),
            unsupported_by_name: scope
                .bindings
                .iter()
                .filter(|binding| binding.binding_kind == TsBindingKind::UnsupportedDynamic)
                .map(|binding| (binding.name.as_str(), binding))
                .collect(),
            module_index: ModuleBindingIndex::new(module_input),
        }
    }

    fn import_binding_for_lookup(&self, lookup: CalleeLookup<'_>) -> Option<&'a TsBindingFact> {
        self.import_by_name.get(lookup.local_name).copied()
    }

    fn module_row(
        &self,
        callsite: &TsInventoryCallsiteFact,
        binding: &'a TsBindingFact,
        lookup: CalleeLookup<'_>,
    ) -> Option<TsDirectBindingFact> {
        let (export_name, kind) = imported_export_target(binding, lookup)?;
        let Some(resolved_import) = self.module_index.resolved_import(binding) else {
            return Some(unresolved_import_row(
                callsite,
                binding,
                None,
                None,
                kind,
                TsDirectBindingStatus::Unresolved,
                TsDirectBindingReason::ImportedBindingUnresolved,
            ));
        };
        let target_node = resolved_import.target_node;
        if resolved_import.status != ResolutionStatus::Resolved {
            let reason = if resolved_import.status == ResolutionStatus::External {
                TsDirectBindingReason::ExternalPackageUnresolved
            } else {
                TsDirectBindingReason::ImportedBindingUnresolved
            };
            let status = if resolved_import.status == ResolutionStatus::External {
                TsDirectBindingStatus::External
            } else {
                TsDirectBindingStatus::Unresolved
            };
            return Some(unresolved_import_row(
                callsite,
                binding,
                Some(resolved_import),
                target_node,
                kind,
                status,
                reason,
            ));
        }

        let Some(target_node) = target_node else {
            return Some(unresolved_import_row(
                callsite,
                binding,
                Some(resolved_import),
                None,
                kind,
                TsDirectBindingStatus::Unresolved,
                TsDirectBindingReason::ImportedBindingUnresolved,
            ));
        };
        if self.module_index.is_external_node(target_node) {
            return Some(unresolved_import_row(
                callsite,
                binding,
                Some(resolved_import),
                Some(target_node),
                kind,
                TsDirectBindingStatus::External,
                TsDirectBindingReason::ExternalPackageUnresolved,
            ));
        }

        let Some(target) = self
            .module_index
            .exported_function(target_node, export_name.as_str())
        else {
            return Some(unresolved_import_row(
                callsite,
                binding,
                Some(resolved_import),
                Some(target_node),
                kind,
                TsDirectBindingStatus::Unresolved,
                TsDirectBindingReason::ImportedBindingUnresolved,
            ));
        };

        Some(resolved_import_row(
            callsite,
            target.function,
            binding,
            resolved_import,
            target.module_node,
            if target.via_reexport {
                TsDirectBindingKind::ReExportedAlias
            } else {
                kind
            },
        ))
    }
}

struct ModuleBindingIndex<'a> {
    resolved_by_file_source: BTreeMap<(FileId, String), &'a ResolvedImportFact>,
    module_kind_by_id: BTreeMap<ModuleNodeId, ModuleNodeKind>,
    module_file_by_node: BTreeMap<ModuleNodeId, &'a TsDirectBindingModuleFile<'a>>,
}

impl<'a> ModuleBindingIndex<'a> {
    fn new(input: &'a TsDirectBindingModuleInput<'a>) -> Self {
        let import_path_by_id = input
            .imports
            .iter()
            .map(|import| (import.id, (import.file, import.path.as_str())))
            .collect::<BTreeMap<_, _>>();
        let resolved_by_file_source = input
            .resolved_imports
            .iter()
            .filter_map(|resolved| {
                let (file, path) = import_path_by_id.get(&resolved.import)?;
                Some(((*file, (*path).to_string()), resolved))
            })
            .collect();

        Self {
            resolved_by_file_source,
            module_kind_by_id: input
                .module_nodes
                .iter()
                .map(|node| (node.id, node.kind))
                .collect(),
            module_file_by_node: input
                .module_files
                .iter()
                .map(|module| (module.module_node, module))
                .collect(),
        }
    }

    fn resolved_import(&self, binding: &TsBindingFact) -> Option<&'a ResolvedImportFact> {
        let module_source = binding.module_source.as_ref()?;
        self.resolved_by_file_source
            .get(&(binding.file, module_source.clone()))
            .copied()
    }

    fn is_external_node(&self, node: ModuleNodeId) -> bool {
        self.module_kind_by_id
            .get(&node)
            .is_some_and(|kind| *kind == ModuleNodeKind::External)
    }

    fn exported_function(
        &self,
        module_node: ModuleNodeId,
        export_name: &str,
    ) -> Option<ModuleFunctionTarget<'a>> {
        self.exported_function_inner(module_node, export_name, 0)
    }

    fn exported_function_inner(
        &self,
        module_node: ModuleNodeId,
        export_name: &str,
        depth: usize,
    ) -> Option<ModuleFunctionTarget<'a>> {
        if depth > 8 {
            return None;
        }

        let module = self.module_file_by_node.get(&module_node)?;
        if let Some(function) = function_by_export_name(module.inventory, export_name) {
            return Some(ModuleFunctionTarget {
                module_node,
                function,
                via_reexport: false,
            });
        }

        for binding in &module.scope.bindings {
            if binding.exported_name.as_deref() == Some(export_name)
                && let Some(function_key) = binding.inventory_function_key.as_deref()
                && let Some(function) = module
                    .inventory
                    .functions
                    .iter()
                    .find(|function| function.stable_key == function_key)
            {
                return Some(ModuleFunctionTarget {
                    module_node,
                    function,
                    via_reexport: false,
                });
            }
            if binding.exported_name.as_deref() == Some(export_name)
                && let Some(imported_name) = binding.imported_name.as_deref()
                && let Some(function) = function_by_export_name(module.inventory, imported_name)
            {
                return Some(ModuleFunctionTarget {
                    module_node,
                    function,
                    via_reexport: false,
                });
            }
            if matches!(binding.binding_kind, TsBindingKind::ReExport)
                && (binding.exported_name.as_deref() == Some(export_name)
                    || binding.exported_name.as_deref() == Some("*"))
                && let Some(source) = binding.module_source.as_ref()
                && let Some(reexport) = self
                    .resolved_by_file_source
                    .get(&(binding.file, source.clone()))
                && reexport.status == ResolutionStatus::Resolved
                && let Some(target_node) = reexport.target_node
            {
                let target_name = binding
                    .imported_name
                    .as_deref()
                    .filter(|name| *name != "*")
                    .unwrap_or(export_name);
                if let Some(mut function) =
                    self.exported_function_inner(target_node, target_name, depth + 1)
                {
                    function.via_reexport = true;
                    return Some(function);
                }
            }
        }

        None
    }
}

struct ModuleFunctionTarget<'a> {
    module_node: ModuleNodeId,
    function: &'a TsInventoryFunctionFact,
    via_reexport: bool,
}

#[derive(Clone, Copy)]
struct CalleeLookup<'a> {
    local_name: &'a str,
    member_name: Option<&'a str>,
    local_kind: TsDirectBindingKind,
}

impl<'a> CalleeLookup<'a> {
    fn new(display_name: &'a str) -> Self {
        display_name
            .rsplit_once('.')
            .map(|(object, property)| Self {
                local_name: object,
                member_name: Some(property),
                local_kind: TsDirectBindingKind::NamespaceMember,
            })
            .unwrap_or(Self {
                local_name: display_name,
                member_name: None,
                local_kind: TsDirectBindingKind::LocalFunction,
            })
    }

    fn target_name(self) -> &'a str {
        self.member_name.unwrap_or(self.local_name)
    }
}

fn unresolved_reason_for_callsite(
    callsite: &TsInventoryCallsiteFact,
    index: &DirectBindingIndex<'_>,
) -> Option<TsDirectBindingReason> {
    if matches!(callsite.kind, TsCallsiteInventoryKind::DynamicImport)
        && matches!(callsite.status, TsInventoryStatus::Unresolved { .. })
    {
        return Some(TsDirectBindingReason::NonStringDynamicImport);
    }
    if callsite.display_name.is_none() {
        return Some(TsDirectBindingReason::ComputedProperty);
    }
    let display = callsite.display_name.as_deref()?;
    let lookup = CalleeLookup::new(display);
    index
        .unsupported_by_name
        .get(lookup.target_name())
        .and_then(|binding| match &binding.status {
            TsBindingStatus::UnsupportedDynamic { reason } if reason.contains("function-token") => {
                Some(TsDirectBindingReason::TokenFlowRequired)
            }
            TsBindingStatus::UnsupportedDynamic { reason } if reason.contains("property-flow") => {
                Some(TsDirectBindingReason::PropertyFlowRequired)
            }
            TsBindingStatus::UnsupportedDynamic { reason } if reason.contains("prototype") => {
                Some(TsDirectBindingReason::PrototypeModelRequired)
            }
            TsBindingStatus::UnsupportedDynamic { reason } if reason.contains("receiver") => {
                Some(TsDirectBindingReason::ThisModelRequired)
            }
            _ => None,
        })
}

fn imported_export_target(
    binding: &TsBindingFact,
    lookup: CalleeLookup<'_>,
) -> Option<(String, TsDirectBindingKind)> {
    match binding.binding_kind {
        TsBindingKind::NamedImport => binding
            .imported_name
            .as_deref()
            .map(|name| (name.to_string(), TsDirectBindingKind::ImportedNamed)),
        TsBindingKind::DefaultImport => {
            Some(("default".to_string(), TsDirectBindingKind::ImportedDefault))
        }
        TsBindingKind::NamespaceImport => lookup.member_name.map(|member| {
            (
                member.to_string(),
                TsDirectBindingKind::ImportedNamespaceMember,
            )
        }),
        TsBindingKind::ReExport => binding
            .imported_name
            .as_deref()
            .map(|name| (name.to_string(), TsDirectBindingKind::ReExportedAlias)),
        TsBindingKind::Import
            if binding.imported_name.as_deref() == Some("module.exports")
                && lookup.member_name.is_some() =>
        {
            lookup.member_name.map(|member| {
                (
                    member.to_string(),
                    TsDirectBindingKind::CommonJsRequireMember,
                )
            })
        }
        TsBindingKind::Import => binding
            .imported_name
            .as_deref()
            .map(|name| (name.to_string(), TsDirectBindingKind::CommonJsRequireMember)),
        _ => None,
    }
}

fn function_by_export_name<'a>(
    inventory: &'a TsInventoryOutput,
    export_name: &str,
) -> Option<&'a TsInventoryFunctionFact> {
    inventory
        .functions
        .iter()
        .find(|function| function.display_name.as_deref() == Some(export_name))
}

fn resolved_row(
    callsite: &TsInventoryCallsiteFact,
    target: &TsInventoryFunctionFact,
    binding: Option<&TsBindingFact>,
    kind: TsDirectBindingKind,
) -> TsDirectBindingFact {
    TsDirectBindingFact {
        id: TsDirectBindingId(0),
        callsite: callsite.id,
        callsite_stable_key: callsite.stable_key.clone(),
        target_function: Some(target.id),
        target_function_stable_key: Some(target.stable_key.clone()),
        scope_binding: binding.map(|binding| binding.id),
        scope_binding_stable_key: binding.map(|binding| binding.stable_key.clone()),
        resolved_import: None,
        module_node: None,
        kind,
        status: TsDirectBindingStatus::Resolved,
        reason: None,
        stable_key: direct_binding_key(
            &callsite.stable_key,
            target.stable_key.as_str(),
            kind,
            TsDirectBindingStatus::Resolved,
            None,
        ),
    }
}

fn resolved_import_row(
    callsite: &TsInventoryCallsiteFact,
    target: &TsInventoryFunctionFact,
    binding: &TsBindingFact,
    resolved_import: &ResolvedImportFact,
    module_node: ModuleNodeId,
    kind: TsDirectBindingKind,
) -> TsDirectBindingFact {
    TsDirectBindingFact {
        id: TsDirectBindingId(0),
        callsite: callsite.id,
        callsite_stable_key: callsite.stable_key.clone(),
        target_function: Some(target.id),
        target_function_stable_key: Some(target.stable_key.clone()),
        scope_binding: Some(binding.id),
        scope_binding_stable_key: Some(binding.stable_key.clone()),
        resolved_import: Some(resolved_import.id),
        module_node: Some(module_node),
        kind,
        status: TsDirectBindingStatus::Resolved,
        reason: None,
        stable_key: direct_binding_key(
            &callsite.stable_key,
            target.stable_key.as_str(),
            kind,
            TsDirectBindingStatus::Resolved,
            None,
        ),
    }
}

fn unresolved_row(
    callsite: &TsInventoryCallsiteFact,
    reason: TsDirectBindingReason,
) -> TsDirectBindingFact {
    TsDirectBindingFact {
        id: TsDirectBindingId(0),
        callsite: callsite.id,
        callsite_stable_key: callsite.stable_key.clone(),
        target_function: None,
        target_function_stable_key: None,
        scope_binding: None,
        scope_binding_stable_key: None,
        resolved_import: None,
        module_node: None,
        kind: TsDirectBindingKind::LocalFunction,
        status: TsDirectBindingStatus::Unresolved,
        reason: Some(reason),
        stable_key: direct_binding_key(
            &callsite.stable_key,
            "<unresolved>",
            TsDirectBindingKind::LocalFunction,
            TsDirectBindingStatus::Unresolved,
            Some(reason),
        ),
    }
}

fn unresolved_import_row(
    callsite: &TsInventoryCallsiteFact,
    binding: &TsBindingFact,
    resolved_import: Option<&ResolvedImportFact>,
    module_node: Option<ModuleNodeId>,
    kind: TsDirectBindingKind,
    status: TsDirectBindingStatus,
    reason: TsDirectBindingReason,
) -> TsDirectBindingFact {
    TsDirectBindingFact {
        id: TsDirectBindingId(0),
        callsite: callsite.id,
        callsite_stable_key: callsite.stable_key.clone(),
        target_function: None,
        target_function_stable_key: None,
        scope_binding: Some(binding.id),
        scope_binding_stable_key: Some(binding.stable_key.clone()),
        resolved_import: resolved_import.map(|resolved| resolved.id),
        module_node,
        kind,
        status,
        reason: Some(reason),
        stable_key: direct_binding_key(
            &callsite.stable_key,
            binding.stable_key.as_str(),
            kind,
            status,
            Some(reason),
        ),
    }
}

fn direct_binding_key(
    callsite_key: &str,
    target_key: &str,
    kind: TsDirectBindingKind,
    status: TsDirectBindingStatus,
    reason: Option<TsDirectBindingReason>,
) -> String {
    format!(
        "ts_direct_binding|callsite={callsite_key}|target={target_key}|kind={kind:?}|status={}|reason={}",
        status.as_str(),
        reason.map(TsDirectBindingReason::as_str).unwrap_or("none")
    )
}
