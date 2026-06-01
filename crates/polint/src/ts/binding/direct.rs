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
use crate::ts::scope::facts::{
    TsBindingFact, TsBindingKind, TsBindingStatus, TsScopeFact, TsScopeKind,
};
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
        let callsite_scope = index.scope_key_for_callsite(callsite);
        if let Some(reason) = intrinsic_unresolved_reason_for_callsite(callsite) {
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
        let Some(visible) = index.visible_binding_for_lookup(callsite_scope, lookup) else {
            rows.push(unresolved_row(
                callsite,
                TsDirectBindingReason::LocalBindingMissing,
            ));
            continue;
        };

        rows.push(index.row_for_visible_binding(callsite, visible.binding, lookup));
    }

    TsDirectBindingOutput { bindings: rows }.normalized()
}

struct DirectBindingIndex<'a> {
    function_by_name: BTreeMap<&'a str, Vec<&'a TsInventoryFunctionFact>>,
    alias_by_scope_name: BTreeMap<(&'a str, &'a str), &'a TsBindingFact>,
    import_by_scope_name: BTreeMap<(&'a str, &'a str), &'a TsBindingFact>,
    local_by_scope_name: BTreeMap<(&'a str, &'a str), &'a TsBindingFact>,
    unsupported_by_scope_name: BTreeMap<(&'a str, &'a str), &'a TsBindingFact>,
    scope_parent_by_key: BTreeMap<&'a str, Option<&'a str>>,
    scopes_by_file: BTreeMap<FileId, Vec<&'a TsScopeFact>>,
    module_scope_by_file: BTreeMap<FileId, &'a str>,
    module_index: ModuleBindingIndex<'a>,
}

