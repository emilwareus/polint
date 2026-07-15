use super::*;

use crate::analysis_kernel::incremental::{
    DemandCacheStatus, DemandQueryTrace, DemandQueryTraceEntry, Digest, DigestKind,
    InputComponentStatus, InputDependencyKey, PrecisionTier, QueryDependencyInputs,
    ValidatedRunMetadata, dependency_free_test_query_key,
};
use crate::analysis_kernel::{AnalysisKernel, KernelInput};
use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::config::load_config;

struct GenerationFinalizedFixture {
    report: crate::analysis_kernel::incremental::KernelRunReport,
    manifests: Vec<crate::analysis_kernel::ProviderManifest>,
    facts: Vec<crate::analysis_kernel::StableFactMetaRow>,
}

mod metadata_invalidation_matrix {
    use super::*;
    use crate::analysis_kernel::incremental::{
        CacheNode, ChangeKind, ChangeSet, ChangeSetRow, DependencyEdge, DependencyKind,
        DiagnosticKey, InputComponentStatus, InputDependencyKey, InputDependencyKind,
        InvalidationAction, InvalidationPlan, ShapeKind,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ExpectedAction {
        Verify,
        Recompute,
        Drop,
        Quarantine,
    }

    #[derive(Clone, Debug)]
    struct MatrixCase {
        label: &'static str,
        input: InputDependencyKey,
        kind: ChangeKind,
        target: CacheNode,
        sibling: CacheNode,
        expected: ExpectedAction,
    }

    #[derive(Clone, Debug)]
    struct MatrixTopology {
        cases: Vec<MatrixCase>,
        statuses: std::collections::BTreeSet<InputComponentStatus>,
    }

    fn rotate<T>(rows: &mut [T], seed: usize) {
        if !rows.is_empty() {
            rows.rotate_left(seed % rows.len());
        }
    }

    fn permuted_validated_run(
        fixture: &GenerationFinalizedFixture,
        seed: usize,
        mutate_telemetry: bool,
    ) -> (ValidatedRunMetadata, MatrixTopology) {
        let mut report = fixture.report.clone();
        let mut manifests = fixture.manifests.clone();
        let mut facts = fixture.facts.clone();

        rotate(&mut report.input_snapshot.files, seed);
        rotate(&mut report.input_snapshot.analysis_settings, seed + 1);
        rotate(&mut report.input_snapshot.requested_capabilities, seed + 2);
        rotate(&mut report.input_snapshot.rules, seed + 3);
        rotate(&mut report.input_snapshot.models, seed + 4);
        rotate(&mut report.input_snapshot.extensions, seed + 5);
        rotate(&mut report.input_snapshot.tool_invocations, seed + 6);
        rotate(&mut report.input_snapshot.provider_schemas, seed + 7);
        rotate(&mut report.input_snapshot.go_lifecycle.components, seed + 8);
        rotate(
            &mut report.input_snapshot.ts_js_lifecycle.components,
            seed + 9,
        );
        rotate(&mut report.provider_outputs, seed + 10);
        rotate(&mut manifests, seed + 11);
        rotate(&mut facts, seed + 12);
        let entries = report.demand_query_trace.entries().to_vec();
        let mut trace = DemandQueryTrace::default();
        for entry in entries
            .iter()
            .cycle()
            .skip(seed % entries.len())
            .take(entries.len())
        {
            let mut entry = entry.clone();
            if mutate_telemetry {
                entry.cache_status = match entry.cache_status {
                    DemandCacheStatus::Computed => DemandCacheStatus::Hit,
                    DemandCacheStatus::Hit | DemandCacheStatus::Miss => DemandCacheStatus::Computed,
                };
                entry.compute_duration_micros = entry.compute_duration_micros.saturating_add(8192);
            }
            trace.record_entry(entry);
        }
        report.demand_query_trace = trace;
        if mutate_telemetry {
            report.cache_stats.hits = report.cache_stats.hits.saturating_add(31);
            for provider in &mut report.provider_outputs {
                provider.cache_stats.misses = provider.cache_stats.misses.saturating_add(17);
                provider.cache_stats.writes = provider.cache_stats.writes.saturating_add(23);
            }
            for file in &mut report.input_snapshot.files {
                file.mtime_hint_present = !file.mtime_hint_present;
            }
        }

        let base = ValidatedRunMetadata::from_finalized_run(
            &report.input_snapshot,
            &report.provider_outputs,
            &report.demand_query_trace,
            report.validation_events(),
            &manifests,
            facts,
        )
        .expect("permuted run remains a valid store handoff");
        attach_matrix_dependencies(base, seed)
    }

    fn attach_matrix_dependencies(
        base: ValidatedRunMetadata,
        seed: usize,
    ) -> (ValidatedRunMetadata, MatrixTopology) {
        let queries = base
            .query_rows()
            .iter()
            .map(|row| row.query_key().clone())
            .collect::<Vec<_>>();
        let target_query = queries.first().cloned().expect("matrix target query");
        let sibling_query = queries.get(1).cloned().expect("matrix sibling query");
        let target_query_node = CacheNode::Query(target_query.clone().into());
        let sibling_query_node = CacheNode::Query(sibling_query.into());

        let target_rule_code = typed_input(
            InputDependencyKind::RuleCode,
            "rules/matrix-target:code",
            DigestKind::RuleCode,
            InputComponentStatus::Present,
        );
        let target_rule_options = typed_input(
            InputDependencyKind::RuleOptions,
            "rules/matrix-target:options",
            DigestKind::RuleOptions,
            InputComponentStatus::SetupMissing,
        );
        let sibling_rule_code = typed_input(
            InputDependencyKind::RuleCode,
            "rules/matrix-sibling:code",
            DigestKind::RuleCode,
            InputComponentStatus::Present,
        );
        let sibling_rule_options = typed_input(
            InputDependencyKind::RuleOptions,
            "rules/matrix-sibling:options",
            DigestKind::RuleOptions,
            InputComponentStatus::Present,
        );
        let target_diagnostic = DiagnosticKey::new(
            "matrix.target",
            "1",
            target_rule_code.digest.clone(),
            target_rule_options.digest.clone(),
            vec![Digest::from_parts(
                DigestKind::ProviderOutput,
                "matrix_requested_view",
                &["target"],
            )],
            Digest::from_parts(DigestKind::Evidence, "matrix_evidence", &["target"]),
        );
        let sibling_diagnostic = DiagnosticKey::new(
            "matrix.sibling",
            "1",
            sibling_rule_code.digest.clone(),
            sibling_rule_options.digest.clone(),
            vec![Digest::from_parts(
                DigestKind::ProviderOutput,
                "matrix_requested_view",
                &["sibling"],
            )],
            Digest::from_parts(DigestKind::Evidence, "matrix_evidence", &["sibling"]),
        );
        let target_diagnostic_node = CacheNode::Diagnostic(target_diagnostic.clone());
        let sibling_diagnostic_node = CacheNode::Diagnostic(sibling_diagnostic.clone());

        let mut declared_edges = vec![
            edge(
                target_diagnostic_node.clone(),
                target_rule_code.clone(),
                DependencyKind::Rule,
                ShapeKind::RuleCode,
            ),
            edge(
                target_diagnostic_node.clone(),
                target_rule_options.clone(),
                DependencyKind::Rule,
                ShapeKind::RuleOptions,
            ),
            edge(
                sibling_diagnostic_node.clone(),
                sibling_rule_code,
                DependencyKind::Rule,
                ShapeKind::RuleCode,
            ),
            edge(
                sibling_diagnostic_node.clone(),
                sibling_rule_options,
                DependencyKind::Rule,
                ShapeKind::RuleOptions,
            ),
        ];
        let mut cases = Vec::new();
        let status_cycle = [
            InputComponentStatus::Present,
            InputComponentStatus::Absent,
            InputComponentStatus::Unsupported,
            InputComponentStatus::SetupMissing,
        ];
        let specifications = [
            (
                "source_file",
                InputDependencyKind::SourceFile,
                DigestKind::SourceText,
                ChangeKind::ContentOnly,
                DependencyKind::SourceText,
                ShapeKind::Content,
                ExpectedAction::Verify,
            ),
            (
                "package_project",
                InputDependencyKind::PackageProject,
                DigestKind::Workspace,
                ChangeKind::ModuleTopology,
                DependencyKind::Input,
                ShapeKind::ModuleTopology,
                ExpectedAction::Recompute,
            ),
            (
                "provider_manifest_version",
                InputDependencyKind::ProviderManifest,
                DigestKind::ProviderManifest,
                ChangeKind::ProviderVersion,
                DependencyKind::Provider,
                ShapeKind::ProviderVersion,
                ExpectedAction::Drop,
            ),
            (
                "provider_schema",
                InputDependencyKind::ProviderSchema,
                DigestKind::ProviderManifest,
                ChangeKind::ProviderVersion,
                DependencyKind::ProviderSchema,
                ShapeKind::ProviderVersion,
                ExpectedAction::Drop,
            ),
            (
                "requested_capability",
                InputDependencyKind::RequestedCapability,
                DigestKind::AnalysisRequirements,
                ChangeKind::Unknown,
                DependencyKind::Input,
                ShapeKind::Unknown,
                ExpectedAction::Recompute,
            ),
            (
                "analysis_setting",
                InputDependencyKind::AnalysisSetting,
                DigestKind::AnalysisSettings,
                ChangeKind::Unknown,
                DependencyKind::Config,
                ShapeKind::Unknown,
                ExpectedAction::Recompute,
            ),
            (
                "go_lifecycle",
                InputDependencyKind::LanguageLifecycle,
                DigestKind::GoLifecycle,
                ChangeKind::Lifecycle,
                DependencyKind::Lifecycle,
                ShapeKind::Lifecycle,
                ExpectedAction::Recompute,
            ),
            (
                "go_tool",
                InputDependencyKind::ToolInvocation,
                DigestKind::ToolInvocation,
                ChangeKind::Toolchain,
                DependencyKind::ToolInvocation,
                ShapeKind::Toolchain,
                ExpectedAction::Recompute,
            ),
            (
                "ts_js_lifecycle",
                InputDependencyKind::LanguageLifecycle,
                DigestKind::TsJsLifecycle,
                ChangeKind::Lifecycle,
                DependencyKind::Lifecycle,
                ShapeKind::Lifecycle,
                ExpectedAction::Recompute,
            ),
            (
                "ts_js_tool",
                InputDependencyKind::ToolInvocation,
                DigestKind::ToolInvocation,
                ChangeKind::Toolchain,
                DependencyKind::ToolInvocation,
                ShapeKind::Toolchain,
                ExpectedAction::Recompute,
            ),
            (
                "upstream_layer",
                InputDependencyKind::UpstreamLayer,
                DigestKind::DependencyLayer,
                ChangeKind::PublicApiShape,
                DependencyKind::UpstreamLayer,
                ShapeKind::PublicApi,
                ExpectedAction::Recompute,
            ),
            (
                "summary_dependency",
                InputDependencyKind::SummaryDependency,
                DigestKind::SummaryDependency,
                ChangeKind::PublicApiShape,
                DependencyKind::UpstreamLayer,
                ShapeKind::PublicApi,
                ExpectedAction::Recompute,
            ),
            (
                "reserved_search_manifest",
                InputDependencyKind::SearchManifest,
                DigestKind::Dependency,
                ChangeKind::Unknown,
                DependencyKind::Input,
                ShapeKind::Unknown,
                ExpectedAction::Recompute,
            ),
            (
                "extension_code",
                InputDependencyKind::ExtensionCode,
                DigestKind::ExtensionCode,
                ChangeKind::ExtensionCode,
                DependencyKind::Extension,
                ShapeKind::ExtensionCode,
                ExpectedAction::Quarantine,
            ),
            (
                "extension_declared_input",
                InputDependencyKind::ExtensionDeclaredInput,
                DigestKind::ExtensionCode,
                ChangeKind::ExtensionDeclaredInput,
                DependencyKind::Extension,
                ShapeKind::ExtensionDeclaredInput,
                ExpectedAction::Quarantine,
            ),
            (
                "model_digest",
                InputDependencyKind::Model,
                DigestKind::ModelFile,
                ChangeKind::ModelFile,
                DependencyKind::Model,
                ShapeKind::Model,
                ExpectedAction::Recompute,
            ),
        ];
        for (
            index,
            (label, input_kind, digest_kind, change_kind, dependency_kind, shape, expected),
        ) in specifications.into_iter().enumerate()
        {
            let input = typed_input(
                input_kind,
                &format!("matrix/{label}"),
                digest_kind,
                status_cycle[index % status_cycle.len()],
            );
            declared_edges.push(edge(
                target_query_node.clone(),
                input.clone(),
                dependency_kind,
                shape,
            ));
            cases.push(MatrixCase {
                label,
                input,
                kind: change_kind,
                target: target_query_node.clone(),
                sibling: sibling_query_node.clone(),
                expected,
            });
        }

        cases.push(MatrixCase {
            label: "rule_code",
            input: target_rule_code,
            kind: ChangeKind::RuleCode,
            target: target_diagnostic_node.clone(),
            sibling: sibling_diagnostic_node.clone(),
            expected: ExpectedAction::Recompute,
        });
        cases.push(MatrixCase {
            label: "rule_options",
            input: target_rule_options,
            kind: ChangeKind::RuleOptions,
            target: target_diagnostic_node,
            sibling: sibling_diagnostic_node,
            expected: ExpectedAction::Recompute,
        });

        let config_edge = base
            .dependency_index()
            .forward_edges(&CacheNode::RunManifest(base.run_manifest_key().clone()))
            .and_then(|edges| {
                edges.iter().find_map(|edge| match &edge.to {
                    CacheNode::DependencyInput(input)
                        if input.kind == InputDependencyKind::Config =>
                    {
                        Some(input.clone())
                    }
                    _ => None,
                })
            })
            .expect("run manifest declares the complete config input");
        cases.push(MatrixCase {
            label: "full_config",
            input: config_edge,
            kind: ChangeKind::Unknown,
            target: CacheNode::RunManifest(base.run_manifest_key().clone()),
            sibling: sibling_query_node.clone(),
            expected: ExpectedAction::Recompute,
        });

        let query_edges = base
            .dependency_index()
            .forward_edges(&target_query_node)
            .expect("query has exact declared inputs");
        for (label, kind) in [
            ("query_parameter_option", InputDependencyKind::QueryOption),
            ("budget_profile", InputDependencyKind::BudgetProfile),
        ] {
            let input = query_edges
                .iter()
                .find_map(|edge| match &edge.to {
                    CacheNode::DependencyInput(input) if input.kind == kind => Some(input.clone()),
                    _ => None,
                })
                .expect("query boundary input");
            cases.push(MatrixCase {
                label,
                input,
                kind: ChangeKind::Unknown,
                target: target_query_node.clone(),
                sibling: sibling_query_node.clone(),
                expected: ExpectedAction::Recompute,
            });
        }
        let exact_query_input = target_query
            .dependency_inputs
            .as_slice()
            .first()
            .cloned()
            .expect("query declares a typed dependency input");
        cases.push(MatrixCase {
            label: "exact_query_dependency_inputs",
            input: exact_query_input,
            kind: ChangeKind::Unknown,
            target: target_query_node,
            sibling: sibling_query_node,
            expected: ExpectedAction::Recompute,
        });

        let edge_count = declared_edges.len();
        declared_edges.rotate_left(seed % edge_count);
        let validated = base
            .with_dependency_fixture(vec![sibling_diagnostic, target_diagnostic], declared_edges)
            .expect("matrix dependency fixture is canonical and complete");
        let statuses = cases.iter().map(|case| case.input.status).collect();
        (validated, MatrixTopology { cases, statuses })
    }

    fn typed_input(
        kind: InputDependencyKind,
        stable_key: &str,
        digest_kind: DigestKind,
        status: InputComponentStatus,
    ) -> InputDependencyKey {
        let digest = Digest::from_parts(digest_kind, "matrix_input", &[stable_key]);
        let result = match kind {
            InputDependencyKind::SourceFile => {
                InputDependencyKey::source_file(stable_key, digest, status)
            }
            InputDependencyKind::PackageProject => {
                InputDependencyKey::package_project(stable_key, digest, status)
            }
            InputDependencyKind::ProviderManifest => {
                InputDependencyKey::provider_manifest(stable_key, digest, status)
            }
            InputDependencyKind::ProviderSchema => {
                InputDependencyKey::provider_schema(stable_key, digest, status)
            }
            InputDependencyKind::RequestedCapability => {
                InputDependencyKey::requested_capability(stable_key, digest, status)
            }
            InputDependencyKind::AnalysisSetting => {
                InputDependencyKey::analysis_setting(stable_key, digest, status)
            }
            InputDependencyKind::LanguageLifecycle => {
                InputDependencyKey::language_lifecycle(stable_key, digest, status)
            }
            InputDependencyKind::ToolInvocation => {
                InputDependencyKey::tool_invocation(stable_key, digest, status)
            }
            InputDependencyKind::Config => InputDependencyKey::config(stable_key, digest, status),
            InputDependencyKind::UpstreamLayer => {
                InputDependencyKey::upstream_layer(stable_key, digest, status)
            }
            InputDependencyKind::SummaryDependency => {
                InputDependencyKey::summary_dependency(stable_key, digest, status)
            }
            InputDependencyKind::QueryOption => {
                InputDependencyKey::query_option(stable_key, digest, status)
            }
            InputDependencyKind::BudgetProfile => {
                InputDependencyKey::budget_profile(stable_key, digest, status)
            }
            InputDependencyKind::SearchManifest => {
                InputDependencyKey::search_manifest(stable_key, digest, status)
            }
            InputDependencyKind::ExtensionCode => {
                InputDependencyKey::extension_code(stable_key, digest, status)
            }
            InputDependencyKind::ExtensionDeclaredInput => {
                InputDependencyKey::extension_declared_input(stable_key, digest, status)
            }
            InputDependencyKind::Model => InputDependencyKey::model(stable_key, digest, status),
            InputDependencyKind::RuleCode => {
                InputDependencyKey::rule_code(stable_key, digest, status)
            }
            InputDependencyKind::RuleOptions => {
                InputDependencyKey::rule_options(stable_key, digest, status)
            }
        };
        result.expect("matrix digest kind matches typed input")
    }

    fn edge(
        from: CacheNode,
        input: InputDependencyKey,
        kind: DependencyKind,
        required_shape: ShapeKind,
    ) -> DependencyEdge {
        DependencyEdge {
            from,
            to: CacheNode::DependencyInput(input),
            kind,
            required_shape,
        }
    }

    fn commit_and_reopen(
        root: &Path,
        validated: &ValidatedRunMetadata,
    ) -> generation::ActiveCompleteGeneration {
        let config = generation_store_config(root);
        let outcome = SemanticStore::commit_validated_run(&config, validated.clone());
        assert_eq!(outcome.status, StoreStatus::Ready);
        let statistics = outcome
            .statistics
            .expect("ready commit returns private statistics");
        assert!(statistics.planned_semantic_row_count > 0);
        assert!(statistics.semantic_logical_bytes > 0);
        assert_eq!(
            statistics.input_digest,
            *validated.identities().input_snapshot()
        );
        assert_eq!(
            statistics.dependency_digest,
            *validated.identities().dependency()
        );
        assert_eq!(
            statistics.validation_digest,
            *validated.identities().validation()
        );
        generation::read_active_complete(&config, validated.identities().workspace())
            .expect("active read succeeds")
            .expect("committed generation is active")
    }

    fn plans_for_cases(
        index: &crate::analysis_kernel::incremental::DependencyIndex,
        topology: &MatrixTopology,
    ) -> Vec<InvalidationPlan> {
        let layers = index
            .all_nodes()
            .into_iter()
            .filter(|node| matches!(node, CacheNode::Layer(_)))
            .collect::<Vec<_>>();
        assert!(!layers.is_empty());
        let unchanged = InvalidationPlan::from_change_set(index, &ChangeSet::from_rows(Vec::new()));
        assert!(
            unchanged
                .actions
                .iter()
                .all(|action| matches!(action, InvalidationAction::Reuse(_)))
        );
        let mut plans = vec![unchanged];
        for case in &topology.cases {
            let plan = InvalidationPlan::from_change_set(
                index,
                &ChangeSet::from_rows(vec![ChangeSetRow {
                    node: CacheNode::DependencyInput(case.input.clone()),
                    kind: case.kind,
                    digest: case.input.digest.clone(),
                }]),
            );
            let target_action = plan
                .action_for(&case.target)
                .unwrap_or_else(|| panic!("{} target has an action", case.label));
            assert!(
                matches!(
                    (case.expected, target_action),
                    (ExpectedAction::Verify, InvalidationAction::Verify(_, _))
                        | (
                            ExpectedAction::Recompute,
                            InvalidationAction::Recompute(_, _)
                        )
                        | (ExpectedAction::Drop, InvalidationAction::Drop(_, _))
                        | (
                            ExpectedAction::Quarantine,
                            InvalidationAction::Quarantine(_, _)
                        )
                ),
                "{} has its exact invalidation class: {target_action:?}",
                case.label
            );
            assert!(
                matches!(
                    plan.action_for(&case.sibling),
                    Some(InvalidationAction::Reuse(_))
                ),
                "{} preserves its unreferenced sibling",
                case.label
            );
            if matches!(case.label, "rule_code" | "rule_options" | "full_config") {
                assert!(
                    layers.iter().all(|layer| matches!(
                        plan.action_for(layer),
                        Some(InvalidationAction::Reuse(_))
                    )),
                    "{} preserves every unrelated analysis layer",
                    case.label
                );
            }
            plans.push(plan);
        }
        plans
    }

    fn transition_run(
        base: &ValidatedRunMetadata,
        stable_key: &str,
        status: InputComponentStatus,
    ) -> (
        ValidatedRunMetadata,
        CacheNode,
        CacheNode,
        InputDependencyKey,
    ) {
        let queries = base
            .query_rows()
            .iter()
            .map(|row| row.query_key().clone())
            .collect::<Vec<_>>();
        let target = CacheNode::Query(
            queries
                .first()
                .cloned()
                .expect("transition target query")
                .into(),
        );
        let sibling = CacheNode::Query(
            queries
                .get(1)
                .cloned()
                .expect("transition sibling query")
                .into(),
        );
        let input = typed_input(
            InputDependencyKind::AnalysisSetting,
            stable_key,
            DigestKind::AnalysisSettings,
            status,
        );
        let validated = base
            .clone()
            .with_dependency_fixture(
                Vec::new(),
                vec![edge(
                    target.clone(),
                    input.clone(),
                    DependencyKind::Config,
                    ShapeKind::Unknown,
                )],
            )
            .expect("status transition fixture remains canonical");
        (validated, target, sibling, input)
    }

    fn status_transition_change(
        prior: &InputDependencyKey,
        next: &InputDependencyKey,
    ) -> ChangeSetRow {
        assert_eq!(prior.kind, next.kind);
        assert_eq!(prior.stable_key, next.stable_key);
        assert_eq!(prior.digest, next.digest);
        assert_ne!(prior.status, next.status);
        ChangeSetRow {
            node: CacheNode::DependencyInput(prior.clone()),
            kind: ChangeKind::Unknown,
            digest: prior.digest.clone(),
        }
    }

    #[test]
    fn persisted_referenced_inputs_invalidate_and_unreferenced_siblings_reuse() {
        let temp = tempfile::tempdir().expect("temporary matrix root");
        let fixture = generation_finalized_fixture(&temp.path().join("repo"), "matrix");
        let (validated, topology) = permuted_validated_run(&fixture, 0, false);
        assert_eq!(
            topology.statuses,
            std::collections::BTreeSet::from([
                InputComponentStatus::Present,
                InputComponentStatus::Absent,
                InputComponentStatus::Unsupported,
                InputComponentStatus::SetupMissing,
            ])
        );
        let active = commit_and_reopen(&temp.path().join("baseline-store"), &validated);
        assert_eq!(active.dependency_index, *validated.dependency_index());
        let plans = plans_for_cases(&active.dependency_index, &topology);
        assert_eq!(plans.len(), topology.cases.len() + 1);
    }

    #[test]
    fn persisted_status_transitions_invalidate_only_the_linked_query() {
        let temp = tempfile::tempdir().expect("temporary transition root");
        let base = generation_validated_fixture(&temp.path().join("repo"), "transitions");
        let transitions = [
            (
                "present-to-absent",
                InputComponentStatus::Present,
                InputComponentStatus::Absent,
            ),
            (
                "absent-to-present",
                InputComponentStatus::Absent,
                InputComponentStatus::Present,
            ),
            (
                "unsupported-to-setup-missing",
                InputComponentStatus::Unsupported,
                InputComponentStatus::SetupMissing,
            ),
            (
                "setup-missing-to-unsupported",
                InputComponentStatus::SetupMissing,
                InputComponentStatus::Unsupported,
            ),
        ];

        for (label, prior_status, next_status) in transitions {
            let stable_key = format!("matrix/status-transition/{label}");
            let (prior, target, sibling, prior_input) =
                transition_run(&base, &stable_key, prior_status);
            let (next, next_target, next_sibling, next_input) =
                transition_run(&base, &stable_key, next_status);
            assert_eq!(target, next_target);
            assert_eq!(sibling, next_sibling);
            assert_eq!(prior_input.stable_key, next_input.stable_key);
            assert_eq!(prior_input.digest, next_input.digest);
            assert_eq!(prior_input.status, prior_status);
            assert_eq!(next_input.status, next_status);

            let prior_active =
                commit_and_reopen(&temp.path().join(format!("{label}-prior")), &prior);
            let next_active = commit_and_reopen(&temp.path().join(format!("{label}-next")), &next);
            assert_ne!(
                prior_active.plan.semantic.identities, next_active.plan.semantic.identities,
                "{label} changes persisted semantic identity"
            );
            assert_ne!(
                prior_active.dependency_index, next_active.dependency_index,
                "{label} changes the persisted typed dependency index"
            );
            for (active, expected_input) in
                [(&prior_active, &prior_input), (&next_active, &next_input)]
            {
                assert!(
                    active
                        .dependency_index
                        .forward_edges(&target)
                        .expect("transition target has persisted edges")
                        .iter()
                        .any(|edge| {
                            edge.to == CacheNode::DependencyInput(expected_input.clone())
                        }),
                    "{label} preserves the exact typed status"
                );
            }

            let unchanged = InvalidationPlan::from_change_set(
                &prior_active.dependency_index,
                &ChangeSet::from_rows(Vec::new()),
            );
            assert!(
                unchanged
                    .actions
                    .iter()
                    .all(|action| matches!(action, InvalidationAction::Reuse(_))),
                "{label} reuses every node when nothing changes"
            );
            let changed = InvalidationPlan::from_change_set(
                &prior_active.dependency_index,
                &ChangeSet::from_rows(vec![status_transition_change(&prior_input, &next_input)]),
            );
            assert!(
                matches!(
                    changed.action_for(&target),
                    Some(InvalidationAction::Recompute(_, _))
                ),
                "{label} invalidates its linked query"
            );
            assert!(
                matches!(
                    changed.action_for(&sibling),
                    Some(InvalidationAction::Reuse(_))
                ),
                "{label} preserves the unlinked sibling"
            );
        }
    }

    #[test]
    fn twenty_persisted_permutations_preserve_rows_maps_digests_and_actions() {
        let temp = tempfile::tempdir().expect("temporary permutation root");
        let fixture = generation_finalized_fixture(&temp.path().join("repo"), "permutations");
        let (baseline_validated, baseline_topology) = permuted_validated_run(&fixture, 0, false);
        let baseline_active = commit_and_reopen(&temp.path().join("store-0"), &baseline_validated);
        let baseline_actions =
            plans_for_cases(&baseline_active.dependency_index, &baseline_topology);

        for seed in 1..20 {
            let (validated, topology) = permuted_validated_run(&fixture, seed, false);
            let active = commit_and_reopen(&temp.path().join(format!("store-{seed}")), &validated);
            assert_eq!(
                active.plan.semantic, baseline_active.plan.semantic,
                "rows {seed}"
            );
            assert_eq!(
                active.dependency_index, baseline_active.dependency_index,
                "maps {seed}"
            );
            assert_eq!(
                active.plan.semantic.identities, baseline_active.plan.semantic.identities,
                "semantic digests {seed}"
            );
            assert_eq!(
                plans_for_cases(&active.dependency_index, &topology),
                baseline_actions,
                "actions {seed}"
            );
        }
    }

    #[test]
    fn persisted_telemetry_mutations_are_semantically_neutral() {
        let temp = tempfile::tempdir().expect("temporary telemetry root");
        let fixture = generation_finalized_fixture(&temp.path().join("repo"), "telemetry");
        let (baseline_validated, baseline_topology) = permuted_validated_run(&fixture, 0, false);
        let (telemetry_validated, telemetry_topology) = permuted_validated_run(&fixture, 7, true);
        let baseline = commit_and_reopen(&temp.path().join("baseline"), &baseline_validated);
        let telemetry = commit_and_reopen(&temp.path().join("telemetry"), &telemetry_validated);

        assert_eq!(telemetry.plan.semantic, baseline.plan.semantic);
        assert_ne!(telemetry.plan.telemetry, baseline.plan.telemetry);
        let baseline_identities = &baseline.plan.semantic.identities;
        let telemetry_identities = &telemetry.plan.semantic.identities;
        assert_eq!(
            telemetry_identities.provider_output,
            baseline_identities.provider_output
        );
        assert_eq!(telemetry_identities.layer, baseline_identities.layer);
        assert_eq!(telemetry_identities.query, baseline_identities.query);
        assert_eq!(telemetry_identities.fact, baseline_identities.fact);
        assert_eq!(
            telemetry_identities.dependency,
            baseline_identities.dependency
        );
        assert_eq!(
            telemetry_identities.validation,
            baseline_identities.validation
        );
        assert_eq!(telemetry_identities.run, baseline_identities.run);
        assert_eq!(
            telemetry_identities.generation,
            baseline_identities.generation
        );

        let baseline_stats = &baseline.plan.semantic.stats;
        let telemetry_stats = &telemetry.plan.semantic.stats;
        assert_eq!(
            telemetry_stats.provider_output_digest,
            baseline_stats.provider_output_digest
        );
        assert_eq!(telemetry_stats.layer_digest, baseline_stats.layer_digest);
        assert_eq!(telemetry_stats.query_digest, baseline_stats.query_digest);
        assert_eq!(telemetry_stats.fact_digest, baseline_stats.fact_digest);
        assert_eq!(
            telemetry_stats.dependency_digest,
            baseline_stats.dependency_digest
        );
        assert_eq!(
            telemetry_stats.validation_digest,
            baseline_stats.validation_digest
        );
        assert_eq!(telemetry.dependency_index, baseline.dependency_index);
        assert_eq!(
            plans_for_cases(&telemetry.dependency_index, &telemetry_topology),
            plans_for_cases(&baseline.dependency_index, &baseline_topology)
        );
    }

    #[test]
    fn diagnostics_that_differ_only_by_requested_views_round_trip_independently() {
        let temp = tempfile::tempdir().expect("temporary diagnostic identity root");
        let base = generation_validated_fixture(&temp.path().join("repo"), "diagnostics");
        let rule_code = typed_input(
            InputDependencyKind::RuleCode,
            "rules/matrix-same-rule:code",
            DigestKind::RuleCode,
            InputComponentStatus::Present,
        );
        let rule_options = typed_input(
            InputDependencyKind::RuleOptions,
            "rules/matrix-same-rule:options",
            DigestKind::RuleOptions,
            InputComponentStatus::Present,
        );
        let diagnostic = |view: &str| {
            DiagnosticKey::new(
                "matrix.same-rule",
                "1",
                rule_code.digest.clone(),
                rule_options.digest.clone(),
                vec![Digest::from_parts(
                    DigestKind::ProviderOutput,
                    "requested_view",
                    &[view],
                )],
                Digest::from_parts(DigestKind::Evidence, "same_rule", &["evidence"]),
            )
        };
        let first = diagnostic("first");
        let second = diagnostic("second");
        assert_ne!(
            first.requested_views_digest(),
            second.requested_views_digest()
        );
        let mut declared_edges = Vec::new();
        for key in [&first, &second] {
            let node = CacheNode::Diagnostic(key.clone());
            declared_edges.push(edge(
                node.clone(),
                rule_code.clone(),
                DependencyKind::Rule,
                ShapeKind::RuleCode,
            ));
            declared_edges.push(edge(
                node,
                rule_options.clone(),
                DependencyKind::Rule,
                ShapeKind::RuleOptions,
            ));
        }
        let validated = base
            .with_dependency_fixture(vec![second.clone(), first.clone()], declared_edges)
            .expect("distinct requested-view identities remain valid");

        let active = commit_and_reopen(&temp.path().join("store"), &validated);

        assert_eq!(active.dependency_index, *validated.dependency_index());
        assert_eq!(active.plan.semantic.diagnostics.len(), 2);
        assert!(
            active
                .plan
                .semantic
                .diagnostics
                .iter()
                .any(|row| row.key == first)
        );
        assert!(
            active
                .plan
                .semantic
                .diagnostics
                .iter()
                .any(|row| row.key == second)
        );
        assert_eq!(active.plan.semantic.diagnostic_requested_views.len(), 2);
        for key in [first, second] {
            let node = CacheNode::Diagnostic(key);
            assert!(active.dependency_index.contains_node(&node));
            assert_eq!(
                active
                    .dependency_index
                    .forward_edges(&node)
                    .expect("diagnostic has reconstructed rule dependencies")
                    .len(),
                2
            );
        }
    }
}

fn generation_query_trace_entry(label: &str) -> DemandQueryTraceEntry {
    let dependency = InputDependencyKey::analysis_setting(
        format!("polint.{label}"),
        Digest::from_parts(DigestKind::AnalysisSettings, label, &[label]),
        InputComponentStatus::Present,
    )
    .expect("query fixture uses an analysis-settings digest");
    let mut query_key = dependency_free_test_query_key(
        format!("query.{label}"),
        "1",
        Digest::from_parts(DigestKind::QueryParameters, label, &[label]),
        Digest::from_parts(DigestKind::Budget, label, &["bounded"]),
        PrecisionTier::SetupAware,
    );
    query_key.dependency_inputs = QueryDependencyInputs::new(vec![dependency]);
    query_key.layer_digests = vec![Digest::from_parts(
        DigestKind::ProviderOutput,
        "upstream",
        &[label],
    )];
    DemandQueryTraceEntry {
        query_key: query_key.into(),
        result_digest: Digest::from_parts(DigestKind::ProviderOutput, "query_result", &[label]),
        precision_tier: PrecisionTier::SetupAware,
        provenance: format!("native:{label}"),
        cache_status: DemandCacheStatus::Computed,
        compute_duration_micros: 10,
    }
}

fn generation_finalized_fixture(root: &Path, label: &str) -> GenerationFinalizedFixture {
    std::fs::create_dir_all(root).expect("create fixture repository");
    if !root.join("main.go").exists() {
        std::fs::write(
            root.join("main.go"),
            "package main\n\nimport \"fmt\"\n\nfunc main() { fmt.Println(\"hello\") }\n",
        )
        .expect("write Go fixture");
    }
    let loaded = load_config(root).expect("default config loads");
    let cache = Cache::new("", false);
    let analysis_plan = AnalysisPlan::from_capability_names_for_test(&["resolved_imports"]);
    let config_digest = format!("store-generation-config-{label}");
    let rule_digest = format!("store-generation-rules-{label}");
    let output = AnalysisKernel::run(KernelInput {
        loaded: &loaded,
        cache: &cache,
        config_digest: &config_digest,
        rule_digest: &rule_digest,
        plan: &analysis_plan,
        parallel: false,
    })
    .expect("kernel fixture completes");
    let facts = output
        .db
        .fact_meta()
        .stable_rows()
        .expect("fact metadata is canonical");
    let mut report = output.run_report;
    let mut query_trace = DemandQueryTrace::default();
    for query_label in ["alpha", "beta"] {
        query_trace.record_entry(generation_query_trace_entry(query_label));
    }
    report.demand_query_trace = query_trace;
    GenerationFinalizedFixture {
        report,
        manifests: AnalysisKernel::provider_manifests().to_vec(),
        facts,
    }
}

fn generation_validated_fixture(root: &Path, label: &str) -> ValidatedRunMetadata {
    let fixture = generation_finalized_fixture(root, label);
    ValidatedRunMetadata::from_finalized_run(
        &fixture.report.input_snapshot,
        &fixture.report.provider_outputs,
        &fixture.report.demand_query_trace,
        fixture.report.validation_events(),
        &fixture.manifests,
        fixture.facts,
    )
    .expect("fixture produces a validated handoff")
}

fn generation_plan_fixture(root: &Path, label: &str) -> commit_plan::StoreCommitPlan {
    let validated = generation_validated_fixture(root, label);
    commit_plan::StoreCommitPlan::from_validated_run(&validated)
        .expect("validated handoff produces a complete plan")
}

fn generation_store_config(root: &Path) -> StoreConfig {
    StoreConfig::new(root.join("cache/semantic-store/store.sqlite3"), true)
}

#[test]
fn config_preserves_path_and_disabled_state() {
    let config = StoreConfig::new("cache/semantic-store/store.sqlite3", false);

    assert_eq!(
        config.path(),
        Path::new("cache/semantic-store/store.sqlite3")
    );
    assert!(!config.is_enabled());
}

#[test]
fn facade_disabled_guard_skips_plan_io_and_path_creation() {
    let temp = tempfile::tempdir().expect("temporary disabled facade root");
    let validated = generation_validated_fixture(&temp.path().join("repo"), "disabled-facade");
    let path = temp.path().join("cache/semantic-store/store.sqlite3");
    let config = StoreConfig::new(&path, false);
    reset_materialization_counters_for_test();

    let outcome = SemanticStore::commit_validated_run(&config, validated);

    assert_eq!(outcome, StoreOutcome::disabled());
    assert_eq!(materialization_counters_for_test(), (0, 0, 0));
    assert!(!path.exists());
    assert!(!path.parent().expect("store directory").exists());
}

#[test]
fn status_vocabulary_is_typed_and_comparable() {
    let statuses = [
        StoreStatus::Disabled,
        StoreStatus::Ready,
        StoreStatus::BusySkipped,
        StoreStatus::Skipped(StoreSkipReason::FutureSchema {
            found: migrations::CURRENT_SCHEMA_VERSION + 1,
            supported: migrations::CURRENT_SCHEMA_VERSION,
        }),
        StoreStatus::Skipped(StoreSkipReason::UnsafePath),
        StoreStatus::Skipped(StoreSkipReason::OpenFailed),
        StoreStatus::Skipped(StoreSkipReason::WorkspaceMismatch),
        StoreStatus::Skipped(StoreSkipReason::StaleReservation),
        StoreStatus::Skipped(StoreSkipReason::InvalidPlan),
        StoreStatus::Skipped(StoreSkipReason::CommitFailed),
        StoreStatus::RebuildNeeded(StoreRebuildReason::Corrupt),
        StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema),
        StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidMetadata),
    ];

    assert_eq!(statuses.len(), 13);
}

