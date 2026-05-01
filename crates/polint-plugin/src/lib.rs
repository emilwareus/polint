use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const RULE_WIT: &str = include_str!("rule.wit");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmRuleManifest {
    pub id: String,
    pub component_path: PathBuf,
    pub sdk_version: String,
    pub source_hash: String,
}

pub type Result<T> = std::result::Result<T, PluginError>;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Wasm plugins are experimental and disabled")]
    ExperimentalDisabled,

    #[error("failed to read plugin manifest {path}: {source}")]
    ReadManifest {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse plugin manifest {path}: {source}")]
    ParseManifest {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("plugin manifest id must not be empty")]
    MissingId,

    #[error("plugin manifest sdk_version must not be empty")]
    MissingSdkVersion,

    #[error("plugin manifest source_hash must not be empty")]
    MissingSourceHash,

    #[error("plugin manifest component_path must not be empty")]
    MissingComponentPath,

    #[error("plugin component does not exist: {path}")]
    ComponentMissing { path: PathBuf },

    #[cfg(feature = "wasmtime-host")]
    #[error("plugin component is invalid: {source}")]
    InvalidComponent {
        #[source]
        source: wasmtime::Error,
    },
}

#[derive(Debug, Clone)]
pub struct PluginHost {
    experimental: bool,
}

impl PluginHost {
    pub fn experimental() -> Self {
        Self { experimental: true }
    }

    #[cfg(test)]
    fn disabled_for_tests() -> Self {
        Self {
            experimental: false,
        }
    }

    pub fn load_manifest(&self, path: impl AsRef<Path>) -> Result<WasmRuleManifest> {
        let manifest_path = path.as_ref();
        if !self.experimental {
            return Err(PluginError::ExperimentalDisabled);
        }

        let raw =
            std::fs::read_to_string(manifest_path).map_err(|source| PluginError::ReadManifest {
                path: manifest_path.to_path_buf(),
                source,
            })?;

        let mut manifest: WasmRuleManifest =
            serde_json::from_str(&raw).map_err(|source| PluginError::ParseManifest {
                path: manifest_path.to_path_buf(),
                source,
            })?;

        if manifest.id.trim().is_empty() {
            return Err(PluginError::MissingId);
        }
        if manifest.sdk_version.trim().is_empty() {
            return Err(PluginError::MissingSdkVersion);
        }
        if manifest.source_hash.trim().is_empty() {
            return Err(PluginError::MissingSourceHash);
        }
        if manifest.component_path.as_os_str().is_empty() {
            return Err(PluginError::MissingComponentPath);
        }

        let component_path = if manifest.component_path.is_absolute() {
            manifest.component_path.clone()
        } else {
            manifest_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(&manifest.component_path)
        };

        if !component_path.exists() {
            return Err(PluginError::ComponentMissing {
                path: component_path,
            });
        }

        manifest.component_path = component_path;
        Ok(manifest)
    }