impl<'a> DirectBindingIndex<'a> {
    fn new(
        inventory: &'a TsInventoryOutput,
        scope: &'a TsScopeOutput,
        module_input: &'a TsDirectBindingModuleInput<'a>,
    ) -> Self {
        let mut function_by_name: BTreeMap<&str, Vec<&TsInventoryFunctionFact>> = BTreeMap::new();
        for function in &inventory.functions {
            if let Some(name) = function.display_name.as_deref() {
                function_by_name.entry(name).or_default().push(function);
            }
        }

        let mut scopes_by_file: BTreeMap<FileId, Vec<&TsScopeFact>> = BTreeMap::new();
        let mut module_scope_by_file = BTreeMap::new();
        for scope_row in &scope.scopes {
            scopes_by_file
                .entry(scope_row.file)
                .or_default()
                .push(scope_row);
            if scope_row.kind == TsScopeKind::Module {
                module_scope_by_file.insert(scope_row.file, scope_row.stable_key.as_str());
            }
        }

        Self {
            function_by_name,
            alias_by_scope_name: scope
                .bindings
                .iter()
                .filter(|binding| binding.module_source.is_none())
                .filter(|binding| binding.imported_name.is_some())
                .filter(|binding| {
                    matches!(
                        binding.binding_kind,
                        TsBindingKind::Alias | TsBindingKind::Destructuring
                    )
                })
                .map(|binding| ((binding.scope_key.as_str(), binding.name.as_str()), binding))
                .collect(),
            import_by_scope_name: scope
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
                .map(|binding| ((binding.scope_key.as_str(), binding.name.as_str()), binding))
                .collect(),
            local_by_scope_name: scope
                .bindings
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.binding_kind,
                        TsBindingKind::FunctionDeclaration
                            | TsBindingKind::FunctionExpression
                            | TsBindingKind::Var
                            | TsBindingKind::Let
                            | TsBindingKind::Const
                            | TsBindingKind::Parameter
                    )
                })
                .map(|binding| ((binding.scope_key.as_str(), binding.name.as_str()), binding))
                .collect(),
            unsupported_by_scope_name: scope
                .bindings
                .iter()
                .filter(|binding| binding.binding_kind == TsBindingKind::UnsupportedDynamic)
                .map(|binding| ((binding.scope_key.as_str(), binding.name.as_str()), binding))
                .collect(),
            scope_parent_by_key: scope
                .scopes
                .iter()
                .map(|scope| (scope.stable_key.as_str(), scope.parent_scope_key.as_deref()))
                .collect(),
            scopes_by_file,
            module_scope_by_file,
            module_index: ModuleBindingIndex::new(module_input),
        }
    }

    fn scope_key_for_callsite(&self, callsite: &TsInventoryCallsiteFact) -> Option<&'a str> {
        self.scopes_by_file
            .get(&callsite.file)
            .and_then(|scopes| {
                scopes
                    .iter()
                    .filter(|scope| scope.kind != TsScopeKind::Module)
                    .filter(|scope| span_contains(&scope.span, &callsite.span))
                    .min_by_key(|scope| scope.span.end_byte - scope.span.start_byte)
                    .map(|scope| scope.stable_key.as_str())
            })
            .or_else(|| self.module_scope_by_file.get(&callsite.file).copied())
    }

    fn visible_binding_for_lookup(
        &self,
        scope_key: Option<&str>,
        lookup: CalleeLookup<'_>,
    ) -> Option<VisibleBinding<'a>> {
        let local = self.visible_any_binding(scope_key, lookup.local_name, false);
        if lookup.member_name.is_none() {
            return local;
        }

        let exact_member = self.visible_any_binding(scope_key, lookup.alias_name(), true);
        choose_visible_binding(local, exact_member)
    }

    fn row_for_visible_binding(
        &self,
        callsite: &TsInventoryCallsiteFact,
        binding: &'a TsBindingFact,
        lookup: CalleeLookup<'_>,
    ) -> TsDirectBindingFact {
        if let Some(reason) = unsupported_binding_reason(binding) {
            return unresolved_row(callsite, reason);
        }

        if is_import_binding(binding) {
            return self
                .module_row(callsite, binding, lookup)
                .unwrap_or_else(|| {
                    unresolved_import_row(
                        callsite,
                        binding,
                        None,
                        None,
                        lookup.local_kind,
                        TsDirectBindingStatus::Unresolved,
                        TsDirectBindingReason::ImportedBindingUnresolved,
                    )
                });
        }

        if matches!(
            binding.binding_kind,
            TsBindingKind::Alias | TsBindingKind::Destructuring
        ) {
            if let Some(target) = self.local_alias_target(binding) {
                return resolved_row(
                    callsite,
                    target,
                    Some(binding),
                    TsDirectBindingKind::LocalAlias,
                );
            }
            return unresolved_row(callsite, TsDirectBindingReason::LocalBindingMissing);
        }

        if matches!(binding.binding_kind, TsBindingKind::Parameter) && lookup.member_name.is_none()
        {
            return unresolved_row(callsite, TsDirectBindingReason::TokenFlowRequired);
        }

        if lookup.member_name.is_none()
            && matches!(
                binding.binding_kind,
                TsBindingKind::FunctionDeclaration | TsBindingKind::FunctionExpression
            )
            && let Some(target) = self.function_by_name(lookup.local_name, Some(binding))
        {
            return resolved_row(callsite, target, None, lookup.local_kind);
        }

        unresolved_row(callsite, TsDirectBindingReason::LocalBindingMissing)
    }

    fn local_alias_target(&self, alias: &'a TsBindingFact) -> Option<&'a TsInventoryFunctionFact> {
        let target_name = alias.imported_name.as_deref()?;
        let target_binding = self.visible_binding_in_map(
            Some(alias.scope_key.as_str()),
            target_name,
            &self.local_by_scope_name,
        )?;
        if !matches!(
            target_binding.binding_kind,
            TsBindingKind::FunctionDeclaration | TsBindingKind::FunctionExpression
        ) {
            return None;
        }
        self.function_by_name(target_name, Some(target_binding))
    }

    fn function_by_name(
        &self,
        name: &str,
        binding: Option<&TsBindingFact>,
    ) -> Option<&'a TsInventoryFunctionFact> {
        let functions = self.function_by_name.get(name)?;
        if let Some(binding) = binding
            && let Some(function) = functions.iter().copied().find(|function| {
                function.file == binding.file && spans_overlap(&function.span, &binding.span)
            })
        {
            return Some(function);
        }
        if let Some(binding) = binding {
            let mut same_file = functions
                .iter()
                .copied()
                .filter(|function| function.file == binding.file);
            let first = same_file.next()?;
            if same_file.next().is_none() {
                return Some(first);
            }
            return None;
        }
        functions.first().copied()
    }

    fn visible_any_binding(
        &self,
        scope_key: Option<&str>,
        name: &str,
        exact_member: bool,
    ) -> Option<VisibleBinding<'a>> {
        let mut current = scope_key;
        let mut distance = 0;
        while let Some(scope) = current {
            if let Some(binding) = self.binding_in_scope(scope, name) {
                return Some(VisibleBinding {
                    binding,
                    distance,
                    exact_member,
                });
            }
            current = self
                .scope_parent_by_key
                .get(scope)
                .and_then(|parent| *parent);
            distance += 1;
        }
        None
    }

    fn binding_in_scope(&self, scope: &str, name: &str) -> Option<&'a TsBindingFact> {
        self.unsupported_by_scope_name
            .get(&(scope, name))
            .copied()
            .or_else(|| self.alias_by_scope_name.get(&(scope, name)).copied())
            .or_else(|| self.import_by_scope_name.get(&(scope, name)).copied())
            .or_else(|| self.local_by_scope_name.get(&(scope, name)).copied())
    }

    fn visible_binding_in_map(
        &self,
        scope_key: Option<&str>,
        name: &str,
        bindings: &BTreeMap<(&'a str, &'a str), &'a TsBindingFact>,
    ) -> Option<&'a TsBindingFact> {
        let mut current = scope_key;
        while let Some(scope) = current {
            if let Some(binding) = bindings.get(&(scope, name)) {
                return Some(*binding);
            }
            current = self
                .scope_parent_by_key
                .get(scope)
                .and_then(|parent| *parent);
        }
        None
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

