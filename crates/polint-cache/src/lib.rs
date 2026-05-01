use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const CACHE_VERSION: &str = "polint-cache-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheKey {
    pub file_hash: String,
    pub config_hash: String,
    pub rule_hash: String,
    pub version: String,
    pub schema: String,
}

impl CacheKey {
    pub fn new(
        file_hash: impl Into<String>,
        config_hash: impl Into<String>,
        rule_hash: impl Into<String>,
    ) -> Self {
        Self {
            file_hash: file_hash.into(),
            config_hash: config_hash.into(),
            rule_hash: rule_hash.into(),
            version: CACHE_VERSION.to_string(),
            schema: "analysis-facts-v1".to_string(),
        }
    }

    pub fn for_file(
        relative_path: &str,
        content_hash: &str,
        config_hash: &str,
        rule_hash: &str,
        schema: &str,
    ) -> Self {
        Self {
            file_hash: stable_hash(&[relative_path, content_hash]),
            config_hash: config_hash.to_string(),
            rule_hash: rule_hash.to_string(),
            version: CACHE_VERSION.to_string(),
            schema: schema.to_string(),
        }
    }

    pub fn stable_id(&self) -> String {
        stable_hash(&[
            &self.file_hash,
            &self.config_hash,
            &self.rule_hash,
            &self.version,
            &self.schema,
        ])
    }
}

#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
    enabled: bool,
}

impl Cache {
    pub fn new(root: impl AsRef<Path>, enabled: bool) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            enabled,
        }
    }

    pub fn default_for_repo(repo: impl AsRef<Path>, enabled: bool) -> Self {
        Self::new(repo.as_ref().join(".polint/cache"), enabled)
    }

    pub fn read_json<T>(&self, key: &CacheKey) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        if !self.enabled {
            return Ok(None);
        }
        let path = self.path_for(key);
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read cache {}", path.display()))?;
        Ok(Some(serde_json::from_str(&raw)?))
    }

    pub fn write_json<T>(&self, key: &CacheKey, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        if !self.enabled {
            return Ok(());
        }
        fs::create_dir_all(&self.root)?;
        let path = self.path_for(key);
        fs::write(&path, serde_json::to_vec_pretty(value)?)
            .with_context(|| format!("failed to write cache {}", path.display()))?;
        Ok(())
    }

    fn path_for(&self, key: &CacheKey) -> PathBuf {
        self.root.join(format!("{}.json", key.stable_id()))
    }
}

pub fn stable_hash(parts: &[&str]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xfe;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_changes_with_config() {
        let a = CacheKey::new("file", "config-a", "rule");
        let b = CacheKey::new("file", "config-b", "rule");
        assert_ne!(a.stable_id(), b.stable_id());
    }

    #[test]
    fn cache_key_changes_with_rule_hash() {
        let a = CacheKey::for_file("src/main.go", "content", "config", "rule-a", "go-facts-v1");
        let b = CacheKey::for_file("src/main.go", "content", "config", "rule-b", "go-facts-v1");

        assert_ne!(a.stable_id(), b.stable_id());
    }

    #[test]
    fn cache_key_changes_with_schema() {
        let a = CacheKey::for_file("src/main.go", "content", "config", "rule", "go-facts-v1");
        let b = CacheKey::for_file("src/main.go", "content", "config", "rule", "ts-facts-v1");

        assert_ne!(a.stable_id(), b.stable_id());
    }

    #[test]
    fn cache_key_changes_with_relative_path() {
        let a = CacheKey::for_file("src/main.go", "content", "config", "rule", "go-facts-v1");
        let b = CacheKey::for_file("src/other.go", "content", "config", "rule", "go-facts-v1");

        assert_ne!(a.stable_id(), b.stable_id());
    }
}
