#[cfg(test)]
mod tests {
    use super::super::{AnalysisKernel, KernelInput};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use serde_json::Value;
    use std::fs;

    fn debug_report_from_kernel_run() -> (tempfile::TempDir, Value) {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("src")).expect("create src directory");
        fs::write(
            temp.path().join(".polint.toml"),
            r#"
[workspace]
include = ["src/**"]
exclude = []
"#,
        )
        .expect("write config");
        fs::write(
            temp.path().join("src/tokens.ts"),
            r#"export const token = "ok";"#,
        )
        .expect("write tokens");
        fs::write(
            temp.path().join("src/app.ts"),
            r#"import { token as importedToken } from "./tokens";

export function answer() {
  return importedToken;
}

export const value = answer();
"#,
        )
        .expect("write app");

        let loaded = load_config(temp.path()).expect("load config");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["imports", "symbols", "references"]);
        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "metadata-debug-config",
            rule_digest: "metadata-debug-rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel run");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.rule_id != "polint/internal"),
            "clean debug fixture should not emit internal diagnostics: {:#?}",
            output.diagnostics
        );

        (
            temp,
            AnalysisKernel::metadata_debug_json_for_test(&output.db),
        )
    }

    #[test]
    fn metadata_debug_json_contains_files_imports_symbols_and_references() {
        let (_temp, report) = debug_report_from_kernel_run();

        for key in ["files", "imports", "symbols", "references"] {
            let rows = report[key]
                .as_array()
                .unwrap_or_else(|| panic!("missing debug array `{key}`: {report:#?}"));
            assert!(!rows.is_empty(), "debug array `{key}` should not be empty");
        }
    }

    #[test]
    fn metadata_debug_json_rows_include_required_metadata_fields() {
        let (_temp, report) = debug_report_from_kernel_run();

        for key in ["files", "imports", "symbols", "references"] {
            for row in report[key]
                .as_array()
                .unwrap_or_else(|| panic!("missing debug array `{key}`: {report:#?}"))
            {
                for field in [
                    "family",
                    "run_id",
                    "stable_key",
                    "producer_id",
                    "layer_id",
                    "precision",
                    "confidence",
                    "validation",
                ] {
                    assert!(
                        row.get(field).is_some(),
                        "debug row in `{key}` missing `{field}`: {row:#?}"
                    );
                }
            }
        }
    }

    #[test]
    fn metadata_debug_json_serializes_byte_identically_for_same_database() {
        let (_temp, first) = debug_report_from_kernel_run();
        let second = first.clone();

        let first_json = serde_json::to_string_pretty(&first).expect("serialize first");
        let second_json = serde_json::to_string_pretty(&second).expect("serialize second");

        assert_eq!(first_json, second_json);
    }

    #[test]
    fn metadata_debug_json_excludes_absolute_paths_and_transient_runtime_details() {
        let (temp, report) = debug_report_from_kernel_run();
        let rendered = serde_json::to_string_pretty(&report).expect("serialize report");
        let temp_root = temp.path().to_string_lossy();

        for forbidden in [
            temp_root.as_ref(),
            "SystemTime",
            "Instant",
            "0x",
            "timestamp",
            "created_at",
            "updated_at",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "debug JSON should not contain `{forbidden}`:\n{rendered}"
            );
        }
    }
}