#[test]
fn semantic_store_is_zero_sized_facade() {
    assert_eq!(std::mem::size_of::<SemanticStore>(), 0);
}

mod connection_policy {
    use super::*;

    #[test]
    fn disabled_maintenance_returns_before_creating_or_opening_the_path() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("cache/semantic-store/store.sqlite3");
        let config = StoreConfig::new(&path, false);

        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Disabled);
        assert!(!temp.path().join("cache").exists());
        assert!(!path.exists());
    }

    #[test]
    fn writer_enforces_locked_pragmas_and_acquires_immediate_lease() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("cache/semantic-store/store.sqlite3");
        let config = StoreConfig::new(&path, true);

        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);
        let mut writer = connection::open_writer(&path).expect("open writer");
        let policy = connection::writer_policy(&writer).expect("read policy");

        assert_eq!(policy.foreign_keys, 1);
        assert_eq!(policy.journal_mode, "wal");
        assert_eq!(policy.synchronous, 1);
        assert_eq!(policy.busy_timeout_ms, 250);
        assert_eq!(policy.wal_autocheckpoint_pages, 0);
        assert!(policy.no_checkpoint_on_close);
        assert_eq!(policy.page_size_bytes, 16 * 1024);
        assert_eq!(
            connection::try_writer_lease(&mut writer).expect("writer lease"),
            connection::LeaseStatus::Acquired
        );
    }

    #[test]
    fn read_only_connection_is_independent_and_rejects_writes() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("semantic-store/store.sqlite3");
        let config = StoreConfig::new(&path, true);
        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);

        let reader = connection::open_read_only(&path).expect("open read-only connection");

        assert_eq!(
            connection::read_schema_version(&reader).expect("schema version"),
            migrations::CURRENT_SCHEMA_VERSION
        );
        assert!(connection::read_only_write_is_rejected(&reader));
    }

    #[test]
    fn journal_mode_validation_accepts_wal_case_insensitively() {
        assert_eq!(connection::validate_journal_mode("WaL"), Ok(()));
    }

    #[test]
    fn journal_mode_validation_rejects_a_successful_non_wal_result() {
        assert_eq!(
            connection::validate_journal_mode("delete"),
            Err(connection::ConnectionError::Policy)
        );
    }
}

