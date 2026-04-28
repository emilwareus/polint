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
