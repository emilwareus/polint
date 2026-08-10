//! Provider and host-context contracts.

use std::any::Any;
use std::collections::BTreeMap;

use polint_core::{Diagnostic, LanguageId};

use crate::digest::{CacheStats, Digest, DigestKind};
use crate::fact_store::FactStore;
use crate::metadata::{FactFamily, FactMetaStore};
use polint_core::StableKeyInterner;

/// Host fact registry neck used by providers and frontends.
pub trait FactDatabase: Any + Send {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn store(&self, family: FactFamily) -> Option<&dyn FactStore>;
    fn store_mut(&mut self, family: FactFamily) -> Option<&mut dyn FactStore>;
    fn fact_meta(&self) -> &FactMetaStore;
    fn fact_meta_mut(&mut self) -> &mut FactMetaStore;
    fn stable_key_interner(&self) -> StableKeyInterner;
}

/// Facade/host services providers need beyond the fact registry (cache, config, plan).
pub trait ProviderHostServices: Any + Send {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Opaque host side-channel (for example SCC-closure outputs owned by the composition root).
pub trait HostAttachment: Any + Send {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Empty attachment used when a provider run does not need a side channel.
#[derive(Debug, Default)]
pub struct NullHostAttachment;

impl HostAttachment for NullHostAttachment {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderManifest {
    pub id: &'static str,
    pub kind: ProviderKind,
    pub inputs: &'static [&'static str],
    pub outputs: &'static [&'static str],
    pub language_ids: &'static [LanguageId],
    pub cache_policy: CachePolicy,
    pub schema_versions: &'static [SchemaVersion],
    pub precision_ceiling: PrecisionCeiling,
}

/// Result of running a single analysis provider stage.
pub struct ProviderRunResult {
    pub diagnostics: Vec<Diagnostic>,
    pub cache_stats: CacheStats,
    pub output_digest: Option<Digest>,
}

pub trait Provider: Send + Sync {
    fn manifest(&self) -> &'static ProviderManifest;
    fn run(&self, ctx: &mut ProviderCtx<'_>) -> ProviderRunResult;
}

pub struct ProviderCtx<'a> {
    pub facts: &'a mut dyn FactDatabase,
    pub host: &'a mut dyn ProviderHostServices,
    pub config_digest: &'a str,
    pub rule_digest: &'a str,
    pub parallel: bool,
    /// Output digests of already-run providers, keyed by provider id.
    pub upstream_digests: &'a BTreeMap<&'static str, Digest>,
    /// Opaque host side-channel filled by selected providers for composition-root plumbing.
    pub host_attachment: &'a mut dyn HostAttachment,
}

impl ProviderCtx<'_> {
    pub fn dependency_digest(&self, provider_id: &'static str) -> Digest {
        self.upstream_digests
            .get(provider_id)
            .cloned()
            .unwrap_or_else(|| Digest::absent(DigestKind::ProviderOutput, provider_id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    SourceDiscovery,
    LanguageSyntax,
    WholeRepoDerived,
    MetricsDerived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    NoCache,
    ExistingFileFactCache { schema: &'static str },
    InMemoryDerived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaVersion {
    pub name: &'static str,
    pub version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecisionCeiling {
    Exact,
    Syntax,
    SetupAware,
}

impl ProviderManifest {
    pub fn provider_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    pub fn primary_schema_label(&self) -> String {
        let mut labels = self
            .schema_versions
            .iter()
            .map(|schema| format!("{}:{}", schema.name, schema.version))
            .collect::<Vec<_>>();
        labels.sort();
        labels.join(",")
    }

    pub fn language_scope_label(&self) -> &'static str {
        match self.language_ids {
            [] => "workspace",
            [id] if *id == LanguageId::GO => "go",
            [id] if *id == LanguageId::TS => "typescript_javascript",
            _ => "multi_language",
        }
    }

    pub fn cache_policy_label(&self) -> String {
        match self.cache_policy {
            CachePolicy::NoCache => "no_cache".to_string(),
            CachePolicy::ExistingFileFactCache { schema } => {
                format!("existing_file_fact_cache:{schema}")
            }
            CachePolicy::InMemoryDerived => "in_memory_derived".to_string(),
        }
    }
}
