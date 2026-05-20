#[cfg(test)]
mod semantic_mir_provider {
    use crate::analysis_kernel::incremental::{Digest, DigestKind};
    use crate::analysis_kernel::{AnalysisKernel, KernelInput};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;

    #[test]
    fn derivation_stores_non_empty_mixed_language_mir() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("main.go"),
            "package main\nfunc add(left int, right int) int { total := left + right; return total }\n",
        )
        .expect("write go");
        std::fs::write(
            temp.path().join("app.ts"),
            "export function render(value: number) { const next = value + 1; return next; }\n",
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");

        assert!(
            output.db.mir_bodies().len() >= 2,
            "expected Go and TS MIR bodies"
        );
        assert!(
            output.db.mir_places().len() >= 2,
            "expected lowered places"
        );
        assert!(
            output.db.mir_operations().len() >= 2,
            "expected lowered operations"
        );
    }

    #[test]
    fn derivation_output_digest_changes_with_upstream_inputs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let loaded = load_config(temp.path()).expect("default config loads");
        let mut db = crate::core::AnalysisDb::new();
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.semantic_mir")
            .expect("semantic MIR manifest");
        let input_snapshot = crate::analysis_kernel::incremental::InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config",
            "rules",
            "plan",
            AnalysisKernel::provider_manifests(),
        );
        let module_topology = Digest::from_parts(
            DigestKind::ProviderOutput,
            "module_topology",
            &["topology-a"],
        );
        let symbol_graph =
            Digest::from_parts(DigestKind::ProviderOutput, "symbol_graph", &["symbols-a"]);
        let syntax_a = vec![Digest::from_parts(
            DigestKind::ProviderOutput,
            "syntax",
            &["go-a"],
        )];
        let syntax_b = vec![Digest::from_parts(
            DigestKind::ProviderOutput,
            "syntax",
            &["go-b"],
        )];

        let first = super::derive_semantic_mir_with_cache_stats(
            &mut db,
            &input_snapshot,
            manifest,
            module_topology.clone(),
            symbol_graph.clone(),
            syntax_a,
        );
        let second = super::derive_semantic_mir_with_cache_stats(
            &mut db,
            &input_snapshot,
            manifest,
            module_topology,
            symbol_graph,
            syntax_b,
        );

        assert_ne!(first.output_digest, second.output_digest);
    }
}
