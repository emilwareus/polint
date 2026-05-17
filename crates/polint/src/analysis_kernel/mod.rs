use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, CapabilitySupportView};
use crate::diagnostics::Diagnostic;

mod metadata;
mod provider;

pub(crate) use metadata::{
    FactConfidence, FactFamily, FactMeta, FactMetaInsert, FactMetaStore, FactPrecision, FactRef,
    ValidationStatus, stable_key_from_parts,
};
pub(crate) use provider::{
    CachePolicy, LanguageScope, PrecisionCeiling, ProviderKind, ProviderManifest, SchemaVersion,
};

pub(crate) struct AnalysisKernel;

pub(crate) struct KernelInput<'a> {
    pub(crate) loaded: &'a LoadedConfig,
    pub(crate) cache: &'a Cache,
    pub(crate) config_digest: &'a str,
    pub(crate) rule_digest: &'a str,
    pub(crate) plan: &'a AnalysisPlan,
    pub(crate) parallel: bool,
}

pub(crate) struct KernelOutput {
    pub(crate) db: AnalysisDb,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: CapabilitySupportView,
}

impl AnalysisKernel {
    pub(crate) fn provider_manifests() -> &'static [ProviderManifest] {
        provider::provider_manifests()
    }

    pub(crate) fn run(input: KernelInput<'_>) -> anyhow::Result<KernelOutput> {
        let _manifest_metadata_token = Self::provider_manifest_metadata_token();
        let mut db = crate::fs::load_analysis_files(input.loaded)?;
        let mut diagnostics = Vec::new();

        diagnostics.extend(crate::go::analyze_with_plan_options(
            &mut db,
            input.cache,
            input.config_digest,
            input.rule_digest,
            input.plan,
            input.parallel,
        ));
        diagnostics.extend(crate::ts::analyze_with_plan_options(
            &mut db,
            input.cache,
            input.config_digest,
            input.rule_digest,
            input.plan,
            input.parallel,
        ));

        let module_graph =
            crate::module_graph::derive_requested_module_graph(&mut db, input.loaded, input.plan);
        let module_support = module_graph.support_view(input.plan.support_view());
        diagnostics.extend(module_graph.diagnostics);

        let symbol_graph =
            crate::symbol_graph::derive_requested_symbols(&mut db, input.loaded, input.plan);
        let capability_support = symbol_graph.support_view(&module_support);
        diagnostics.extend(symbol_graph.diagnostics);

        crate::metrics::derive_requested_metrics(&mut db, input.plan);

        Ok(KernelOutput {
            db,
            diagnostics,
            capability_support,
        })
    }

    fn provider_manifest_metadata_token() -> usize {
        metadata::metadata_vocabulary_weight()
            + Self::provider_manifests()
                .iter()
                .map(provider_manifest_metadata_weight)
                .sum::<usize>()
    }
}

fn provider_manifest_metadata_weight(manifest: &ProviderManifest) -> usize {
    manifest.id.len()
        + provider_kind_weight(manifest.kind)
        + language_scope_weight(manifest.language_scope)
        + cache_policy_weight(manifest.cache_policy)
        + precision_ceiling_weight(manifest.precision_ceiling)
        + manifest
            .inputs
            .iter()
            .map(|input| input.len())
            .sum::<usize>()
        + manifest
            .outputs
            .iter()
            .map(|output| output.len())
            .sum::<usize>()
        + manifest
            .schema_versions
            .iter()
            .map(schema_version_weight)
            .sum::<usize>()
}

fn provider_kind_weight(kind: ProviderKind) -> usize {
    match kind {
        ProviderKind::SourceDiscovery => 1,
        ProviderKind::LanguageSyntax => 2,
        ProviderKind::WholeRepoDerived => 3,
        ProviderKind::MetricsDerived => 4,
    }
}

fn language_scope_weight(scope: LanguageScope) -> usize {
    match scope {
        LanguageScope::Workspace => 1,
        LanguageScope::Go => 2,
        LanguageScope::TypeScriptJavaScript => 3,
        LanguageScope::MultiLanguage => 4,
    }
}

fn cache_policy_weight(policy: CachePolicy) -> usize {
    match policy {
        CachePolicy::NoCache => 1,
        CachePolicy::ExistingFileFactCache { schema } => schema.len(),
        CachePolicy::InMemoryDerived => 2,
    }
}

fn precision_ceiling_weight(precision: PrecisionCeiling) -> usize {
    match precision {
        PrecisionCeiling::Exact => 1,
        PrecisionCeiling::Syntax => 2,
        PrecisionCeiling::SetupAware => 3,
    }
}

fn schema_version_weight(schema: &SchemaVersion) -> usize {
    schema.name.len() + schema.version as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_config;

    #[test]
    fn run_with_empty_plan_returns_empty_db_and_plan_support() {
        let temp = tempfile::tempdir().expect("tempdir");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::empty();

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run on an empty repo");

        assert!(output.db.files().is_empty());
        assert!(output.diagnostics.is_empty());
        assert_eq!(&output.capability_support, plan.support_view());
    }

    #[test]
    fn run_propagates_file_loading_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut loaded = load_config(temp.path()).expect("default config loads");
        loaded.config.workspace.include = vec!["[".to_string()];
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::empty();

        let result = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        });
        let Err(error) = result else {
            panic!("kernel should propagate load_analysis_files errors");
        };

        assert!(
            error.to_string().contains("invalid glob"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn provider_manifests_cover_existing_kernel_providers() {
        let ids = AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| manifest.id)
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            [
                "polint.source",
                "polint.go.syntax",
                "polint.ts.syntax",
                "polint.module_graph",
                "polint.symbol_graph",
                "polint.metrics",
            ]
        );
    }
}