    #[cfg(feature = "wasmtime-host")]
    pub fn validate_component_bytes(&self, bytes: &[u8]) -> Result<()> {
        if !self.experimental {
            return Err(PluginError::ExperimentalDisabled);
        }

        let engine = wasmtime::Engine::default();
        wasmtime::component::Component::from_binary(&engine, bytes)
            .map_err(|source| PluginError::InvalidComponent { source })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{PluginError, PluginHost, RULE_WIT};
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn write_manifest(dir: &Path, component_path: &str) -> PathBuf {
        write_manifest_with(
            dir,
            component_path,
            "local.no_debug_prints",
            "0.1.0",
            "sha256:demo",
        )
    }

    fn write_manifest_with(
        dir: &Path,
        component_path: &str,
        id: &str,
        sdk_version: &str,
        source_hash: &str,
    ) -> PathBuf {
        let manifest_path = dir.join("polint-plugin.json");
        fs::write(
            &manifest_path,
            json!({
                "id": id,
                "component_path": component_path,
                "sdk_version": sdk_version,
                "source_hash": source_hash,
            })
            .to_string(),
        )
        .unwrap();
        manifest_path
    }

    #[test]
    fn wit_contract_contains_rule_boundary() {
        for anchor in [
            "package polint:rule;",
            "world rule",
            "export metadata",
            "export capabilities",
            "export run",
        ] {
            assert!(
                RULE_WIT.contains(anchor),
                "WIT contract is missing anchor {anchor:?}"
            );
        }
    }

    #[test]
    fn wit_contract_contains_stable_id_host_queries() {
        for anchor in [
            "type file-id = u32",
            "type function-id = u64",
            "type branch-id = u64",
            "report: func",
            "get-file-path: func",
            "get-function-name: func",
            "get-branch-condition: func",
        ] {
            assert!(
                RULE_WIT.contains(anchor),
                "WIT contract is missing anchor {anchor:?}"
            );
        }
    }

    #[test]
    fn wit_contract_contains_typed_metadata_and_diagnostics() {
        for anchor in [
            "enum severity",
            "record rule-metadata",
            "record diagnostic",
            "record text-range",
            "export metadata: func() -> host.rule-metadata",
            "report: func(diagnostic: diagnostic)",
        ] {
            assert!(
                RULE_WIT.contains(anchor),
                "WIT contract is missing anchor {anchor:?}"
            );
        }
    }

    #[test]
    fn wit_contract_does_not_define_ast_payloads() {
        for forbidden in ["ast-json", "source-text", "syntax-tree"] {
            assert!(
                !RULE_WIT.contains(forbidden),
                "WIT contract must not expose full AST/source payload {forbidden:?}"
            );
        }
    }

    #[test]
    fn manifest_loads_relative_component_path() {
        let temp = tempfile::tempdir().unwrap();
        let rules_dir = temp.path().join("rules");
        fs::create_dir(&rules_dir).unwrap();
        let component_path = rules_dir.join("demo.wasm");
        fs::write(&component_path, b"demo").unwrap();
        let manifest_path = write_manifest(temp.path(), "rules/demo.wasm");

        let manifest = PluginHost::experimental()
            .load_manifest(&manifest_path)
            .unwrap();

        assert_eq!(manifest.component_path, component_path);
    }

    #[test]
    fn manifest_missing_component_path_is_structured_error() {
        let temp = tempfile::tempdir().unwrap();
        let manifest_path = write_manifest(temp.path(), "rules/missing.wasm");

        let error = PluginHost::experimental()
            .load_manifest(&manifest_path)
            .unwrap_err();

        assert!(matches!(error, PluginError::ComponentMissing { .. }));
    }

    #[test]
    fn manifest_rejects_empty_required_fields() {
        type ErrorPredicate = fn(&PluginError) -> bool;

        let cases: [(&str, &str, &str, &str, &str, ErrorPredicate); 4] = [
            (
                "",
                "rules/demo.wasm",
                "0.1.0",
                "sha256:demo",
                "empty id",
                |error: &PluginError| matches!(error, PluginError::MissingId),
            ),
            (
                "local.no_debug_prints",
                "rules/demo.wasm",
                "",
                "sha256:demo",
                "empty sdk_version",
                |error: &PluginError| matches!(error, PluginError::MissingSdkVersion),
            ),
            (
                "local.no_debug_prints",
                "rules/demo.wasm",
                "0.1.0",
                "",
                "empty source_hash",
                |error: &PluginError| matches!(error, PluginError::MissingSourceHash),
            ),
            (
                "local.no_debug_prints",
                "",
                "0.1.0",
                "sha256:demo",
                "empty component_path",
                |error: &PluginError| matches!(error, PluginError::MissingComponentPath),
            ),
        ];

        for (id, component_path, sdk_version, source_hash, label, is_expected_error) in cases {
            let temp = tempfile::tempdir().unwrap();
            let manifest_path =
                write_manifest_with(temp.path(), component_path, id, sdk_version, source_hash);

            let error = PluginHost::experimental()
                .load_manifest(&manifest_path)
                .unwrap_err();

            assert!(is_expected_error(&error), "{label}: {error:?}");
        }
    }

    #[test]
    fn non_experimental_host_rejects_manifest_loading() {
        let error = PluginHost::disabled_for_tests()
            .load_manifest("missing-manifest.json")
            .unwrap_err();

        assert!(matches!(error, PluginError::ExperimentalDisabled));
    }

    #[cfg(feature = "wasmtime-host")]
    #[test]
    fn invalid_component_bytes_are_rejected() {
        let error = PluginHost::experimental()
            .validate_component_bytes(b"not a wasm component")
            .unwrap_err();

        assert!(matches!(error, PluginError::InvalidComponent { .. }));
    }
}