mod writer_contention {
    use std::fs;
    use std::time::{Duration, Instant};

    use super::*;

    // The exact SQLite policy is asserted as 250 ms in `connection_policy`.
    // This wall-clock check is only an anti-hang guard, so leave headroom for a
    // loaded CI runner to deschedule the test around the busy-handler sleeps.
    const CONTENTION_ANTI_HANG_LIMIT: Duration = Duration::from_secs(2);

    #[test]
    fn losing_writer_skips_within_bound_then_acquires_after_release() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("semantic-store/store.sqlite3");
        let config = StoreConfig::new(&path, true);
        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);

        let mut first = connection::open_writer(&path).expect("open first writer");
        let mut second = connection::open_writer(&path).expect("open second writer");
        let first_lease = connection::hold_writer_lease(&mut first).expect("hold first lease");

        let started = Instant::now();
        let losing_status = connection::try_writer_lease(&mut second).expect("bounded result");
        let elapsed = started.elapsed();

        assert_eq!(losing_status, connection::LeaseStatus::Busy);
        assert!(elapsed < CONTENTION_ANTI_HANG_LIMIT, "elapsed: {elapsed:?}");
        assert_eq!(
            connection::bootstrap_marker_count(&second).expect("marker count"),
            1
        );

        first_lease.release().expect("release first lease");
        assert_eq!(
            connection::try_writer_lease(&mut second).expect("second acquisition"),
            connection::LeaseStatus::Acquired
        );
        assert_eq!(
            connection::integrity_check(&second).expect("integrity check"),
            "ok"
        );
        assert_eq!(
            connection::bootstrap_marker_count(&second).expect("marker count"),
            1
        );
    }

    #[test]
    fn absent_store_initialization_is_serialized_by_the_immediate_lease() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("semantic-store/store.sqlite3");
        fs::create_dir_all(path.parent().expect("store parent")).expect("create store directory");
        assert!(!path.exists());

        let mut first = connection::open_uninitialized_writer_for_test(&path)
            .expect("prepare first absent-store writer");
        let mut second = connection::open_uninitialized_writer_for_test(&path)
            .expect("prepare second absent-store writer");
        let first_lease = connection::hold_initialization_lease(&mut first)
            .expect("hold first initialization lease");

        let started = Instant::now();
        let losing_status = connection::try_initialize_writer_for_test(&mut second)
            .expect("bounded initialization result");
        let elapsed = started.elapsed();

        assert_eq!(losing_status, connection::LeaseStatus::Busy);
        assert!(elapsed < CONTENTION_ANTI_HANG_LIMIT, "elapsed: {elapsed:?}");

        first_lease
            .initialize_and_release()
            .expect("finish first initialization");
        assert_eq!(
            connection::try_initialize_writer_for_test(&mut second)
                .expect("second initialization after release"),
            connection::LeaseStatus::Acquired
        );
        assert_eq!(
            connection::bootstrap_marker_count(&second).expect("marker count"),
            1
        );
        assert_eq!(
            connection::integrity_check(&second).expect("integrity check"),
            "ok"
        );
    }
}

