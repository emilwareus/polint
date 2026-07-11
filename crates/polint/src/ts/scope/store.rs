#![allow(dead_code, reason = "kept for private internal consumers")]

use crate::analysis::ids::{TsBindingId, TsScopeId};
use crate::core::FileId;
use crate::ts::scope::facts::{TsBindingFact, TsBindingKind, TsScopeFact};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TsScopeOutput {
    pub(crate) scopes: Vec<TsScopeFact>,
    pub(crate) bindings: Vec<TsBindingFact>,
}

impl TsScopeOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.scopes.sort_by(|left, right| {
            (
                left.stable_key.as_str(),
                left.span.start_byte,
                left.span.end_byte,
            )
                .cmp(&(
                    right.stable_key.as_str(),
                    right.span.start_byte,
                    right.span.end_byte,
                ))
        });
        for (index, scope) in self.scopes.iter_mut().enumerate() {
            scope.id = TsScopeId(index as u64);
        }

        self.bindings.sort_by(|left, right| {
            (
                left.stable_key.as_str(),
                left.span.start_byte,
                left.span.end_byte,
            )
                .cmp(&(
                    right.stable_key.as_str(),
                    right.span.start_byte,
                    right.span.end_byte,
                ))
        });
        for (index, binding) in self.bindings.iter_mut().enumerate() {
            binding.id = TsBindingId(index as u64);
        }

        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TsScopeStore {
    output: TsScopeOutput,
    scopes_by_file: std::collections::BTreeMap<FileId, Vec<usize>>,
    scopes_by_stable_key: std::collections::BTreeMap<String, usize>,
    bindings_by_file: std::collections::BTreeMap<FileId, Vec<usize>>,
    bindings_by_name: std::collections::BTreeMap<String, Vec<usize>>,
    bindings_by_scope_name: std::collections::BTreeMap<(String, String), Vec<usize>>,
    bindings_by_kind: std::collections::BTreeMap<TsBindingKind, Vec<usize>>,
    imports_by_module_imported: std::collections::BTreeMap<(String, String), Vec<usize>>,
    exports_by_name: std::collections::BTreeMap<String, Vec<usize>>,
}

impl TsScopeStore {
    pub(crate) fn from_output(output: TsScopeOutput) -> Self {
        let output = output.normalized();
        let mut store = Self {
            output,
            ..Self::default()
        };

        for (index, scope) in store.output.scopes.iter().enumerate() {
            store
                .scopes_by_file
                .entry(scope.file)
                .or_default()
                .push(index);
            store
                .scopes_by_stable_key
                .insert(scope.stable_key.clone(), index);
        }

        for (index, binding) in store.output.bindings.iter().enumerate() {
            store
                .bindings_by_file
                .entry(binding.file)
                .or_default()
                .push(index);
            store
                .bindings_by_name
                .entry(binding.name.clone())
                .or_default()
                .push(index);
            store
                .bindings_by_scope_name
                .entry((binding.scope_key.clone(), binding.name.clone()))
                .or_default()
                .push(index);
            store
                .bindings_by_kind
                .entry(binding.binding_kind)
                .or_default()
                .push(index);
            if let (Some(module), Some(imported)) = (&binding.module_source, &binding.imported_name)
            {
                store
                    .imports_by_module_imported
                    .entry((module.clone(), imported.clone()))
                    .or_default()
                    .push(index);
            }
            if let Some(exported) = &binding.exported_name {
                store
                    .exports_by_name
                    .entry(exported.clone())
                    .or_default()
                    .push(index);
            }
        }

        store
    }

    pub(crate) fn scopes(&self) -> &[TsScopeFact] {
        &self.output.scopes
    }

    pub(crate) fn bindings(&self) -> &[TsBindingFact] {
        &self.output.bindings
    }

    pub(crate) fn scopes_for_file(&self, file: FileId) -> Vec<&TsScopeFact> {
        self.scope_refs(self.scopes_by_file.get(&file))
    }

    pub(crate) fn bindings_for_file(&self, file: FileId) -> Vec<&TsBindingFact> {
        self.binding_refs(self.bindings_by_file.get(&file))
    }

    pub(crate) fn scope_by_stable_key(&self, stable_key: &str) -> Option<&TsScopeFact> {
        self.scopes_by_stable_key
            .get(stable_key)
            .map(|index| &self.output.scopes[*index])
    }

    pub(crate) fn bindings_by_name(&self, name: &str) -> Vec<&TsBindingFact> {
        self.binding_refs(self.bindings_by_name.get(name))
    }

    pub(crate) fn lookup_binding_in_scope(
        &self,
        scope_key: &str,
        name: &str,
    ) -> Vec<&TsBindingFact> {
        self.binding_refs(
            self.bindings_by_scope_name
                .get(&(scope_key.to_string(), name.to_string())),
        )
    }

