#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{FileId, Language, ModuleNodeId, PackageId, SymbolNamespace};

    #[test]
    fn scope_stable_keys_are_deterministic_across_input_order() {
        let first = ScopeFact::stable_key_for(
            Language::TypeScript,
            &["module".to_string(), "function:handler".to_string()],
            Some("src/api.ts".to_string()),
            Some("pkg:web".to_string()),
            Some("mod:web".to_string()),
            ScopeKind::Function,
            SemanticStatus::Resolved,
        );
        let second = ScopeFact::stable_key_for(
            Language::TypeScript,
            &["function:handler".to_string(), "module".to_string()],
            Some("src/api.ts".to_string()),
            Some("pkg:web".to_string()),
            Some("mod:web".to_string()),
            ScopeKind::Function,
            SemanticStatus::Resolved,
        );

        assert_eq!(first, second);
    }

    #[test]
    fn stable_export_identity_key_includes_required_context() {
        let identity = StableExportIdentity {
            id: StableExportId(17),
            export: ExportId(3),
            language: Language::Go,
            package_key: Some("pkg:service".to_string()),
            module_key: Some("mod:service".to_string()),
            export_name: "Handler".to_string(),
            namespace: SymbolNamespace::Value,
            symbol_stable_key: "symbol:handler".to_string(),
            generated_discriminator: Some("route:/users".to_string()),
            stable_key: String::new(),
            status: SemanticStatus::Generated,
        };

        let stable_key = identity.computed_stable_key();

        assert!(stable_key.contains("language"));
        assert!(stable_key.contains("package"));
        assert!(stable_key.contains("module"));
        assert!(stable_key.contains("export_name"));
        assert!(stable_key.contains("namespace"));
        assert!(stable_key.contains("symbol_stable_key"));
        assert!(stable_key.contains("generated_discriminator"));
    }

    #[test]
    fn alias_status_supports_all_closure_outcomes() {
        let statuses = [
            SemanticStatus::Resolved,
            SemanticStatus::Ambiguous,
            SemanticStatus::Unresolved,
            SemanticStatus::Cycle,
            SemanticStatus::Generated,
            SemanticStatus::Dynamic,
            SemanticStatus::External,
            SemanticStatus::SetupMissing,
            SemanticStatus::Unsupported,
        ];

        let aliases = statuses
            .into_iter()
            .enumerate()
            .map(|(index, status)| AliasFact {
                id: AliasId(index as u64),
                language: Language::JavaScript,
                file: Some(FileId(0)),
                package: Some(PackageId(1)),
                module: Some(ModuleNodeId(2)),
                source_symbol_stable_key: format!("alias:{index}"),
                target_symbol_stable_keys: Vec::new(),
                kind: AliasKind::ReExport,
                stable_key: format!("alias-key:{index}"),
                status,
            })
            .collect::<Vec<_>>();

        assert_eq!(aliases.len(), 9);
    }
}