mod first_binding_race {
    use std::thread;

    use rusqlite::{Connection, OptionalExtension};

    use super::*;

    fn force_binding_order(winner_label: &str, loser_label: &str) {
        let temp = tempfile::tempdir().expect("temp directory");
        let winner = generation_plan_fixture(
            &temp.path().join(format!("repo-{winner_label}")),
            winner_label,
        );
        let loser = generation_plan_fixture(
            &temp.path().join(format!("repo-{loser_label}")),
            loser_label,
        );
        assert_ne!(
            winner.semantic.identities.workspace,
            loser.semantic.identities.workspace
        );
        let config = generation_store_config(temp.path());
        let interlock: &'static generation::ReservationInterlock =
            Box::leak(Box::new(generation::ReservationInterlock::new()));
        let winner_config = config.clone();
        let winner_plan = winner.clone();
        let winning_writer = thread::spawn(move || {
            generation::commit_generation_with_control(
                &winner_config,
                &winner_plan,
                generation::CommitControl::with_reservation_interlock(interlock),
            )
        });

        interlock.entered.wait();
        assert_eq!(
            generation::commit_generation(&config, &loser),
            StoreStatus::BusySkipped
        );
        interlock.release.wait();
        let winning_status = winning_writer.join().expect("winning writer joins");
        if winning_status != StoreStatus::Ready {
            let database = Connection::open(config.path()).expect("open failed store");
            let failure: Option<(String, String)> = database
                .query_row(
                    "SELECT reason_code, stage_code FROM generation_failure_events",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .expect("read publication failure");
            panic!("winning status {winning_status:?}, failure {failure:?}");
        }
        assert_eq!(
            generation::commit_generation(&config, &loser),
            StoreStatus::Skipped(StoreSkipReason::WorkspaceMismatch)
        );

        let database = Connection::open(config.path()).expect("open store");
        let (workspace_value, active_generation): (String, i64) = database
            .query_row(
                "SELECT workspace_value, active_generation_id FROM store_manifest WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read bound manifest");
        assert_eq!(
            workspace_value,
            winner.semantic.identities.workspace.digest().value
        );
        let rows: i64 = database
            .query_row("SELECT count(*) FROM generations", [], |row| row.get(0))
            .expect("count generations");
        let loser_rows: i64 = database
            .query_row(
                "SELECT count(*) FROM generations WHERE workspace_value = ?1",
                [loser.semantic.identities.workspace.digest().value.as_str()],
                |row| row.get(0),
            )
            .expect("count loser generations");
        let active_status: String = database
            .query_row(
                "SELECT status FROM generations WHERE id = ?1",
                [active_generation],
                |row| row.get(0),
            )
            .expect("read active status");
        assert_eq!(rows, 1);
        assert_eq!(loser_rows, 0);
        assert_eq!(active_status, schema::GenerationStatus::Complete.label());
    }

    #[test]
    fn both_first_binding_orders_have_one_atomic_winner() {
        force_binding_order("a", "b");
        force_binding_order("b", "a");
    }
}

mod generation_lifecycle {
    use std::thread;