    pub(crate) fn bindings_by_kind(&self, kind: TsBindingKind) -> Vec<&TsBindingFact> {
        self.binding_refs(self.bindings_by_kind.get(&kind))
    }

    pub(crate) fn import_aliases(
        &self,
        module_source: &str,
        imported_name: &str,
    ) -> Vec<&TsBindingFact> {
        self.binding_refs(
            self.imports_by_module_imported
                .get(&(module_source.to_string(), imported_name.to_string())),
        )
    }

    pub(crate) fn exports_by_name(&self, exported_name: &str) -> Vec<&TsBindingFact> {
        self.binding_refs(self.exports_by_name.get(exported_name))
    }

    fn scope_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&TsScopeFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.scopes[index])
                .collect()
        })
    }

    fn binding_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&TsBindingFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.bindings[index])
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{FileId, Span};
    use crate::ts::scope::facts::{
        TsBindingStatus, TsDeclarationKind, TsImportExportKind, TsScopeKind,
    };

    use super::*;

    #[test]
    fn normalized_assigns_dense_ids_after_stable_key_sort() {
        let output = TsScopeOutput {
            scopes: vec![
                scope("scope:b", TsScopeId(22)),
                scope("scope:a", TsScopeId(11)),
            ],
            bindings: vec![
                binding("binding:b", TsBindingId(22), "second", "scope:b"),
                binding("binding:a", TsBindingId(11), "first", "scope:a"),
            ],
        }
        .normalized();

        assert_eq!(
            output
                .scopes
                .iter()
                .map(|scope| (scope.stable_key.as_str(), scope.id.0))
                .collect::<Vec<_>>(),
            vec![("scope:a", 0), ("scope:b", 1)]
        );
        assert_eq!(
            output
                .bindings
                .iter()
                .map(|binding| (binding.stable_key.as_str(), binding.id.0))
                .collect::<Vec<_>>(),
            vec![("binding:a", 0), ("binding:b", 1)]
        );
    }

    #[test]
    fn store_indexes_file_scope_name_kind_import_and_export_lookups() {
        let mut import = binding("binding:import", TsBindingId(1), "local", "scope:a");
        import.binding_kind = TsBindingKind::NamedImport;
        import.module_source = Some("./dep".to_string());
        import.imported_name = Some("remote".to_string());

        let mut export = binding("binding:export", TsBindingId(2), "reExport", "scope:a");
        export.binding_kind = TsBindingKind::ReExport;
        export.exported_name = Some("reExport".to_string());

        let store = TsScopeStore::from_output(TsScopeOutput {
            scopes: vec![scope("scope:a", TsScopeId(1))],
            bindings: vec![
                binding("binding:local", TsBindingId(3), "local", "scope:a"),
                import,
                export,
            ],
        });

        assert_eq!(store.scopes().len(), 1);
        assert_eq!(store.bindings().len(), 3);
        assert_eq!(store.scopes_for_file(FileId(1)).len(), 1);
        assert_eq!(store.bindings_for_file(FileId(1)).len(), 3);
        assert!(store.scope_by_stable_key("scope:a").is_some());
        assert_eq!(store.bindings_by_name("local").len(), 2);
        assert_eq!(store.lookup_binding_in_scope("scope:a", "local").len(), 2);
        assert_eq!(store.bindings_by_kind(TsBindingKind::NamedImport).len(), 1);
        assert_eq!(store.import_aliases("./dep", "remote").len(), 1);
        assert_eq!(store.exports_by_name("reExport").len(), 1);
    }

    fn scope(stable_key: &str, id: TsScopeId) -> TsScopeFact {
        TsScopeFact {
            id,
            file: FileId(1),
            span: span(),
            stable_key: stable_key.to_string(),
            parent_scope_key: None,
            kind: TsScopeKind::Module,
        }
    }

    fn binding(stable_key: &str, id: TsBindingId, name: &str, scope_key: &str) -> TsBindingFact {
        TsBindingFact {
            id,
            file: FileId(1),
            span: span(),
            stable_key: stable_key.to_string(),
            scope_key: scope_key.to_string(),
            parent_scope_key: None,
            name: name.to_string(),
            declaration_kind: TsDeclarationKind::Const,
            binding_kind: TsBindingKind::Const,
            import_export_kind: TsImportExportKind::None,
            module_source: None,
            imported_name: None,
            exported_name: None,
            inventory_function_key: None,
            inventory_callsite_key: None,
            status: TsBindingStatus::present(),
        }
    }

    fn span() -> Span {
        Span {
            file: FileId(1),
            start_byte: 1,
            end_byte: 4,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 4,
        }
    }
}
