use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const RULE_WIT: &str = include_str!("rule.wit");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmRuleManifest {
    pub id: String,
    pub component_path: PathBuf,
    pub sdk_version: String,
    pub source_hash: String,
}

#[derive(Debug, Clone)]
pub struct PluginHost {
    experimental: bool,
}

impl PluginHost {
    pub fn experimental() -> Self {
        Self { experimental: true }
    }

    pub fn load_manifest(&self, path: impl AsRef<Path>) -> Result<WasmRuleManifest> {
        let path = path.as_ref();
        if !self.experimental {
            bail!("Wasm plugins are experimental and disabled");
        }
        let raw = std::fs::read_to_string(path)?;
        let manifest: WasmRuleManifest = serde_json::from_str(&raw)?;
        if !manifest.component_path.exists() {
            bail!(
                "plugin component does not exist: {}",
                manifest.component_path.display()
            );
        }
        Ok(manifest)
    }

    #[cfg(feature = "wasmtime-host")]
    pub fn validate_component_bytes(&self, bytes: &[u8]) -> Result<()> {
        let engine = wasmtime::Engine::default();
        wasmtime::component::Component::from_binary(&engine, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RULE_WIT;

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
}
