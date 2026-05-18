#[cfg(test)]
mod source_config_rule_model_extension {
    use super::*;
    use crate::analysis_kernel::{
        CachePolicy, LanguageScope, PrecisionCeiling, ProviderKind, ProviderManifest,
        SchemaVersion,
    };
    use crate::config::{LoadedConfig, PolintConfig};
    use crate::core::AnalysisDb;
    use std::path::Path;
    use tempfile::TempDir;

    fn loaded_config(root: &Path) -> LoadedConfig {
        LoadedConfig {
            root: root.to_path_buf(),
            config: PolintConfig::default(),
            missing: false,
        }
    }

    fn db_with_files(root: &Path, files: &[(&str, &str)]) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        for (relative_path, source) in files {
            let path = root.join(relative_path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create fixture parent");
            }
            std::fs::write(&path, source).expect("write fixture source");
            db.add_file(path, (*relative_path).to_string(), (*source).to_string());
        }
        db
    }

    fn snapshot_for(
        loaded: &LoadedConfig,
        db: &AnalysisDb,
        provider_manifests: &[ProviderManifest],
    ) -> InputSnapshot {
        InputSnapshot::from_run_inputs(
            loaded,
            db,
            "config-digest",
            "rule-digest",
            "plan-digest",
            provider_manifests,
        )
    }

    #[test]
    fn snapshots_from_same_inputs_serialize_to_identical_pretty_json() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(
            temp.path(),
            &[
                ("src/z.ts", "export const z = 1;\n"),
                ("cmd/main.go", "package main\n"),
            ],
        );

        let first = snapshot_for(&loaded, &db, crate::analysis_kernel::AnalysisKernel::provider_manifests());
        let second = snapshot_for(&loaded, &db, crate::analysis_kernel::AnalysisKernel::provider_manifests());

        assert_eq!(
            serde_json::to_string_pretty(&first).expect("serialize first snapshot"),
            serde_json::to_string_pretty(&second).expect("serialize second snapshot")
        );
    }

    #[test]
    fn file_rows_expose_safe_identity_without_source_or_machine_paths() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(temp.path(), &[("src/app.ts", "const secret = 'raw text';\n")]);

        let snapshot = snapshot_for(&loaded, &db, crate::analysis_kernel::AnalysisKernel::provider_manifests());
        let file = &snapshot.files[0];
        let rendered = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");

        assert_eq!(file.relative_path, "src/app.ts");
        assert_eq!(file.language, crate::core::Language::TypeScript);
        assert_eq!(file.source_text_digest.kind, DigestKind::SourceText);
        assert_eq!(file.size_bytes, "const secret = 'raw text';\n".len());
        assert!(file.mtime_hint_present);
        assert!(!rendered.contains("const secret"));
        assert!(!rendered.contains(temp.path().to_string_lossy().as_ref()));
        assert!(!rendered.contains(db.files()[0].path.to_string_lossy().as_ref()));
    }

    #[test]
    fn config_rule_plan_model_extension_and_provider_components_are_typed() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(temp.path(), &[("src/app.ts", "export const app = 1;\n")]);

        let snapshot = snapshot_for(&loaded, &db, crate::analysis_kernel::AnalysisKernel::provider_manifests());

        assert_eq!(snapshot.config.status, InputComponentStatus::Present);
        assert_eq!(snapshot.config.digest.kind, DigestKind::Config);
        assert!(snapshot
            .rules
            .iter()
            .any(|component| component.digest.kind == DigestKind::RuleCode));
        assert!(snapshot
            .rules
            .iter()
            .any(|component| component.digest.kind == DigestKind::RuleOptions));
        assert_eq!(snapshot.models[0].name, "model.files");
        assert_eq!(snapshot.models[0].status, InputComponentStatus::Absent);
        assert_eq!(snapshot.models[0].digest.kind, DigestKind::ModelFile);
        assert_eq!(snapshot.extensions[0].name, "extension.providers");
        assert_eq!(snapshot.extensions[0].status, InputComponentStatus::Absent);
        assert_eq!(snapshot.extensions[0].digest.kind, DigestKind::ExtensionCode);
        assert!(snapshot
            .provider_schemas
            .iter()
            .all(|provider| provider.provider_manifest_digest.kind == DigestKind::ProviderParameters));
    }

    #[test]
    fn source_file_rows_are_sorted_by_normalized_relative_path() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(
            temp.path(),
            &[
                ("src/z.ts", "export const z = 1;\n"),
                ("cmd/main.go", "package main\n"),
                ("src/a.tsx", "export function A() { return null; }\n"),
            ],
        );

        let snapshot = snapshot_for(&loaded, &db, crate::analysis_kernel::AnalysisKernel::provider_manifests());

        assert_eq!(
            snapshot
                .files
                .iter()
                .map(|file| file.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec!["cmd/main.go", "src/a.tsx", "src/z.ts"]
        );
    }

    #[test]
    fn provider_schema_rows_include_manifest_identity_and_digest_scope_policy() {
        const SCHEMAS: &[SchemaVersion] = &[SchemaVersion {
            name: "example-facts",
            version: 1,
        }];
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(temp.path(), &[("src/app.ts", "export const app = 1;\n")]);
        let base = ProviderManifest {
            id: "polint.example",
            kind: ProviderKind::LanguageSyntax,
            inputs: &["source_files", "config"],
            outputs: &["facts"],
            language_scope: LanguageScope::Go,
            cache_policy: CachePolicy::NoCache,
            schema_versions: SCHEMAS,
            precision_ceiling: PrecisionCeiling::Syntax,
        };
        let scope_changed = ProviderManifest {
            language_scope: LanguageScope::TypeScriptJavaScript,
            ..base
        };
        let policy_changed = ProviderManifest {
            cache_policy: CachePolicy::InMemoryDerived,
            ..base
        };

        let base_snapshot = snapshot_for(&loaded, &db, &[base]);
        let scope_snapshot = snapshot_for(&loaded, &db, &[scope_changed]);
        let policy_snapshot = snapshot_for(&loaded, &db, &[policy_changed]);
        let row = &base_snapshot.provider_schemas[0];

        assert_eq!(row.provider_id, "polint.example");
        assert_eq!(row.schema_versions, vec!["example-facts:1"]);
        assert_eq!(row.language_scope, "go");
        assert_eq!(row.cache_policy, "no_cache");
        assert_eq!(row.precision_ceiling, "syntax");
        assert_ne!(
            row.provider_manifest_digest,
            scope_snapshot.provider_schemas[0].provider_manifest_digest
        );
        assert_ne!(
            row.provider_manifest_digest,
            policy_snapshot.provider_schemas[0].provider_manifest_digest
        );
    }
}