    use rusqlite::Connection;

    use super::*;

    fn active_generation(database: &Connection) -> i64 {
        database
            .query_row(
                "SELECT active_generation_id FROM store_manifest WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("read active generation")
    }

    fn generation_attempt(
        database: &Connection,
        plan: &commit_plan::StoreCommitPlan,
        ordinal: i64,
    ) -> (i64, String) {
        database
            .query_row(
                "SELECT id, status FROM generations
                 WHERE generation_value = ?1 AND reservation_ordinal = ?2",
                rusqlite::params![plan.semantic.identities.generation.digest().value, ordinal],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read generation attempt")
    }

    #[test]
    fn complete_same_workspace_retry_uses_contiguous_ordinals() {
        let temp = tempfile::tempdir().expect("temp directory");
        let plan = generation_plan_fixture(&temp.path().join("repo"), "retry");
        let config = generation_store_config(temp.path());

        assert_eq!(
            generation::commit_generation(&config, &plan),
            StoreStatus::Ready
        );
        assert_eq!(
            generation::commit_generation(&config, &plan),
            StoreStatus::Ready
        );

        let database = Connection::open(config.path()).expect("open store");
        let mut statement = database
            .prepare(
                "SELECT reservation_ordinal, status FROM generations
                 WHERE generation_value = ?1 ORDER BY reservation_ordinal",
            )
            .expect("prepare attempt query");
        let attempts = statement
            .query_map(
                [plan.semantic.identities.generation.digest().value.as_str()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("query attempts")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect attempts");
        assert_eq!(
            attempts,
            vec![
                (0, schema::GenerationStatus::Complete.label().to_string()),
                (1, schema::GenerationStatus::Complete.label().to_string()),
            ]
        );
        let active = generation::read_active_complete(&config, &plan.semantic.identities.workspace)
            .expect("read active generation")
            .expect("active generation exists");
        assert_eq!(active.plan, plan);
    }

    #[test]
    fn every_injected_boundary_rolls_back_and_preserves_active_truth() {
        let temp = tempfile::tempdir().expect("temp directory");
        let repository = temp.path().join("repo");
        let active_plan = generation_plan_fixture(&repository, "active");
        let retry_plan = generation_plan_fixture(&repository, "retry-after-failure");
        let config = generation_store_config(temp.path());
        assert_ne!(
            active_plan.semantic.identities.generation,
            retry_plan.semantic.identities.generation
        );
        assert_eq!(
            generation::commit_generation(&config, &active_plan),
            StoreStatus::Ready
        );
        let database = Connection::open(config.path()).expect("open store");
        let original_active = active_generation(&database);
        let mut committed_attempts = 0_i64;

        for stage in schema::GenerationFailureStage::ALL {
            assert_eq!(
                generation::commit_generation_with_control(
                    &config,
                    &retry_plan,
                    generation::CommitControl::fail_after(stage),
                ),
                StoreStatus::Skipped(StoreSkipReason::CommitFailed),
                "stage {}",
                stage.label()
            );
            assert_eq!(active_generation(&database), original_active);
            let active = generation::read_active_complete(
                &config,
                &active_plan.semantic.identities.workspace,
            )
            .expect("read active generation")
            .expect("active generation exists");
            assert_eq!(active.plan, active_plan);

            if stage == schema::GenerationFailureStage::Reservation {
                let count: i64 = database
                    .query_row(
                        "SELECT count(*) FROM generations WHERE generation_value = ?1",
                        [retry_plan
                            .semantic
                            .identities
                            .generation
                            .digest()
                            .value
                            .as_str()],
                        |row| row.get(0),
                    )
                    .expect("count rolled-back reservations");
                assert_eq!(count, 0);
                continue;
            }

            let (generation_id, status) =
                generation_attempt(&database, &retry_plan, committed_attempts);
            assert_eq!(status, schema::GenerationStatus::Failed.label());
            assert_eq!(
                generation::payload_row_count_for_test(&config, generation_id)
                    .expect("count generation payload"),
                0
            );
            let (event, reason, recorded_stage): (String, String, String) = database
                .query_row(
                    "SELECT event_code, reason_code, stage_code
                     FROM generation_failure_events WHERE generation_id = ?1",
                    [generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("read failure audit");
            let expected_reason = if stage == schema::GenerationFailureStage::TransactionCommit {
                schema::GenerationFailureReason::PublicationCommitFailed
            } else {
                schema::GenerationFailureReason::WriteFailed
            };
            assert_eq!(
                event,
                schema::GenerationFailureEvent::CommitAttemptFailed.label()
            );
            assert_eq!(reason, expected_reason.label());
            assert_eq!(recorded_stage, stage.label());
            committed_attempts += 1;
        }

        assert_eq!(
            generation::commit_generation(&config, &retry_plan),
            StoreStatus::Ready
        );
        let (_, final_status) = generation_attempt(&database, &retry_plan, committed_attempts);
        assert_eq!(final_status, schema::GenerationStatus::Complete.label());
        let active =
            generation::read_active_complete(&config, &retry_plan.semantic.identities.workspace)
                .expect("read final active generation")
                .expect("final active generation exists");
        assert_eq!(active.plan, retry_plan);
    }

    #[test]
    fn incomplete_provider_children_never_complete_or_activate() {
        let temp = tempfile::tempdir().expect("temp directory");
        let repository = temp.path().join("repo");
        let active_plan = generation_plan_fixture(&repository, "provider-active");
        let candidate = generation_plan_fixture(&repository, "provider-candidate");
        let config = generation_store_config(temp.path());
        assert_eq!(
            generation::commit_generation(&config, &active_plan),
            StoreStatus::Ready
        );
        let database = Connection::open(config.path()).expect("open store");
        let original_active = active_generation(&database);

        for (ordinal, tamper) in [
            generation::ProviderChildTamper::Missing,
            generation::ProviderChildTamper::Mismatched,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                generation::commit_generation_with_control(
                    &config,
                    &candidate,
                    generation::CommitControl::with_provider_child_tamper(tamper),
                ),
                StoreStatus::Skipped(StoreSkipReason::CommitFailed)
            );
            let (generation_id, status) = generation_attempt(&database, &candidate, ordinal as i64);
            assert_eq!(status, schema::GenerationStatus::Failed.label());
            assert_eq!(
                generation::payload_row_count_for_test(&config, generation_id)
                    .expect("count generation payload"),
                0
            );
            let (reason, stage): (String, String) = database
                .query_row(
                    "SELECT reason_code, stage_code FROM generation_failure_events
                     WHERE generation_id = ?1",
                    [generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("read validation audit");
            assert_eq!(
                reason,
                schema::GenerationFailureReason::PostWriteValidationFailed.label()
            );
            assert_eq!(stage, schema::GenerationFailureStage::Statistics.label());
            assert_eq!(active_generation(&database), original_active);
        }
    }

    #[test]
    fn busy_audit_leaves_exact_pending_attempt_untouched() {
        let temp = tempfile::tempdir().expect("temp directory");
        let plan = generation_plan_fixture(&temp.path().join("repo"), "audit-busy");
        let config = generation_store_config(temp.path());
        let reservation =
            generation::reserve_only_for_test(&config, &plan).expect("reserve pending generation");
        let mut audit_writer = connection::open_writer(config.path()).expect("open audit writer");
        let mut lock_writer = connection::open_writer(config.path()).expect("open lock writer");
        let lock = connection::hold_writer_lease(&mut lock_writer).expect("hold writer lease");

        assert_eq!(
            generation::audit_reservation_for_test(
                &mut audit_writer,
                &reservation,
                schema::GenerationFailureReason::WriteFailed,
                schema::GenerationFailureStage::Providers,
            ),
            generation::AuditOutcome::Busy
        );
        let database = Connection::open(config.path()).expect("open store");
        let (status, event_count): (String, i64) = database
            .query_row(
                "SELECT generation.status,
                        (SELECT count(*) FROM generation_failure_events AS failure
                         WHERE failure.generation_id = generation.id)
                 FROM generations AS generation WHERE generation.id = ?1",
                [reservation.handle()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read pending audit target");
        assert_eq!(status, schema::GenerationStatus::Pending.label());
        assert_eq!(event_count, 0);
        assert_eq!(
            generation::payload_row_count_for_test(&config, reservation.handle())
                .expect("count pending payload"),
            0
        );

        lock.release().expect("release writer lease");
        assert_eq!(
            generation::audit_reservation_for_test(
                &mut audit_writer,
                &reservation,
                schema::GenerationFailureReason::WriteFailed,
                schema::GenerationFailureStage::Providers,
            ),
            generation::AuditOutcome::Recorded
        );
        assert_eq!(
            generation::audit_reservation_for_test(
                &mut audit_writer,
                &reservation,
                schema::GenerationFailureReason::WriteFailed,
                schema::GenerationFailureStage::Providers,
            ),
            generation::AuditOutcome::NotTrusted
        );
        let (status, event_count): (String, i64) = database
            .query_row(
                "SELECT generation.status,
                        (SELECT count(*) FROM generation_failure_events AS failure
                         WHERE failure.generation_id = generation.id)
                 FROM generations AS generation WHERE generation.id = ?1",
                [reservation.handle()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read recorded audit target");
        assert_eq!(status, schema::GenerationStatus::Failed.label());
        assert_eq!(event_count, 1);
    }

    #[test]
    fn stale_reservation_is_pending_unaudited_and_does_not_replace_active() {
        let temp = tempfile::tempdir().expect("temp directory");
        let repository = temp.path().join("repo");
        let initial = generation_plan_fixture(&repository, "stale-initial");
        let stale = generation_plan_fixture(&repository, "stale-paused");
        let winner = generation_plan_fixture(&repository, "stale-winner");
        let config = generation_store_config(temp.path());
        assert_eq!(
            generation::commit_generation(&config, &initial),
            StoreStatus::Ready
        );
        let interlock: &'static generation::ReservationInterlock =
            Box::leak(Box::new(generation::ReservationInterlock::new()));
        let stale_config = config.clone();
        let stale_plan = stale.clone();
        let stale_writer = thread::spawn(move || {
            generation::commit_generation_with_control(
                &stale_config,
                &stale_plan,
                generation::CommitControl::with_publication_interlock(interlock),
            )
        });

        interlock.entered.wait();
        let visible =
            generation::read_active_complete(&config, &initial.semantic.identities.workspace)
                .expect("read while newer generation is pending")
                .expect("initial generation remains active");
        assert_eq!(visible.plan, initial);
        assert_eq!(
            generation::commit_generation(&config, &winner),
            StoreStatus::Ready
        );
        interlock.release.wait();
        assert_eq!(
            stale_writer.join().expect("stale writer joins"),
            StoreStatus::Skipped(StoreSkipReason::StaleReservation)
        );

        let database = Connection::open(config.path()).expect("open store");
        let (stale_id, stale_status) = generation_attempt(&database, &stale, 0);
        assert_eq!(stale_status, schema::GenerationStatus::Pending.label());
        let failure_count: i64 = database
            .query_row(
                "SELECT count(*) FROM generation_failure_events WHERE generation_id = ?1",
                [stale_id],
                |row| row.get(0),
            )
            .expect("count stale failure events");
        assert_eq!(failure_count, 0);
        assert_eq!(
            generation::payload_row_count_for_test(&config, stale_id).expect("count stale payload"),
            0
        );
        let active =
            generation::read_active_complete(&config, &winner.semantic.identities.workspace)
                .expect("read winner")
                .expect("winner is active");
        assert_eq!(active.plan, winner);
    }

    #[test]
    fn invalid_plan_is_rejected_before_store_creation() {
        let temp = tempfile::tempdir().expect("temp directory");
        let mut plan = generation_plan_fixture(&temp.path().join("repo"), "invalid-plan");
        plan.semantic.provider_manifest_schemas.pop();
        let config = generation_store_config(temp.path());

        assert_eq!(
            generation::commit_generation(&config, &plan),
            StoreStatus::Skipped(StoreSkipReason::InvalidPlan)
        );
        assert!(!config.path().exists());
    }

    #[test]
    fn malformed_kernel_handoffs_never_reserve_complete_or_activate() {
        let temp = tempfile::tempdir().expect("temp directory");
        let repository = temp.path().join("repo");
        let active = generation_validated_fixture(&repository, "handoff-active");
        let active_plan = commit_plan::StoreCommitPlan::from_validated_run(&active)
            .expect("active handoff produces a plan");
        let config = generation_store_config(temp.path());
        assert_eq!(
            SemanticStore::commit_validated_run(&config, active).status,
            StoreStatus::Ready
        );
        let database = Connection::open(config.path()).expect("open store");
        let original_active = active_generation(&database);

        type CorruptHandoff = fn(&mut ValidatedRunMetadata);
        let corruptions: [(&str, CorruptHandoff); 3] = [
            (
                "empty-stable-key",
                ValidatedRunMetadata::corrupt_first_fact_stable_key_for_store_test,
            ),
            (
                "unknown-producer",
                ValidatedRunMetadata::corrupt_first_fact_producer_for_store_test,
            ),
            (
                "absolute-path",
                ValidatedRunMetadata::corrupt_first_file_path_for_store_test,
            ),
        ];
        for (label, corrupt) in corruptions {
            let mut candidate = generation_validated_fixture(&repository, label);
            let candidate_generation = candidate.identities().generation().digest().value.clone();
            corrupt(&mut candidate);

            let outcome = SemanticStore::commit_validated_run(&config, candidate);

            assert_eq!(
                outcome.status,
                StoreStatus::Skipped(StoreSkipReason::InvalidPlan),
                "{label}"
            );
            let candidate_count: i64 = database
                .query_row(
                    "SELECT count(*) FROM generations WHERE generation_value = ?1",
                    [candidate_generation],
                    |row| row.get(0),
                )
                .expect("count candidate attempts");
            assert_eq!(candidate_count, 0, "{label}");
            assert_eq!(active_generation(&database), original_active, "{label}");
        }

        let visible =
            generation::read_active_complete(&config, &active_plan.semantic.identities.workspace)
                .expect("read active generation")
                .expect("active generation remains available");
        assert_eq!(visible.plan, active_plan);
    }
}

mod active_complete_reader {
    use rusqlite::Connection;

    use super::*;

    #[test]
    fn full_typed_projection_round_trips_queries_edges_and_attempt_isolation() {
        let temp = tempfile::tempdir().expect("temp directory");
        let repository = temp.path().join("repo");
        let active_plan = generation_plan_fixture(&repository, "reader-active");
        let later_plan = generation_plan_fixture(&repository, "reader-later");
        let config = generation_store_config(temp.path());
        assert_eq!(
            generation::commit_generation(&config, &active_plan),
            StoreStatus::Ready
        );

        let expected_edges = active_plan
            .semantic
            .dependency_edges
            .iter()
            .map(|row| crate::analysis_kernel::incremental::DependencyEdge {
                from: expanded_node(&active_plan.semantic, &row.from),
                to: expanded_node(&active_plan.semantic, &row.to),
                kind: row.kind,
                required_shape: row.required_shape,
            })
            .collect();
        let expected_index =
            crate::analysis_kernel::incremental::DependencyIndex::from_edges(expected_edges);
        let active =
            generation::read_active_complete(&config, &active_plan.semantic.identities.workspace)
                .expect("read active generation")
                .expect("active generation exists");
        assert_eq!(active.plan, active_plan);
        assert!(!active.plan.semantic.query_inputs.is_empty());
        assert_eq!(
            active.plan.semantic.query_inputs,
            active_plan.semantic.query_inputs
        );
        assert_eq!(active.dependency_index, expected_index);

        let pending = generation::reserve_only_for_test(&config, &later_plan)
            .expect("reserve later generation");
        let visible =
            generation::read_active_complete(&config, &active_plan.semantic.identities.workspace)
                .expect("read with pending attempt")
                .expect("active generation remains visible");
        assert_eq!(visible.plan, active_plan);
        assert_eq!(
            generation::payload_row_count_for_test(&config, pending.handle())
                .expect("count pending payload"),
            0
        );

        let mut writer = connection::open_writer(config.path()).expect("open audit writer");
        assert_eq!(
            generation::audit_reservation_for_test(
                &mut writer,
                &pending,
                schema::GenerationFailureReason::WriteFailed,
                schema::GenerationFailureStage::StoreRunInput,
            ),
            generation::AuditOutcome::Recorded
        );
        let visible =
            generation::read_active_complete(&config, &active_plan.semantic.identities.workspace)
                .expect("read with failed attempt")
                .expect("active generation remains visible");
        assert_eq!(visible.plan, active_plan);
        assert_eq!(visible.dependency_index, expected_index);
    }

    fn expanded_node(
        semantic: &commit_plan::StoreSemanticPlan,
        node: &commit_plan::StoreNodeRef,
    ) -> crate::analysis_kernel::incremental::CacheNode {
        use crate::analysis_kernel::incremental::CacheNode;
        use commit_plan::StoreNodeRef;

        match node {
            StoreNodeRef::DependencyInput(input) => CacheNode::DependencyInput(input.clone()),
            StoreNodeRef::RunManifest => CacheNode::RunManifest(semantic.run_manifest.key.clone()),
            StoreNodeRef::Layer(ordinal) => {
                CacheNode::layer(semantic.layers[*ordinal as usize].key.clone())
            }
            StoreNodeRef::Query(ordinal) => {
                CacheNode::Query(semantic.queries[*ordinal as usize].key.clone().into())
            }
            StoreNodeRef::Summary(ordinal) => {
                CacheNode::Summary(semantic.summaries[*ordinal as usize].key.clone())
            }
            StoreNodeRef::Diagnostic(ordinal) => {
                CacheNode::Diagnostic(semantic.diagnostics[*ordinal as usize].key.clone())
            }
        }
    }

    #[test]
    fn pristine_and_bound_without_active_return_none() {
        let temp = tempfile::tempdir().expect("temp directory");
        let plan = generation_plan_fixture(&temp.path().join("repo"), "reader-empty");
        let other_workspace =
            generation_plan_fixture(&temp.path().join("other-repo"), "reader-other-workspace");
        let config = generation_store_config(temp.path());
        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);
        assert_eq!(
            generation::read_active_complete(&config, &plan.semantic.identities.workspace),
            Ok(None)
        );
        let pending =
            generation::reserve_only_for_test(&config, &plan).expect("reserve pending generation");
        assert_eq!(
            generation::read_active_complete(&config, &plan.semantic.identities.workspace),
            Ok(None)
        );
        assert_eq!(
            generation::read_active_complete(
                &config,
                &other_workspace.semantic.identities.workspace,
            ),
            Err(StoreStatus::Skipped(StoreSkipReason::WorkspaceMismatch))
        );
        assert_eq!(
            generation::payload_row_count_for_test(&config, pending.handle())
                .expect("count pending payload"),
            0
        );
    }

    #[test]
    fn corrupt_telemetry_is_non_gating_for_semantic_selection() {
        let temp = tempfile::tempdir().expect("temp directory");
        let plan = generation_plan_fixture(&temp.path().join("repo"), "reader-telemetry");
        let config = generation_store_config(temp.path());
        assert_eq!(
            generation::commit_generation(&config, &plan),
            StoreStatus::Ready
        );
        let baseline =
            generation::read_active_complete(&config, &plan.semantic.identities.workspace)
                .expect("read baseline")
                .expect("baseline is active");
        assert!(!baseline.plan.telemetry.is_empty());

        let database = Connection::open(config.path()).expect("open store");
        database
            .pragma_update(None, "ignore_check_constraints", true)
            .expect("allow corrupt telemetry fixture");
        let changed = database
            .execute(
                "UPDATE generation_telemetry SET file_mtime_hint_present = 2
                 WHERE generation_id = (
                     SELECT active_generation_id FROM store_manifest WHERE id = 1
                 )",
                [],
            )
            .expect("corrupt telemetry flag");
        assert!(changed > 0);
        drop(database);

        let after = generation::read_active_complete(&config, &plan.semantic.identities.workspace)
            .expect("read after telemetry corruption")
            .expect("same semantic generation remains active");
        assert_eq!(after.plan.semantic, baseline.plan.semantic);
        assert_eq!(after.dependency_index, baseline.dependency_index);
        assert!(after.plan.telemetry.is_empty());
    }

    #[test]
    fn active_pointer_to_pending_is_rejected_fail_closed() {
        let temp = tempfile::tempdir().expect("temp directory");
        let repository = temp.path().join("repo");
        let active_plan = generation_plan_fixture(&repository, "reader-pointer-active");
        let pending_plan = generation_plan_fixture(&repository, "reader-pointer-pending");
        let config = generation_store_config(temp.path());
        assert_eq!(
            generation::commit_generation(&config, &active_plan),
            StoreStatus::Ready
        );
        let pending = generation::reserve_only_for_test(&config, &pending_plan)
            .expect("reserve pending generation");

        let database = Connection::open(config.path()).expect("open store");
        database
            .execute_batch(
                "DROP TRIGGER store_manifest_active_must_be_complete;
                 CREATE TRIGGER store_manifest_active_must_be_complete
                 BEFORE UPDATE OF active_generation_id ON store_manifest
                 BEGIN
                     SELECT 1;
                 END;",
            )
            .expect("replace activation guard for tamper fixture");
        database
            .execute(
                "UPDATE store_manifest SET active_generation_id = ?1 WHERE id = 1",
                [pending.handle()],
            )
            .expect("point active manifest at pending generation");
        drop(database);

        assert_eq!(
            generation::read_active_complete(&config, &active_plan.semantic.identities.workspace,),
            Err(StoreStatus::RebuildNeeded(
                StoreRebuildReason::InvalidSchema
            ))
        );
        assert_eq!(
            generation::payload_row_count_for_test(&config, pending.handle())
                .expect("count pending payload"),
            0
        );
    }

    #[test]
    fn active_identity_tampering_is_rejected_without_fallback() {
        let temp = tempfile::tempdir().expect("temp directory");
        let repository = temp.path().join("repo");
        let older = generation_plan_fixture(&repository, "reader-identity-older");
        let plan = generation_plan_fixture(&repository, "reader-identity-active");
        let config = generation_store_config(temp.path());
        assert_eq!(
            generation::commit_generation(&config, &older),
            StoreStatus::Ready
        );
        assert_eq!(
            generation::commit_generation(&config, &plan),
            StoreStatus::Ready
        );

        let database = Connection::open(config.path()).expect("open store");
        let changed = database
            .execute(
                "UPDATE generations SET run_value = run_value || '.tampered'
                 WHERE id = (SELECT active_generation_id FROM store_manifest WHERE id = 1)",
                [],
            )
            .expect("tamper active identity");
        assert_eq!(changed, 1);
        drop(database);

        assert_eq!(
            generation::read_active_complete(&config, &plan.semantic.identities.workspace),
            Err(StoreStatus::RebuildNeeded(
                StoreRebuildReason::InvalidMetadata
            ))
        );
    }

    #[test]
    fn identical_rerun_rejects_tampered_active_scalar_and_child_rows() {
        let tampers = [
            (
                "input-scalar",
                "UPDATE input_files
                 SET source_digest_value = source_digest_value || '.tampered'
                 WHERE generation_id = (
                     SELECT active_generation_id FROM store_manifest WHERE id = 1
                 );",
            ),
            (
                "fact-child",
                "DELETE FROM fact_metadata
                 WHERE (generation_id, ordinal) IN (
                     SELECT generation_id, ordinal FROM fact_metadata
                     WHERE generation_id = (
                         SELECT active_generation_id FROM store_manifest WHERE id = 1
                     ) LIMIT 1
                 );",
            ),
            (
                "query-child",
                "DELETE FROM query_inputs
                 WHERE (generation_id, query_id, ordinal) IN (
                     SELECT generation_id, query_id, ordinal FROM query_inputs
                     WHERE generation_id = (
                         SELECT active_generation_id FROM store_manifest WHERE id = 1
                     ) LIMIT 1
                 );",
            ),
            (
                "dependency-child",
                "DELETE FROM dependency_edges
                 WHERE (generation_id, ordinal) IN (
                     SELECT generation_id, ordinal FROM dependency_edges
                     WHERE generation_id = (
                         SELECT active_generation_id FROM store_manifest WHERE id = 1
                     ) LIMIT 1
                 );",
            ),
        ];

        for (label, tamper) in tampers {
            let temp = tempfile::tempdir().expect("temp directory");
            let validated = generation_validated_fixture(&temp.path().join("repo"), label);
            let config = generation_store_config(temp.path());
            assert_eq!(
                SemanticStore::commit_validated_run(&config, validated.clone()).status,
                StoreStatus::Ready,
                "{label}"
            );
            let database = Connection::open(config.path()).expect("open store");
            database.execute_batch(tamper).expect("tamper active rows");
            drop(database);

            let outcome = SemanticStore::commit_validated_run(&config, validated);

            assert_eq!(
                outcome.status,
                StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidMetadata),
                "{label}"
            );
            assert_eq!(outcome.statistics, None, "{label}");
        }
    }

    #[test]
    fn identical_rerun_rejects_duplicate_fact_identity_with_alternate_lz4_bytes() {
        use crate::analysis_kernel::metadata::StableFactKey;

        let temp = tempfile::tempdir().expect("temp directory");
        let validated =
            generation_validated_fixture(&temp.path().join("repo"), "duplicate-alternate-fact-key");
        let config = generation_store_config(temp.path());
        assert_eq!(
            SemanticStore::commit_validated_run(&config, validated.clone()).status,
            StoreStatus::Ready
        );
        let database = Connection::open(config.path()).expect("open store");
        let active: i64 = database
            .query_row(
                "SELECT active_generation_id FROM store_manifest WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("read active generation");
        let family: String = database
            .query_row(
                "SELECT family FROM fact_metadata WHERE generation_id = ?1
                 GROUP BY family HAVING count(*) > 1 ORDER BY family LIMIT 1",
                [active],
                |row| row.get(0),
            )
            .expect("fixture has a repeated fact family");
        let mut statement = database
            .prepare(
                "SELECT ordinal, stable_key FROM fact_metadata
                 WHERE generation_id = ?1 AND family = ?2 ORDER BY ordinal LIMIT 2",
            )
            .expect("prepare fact selection");
        let rows = statement
            .query_map(rusqlite::params![active, family], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .expect("query facts")
            .collect::<Result<Vec<_>, _>>()
            .expect("decode facts");
        drop(statement);
        let stable_key = StableFactKey::from_storage(rows[0].1.clone())
            .expect("stored key decodes")
            .decoded()
            .into_owned();
        let mut alternate = Vec::with_capacity(32 + stable_key.len());
        alternate.extend_from_slice(b"polint-lz4-v1\0");
        alternate.extend_from_slice(
            &u32::try_from(stable_key.len())
                .expect("fixture key fits codec")
                .to_le_bytes(),
        );
        if stable_key.len() < 15 {
            alternate.push(u8::try_from(stable_key.len()).expect("short literal length") << 4);
        } else {
            alternate.push(0xf0);
            let mut remaining = stable_key.len() - 15;
            while remaining >= 255 {
                alternate.push(255);
                remaining -= 255;
            }
            alternate.push(u8::try_from(remaining).expect("literal extension"));
        }
        alternate.extend_from_slice(stable_key.as_bytes());
        let changed = database
            .execute(
                "UPDATE fact_metadata SET stable_key = ?1
                 WHERE generation_id = ?2 AND ordinal = ?3",
                rusqlite::params![alternate, active, rows[1].0],
            )
            .expect("replace fact key with alternate encoding");
        assert_eq!(changed, 1);
        drop(database);

        let outcome = SemanticStore::commit_validated_run(&config, validated);

        assert_eq!(
            outcome.status,
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidMetadata)
        );
        assert_eq!(outcome.statistics, None);
    }
}

mod recovery {
    use std::fs;

    use rusqlite::Connection;

    use super::*;

    fn store_path(temp: &tempfile::TempDir) -> std::path::PathBuf {
        temp.path().join("cache/semantic-store/store.sqlite3")
    }

    #[test]
    fn corrupt_and_invalid_stores_are_preserved_as_rebuild_needed() {
        let corrupt_temp = tempfile::tempdir().expect("temp directory");
        let corrupt_path = store_path(&corrupt_temp);
        fs::create_dir_all(corrupt_path.parent().expect("store parent"))
            .expect("create store directory");
        let corrupt_bytes = b"not a sqlite database";
        fs::write(&corrupt_path, corrupt_bytes).expect("write corrupt store");
        let corrupt_config = StoreConfig::new(&corrupt_path, true);

        assert_eq!(
            SemanticStore::maintain(&corrupt_config),
            StoreStatus::RebuildNeeded(StoreRebuildReason::Corrupt)
        );
        assert_eq!(
            fs::read(&corrupt_path).expect("read corrupt store"),
            corrupt_bytes
        );

        let invalid_temp = tempfile::tempdir().expect("temp directory");
        let invalid_path = store_path(&invalid_temp);
        fs::create_dir_all(invalid_path.parent().expect("store parent"))
            .expect("create store directory");
        let invalid = Connection::open(&invalid_path).expect("open invalid store");
        invalid
            .pragma_update(None, "user_version", migrations::CURRENT_SCHEMA_VERSION)
            .expect("set current version without marker");
        drop(invalid);
        let invalid_config = StoreConfig::new(&invalid_path, true);

        assert_eq!(
            SemanticStore::maintain(&invalid_config),
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)
        );
        assert!(invalid_path.exists());
    }

    #[test]
    fn incomplete_current_version_result_boundaries_require_controlled_rebuild() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = store_path(&temp);
        fs::create_dir_all(path.parent().expect("store parent")).expect("create store directory");
        connection::install_incomplete_current_v2_fixture_for_test(&path)
            .expect("install incomplete current-version fixture");
        assert!(!connection::current_schema_is_valid_for_test(&path));
        let before = connection::fixture_snapshot_for_test(&path).expect("snapshot fixture");
        let config = StoreConfig::new(&path, true);

        assert_eq!(
            SemanticStore::maintain(&config),
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)
        );
        assert_eq!(
            connection::fixture_snapshot_for_test(&path).expect("snapshot preserved fixture"),
            before
        );
        assert!(!connection::current_schema_is_valid_for_test(&path));

        assert_eq!(
            rebuild_owned_cache_store(&config, &path),
            StoreStatus::Ready
        );
        assert!(connection::current_schema_is_valid_for_test(&path));
    }

    #[test]
    fn future_store_is_preserved_and_explicit_rebuild_refuses_it() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = store_path(&temp);
        fs::create_dir_all(path.parent().expect("store parent")).expect("create store directory");
        let future = Connection::open(&path).expect("open future store");
        future
            .execute_batch(
                "CREATE TABLE sentinel (value TEXT NOT NULL);\
                 INSERT INTO sentinel (value) VALUES ('future-data');",
            )
            .expect("create future fixture");
        let future_version = migrations::CURRENT_SCHEMA_VERSION + 1;
        future
            .pragma_update(None, "user_version", future_version)
            .expect("set future schema version");
        drop(future);
        let original_bytes = fs::read(&path).expect("read original future store");
        let config = StoreConfig::new(&path, true);
        let future_status = StoreStatus::Skipped(StoreSkipReason::FutureSchema {
            found: future_version,
            supported: migrations::CURRENT_SCHEMA_VERSION,
        });

        assert_eq!(SemanticStore::maintain(&config), future_status);
        assert_eq!(
            fs::read(&path).expect("read future store after maintenance"),
            original_bytes
        );
        assert_eq!(rebuild_owned_cache_store(&config, &path), future_status);
        assert!(path.exists());

        let preserved =
            Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("reopen future store");
        let version: i32 = preserved
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("future version");
        let value: String = preserved
            .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
            .expect("sentinel value");
        assert_eq!(version, future_version);
        assert_eq!(value, "future-data");
    }

    #[test]
    fn rebuild_refuses_outside_candidate_and_rebuilds_exact_corrupt_owned_file() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = store_path(&temp);
        fs::create_dir_all(path.parent().expect("store parent")).expect("create store directory");
        fs::write(&path, b"corrupt owned store").expect("write corrupt owned store");
        let config = StoreConfig::new(&path, true);
        let outside = temp.path().join("outside.sqlite3");
        fs::write(&outside, b"outside data").expect("write outside file");

        assert_eq!(
            rebuild_owned_cache_store(&config, &outside),
            StoreStatus::Skipped(StoreSkipReason::UnsafePath)
        );
        assert_eq!(
            fs::read(&outside).expect("read outside file"),
            b"outside data"
        );

        assert_eq!(
            rebuild_owned_cache_store(&config, &path),
            StoreStatus::Ready
        );
        let rebuilt = connection::open_writer(&path).expect("open rebuilt store");
        assert_eq!(
            connection::integrity_check(&rebuilt).expect("integrity check"),
            "ok"
        );
        drop(rebuilt);
        let reader = connection::open_read_only(&path).expect("open rebuilt reader");
        assert_eq!(
            connection::read_schema_version(&reader).expect("schema version"),
            migrations::CURRENT_SCHEMA_VERSION
        );
    }

    #[cfg(unix)]
    #[test]
    fn rebuild_refuses_symlink_target_and_symlinked_store_directory() {
        let target_temp = tempfile::tempdir().expect("target temp directory");
        let target_path = store_path(&target_temp);
        fs::create_dir_all(target_path.parent().expect("store parent"))
            .expect("create store directory");
        let outside = target_temp.path().join("outside.sqlite3");
        fs::write(&outside, b"outside target").expect("write outside target");
        std::os::unix::fs::symlink(&outside, &target_path).expect("symlink store target");
        let target_config = StoreConfig::new(&target_path, true);

        assert_eq!(
            rebuild_owned_cache_store(&target_config, &target_path),
            StoreStatus::Skipped(StoreSkipReason::UnsafePath)
        );
        assert_eq!(
            fs::read(&outside).expect("read outside target"),
            b"outside target"
        );

        let ancestor_temp = tempfile::tempdir().expect("ancestor temp directory");
        let cache_root = ancestor_temp.path().join("cache");
        fs::create_dir_all(&cache_root).expect("create cache root");
        let outside_dir = ancestor_temp.path().join("outside-store");
        fs::create_dir_all(&outside_dir).expect("create outside store");
        let outside_db = outside_dir.join("store.sqlite3");
        fs::write(&outside_db, b"outside ancestor target").expect("write outside database");
        std::os::unix::fs::symlink(&outside_dir, cache_root.join("semantic-store"))
            .expect("symlink store directory");
        let ancestor_path = cache_root.join("semantic-store/store.sqlite3");
        let ancestor_config = StoreConfig::new(&ancestor_path, true);

        assert_eq!(
            rebuild_owned_cache_store(&ancestor_config, &ancestor_path),
            StoreStatus::Skipped(StoreSkipReason::UnsafePath)
        );
        assert_eq!(
            fs::read(&outside_db).expect("read outside database"),
            b"outside ancestor target"
        );
    }
}
