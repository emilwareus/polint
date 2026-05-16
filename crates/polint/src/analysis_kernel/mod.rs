use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, CapabilitySupportView};
use crate::diagnostics::Diagnostic;

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
    pub(crate) fn run(input: KernelInput<'_>) -> anyhow::Result<KernelOutput> {
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
}