#[derive(Clone, Copy)]
struct VisibleBinding<'a> {
    binding: &'a TsBindingFact,
    distance: usize,
    exact_member: bool,
}

fn choose_visible_binding<'a>(
    left: Option<VisibleBinding<'a>>,
    right: Option<VisibleBinding<'a>>,
) -> Option<VisibleBinding<'a>> {
    match (left, right) {
        (Some(left), Some(right)) => {
            if right.distance < left.distance
                || (right.distance == left.distance && right.exact_member && !left.exact_member)
            {
                Some(right)
            } else {
                Some(left)
            }
        }
        (Some(binding), None) | (None, Some(binding)) => Some(binding),
        (None, None) => None,
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
                && binding.module_source.is_none()
                && let Some(imported_name) = binding.imported_name.as_deref()
                && let Some(function) = function_for_local_export(
                    module.inventory,
                    module.scope,
                    binding,
                    imported_name,
                )
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
    display_name: &'a str,
    local_name: &'a str,
    member_name: Option<&'a str>,
    local_kind: TsDirectBindingKind,
}

impl<'a> CalleeLookup<'a> {
    fn new(display_name: &'a str) -> Self {
        display_name
            .rsplit_once('.')
            .map(|(object, property)| Self {
                display_name,
                local_name: object,
                member_name: Some(property),
                local_kind: TsDirectBindingKind::NamespaceMember,
            })
            .unwrap_or(Self {
                display_name,
                local_name: display_name,
                member_name: None,
                local_kind: TsDirectBindingKind::LocalFunction,
            })
    }

    fn target_name(self) -> &'a str {
        self.member_name.unwrap_or(self.local_name)
    }

    fn alias_name(self) -> &'a str {
        self.member_name
            .map(|_| self.display_name)
            .unwrap_or(self.local_name)
    }
}

fn intrinsic_unresolved_reason_for_callsite(
    callsite: &TsInventoryCallsiteFact,
) -> Option<TsDirectBindingReason> {
    if matches!(callsite.kind, TsCallsiteInventoryKind::DynamicImport)
        && matches!(callsite.status, TsInventoryStatus::Unresolved { .. })
    {
        return Some(TsDirectBindingReason::NonStringDynamicImport);
    }
    if callsite.display_name.is_none() {
        return Some(TsDirectBindingReason::ComputedProperty);
    }
    None
}

fn unsupported_binding_reason(binding: &TsBindingFact) -> Option<TsDirectBindingReason> {
    match &binding.status {
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
    }
}

fn is_import_binding(binding: &TsBindingFact) -> bool {
    binding.module_source.is_some()
        && matches!(
            binding.binding_kind,
            TsBindingKind::Import
                | TsBindingKind::NamedImport
                | TsBindingKind::DefaultImport
                | TsBindingKind::NamespaceImport
                | TsBindingKind::ReExport
        )
}

fn span_contains(container: &crate::core::Span, span: &crate::core::Span) -> bool {
    container.file == span.file
        && container.start_byte <= span.start_byte
        && container.end_byte >= span.end_byte
}

fn spans_overlap(left: &crate::core::Span, right: &crate::core::Span) -> bool {
    left.file == right.file
        && left.start_byte <= right.end_byte
        && right.start_byte <= left.end_byte
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

fn function_for_local_export<'a>(
    inventory: &'a TsInventoryOutput,
    scope: &'a TsScopeOutput,
    export: &TsBindingFact,
    local_name: &str,
) -> Option<&'a TsInventoryFunctionFact> {
    if let Some(binding) = visible_function_binding(scope, export.scope_key.as_str(), local_name)
        && let Some(function) = function_by_binding(inventory, local_name, binding)
    {
        return Some(function);
    }

    let mut overlapping = inventory
        .functions
        .iter()
        .filter(|function| function.display_name.as_deref() == Some(local_name))
        .filter(|function| spans_overlap(&function.span, &export.span));
    let first = overlapping.next()?;
    if overlapping.next().is_none() {
        Some(first)
    } else {
        None
    }
}

fn visible_function_binding<'a>(
    scope: &'a TsScopeOutput,
    scope_key: &str,
    name: &str,
) -> Option<&'a TsBindingFact> {
    let parent_by_key = scope
        .scopes
        .iter()
        .map(|scope| (scope.stable_key.as_str(), scope.parent_scope_key.as_deref()))
        .collect::<BTreeMap<_, _>>();
    let mut current = Some(scope_key);
    while let Some(scope_key) = current {
        if let Some(binding) = scope.bindings.iter().find(|binding| {
            binding.scope_key == scope_key
                && binding.name == name
                && matches!(
                    binding.binding_kind,
                    TsBindingKind::FunctionDeclaration | TsBindingKind::FunctionExpression
                )
        }) {
            return Some(binding);
        }
        current = parent_by_key.get(scope_key).and_then(|parent| *parent);
    }
    None
}

fn function_by_binding<'a>(
    inventory: &'a TsInventoryOutput,
    name: &str,
    binding: &TsBindingFact,
) -> Option<&'a TsInventoryFunctionFact> {
    let functions = inventory
        .functions
        .iter()
        .filter(|function| function.display_name.as_deref() == Some(name))
        .collect::<Vec<_>>();
    if let Some(function) = functions.iter().copied().find(|function| {
        function.file == binding.file && spans_overlap(&function.span, &binding.span)
    }) {
        return Some(function);
    }

    let mut same_file = functions
        .iter()
        .copied()
        .filter(|function| function.file == binding.file);
    let first = same_file.next()?;
    if same_file.next().is_none() {
        Some(first)
    } else {
        None
    }
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
