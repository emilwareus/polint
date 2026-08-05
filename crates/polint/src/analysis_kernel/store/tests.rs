use super::*;
use crate::analysis_kernel::go_syntax_projection::{
    CanonicalGoSyntaxOutput, GoSyntaxProviderProjection,
};
use crate::analysis_kernel::incremental::{
    Digest, DigestKind, FileSnapshot, PrecisionTier, RunManifest, RunManifestInputs,
};
use crate::analysis_kernel::metrics_projection::{
    CanonicalMetricsInputs, MetricsProviderProjection,
};
use crate::analysis_kernel::{
    AnalysisKernel, KernelInput, ProviderFailureReason, ProviderFailureStage, ProviderOutcome,
    ProviderOutcomeStatus,
};
use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::config::load_config;
use crate::core::{AnalysisDb, Language};

fn metrics_projection() -> MetricsProviderProjection {
    let outcome = ProviderOutcome::from_closed_parts(
        "polint.metrics".into(),
        ProviderOutcomeStatus::PlannedAbsent,
        None,
        Some(ProviderFailureStage::Planning),
        Some(ProviderFailureReason::NotSelected),
        Vec::new(),
    )
    .expect("sealed planned-absent metrics outcome");
    MetricsProviderProjection::from_db(outcome, &AnalysisDb::new()).expect("metrics projection")
}

fn go_syntax_projection() -> GoSyntaxProviderProjection {
    let outcome = ProviderOutcome::from_closed_parts(
        "polint.go.syntax".into(),
        ProviderOutcomeStatus::PlannedAbsent,
        None,
        Some(ProviderFailureStage::Planning),
        Some(ProviderFailureReason::NotSelected),
        Vec::new(),
    )
    .expect("sealed planned-absent Go syntax outcome");
    GoSyntaxProviderProjection::from_db(outcome, &AnalysisDb::new(), &[])
        .expect("Go syntax projection")
}

#[rustfmt::skip]
fn successful_go_syntax_projection() -> (GoSyntaxProviderProjection, FileSnapshot) {
    const SOURCE: &str = "package main\nfunc main() {}\n";
    let mut db = AnalysisDb::new();
    db.add_file("main.go".into(), "main.go".into(), SOURCE.into());
    let manifest = crate::analysis_kernel::provider::provider_manifests().iter()
        .find(|manifest| manifest.id == "polint.go.syntax").expect("Go syntax manifest");
    let output = CanonicalGoSyntaxOutput::from_db(&db, &[]).unwrap().digest();
    let identity = crate::analysis_kernel::incremental::provider_output_identity_from_manifest(manifest, output);
    let outcome = ProviderOutcome::from_closed_parts(
        manifest.id.into(), ProviderOutcomeStatus::Succeeded, Some(identity), None, None, Vec::new(),
    ).unwrap();
    let projection = GoSyntaxProviderProjection::from_db(outcome, &db, &[]).unwrap();
    let input = &projection.inputs.as_ref().unwrap().sources[0];
    let snapshot = FileSnapshot {
        relative_path: input.path.clone(), language: Language::Go,
        source_text_digest: input.source_digest.clone(), size_bytes: SOURCE.len(),
        mtime_hint_present: false,
    };
    (projection, snapshot)
}

fn publication(inputs: RunManifestInputs<'_>) -> PublicationInputs<'_> {
    PublicationInputs::new(inputs, metrics_projection(), go_syntax_projection())
}

#[derive(Clone)]
struct ManifestFixture {
    config_hash: String,
    files: Vec<FileSnapshot>,
}

impl ManifestFixture {
    fn new(config_seed: &str, files: Vec<FileSnapshot>) -> Self {
        Self {
            config_hash: Digest::from_parts(DigestKind::Config, "store-test", &[config_seed]).value,
            files,
        }
    }

    fn inputs<'a>(&'a self, workspace: &'a Path) -> RunManifestInputs<'a> {
        RunManifestInputs::new(workspace, &self.config_hash, &self.files)
    }
}

fn source(path: &str, language: Language, digest_seed: &str, size_bytes: usize) -> FileSnapshot {
    FileSnapshot {
        relative_path: path.to_string(),
        language,
        source_text_digest: Digest::from_parts(
            DigestKind::SourceText,
            "store-test",
            &[digest_seed],
        ),
        size_bytes,
        mtime_hint_present: false,
    }
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
fn status_vocabulary_is_typed_and_comparable() {
    let statuses = [
        StoreStatus::Disabled,
        StoreStatus::Ready,
        StoreStatus::BusySkipped,
        StoreStatus::Skipped(StoreSkipReason::FutureSchema {
            found: 2,
            supported: 1,
        }),
        StoreStatus::Skipped(StoreSkipReason::UnsafePath),
        StoreStatus::Skipped(StoreSkipReason::OpenFailed),
        StoreStatus::RebuildNeeded(StoreRebuildReason::Corrupt),
        StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema),
    ];

    assert_eq!(statuses.len(), 8);
}

#[test]
fn semantic_store_is_zero_sized_facade() {
    assert_eq!(std::mem::size_of::<SemanticStore>(), 0);
}

mod run_manifest_storage {
    use super::*;

    fn config(temp: &tempfile::TempDir) -> StoreConfig {
        StoreConfig::new(temp.path().join("cache/semantic-store/store.sqlite3"), true)
    }

    fn publish(
        temp: &tempfile::TempDir,
        files: &[FileSnapshot],
    ) -> (StoreConfig, GenerationHandle, RunManifest) {
        let store_config = config(temp);
        assert_eq!(SemanticStore::maintain(&store_config), StoreStatus::Ready);
        let fixture = ManifestFixture::new("config", files.to_vec());
        let expected =
            RunManifest::from_inputs(fixture.inputs(temp.path())).expect("construct run manifest");
        let mut writer = connection::open_writer(store_config.path()).expect("open writer");
        let handle = generation::reserve(&mut writer).expect("reserve generation");
        generation::publish(
            &mut writer,
            handle,
            &expected,
            &metrics_projection(),
            &go_syntax_projection(),
        )
        .expect("publish manifest storage");
        let reader = connection::open_read_only(store_config.path()).expect("open read-only store");
        let decoded =
            generation::read_manifest_for_test(&reader, handle).expect("read manifest storage");
        assert!(decoded.exact_match(&expected));
        (store_config, handle, expected)
    }

    #[test]
    fn exact_manifest_storage_round_trips_empty_and_populated_sources() {
        for files in [
            Vec::new(),
            vec![source("src/app.ts", Language::TypeScript, "app", 12)],
        ] {
            let temp = tempfile::tempdir().expect("temp directory");
            let (store_config, handle, expected) = publish(&temp, &files);
            let reader =
                connection::open_read_only(store_config.path()).expect("open read-only store");
            let decoded =
                generation::read_manifest_for_test(&reader, handle).expect("read manifest");

            assert!(decoded.exact_match(&expected));
        }
    }

    #[test]
    fn storage_preflight_rejects_wrong_types_counts_lengths_and_aggregate_bytes() {
        enum Tamper {
            StorageClass,
            Count,
            ScalarLength,
            Aggregate,
        }
        for tamper in [
            Tamper::StorageClass,
            Tamper::Count,
            Tamper::ScalarLength,
            Tamper::Aggregate,
        ] {
            let temp = tempfile::tempdir().expect("temp directory");
            let (store_config, handle, _) = publish(
                &temp,
                &[source("src/app.ts", Language::TypeScript, "app", 12)],
            );
            let connection =
                rusqlite::Connection::open(store_config.path()).expect("open tamper connection");
            connection
                .pragma_update(None, "ignore_check_constraints", true)
                .expect("allow malformed fixture");
            match tamper {
                Tamper::StorageClass => {
                    connection
                        .execute(
                            "UPDATE run_manifest_sources SET size_bytes = 'not-an-integer'",
                            [],
                        )
                        .expect("tamper storage class");
                }
                Tamper::Count => {
                    connection
                        .execute("UPDATE run_manifests SET source_count = 1000001", [])
                        .expect("tamper count");
                }
                Tamper::ScalarLength => {
                    connection
                        .execute(
                            "UPDATE run_manifests SET workspace_value = ?1",
                            ["f".repeat(17)],
                        )
                        .expect("tamper scalar length");
                }
                Tamper::Aggregate => {
                    connection
                        .execute(
                            "UPDATE run_manifest_sources SET relative_path = ?1",
                            [format!("src/{}.ts", "a".repeat(300))],
                        )
                        .expect("tamper aggregate bytes");
                }
            }

            // Only the aggregate case needs a lowered ceiling; the rest must
            // still be rejected under the production limit.
            let limit = match tamper {
                Tamper::Aggregate => 384,
                _ => super::super::MAX_AGGREGATE_BYTES,
            };
            assert_eq!(
                super::super::with_aggregate_bytes_limit(limit, || {
                    generation::read_manifest(&connection, handle).map(|_| ())
                }),
                Err(GenerationError::InvalidManifest)
            );
        }
    }
}

mod provider_mirror_storage {
    use super::*;

    #[test]
    fn legal_non_success_round_trips_but_never_matches_reusable() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = StoreConfig::new(temp.path().join("cache/semantic-store/store.sqlite3"), true);
        let manifest = ManifestFixture::new("provider", Vec::new());
        let expected = metrics_projection();
        let handle = SemanticStore::reserve_generation(&config).expect("reserve generation");
        SemanticStore::publish_generation(
            &config,
            handle,
            PublicationInputs::new(
                manifest.inputs(temp.path()),
                expected.clone(),
                go_syntax_projection(),
            ),
        )
        .expect("publish provider mirror");
        assert_eq!(
            SemanticStore::active_metrics(&config).unwrap(),
            Some((handle, expected.clone()))
        );
        assert_eq!(
            SemanticStore::match_active_metrics(&config, manifest.inputs(temp.path()), &expected)
                .unwrap(),
            MetricsMatch::SemanticMiss
        );
    }
}

mod go_syntax_provider_mirror_storage {
    use super::*;

    #[test]
    #[rustfmt::skip]
    fn required_projection_round_trips_with_exact_catalog_and_members() {
        let temp = tempfile::tempdir().unwrap();
        let config = StoreConfig::new(temp.path().join("store.sqlite3"), true);
        let (expected, source) = successful_go_syntax_projection();
        let manifest = ManifestFixture::new("go", vec![source]);
        assert_eq!(SemanticStore::match_active_go_syntax(&config, manifest.inputs(temp.path()), &expected).unwrap(), GoSyntaxMatch::NoActive);
        let handle = SemanticStore::reserve_generation(&config).unwrap();
        SemanticStore::publish_generation(
            &config, handle, PublicationInputs::new(manifest.inputs(temp.path()), metrics_projection(), expected.clone()),
        ).unwrap();
        assert_eq!(SemanticStore::active_go_syntax(&config).unwrap(), Some((handle, expected.clone())));
        assert_eq!(
            SemanticStore::match_active_go_syntax(
                &config,
                ManifestFixture::new("other-config", manifest.files).inputs(temp.path()),
                &expected,
            ).unwrap(),
            GoSyntaxMatch::Exact(handle)
        );
        let connection = rusqlite::Connection::open(config.path()).unwrap();
        let tables = connection.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'go_syntax_provider_%' ORDER BY name").unwrap()
            .query_map([], |row| row.get(0)).unwrap().collect::<Result<Vec<String>, _>>().unwrap();
        assert_eq!(tables, ["go_syntax_provider_blockers", "go_syntax_provider_members", "go_syntax_provider_mirror", "go_syntax_provider_parser", "go_syntax_provider_sources"]);
        let members = connection.prepare("SELECT category||':'||name||':'||version FROM go_syntax_provider_members ORDER BY CASE category WHEN 'input' THEN 0 WHEN 'output' THEN 1 ELSE 2 END, ordinal").unwrap()
            .query_map([], |row| row.get(0)).unwrap().collect::<Result<Vec<String>, _>>().unwrap();
        assert_eq!(members, ["input:source_files:0", "output:packages:0", "output:functions:0", "output:imports:0", "output:go_tests:0", "output:branch_obligations:0", "output:string_literals:0", "schema:go-facts-v2:2"]);

        let absent = go_syntax_projection();
        let second = tempfile::tempdir().unwrap();
        let second_config = StoreConfig::new(second.path().join("store.sqlite3"), true);
        let second_manifest = ManifestFixture::new("absent", Vec::new());
        let second_handle = SemanticStore::reserve_generation(&second_config).unwrap();
        SemanticStore::publish_generation(&second_config, second_handle,
            PublicationInputs::new(second_manifest.inputs(second.path()), metrics_projection(), absent.clone())).unwrap();
        assert_eq!(SemanticStore::active_go_syntax(&second_config).unwrap(), Some((second_handle, absent.clone())));
        assert_eq!(SemanticStore::match_active_go_syntax(&second_config, second_manifest.inputs(second.path()), &absent).unwrap(), GoSyntaxMatch::SemanticMiss);
    }
}

mod provider_mirror {
    use super::*;

    const SOURCE: &str =
        "export function score(value: number) {\n  return value > 0 ? value : 0;\n}\n";

    fn configured_projection(
        root: &Path,
        source_text: &str,
        config_digest: &str,
        rule_digest: &str,
        capability: &str,
        cache_enabled: bool,
    ) -> MetricsProviderProjection {
        std::fs::write(root.join("main.ts"), source_text).expect("write TypeScript source");
        let loaded = load_config(root).expect("load default config");
        let cache = Cache::new(root.join("analysis-cache"), cache_enabled);
        let plan = AnalysisPlan::from_capability_names_for_test(&[capability]);
        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest,
            rule_digest,
            plan: &plan,
            parallel: false,
        })
        .expect("run real metrics kernel");
        let outcome = output
            .run_report
            .provider_outcomes
            .iter()
            .find(|row| row.provider_id == "polint.metrics")
            .expect("sealed metrics outcome")
            .clone();
        assert_eq!(outcome.status, ProviderOutcomeStatus::Succeeded);
        MetricsProviderProjection::from_db(outcome, &output.db).expect("canonical real projection")
    }

    fn real_projection(root: &Path, source_text: &str) -> MetricsProviderProjection {
        configured_projection(
            root,
            source_text,
            "broad-config",
            "broad-rules",
            "file_metrics",
            false,
        )
    }

    fn manifest(projection: &MetricsProviderProjection, config_seed: &str) -> ManifestFixture {
        let files = projection
            .inputs
            .as_ref()
            .expect("successful projection inputs")
            .sources
            .iter()
            .map(|row| FileSnapshot {
                relative_path: row.path.clone(),
                language: row.language,
                source_text_digest: row.source_digest.clone(),
                size_bytes: row.byte_count as usize,
                mtime_hint_present: false,
            })
            .collect();
        ManifestFixture::new(config_seed, files)
    }

    fn publish_real(
        temp: &tempfile::TempDir,
    ) -> (
        StoreConfig,
        GenerationHandle,
        ManifestFixture,
        MetricsProviderProjection,
    ) {
        let projection = real_projection(temp.path(), SOURCE);
        let manifest = manifest(&projection, "real-config");
        let config = StoreConfig::new(temp.path().join("cache/semantic-store/store.sqlite3"), true);
        let handle = SemanticStore::reserve_generation(&config).expect("reserve real generation");
        SemanticStore::publish_generation(
            &config,
            handle,
            PublicationInputs::new(
                manifest.inputs(temp.path()),
                projection.clone(),
                go_syntax_projection(),
            ),
        )
        .expect("publish real projection");
        (config, handle, manifest, projection)
    }

    fn changed(
        baseline: &MetricsProviderProjection,
        edit: impl FnOnce(&mut CanonicalMetricsInputs),
    ) -> MetricsProviderProjection {
        let mut changed = baseline.clone();
        edit(changed.inputs.as_mut().expect("successful inputs"));
        changed
    }

    #[test]
    fn real_sealed_success_reopens_and_matches_exactly() {
        let temp = tempfile::tempdir().expect("temp directory");
        let (config, handle, baseline, expected) = publish_real(&temp);

        assert!(expected.outcome.output_identity.is_some());
        assert!(expected.outcome.blockers.is_empty());
        assert!(!expected.inputs.as_ref().unwrap().sources.is_empty());
        assert!(!expected.inputs.as_ref().unwrap().functions.is_empty());
        assert_eq!(
            SemanticStore::active_metrics(&config).expect("reopen active projection"),
            Some((handle, expected.clone()))
        );
        assert_eq!(
            SemanticStore::match_active_metrics(&config, baseline.inputs(temp.path()), &expected,)
                .expect("match exact projection"),
            MetricsMatch::Exact(handle)
        );
        assert_eq!(
            SemanticStore::match_active_manifest(&config, baseline.inputs(temp.path())).unwrap(),
            ManifestMatch::Exact(handle)
        );

        let broad_variant = configured_projection(
            temp.path(),
            SOURCE,
            "changed-config",
            "changed-rules",
            "complexity_metrics",
            true,
        );
        assert_eq!(broad_variant, expected);
        let changed_manifest = manifest(&expected, "changed-run-config");
        assert_eq!(
            SemanticStore::match_active_metrics(
                &config,
                changed_manifest.inputs(temp.path()),
                &broad_variant,
            )
            .unwrap(),
            MetricsMatch::Exact(handle)
        );
        assert_eq!(
            SemanticStore::match_active_manifest(&config, changed_manifest.inputs(temp.path()),)
                .unwrap(),
            ManifestMatch::Mismatch
        );
    }

    #[test]
    fn every_legal_non_success_shape_round_trips_and_rejects_dependency_rows() {
        for (status, stage, reason, blockers) in [
            (
                ProviderOutcomeStatus::Failed,
                ProviderFailureStage::Execution,
                ProviderFailureReason::ExecutionFailed,
                Vec::new(),
            ),
            (
                ProviderOutcomeStatus::Failed,
                ProviderFailureStage::Validation,
                ProviderFailureReason::ValidationRejected,
                Vec::new(),
            ),
            (
                ProviderOutcomeStatus::DependencyBlocked,
                ProviderFailureStage::Dependency,
                ProviderFailureReason::DependencyUnavailable,
                vec!["polint.ts.syntax".into()],
            ),
            (
                ProviderOutcomeStatus::Unsupported,
                ProviderFailureStage::Setup,
                ProviderFailureReason::Unsupported,
                Vec::new(),
            ),
            (
                ProviderOutcomeStatus::SetupMissing,
                ProviderFailureStage::Setup,
                ProviderFailureReason::SetupMissing,
                Vec::new(),
            ),
            (
                ProviderOutcomeStatus::PlannedAbsent,
                ProviderFailureStage::Planning,
                ProviderFailureReason::NotSelected,
                Vec::new(),
            ),
        ] {
            let temp = tempfile::tempdir().expect("temp directory");
            let config =
                StoreConfig::new(temp.path().join("cache/semantic-store/store.sqlite3"), true);
            let manifest = ManifestFixture::new(status.label(), Vec::new());
            let outcome = ProviderOutcome::from_closed_parts(
                "polint.metrics".into(),
                status,
                None,
                Some(stage),
                Some(reason),
                blockers,
            )
            .expect("seal legal non-success");
            let projection = MetricsProviderProjection::from_db(outcome, &AnalysisDb::new())
                .expect("project legal non-success");
            let handle = SemanticStore::reserve_generation(&config).expect("reserve generation");
            SemanticStore::publish_generation(
                &config,
                handle,
                PublicationInputs::new(
                    manifest.inputs(temp.path()),
                    projection.clone(),
                    go_syntax_projection(),
                ),
            )
            .expect("publish non-success history");
            assert_eq!(
                SemanticStore::active_metrics(&config).unwrap(),
                Some((handle, projection.clone()))
            );
            assert!(projection.outcome.output_identity.is_none());
            assert!(projection.inputs.is_none());
            assert_eq!(
                SemanticStore::match_active_metrics(
                    &config,
                    manifest.inputs(temp.path()),
                    &projection,
                )
                .unwrap(),
                MetricsMatch::SemanticMiss
            );
            let connection =
                rusqlite::Connection::open(config.path()).expect("open tamper connection");
            connection
                .pragma_update(None, "ignore_check_constraints", true)
                .expect("allow malformed fixture");
            connection
                .execute_batch(
                    "INSERT INTO metrics_provider_sources SELECT generation_id,0,'main.ts','typescript','source_text','0000000000000000',1,1,1 FROM metrics_provider_mirror; \
                     INSERT INTO metrics_provider_functions SELECT generation_id,0,'main.ts','f',0,1,1,1,'typescript',1 FROM metrics_provider_mirror; \
                     UPDATE metrics_provider_mirror SET source_count=1,function_count=1;",
                )
                .expect("add coherent non-success dependencies");
            assert_eq!(
                super::super::provider_mirror::read(&connection, handle),
                Err(GenerationError::InvalidProviderMirror)
            );
            drop(connection);
            assert_eq!(
                SemanticStore::active_metrics(&config),
                Err(GenerationError::Store(StoreStatus::RebuildNeeded(
                    StoreRebuildReason::InvalidSchema
                )))
            );
        }
    }

    #[test]
    fn oversized_provider_catalog_sql_is_rejected_as_invalid_schema() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = StoreConfig::new(temp.path().join("cache/semantic-store/store.sqlite3"), true);
        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);
        let connection = rusqlite::Connection::open(config.path()).expect("open tamper connection");
        connection
            .pragma_update(None, "writable_schema", true)
            .expect("allow hostile catalog fixture");
        let updated = connection
            .execute(
                "UPDATE sqlite_master SET sql=substr(sql,1,instr(sql,'(')) || \
                 printf('%100000s',' ') || substr(sql,instr(sql,'(')+1) \
                 WHERE type='table' AND name='metrics_provider_sources'",
                [],
            )
            .expect("inflate provider declaration inside SQLite");
        assert_eq!(updated, 1);
        connection
            .pragma_update(None, "writable_schema", false)
            .expect("seal hostile catalog fixture");
        drop(connection);

        assert_eq!(
            SemanticStore::maintain(&config),
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)
        );
    }

    #[test]
    fn consumed_source_and_function_matrix_misses_but_locked_exclusions_preserve() {
        let temp = tempfile::tempdir().expect("temp directory");
        let (config, handle, baseline, expected) = publish_real(&temp);
        let mut mutations = Vec::new();
        mutations.push(changed(&expected, |rows| rows.sources.clear()));
        mutations.push(changed(&expected, |rows| {
            rows.sources.push(rows.sources[0].clone())
        }));
        mutations.push(changed(&expected, |rows| {
            rows.sources[0].path = "renamed.ts".into()
        }));
        mutations.push(changed(&expected, |rows| {
            rows.sources[0].source_digest.value = "0000000000000000".into()
        }));
        mutations.push(changed(&expected, |rows| rows.sources[0].byte_count += 1));
        mutations.push(changed(&expected, |rows| rows.sources[0].line_count += 1));
        mutations.push(changed(&expected, |rows| {
            rows.sources[0].non_empty_line_count -= 1
        }));
        mutations.push(changed(&expected, |rows| {
            rows.sources[0].language = Language::JavaScript
        }));
        mutations.push(changed(&expected, |rows| rows.functions.clear()));
        mutations.push(changed(&expected, |rows| {
            rows.functions.push(rows.functions[0].clone())
        }));
        mutations.push(changed(&expected, |rows| {
            rows.functions[0].path = "renamed.ts".into()
        }));
        mutations.push(changed(&expected, |rows| {
            rows.functions[0].name = "renamed".into()
        }));
        mutations.push(changed(&expected, |rows| rows.functions[0].start_byte += 1));
        mutations.push(changed(&expected, |rows| rows.functions[0].end_byte -= 1));
        mutations.push(changed(&expected, |rows| rows.functions[0].start_line += 1));
        mutations.push(changed(&expected, |rows| rows.functions[0].end_line -= 1));
        mutations.push(changed(&expected, |rows| {
            rows.functions[0].language = Language::JavaScript
        }));
        mutations.push(changed(&expected, |rows| {
            rows.functions[0].cyclomatic_complexity += 1
        }));
        for requested in &mutations {
            assert_eq!(
                SemanticStore::match_active_metrics(
                    &config,
                    baseline.inputs(temp.path()),
                    requested,
                )
                .unwrap(),
                MetricsMatch::SemanticMiss
            );
        }

        let edited = real_projection(
            temp.path(),
            "export function score(value: number) { return value + 1; }\n",
        );
        assert_eq!(
            SemanticStore::match_active_metrics(
                &config,
                manifest(&edited, "edited").inputs(temp.path()),
                &edited,
            )
            .unwrap(),
            MetricsMatch::SemanticMiss
        );
        std::fs::write(temp.path().join("extra.ts"), "export const extra = 1;\n")
            .expect("write membership edit");
        let added = real_projection(temp.path(), SOURCE);
        assert_eq!(
            SemanticStore::match_active_metrics(
                &config,
                manifest(&added, "added").inputs(temp.path()),
                &added,
            )
            .unwrap(),
            MetricsMatch::SemanticMiss
        );
        assert_eq!(
            SemanticStore::match_active_metrics(
                &config,
                manifest(&expected, "cache-mode-changed").inputs(temp.path()),
                &expected,
            )
            .unwrap(),
            MetricsMatch::Exact(handle)
        );
    }

    fn tamper_then_refuse(statement: &str) {
        let temp = tempfile::tempdir().expect("temp directory");
        let (config, _, _, _) = publish_real(&temp);
        let connection = rusqlite::Connection::open(config.path()).expect("open tamper connection");
        connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .expect("allow malformed fixture");
        connection.execute_batch(statement).expect("apply tamper");
        drop(connection);
        assert!(
            SemanticStore::active_metrics(&config).is_err(),
            "provider tamper was trusted: {statement}"
        );
        let preserved = rusqlite::Connection::open(config.path()).expect("reopen refused bytes");
        let lifecycle: (i64, i64) = preserved
            .query_row(
                "SELECT (SELECT count(*) FROM generations), \
                 (SELECT count(*) FROM active_generation)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read preserved lifecycle");
        assert_eq!(lifecycle, (1, 1), "refusal mutated lifecycle: {statement}");
    }

    #[test]
    fn relational_semantic_bounds_and_catalog_tamper_fail_closed() {
        for statement in [
            "UPDATE metrics_provider_mirror SET mirror_schema='wrong'",
            "UPDATE metrics_provider_mirror SET provider_id='wrong'",
            "UPDATE metrics_provider_mirror SET provider_version='wrong'",
            "UPDATE metrics_provider_mirror SET provider_kind='wrong'",
            "UPDATE metrics_provider_mirror SET language_scope='wrong'",
            "UPDATE metrics_provider_mirror SET cache_policy='wrong'",
            "UPDATE metrics_provider_mirror SET precision_ceiling='wrong'",
            "UPDATE metrics_provider_mirror SET outcome_status='failed'",
            "UPDATE metrics_provider_mirror SET failure_stage='execution'",
            "UPDATE metrics_provider_mirror SET failure_reason='execution_failed'",
            "UPDATE metrics_provider_mirror SET witness_kind='wrong'",
            "UPDATE metrics_provider_mirror SET witness_value='0000000000000000'",
            "UPDATE metrics_provider_mirror SET identity_provider_id='wrong'",
            "UPDATE metrics_provider_mirror SET identity_provider_version='wrong'",
            "UPDATE metrics_provider_mirror SET identity_schema_version='wrong'",
            "UPDATE metrics_provider_mirror SET identity_digest_kind='wrong'",
            "UPDATE metrics_provider_mirror SET identity_digest_value='0000000000000000'",
            "UPDATE metrics_provider_mirror SET identity_precision='exact'",
            "UPDATE metrics_provider_mirror SET identity_provider_id=printf('%9000s','x')",
            "UPDATE metrics_provider_mirror SET member_count=5",
            "UPDATE metrics_provider_mirror SET source_count=1000001",
            "UPDATE metrics_provider_members SET name='wrong' WHERE category='input' AND ordinal=0",
            "UPDATE metrics_provider_members SET version=7 WHERE category='schema'",
            "UPDATE metrics_provider_sources SET ordinal=7",
            "UPDATE metrics_provider_sources SET relative_path='renamed.ts'",
            "UPDATE metrics_provider_sources SET language='javascript'",
            "UPDATE metrics_provider_sources SET digest_kind='wrong'",
            "UPDATE metrics_provider_sources SET digest_value='0000000000000000'",
            "UPDATE metrics_provider_sources SET byte_count='wrong'",
            "UPDATE metrics_provider_sources SET line_count=0",
            "UPDATE metrics_provider_sources SET non_empty_line_count=0",
            "UPDATE metrics_provider_functions SET ordinal=7",
            "UPDATE metrics_provider_functions SET relative_path='renamed.ts'",
            "UPDATE metrics_provider_functions SET name='wrong'",
            "UPDATE metrics_provider_functions SET start_byte=1",
            "UPDATE metrics_provider_functions SET end_byte=1",
            "UPDATE metrics_provider_functions SET start_line=2",
            "UPDATE metrics_provider_functions SET end_line=1",
            "UPDATE metrics_provider_functions SET language='javascript'",
            "UPDATE metrics_provider_functions SET complexity=99",
            "DELETE FROM metrics_provider_mirror",
            "DELETE FROM metrics_provider_members WHERE category='input' AND ordinal=0",
            "DELETE FROM metrics_provider_sources",
            "DELETE FROM metrics_provider_functions",
            "INSERT INTO metrics_provider_sources SELECT generation_id,99,'extra.ts','typescript','source_text','0000000000000000',0,0,0 FROM metrics_provider_mirror",
            "INSERT INTO metrics_provider_functions SELECT generation_id,99,relative_path,'extra',0,0,1,1,language,1 FROM metrics_provider_sources",
            "UPDATE metrics_provider_mirror SET blocker_count=1; INSERT INTO metrics_provider_blockers SELECT generation_id,0,'unknown.provider' FROM metrics_provider_mirror",
            "UPDATE metrics_provider_sources SET relative_path=printf('%9000s','x')",
            "UPDATE metrics_provider_sources SET relative_path=replace(hex(zeroblob(4090)),'00','a'); UPDATE metrics_provider_functions SET relative_path=replace(hex(zeroblob(4090)),'00','a'); INSERT INTO metrics_provider_sources SELECT generation_id,1,replace(hex(zeroblob(4090)),'00','b'),'typescript','source_text','0000000000000000',0,0,0 FROM metrics_provider_mirror; INSERT INTO metrics_provider_sources SELECT generation_id,2,replace(hex(zeroblob(4090)),'00','c'),'typescript','source_text','0000000000000000',0,0,0 FROM metrics_provider_mirror; UPDATE metrics_provider_mirror SET source_count=3",
            "UPDATE metrics_provider_sources SET generation_id=999",
            "CREATE INDEX metrics_provider_sources_extra ON metrics_provider_sources(language)",
            "CREATE TRIGGER metrics_provider_mirror_extra AFTER UPDATE ON metrics_provider_mirror BEGIN SELECT 1; END",
            "ALTER TABLE metrics_provider_functions ADD COLUMN surprise TEXT",
        ] {
            tamper_then_refuse(statement);
        }
    }
}

mod go_syntax_provider_mirror {
    use super::*;

    const GO_MAIN: &str = "package sample\nimport \"fmt\"\nfunc Answer(v string) string { if v == \"\" { return \"empty\" }; return fmt.Sprintf(\"%s\", v) }\n";
    const GO_TEST: &str = "package sample\nimport \"testing\"\nfunc TestAnswer(t *testing.T) { t.Run(\"empty\", func(t *testing.T) { if Answer(\"\") != \"empty\" { t.Fatal(\"bad\") } }) }\n";
    const TS_SOURCE: &str = "export const unrelated = 'ts-only';\n";

    fn write_fixture(root: &Path) {
        std::fs::write(root.join("a.go"), GO_MAIN).unwrap();
        std::fs::write(root.join("b_test.go"), GO_TEST).unwrap();
        std::fs::write(root.join("unrelated.ts"), TS_SOURCE).unwrap();
    }

    #[rustfmt::skip]
    fn projections(root: &Path, config_digest: &str, rule_digest: &str, metric_capability: &str, cache_enabled: bool) -> (ManifestFixture, MetricsProviderProjection, GoSyntaxProviderProjection) {
        let loaded = load_config(root).unwrap();
        let cache = Cache::new(root.join("analysis-cache"), cache_enabled);
        let plan = AnalysisPlan::from_capability_names_for_test(&[metric_capability, "string_literals", "go_tests", "branch_obligations"]);
        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded, cache: &cache, config_digest, rule_digest, plan: &plan, parallel: false,
        }).unwrap();
        let outcome = |provider: &str| output.run_report.provider_outcomes.iter().find(|outcome| outcome.provider_id == provider).unwrap().clone();
        let metrics = MetricsProviderProjection::from_db(outcome("polint.metrics"), &output.db).unwrap();
        let go = GoSyntaxProviderProjection::from_db(outcome("polint.go.syntax"), &output.db, &output.diagnostics).unwrap();
        let files = metrics.inputs.as_ref().unwrap().sources.iter()
            .map(|source| FileSnapshot {
                relative_path: source.path.clone(), language: source.language,
                source_text_digest: source.source_digest.clone(), size_bytes: source.byte_count as usize, mtime_hint_present: false,
            }).collect();
        (ManifestFixture::new(config_digest, files), metrics, go)
    }

    #[rustfmt::skip]
    fn publish_real(temp: &tempfile::TempDir) -> (StoreConfig, GenerationHandle, ManifestFixture, MetricsProviderProjection, GoSyntaxProviderProjection) {
        write_fixture(temp.path());
        let (manifest, metrics, go) = projections(temp.path(), "config", "rules", "file_metrics", false);
        let config = StoreConfig::new(temp.path().join("semantic-store.sqlite3"), true);
        let handle = SemanticStore::reserve_generation(&config).unwrap();
        SemanticStore::publish_generation(&config, handle, PublicationInputs::new(manifest.inputs(temp.path()), metrics.clone(), go.clone())).unwrap();
        (config, handle, manifest, metrics, go)
    }

    #[rustfmt::skip]
    fn changed(baseline: &GoSyntaxProviderProjection, mutate: fn(&mut GoSyntaxProviderProjection)) -> GoSyntaxProviderProjection {
        let mut changed = baseline.clone();
        mutate(&mut changed);
        changed
    }

    #[test]
    #[rustfmt::skip]
    fn real_success_reopens_exactly_and_invalidation_polarity_is_provider_scoped() {
        let temp = tempfile::tempdir().unwrap();
        let (config, handle, manifest, metrics, expected) = publish_real(&temp);
        assert!(expected.outcome.status == ProviderOutcomeStatus::Succeeded && expected.outcome.output_identity.is_some() && expected.inputs.as_ref().unwrap().sources.len() == 2 && expected.parser.is_some());
        assert_eq!(SemanticStore::match_active_manifest(&config, manifest.inputs(temp.path())).unwrap(), ManifestMatch::Exact(handle));
        assert_eq!(SemanticStore::active_metrics(&config).unwrap(), Some((handle, metrics)));
        assert_eq!(SemanticStore::active_go_syntax(&config).unwrap(), Some((handle, expected.clone())));
        assert_eq!(SemanticStore::match_active_go_syntax(&config, manifest.inputs(temp.path()), &expected).unwrap(), GoSyntaxMatch::Exact(handle));

        for (config_digest, rule_digest, metric_capability, cache_enabled) in [
            ("changed-config", "rules", "file_metrics", false),
            ("config", "changed-rules", "file_metrics", false),
            ("config", "rules", "complexity_metrics", false),
            ("config", "rules", "file_metrics", true),
        ] {
            let (preserved_manifest, _, preserved) = projections(temp.path(), config_digest, rule_digest, metric_capability, cache_enabled);
            assert_eq!(preserved, expected);
            assert_eq!(SemanticStore::match_active_go_syntax(&config, preserved_manifest.inputs(temp.path()), &preserved).unwrap(), GoSyntaxMatch::Exact(handle));
        }
        std::fs::write(temp.path().join("unrelated.ts"), "export const changed = 2;\n").unwrap();
        let (ts_manifest, _, ts_changed) = projections(temp.path(), "config", "rules", "file_metrics", false);
        assert_eq!(ts_changed, expected);
        assert_eq!(SemanticStore::match_active_go_syntax(&config, ts_manifest.inputs(temp.path()), &ts_changed).unwrap(), GoSyntaxMatch::Exact(handle));
        std::fs::rename(temp.path().join("unrelated.ts"), temp.path().join("renamed.ts")).unwrap();
        let (ts_path_manifest, _, ts_path) = projections(temp.path(), "config", "rules", "file_metrics", false);
        assert_eq!(ts_path, expected);
        assert_eq!(SemanticStore::match_active_go_syntax(&config, ts_path_manifest.inputs(temp.path()), &ts_path).unwrap(), GoSyntaxMatch::Exact(handle));
        std::fs::write(temp.path().join("extra.ts"), "export {};\n").unwrap();
        let (ts_membership, _, ts_only) = projections(temp.path(), "config", "rules", "file_metrics", false);
        assert_eq!(ts_only, expected);
        assert_eq!(SemanticStore::match_active_go_syntax(&config, ts_membership.inputs(temp.path()), &ts_only).unwrap(), GoSyntaxMatch::Exact(handle));

        std::fs::write(temp.path().join("a.go"), "package sample\nfunc Changed() {}\n").unwrap();
        let (content_manifest, _, content) = projections(temp.path(), "config", "rules", "file_metrics", false);
        assert_eq!(SemanticStore::match_active_go_syntax(&config, content_manifest.inputs(temp.path()), &content).unwrap(), GoSyntaxMatch::SemanticMiss);
        std::fs::write(temp.path().join("a.go"), GO_MAIN).unwrap();
        std::fs::remove_file(temp.path().join("b_test.go")).unwrap();
        let (membership_manifest, _, membership) = projections(temp.path(), "config", "rules", "file_metrics", false);
        assert_eq!(SemanticStore::match_active_go_syntax(&config, membership_manifest.inputs(temp.path()), &membership).unwrap(), GoSyntaxMatch::SemanticMiss);
        std::fs::write(temp.path().join("b_test.go"), GO_TEST).unwrap();
        std::fs::rename(temp.path().join("a.go"), temp.path().join("renamed.go")).unwrap();
        let (path_manifest, _, path) = projections(temp.path(), "config", "rules", "file_metrics", false);
        assert_eq!(SemanticStore::match_active_go_syntax(&config, path_manifest.inputs(temp.path()), &path).unwrap(), GoSyntaxMatch::SemanticMiss);
        std::fs::rename(temp.path().join("renamed.go"), temp.path().join("a.go")).unwrap();

        let mutations: [fn(&mut GoSyntaxProviderProjection); 15] = [
            |row| { let source = row.inputs.as_ref().unwrap().sources[0].clone(); row.inputs.as_mut().unwrap().sources.push(source); }, |row| row.inputs.as_mut().unwrap().sources[0].language = Language::TypeScript,
            |row| row.inputs.as_mut().unwrap().sources.swap(0, 1), |row| row.parser.as_mut().unwrap().provider_id.push('x'),
            |row| row.parser.as_mut().unwrap().provider_version.push('x'), |row| row.parser.as_mut().unwrap().fact_schema.push('x'),
            |row| row.parser.as_mut().unwrap().payload_schema.push('x'), |row| row.parser.as_mut().unwrap().backend.push('x'),
            |row| row.parser.as_mut().unwrap().grammar.push('x'), |row| row.outcome.output_identity.as_mut().unwrap().provider_id.push('x'),
            |row| row.outcome.output_identity.as_mut().unwrap().provider_version.push('x'), |row| row.outcome.output_identity.as_mut().unwrap().schema_version.push('x'),
            |row| row.outcome.output_identity.as_mut().unwrap().output_digest.kind = DigestKind::Config, |row| row.outcome.output_identity.as_mut().unwrap().output_digest.value = "0000000000000000".into(),
            |row| row.outcome.output_identity.as_mut().unwrap().precision = PrecisionTier::Exact,
        ];
        for mutate in mutations {
            let requested = changed(&expected, mutate);
            assert_eq!(SemanticStore::match_active_go_syntax(&config, manifest.inputs(temp.path()), &requested).unwrap(), GoSyntaxMatch::SemanticMiss);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn legal_non_success_history_has_no_reusable_rows() {
        for (status, stage, reason, blockers) in [
            (ProviderOutcomeStatus::PlannedAbsent, ProviderFailureStage::Planning, ProviderFailureReason::NotSelected, Vec::new()),
            (ProviderOutcomeStatus::DependencyBlocked, ProviderFailureStage::Dependency, ProviderFailureReason::DependencyUnavailable, vec!["polint.source".into()]),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let config = StoreConfig::new(temp.path().join("store.sqlite3"), true);
            let manifest = ManifestFixture::new(status.label(), Vec::new());
            let outcome = ProviderOutcome::from_closed_parts("polint.go.syntax".into(), status, None, Some(stage), Some(reason), blockers).unwrap();
            let expected = GoSyntaxProviderProjection::from_db(outcome, &AnalysisDb::new(), &[]).unwrap();
            let handle = SemanticStore::reserve_generation(&config).unwrap();
            SemanticStore::publish_generation(&config, handle, PublicationInputs::new(manifest.inputs(temp.path()), metrics_projection(), expected.clone())).unwrap();
            assert_eq!(SemanticStore::active_go_syntax(&config).unwrap(), Some((handle, expected.clone())));
            assert_eq!(SemanticStore::match_active_go_syntax(&config, manifest.inputs(temp.path()), &expected).unwrap(), GoSyntaxMatch::SemanticMiss);
            let connection = rusqlite::Connection::open(config.path()).unwrap();
            let shape: (i64, i64, i64, i64) = connection.query_row("SELECT identity_provider_id IS NOT NULL,source_count,parser_count,blocker_count FROM go_syntax_provider_mirror", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).unwrap();
            assert_eq!(shape, (0, 0, 0, i64::from(status == ProviderOutcomeStatus::DependencyBlocked)));
        }
    }

    #[rustfmt::skip]
    fn tamper_then_refuse(statement: &str) -> (GenerationError, GenerationError) {
        let temp = tempfile::tempdir().unwrap();
        let (config, _, manifest, _, expected) = publish_real(&temp);
        let connection = rusqlite::Connection::open(config.path()).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;").unwrap();
        connection.execute_batch(statement).unwrap();
        drop(connection);
        let active = SemanticStore::active_go_syntax(&config).expect_err("trusted tamper");
        let matched = SemanticStore::match_active_go_syntax(&config, manifest.inputs(temp.path()), &expected).expect_err("matched tamper");
        let preserved = rusqlite::Connection::open(config.path()).unwrap();
        let lifecycle: (i64, i64) = preserved.query_row("SELECT (SELECT count(*) FROM generations),(SELECT count(*) FROM active_generation)", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!(lifecycle, (1, i64::from(statement != "DELETE FROM active_generation")));
        (active, matched)
    }

    #[test]
    #[rustfmt::skip]
    fn quoted_literal_whitespace_and_reserved_catalog_objects_are_refused() {
        let refused = GenerationError::Store(StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema));
        for statement in [
            "PRAGMA writable_schema=ON; UPDATE sqlite_master SET sql=replace(sql,'''unsupported''','''unsup ported''') WHERE type='table' AND name='go_syntax_provider_mirror'; PRAGMA writable_schema=OFF",
            "CREATE TABLE go_syntax_provider_shadow (value INTEGER); CREATE INDEX go_syntax_provider_shadow_index ON go_syntax_provider_shadow(value); CREATE TRIGGER go_syntax_provider_shadow_trigger AFTER INSERT ON go_syntax_provider_shadow BEGIN SELECT 1; END",
        ] { assert_eq!(tamper_then_refuse(statement), (refused.clone(), refused.clone())); }
    }

    #[test]
    #[rustfmt::skip]
    fn go_source_aggregate_is_bounded_after_relationship_preflight() {
        let temp = tempfile::tempdir().unwrap();
        let (config, handle, _, _, _) = publish_real(&temp);
        let connection = rusqlite::Connection::open(config.path()).unwrap();
        let first = format!("{}.go", "a".repeat(4087));
        let second = format!("{}.go", "b".repeat(4087));
        connection.execute("UPDATE go_syntax_provider_sources SET relative_path=?1 WHERE ordinal=0", [&first]).unwrap();
        connection.execute("UPDATE go_syntax_provider_sources SET relative_path=?1 WHERE ordinal=1", [&second]).unwrap();
        connection.execute("UPDATE run_manifest_sources SET relative_path=?1 WHERE relative_path='a.go'", [&first]).unwrap();
        connection.execute("UPDATE run_manifest_sources SET relative_path=?1 WHERE relative_path='b_test.go'", [&second]).unwrap();
        assert_eq!(super::super::with_aggregate_bytes_limit(8_192, || super::super::go_syntax_mirror::read(&connection, handle).map(|_| ())), Err(GenerationError::InvalidProviderMirror));
    }

    #[test]
    #[rustfmt::skip]
    fn manifest_outcome_dependency_bounds_and_catalog_tamper_fail_closed() {
        for statement in [
            "UPDATE go_syntax_provider_mirror SET mirror_schema='wrong'", "UPDATE go_syntax_provider_mirror SET provider_id='wrong'",
            "UPDATE go_syntax_provider_mirror SET provider_version='wrong'", "UPDATE go_syntax_provider_mirror SET provider_kind='wrong'",
            "UPDATE go_syntax_provider_mirror SET language_scope='wrong'", "UPDATE go_syntax_provider_mirror SET cache_policy='wrong'",
            "UPDATE go_syntax_provider_mirror SET precision_ceiling='exact'", "UPDATE go_syntax_provider_mirror SET outcome_status='failed'",
            "UPDATE go_syntax_provider_mirror SET outcome_status='dependency_blocked'", "UPDATE go_syntax_provider_mirror SET outcome_status='unsupported'",
            "UPDATE go_syntax_provider_mirror SET outcome_status='setup_missing'", "UPDATE go_syntax_provider_mirror SET outcome_status='planned_absent'",
            "UPDATE go_syntax_provider_mirror SET failure_stage='execution'", "UPDATE go_syntax_provider_mirror SET failure_reason='execution_failed'",
            "UPDATE go_syntax_provider_mirror SET member_count=7", "UPDATE go_syntax_provider_mirror SET blocker_count=1",
            "UPDATE go_syntax_provider_mirror SET source_count=0", "UPDATE go_syntax_provider_mirror SET parser_count=0",
            "UPDATE go_syntax_provider_mirror SET witness_kind='wrong'", "UPDATE go_syntax_provider_mirror SET witness_value='0000000000000000'",
            "UPDATE go_syntax_provider_mirror SET identity_provider_id='wrong'", "UPDATE go_syntax_provider_mirror SET identity_provider_version='wrong'",
            "UPDATE go_syntax_provider_mirror SET identity_schema_version='wrong'", "UPDATE go_syntax_provider_mirror SET identity_digest_kind='wrong'",
            "UPDATE go_syntax_provider_mirror SET identity_digest_value='0000000000000000'", "UPDATE go_syntax_provider_mirror SET identity_precision='exact'",
            "UPDATE go_syntax_provider_mirror SET identity_provider_id=zeroblob(16)", "UPDATE go_syntax_provider_mirror SET provider_version=printf('%9000s','x')",
            "UPDATE go_syntax_provider_mirror SET source_count=1000001", "UPDATE go_syntax_provider_members SET name='wrong' WHERE category='input'",
            "UPDATE go_syntax_provider_members SET name='wrong' WHERE category='output' AND ordinal=0", "UPDATE go_syntax_provider_members SET name='wrong' WHERE category='output' AND ordinal=1",
            "UPDATE go_syntax_provider_members SET name='wrong' WHERE category='output' AND ordinal=2", "UPDATE go_syntax_provider_members SET name='wrong' WHERE category='output' AND ordinal=3",
            "UPDATE go_syntax_provider_members SET name='wrong' WHERE category='output' AND ordinal=4", "UPDATE go_syntax_provider_members SET name='wrong' WHERE category='output' AND ordinal=5",
            "UPDATE go_syntax_provider_members SET name='wrong' WHERE category='schema'", "UPDATE go_syntax_provider_members SET category='wrong' WHERE category='schema'",
            "UPDATE go_syntax_provider_members SET version=7 WHERE category='schema'", "UPDATE go_syntax_provider_members SET ordinal=9 WHERE category='input'",
            "UPDATE go_syntax_provider_members SET version='wrong' WHERE category='output' AND ordinal=0",
            "DELETE FROM go_syntax_provider_members WHERE category='input'",
            "UPDATE go_syntax_provider_mirror SET outcome_status='dependency_blocked',failure_stage='dependency',failure_reason='dependency_unavailable',blocker_count=1,source_count=0,parser_count=0,identity_provider_id=NULL,identity_provider_version=NULL,identity_schema_version=NULL,identity_digest_kind=NULL,identity_digest_value=NULL,identity_precision=NULL; DELETE FROM go_syntax_provider_sources; DELETE FROM go_syntax_provider_parser; INSERT INTO go_syntax_provider_blockers VALUES (1,0,'wrong')",
            "UPDATE go_syntax_provider_mirror SET blocker_count=1; INSERT INTO go_syntax_provider_blockers VALUES (1,1,'polint.source')",
            "UPDATE go_syntax_provider_sources SET relative_path='renamed.go' WHERE ordinal=0", "UPDATE go_syntax_provider_sources SET relative_path='/absolute.go' WHERE ordinal=0",
            "UPDATE go_syntax_provider_sources SET language='typescript' WHERE ordinal=0", "UPDATE go_syntax_provider_sources SET digest_kind='wrong' WHERE ordinal=0",
            "UPDATE go_syntax_provider_sources SET digest_value='0000000000000000' WHERE ordinal=0", "UPDATE go_syntax_provider_sources SET digest_value=zeroblob(16) WHERE ordinal=0",
            "UPDATE go_syntax_provider_sources SET ordinal=99 WHERE ordinal=0; UPDATE go_syntax_provider_sources SET ordinal=0 WHERE ordinal=1; UPDATE go_syntax_provider_sources SET ordinal=1 WHERE ordinal=99",
            "DELETE FROM go_syntax_provider_sources WHERE ordinal=0",
            "INSERT INTO go_syntax_provider_sources VALUES (1,2,'extra.go','go','source_text','0000000000000000'); UPDATE go_syntax_provider_mirror SET source_count=3",
            "UPDATE go_syntax_provider_sources SET relative_path=printf('%9000s','x') WHERE ordinal=0", "UPDATE go_syntax_provider_sources SET generation_id=999 WHERE ordinal=0",
            "UPDATE go_syntax_provider_parser SET provider_id='wrong'", "UPDATE go_syntax_provider_parser SET provider_version='wrong'",
            "UPDATE go_syntax_provider_parser SET fact_schema='wrong'", "UPDATE go_syntax_provider_parser SET payload_schema='wrong'",
            "UPDATE go_syntax_provider_parser SET backend='wrong'", "UPDATE go_syntax_provider_parser SET grammar='wrong'",
            "UPDATE go_syntax_provider_parser SET digest_kind='wrong'", "UPDATE go_syntax_provider_parser SET digest_value='0000000000000000'",
            "UPDATE go_syntax_provider_parser SET provider_version=zeroblob(16)", "DELETE FROM go_syntax_provider_parser",
            "UPDATE go_syntax_provider_mirror SET parser_count=2", "UPDATE generations SET status='pending'",
            "DELETE FROM active_generation", "DELETE FROM go_syntax_provider_mirror",
            "UPDATE go_syntax_provider_mirror SET generation_id=999",
            "ALTER TABLE go_syntax_provider_mirror RENAME TO go_syntax_provider_old; CREATE TABLE go_syntax_provider_mirror AS SELECT * FROM go_syntax_provider_old; INSERT INTO go_syntax_provider_mirror SELECT * FROM go_syntax_provider_old",
            "CREATE INDEX go_syntax_provider_sources_extra ON go_syntax_provider_sources(language)",
            "CREATE TRIGGER go_syntax_provider_extra AFTER UPDATE ON go_syntax_provider_mirror BEGIN SELECT 1; END",
            "ALTER TABLE go_syntax_provider_parser ADD COLUMN surprise TEXT",
            "ALTER TABLE go_syntax_provider_parser RENAME TO go_syntax_provider_parser_old; CREATE TABLE go_syntax_provider_parser AS SELECT * FROM go_syntax_provider_parser_old",
            "PRAGMA writable_schema=ON; UPDATE sqlite_master SET sql=substr(sql,1,instr(sql,'('))||printf('%100000s',' ')||substr(sql,instr(sql,'(')+1) WHERE type='table' AND name='go_syntax_provider_sources'; PRAGMA writable_schema=OFF",
        ] {
            let _ = tamper_then_refuse(statement);
        }
    }
}

mod run_manifest {
    use super::*;

    fn config(temp: &tempfile::TempDir) -> StoreConfig {
        StoreConfig::new(temp.path().join("cache/semantic-store/store.sqlite3"), true)
    }

    #[test]
    fn active_match_distinguishes_absence_exactness_and_every_semantic_mismatch() {
        let temp = tempfile::tempdir().expect("temp directory");
        let store_config = config(&temp);
        let first = source("src/a.ts", Language::TypeScript, "a", 10);
        let second = source("src/b.go", Language::Go, "b", 20);
        let baseline = ManifestFixture::new("config", vec![first.clone(), second.clone()]);

        assert_eq!(
            SemanticStore::match_active_manifest(&store_config, baseline.inputs(temp.path()))
                .expect("match absent store"),
            ManifestMatch::NoActiveManifest
        );
        let handle = SemanticStore::reserve_generation(&store_config).expect("reserve generation");
        SemanticStore::publish_generation(
            &store_config,
            handle,
            publication(baseline.inputs(temp.path())),
        )
        .expect("publish manifest");
        assert_eq!(
            SemanticStore::match_active_manifest(&store_config, baseline.inputs(temp.path()))
                .expect("match reopened manifest"),
            ManifestMatch::Exact(handle)
        );

        let reordered = ManifestFixture::new("config", vec![second.clone(), first.clone()]);
        assert_eq!(
            SemanticStore::match_active_manifest(&store_config, reordered.inputs(temp.path()))
                .expect("match reordered sources"),
            ManifestMatch::Exact(handle)
        );

        let mut changed_digest = first.clone();
        changed_digest.source_text_digest =
            Digest::from_parts(DigestKind::SourceText, "store-test", &["changed"]);
        let mut changed_size = first.clone();
        changed_size.size_bytes += 1;
        let mismatches = [
            ManifestFixture::new("other-config", vec![first.clone(), second.clone()]),
            ManifestFixture::new("config", vec![first.clone()]),
            ManifestFixture::new(
                "config",
                vec![
                    first,
                    second.clone(),
                    source("src/c.ts", Language::TypeScript, "c", 30),
                ],
            ),
            ManifestFixture::new(
                "config",
                vec![
                    source("src/renamed.ts", Language::TypeScript, "a", 10),
                    second.clone(),
                ],
            ),
            ManifestFixture::new(
                "config",
                vec![
                    source("src/a.ts", Language::JavaScript, "a", 10),
                    second.clone(),
                ],
            ),
            ManifestFixture::new("config", vec![changed_digest, second.clone()]),
            ManifestFixture::new("config", vec![changed_size, second]),
        ];
        for mismatch in &mismatches {
            assert_eq!(
                SemanticStore::match_active_manifest(&store_config, mismatch.inputs(temp.path()))
                    .expect("compare semantic mismatch"),
                ManifestMatch::Mismatch
            );
        }

        let other_workspace = tempfile::tempdir().expect("other workspace");
        assert_eq!(
            SemanticStore::match_active_manifest(
                &store_config,
                baseline.inputs(other_workspace.path()),
            )
            .expect("compare workspace mismatch"),
            ManifestMatch::Mismatch
        );
    }

    #[test]
    fn shared_store_refuses_a_different_workspace_before_candidate_mutation() {
        let store_temp = tempfile::tempdir().expect("store directory");
        let first_workspace = tempfile::tempdir().expect("first workspace");
        let second_workspace = tempfile::tempdir().expect("second workspace");
        let store_config = config(&store_temp);
        let manifest = ManifestFixture::new("config", Vec::new());
        let active = SemanticStore::reserve_generation(&store_config).expect("reserve active");
        SemanticStore::publish_generation(
            &store_config,
            active,
            publication(manifest.inputs(first_workspace.path())),
        )
        .expect("publish first workspace");
        let candidate =
            SemanticStore::reserve_generation(&store_config).expect("reserve candidate");

        assert_eq!(
            SemanticStore::publish_generation(
                &store_config,
                candidate,
                publication(manifest.inputs(second_workspace.path())),
            ),
            Err(GenerationError::WorkspaceOwnershipMismatch)
        );
        let reopened = fixture_snapshot_for_test(store_config.path()).expect("reopen snapshot");
        assert_eq!(reopened.selected_generation, Some(active));
        assert_eq!(reopened.manifested_generations, vec![active]);
        assert_eq!(
            reopened.generations,
            vec![
                (active, GenerationStatus::Complete),
                (candidate, GenerationStatus::Pending)
            ]
        );
        assert_eq!(
            SemanticStore::match_active_manifest(
                &store_config,
                manifest.inputs(first_workspace.path()),
            )
            .expect("reopen original owner"),
            ManifestMatch::Exact(active)
        );
    }

    #[test]
    fn deleted_active_pointer_is_malformed_and_cannot_transfer_workspace_ownership() {
        let store_temp = tempfile::tempdir().expect("store directory");
        let first_workspace = tempfile::tempdir().expect("first workspace");
        let second_workspace = tempfile::tempdir().expect("second workspace");
        let store_config = config(&store_temp);
        let manifest = ManifestFixture::new("config", Vec::new());
        let active = SemanticStore::reserve_generation(&store_config).expect("reserve active");
        SemanticStore::publish_generation(
            &store_config,
            active,
            publication(manifest.inputs(first_workspace.path())),
        )
        .expect("publish first workspace");
        let candidate =
            SemanticStore::reserve_generation(&store_config).expect("reserve candidate");
        let tamper_connection =
            rusqlite::Connection::open(store_config.path()).expect("open tamper connection");
        tamper_connection
            .execute("DELETE FROM active_generation", [])
            .expect("delete active pointer");
        drop(tamper_connection);
        let malformed = fixture_snapshot_for_test(store_config.path()).expect("malformed snapshot");
        let expected_error = GenerationError::Store(StoreStatus::RebuildNeeded(
            StoreRebuildReason::InvalidSchema,
        ));

        assert_eq!(
            SemanticStore::active_generation(&store_config),
            Err(expected_error.clone())
        );
        assert_eq!(
            SemanticStore::match_active_manifest(
                &store_config,
                manifest.inputs(first_workspace.path()),
            ),
            Err(expected_error.clone())
        );
        assert_eq!(
            SemanticStore::maintain(&store_config),
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)
        );
        assert_eq!(
            SemanticStore::publish_generation(
                &store_config,
                candidate,
                publication(manifest.inputs(first_workspace.path())),
            ),
            Err(expected_error.clone())
        );
        assert_eq!(
            fixture_snapshot_for_test(store_config.path())
                .expect("reopen after same-owner refusal"),
            malformed
        );
        assert_eq!(
            SemanticStore::publish_generation(
                &store_config,
                candidate,
                publication(manifest.inputs(second_workspace.path())),
            ),
            Err(expected_error)
        );

        let reopened =
            fixture_snapshot_for_test(store_config.path()).expect("reopen malformed store");
        assert_eq!(reopened, malformed);
        assert_eq!(
            reopened.generations,
            vec![
                (active, GenerationStatus::Complete),
                (candidate, GenerationStatus::Pending),
            ]
        );
        assert_eq!(reopened.selected_generation, None);
        assert_eq!(reopened.manifested_generations, vec![active]);
        assert!(reopened.manifest_sources.is_empty());
    }

    #[test]
    fn writer_preflight_rejects_many_extra_manifest_indexes_without_mutation() {
        let temp = tempfile::tempdir().expect("store directory");
        let store_config = config(&temp);
        assert_eq!(SemanticStore::maintain(&store_config), StoreStatus::Ready);
        let connection =
            rusqlite::Connection::open(store_config.path()).expect("open fixture connection");
        for index in 0..128 {
            connection
                .execute(
                    &format!(
                        "CREATE INDEX run_manifest_sources_extra_{index} \
                         ON run_manifest_sources (language)"
                    ),
                    [],
                )
                .expect("create extra manifest index");
        }
        let index_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM pragma_index_list('run_manifest_sources')",
                [],
                |row| row.get(0),
            )
            .expect("count fixture indexes");
        assert_eq!(index_count, 129);
        drop(connection);

        assert_eq!(
            SemanticStore::maintain(&store_config),
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)
        );
        let reopened =
            rusqlite::Connection::open(store_config.path()).expect("reopen refused store");
        let reopened_count: i64 = reopened
            .query_row(
                "SELECT count(*) FROM pragma_index_list('run_manifest_sources')",
                [],
                |row| row.get(0),
            )
            .expect("count preserved fixture indexes");
        assert_eq!(reopened_count, index_count);
    }

    fn tamper_then_refuse(statement: &str) {
        let temp = tempfile::tempdir().expect("temp directory");
        let store_config = config(&temp);
        let manifest = ManifestFixture::new(
            "config",
            vec![source("src/app.ts", Language::TypeScript, "app", 12)],
        );
        let handle = SemanticStore::reserve_generation(&store_config).expect("reserve generation");
        SemanticStore::publish_generation(
            &store_config,
            handle,
            publication(manifest.inputs(temp.path())),
        )
        .expect("publish manifest");
        let connection =
            rusqlite::Connection::open(store_config.path()).expect("open tamper connection");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = OFF;\
                 PRAGMA ignore_check_constraints = ON;",
            )
            .expect("allow malformed fixture");
        connection
            .execute_batch(statement)
            .expect("tamper manifest");
        drop(connection);

        assert!(
            SemanticStore::active_generation(&store_config).is_err(),
            "tamper was trusted: {statement}"
        );
        assert!(
            SemanticStore::match_active_manifest(&store_config, manifest.inputs(temp.path()))
                .is_err(),
            "tamper matched: {statement}"
        );
    }

    #[test]
    fn every_header_scalar_tamper_is_refused_with_the_stored_identity_unchanged() {
        for statement in [
            "UPDATE run_manifests SET manifest_schema = 'wrong'",
            "UPDATE run_manifests SET workspace_purpose = 'wrong'",
            "UPDATE run_manifests SET workspace_value = '0000000000000000'",
            "UPDATE run_manifests SET config_purpose = 'wrong'",
            "UPDATE run_manifests SET config_value = '0000000000000000'",
            "UPDATE run_manifests SET source_count = 0",
            "UPDATE run_manifests SET run_purpose = 'wrong'",
            "UPDATE run_manifests SET run_value = '0000000000000000'",
        ] {
            tamper_then_refuse(statement);
        }
    }

    #[test]
    fn representative_source_membership_and_ownership_tamper_is_refused() {
        for statement in [
            "INSERT INTO run_manifest_sources VALUES \
             ((SELECT generation_id FROM run_manifests), 'src/extra.ts', 'typescript', \
              'source-text-v1', '0000000000000000', 1)",
            "DELETE FROM run_manifest_sources",
            "UPDATE run_manifest_sources SET relative_path = 'src/renamed.ts'",
            "UPDATE run_manifest_sources SET language = 'javascript'",
            "UPDATE run_manifest_sources SET source_purpose = 'wrong'",
            "UPDATE run_manifest_sources SET source_value = '0000000000000000'",
            "UPDATE run_manifest_sources SET size_bytes = 13",
            "UPDATE run_manifest_sources SET size_bytes = 'wrong'",
            "UPDATE run_manifest_sources SET generation_id = 999",
            "INSERT INTO generations (status) VALUES ('pending');\
             UPDATE run_manifest_sources SET generation_id = \
                 (SELECT max(generation_id) FROM generations)",
        ] {
            tamper_then_refuse(statement);
        }
    }
}

mod generation_lifecycle {
    use super::*;

    fn store_config(temp: &tempfile::TempDir) -> StoreConfig {
        StoreConfig::new(temp.path().join("cache/semantic-store/store.sqlite3"), true)
    }

    fn snapshot(config: &StoreConfig) -> StoreFixtureSnapshot {
        fixture_snapshot_for_test(config.path()).expect("fixture snapshot")
    }

    fn publish(
        config: &StoreConfig,
        workspace: &Path,
        handle: GenerationHandle,
    ) -> Result<GenerationHandle, GenerationError> {
        let manifest = ManifestFixture::new("lifecycle", Vec::new());
        SemanticStore::publish_generation(config, handle, publication(manifest.inputs(workspace)))
    }

    fn publish_with_failure(
        config: &StoreConfig,
        workspace: &Path,
        handle: GenerationHandle,
        manifest: &ManifestFixture,
        failure_point: PublicationFailurePoint,
    ) -> Result<GenerationHandle, GenerationError> {
        SemanticStore::publish_generation_with_failure_for_test(
            config,
            handle,
            publication(manifest.inputs(workspace)),
            failure_point,
        )
    }

    fn candidate_projection_rows(config: &StoreConfig, handle: GenerationHandle) -> i64 {
        let connection = rusqlite::Connection::open(config.path()).unwrap();
        connection.query_row(
            "SELECT (SELECT count(*) FROM run_manifests WHERE generation_id=?1)+(SELECT count(*) FROM run_manifest_sources WHERE generation_id=?1)+(SELECT count(*) FROM metrics_provider_mirror WHERE generation_id=?1)+(SELECT count(*) FROM metrics_provider_members WHERE generation_id=?1)+(SELECT count(*) FROM metrics_provider_blockers WHERE generation_id=?1)+(SELECT count(*) FROM metrics_provider_sources WHERE generation_id=?1)+(SELECT count(*) FROM metrics_provider_functions WHERE generation_id=?1)+(SELECT count(*) FROM go_syntax_provider_mirror WHERE generation_id=?1)+(SELECT count(*) FROM go_syntax_provider_members WHERE generation_id=?1)+(SELECT count(*) FROM go_syntax_provider_blockers WHERE generation_id=?1)+(SELECT count(*) FROM go_syntax_provider_sources WHERE generation_id=?1)+(SELECT count(*) FROM go_syntax_provider_parser WHERE generation_id=?1)",
            [handle.scalar()], |row| row.get(0),
        ).unwrap()
    }

    #[test]
    fn direct_active_read_initializes_an_absent_owned_store() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = store_config(&temp);
        assert!(!config.path().exists());

        assert_eq!(
            SemanticStore::active_generation(&config).expect("initialize active-generation read"),
            None
        );

        let initialized = snapshot(&config);
        assert_eq!(initialized.version, migrations::CURRENT_SCHEMA_VERSION);
        assert!(initialized.generations.is_empty());
        assert_eq!(initialized.selected_generation, None);
        assert!(current_schema_is_valid_for_test(config.path()));
    }

    #[test]
    fn direct_active_read_migrates_exact_version_one_and_preserves_data() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = store_config(&temp);
        std::fs::create_dir_all(config.path().parent().expect("store directory"))
            .expect("create store directory");
        connection::install_version_one_fixture_for_test(config.path())
            .expect("install exact version-one fixture");
        let before = snapshot(&config);
        assert_eq!(before.version, 1);
        assert_eq!(before.sentinel.as_deref(), Some("preserve-me"));

        assert_eq!(
            SemanticStore::active_generation(&config).expect("migrate active-generation read"),
            None
        );

        let migrated = snapshot(&config);
        assert_eq!(migrated.version, migrations::CURRENT_SCHEMA_VERSION);
        assert_eq!(migrated.sentinel.as_deref(), Some("preserve-me"));
        assert!(migrated.generations.is_empty());
        assert_eq!(migrated.selected_generation, None);
        assert!(current_schema_is_valid_for_test(config.path()));
    }

    #[test]
    fn direct_active_read_maps_writer_contention_within_the_bounded_policy() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = store_config(&temp);
        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);
        let held =
            connection::hold_writer_connection_for_test(config.path()).expect("hold writer lease");

        let started = std::time::Instant::now();
        let result = SemanticStore::active_generation(&config);
        let elapsed = started.elapsed();

        assert_eq!(
            result,
            Err(GenerationError::Store(StoreStatus::BusySkipped))
        );
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "elapsed: {elapsed:?}"
        );
        drop(held);
    }

    #[test]
    fn reservation_is_unreadable_until_the_same_handle_is_published() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = store_config(&temp);
        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);
        assert_eq!(
            SemanticStore::active_generation(&config).expect("fresh active generation"),
            None
        );

        let reserved = SemanticStore::reserve_generation(&config).expect("reserve generation");
        assert_eq!(
            SemanticStore::active_generation(&config).expect("active after reservation"),
            None
        );
        assert_eq!(
            publish(&config, temp.path(), reserved).expect("publish generation"),
            reserved
        );
        assert_eq!(
            SemanticStore::active_generation(&config).expect("published active generation"),
            Some(reserved)
        );
    }

    #[test]
    fn explicit_publication_rotates_active_while_newer_pending_stays_unreadable() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = store_config(&temp);
        let first = SemanticStore::reserve_generation(&config).expect("reserve first");
        publish(&config, temp.path(), first).expect("publish first");
        let second = SemanticStore::reserve_generation(&config).expect("reserve second");
        publish(&config, temp.path(), second).expect("publish second");
        let pending = SemanticStore::reserve_generation(&config).expect("reserve pending");

        assert_ne!(pending, second);
        assert_eq!(
            SemanticStore::active_generation(&config).expect("active generation"),
            Some(second)
        );
    }

    #[test]
    fn repeated_and_unknown_publication_are_typed_rejections() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = store_config(&temp);
        let active = SemanticStore::reserve_generation(&config).expect("reserve active");
        publish(&config, temp.path(), active).expect("publish active");

        assert_eq!(
            publish(&config, temp.path(), active),
            Err(GenerationError::InvalidTransition)
        );

        let foreign_temp = tempfile::tempdir().expect("foreign temp directory");
        let foreign_config = store_config(&foreign_temp);
        let _first_foreign =
            SemanticStore::reserve_generation(&foreign_config).expect("reserve foreign first");
        let unknown =
            SemanticStore::reserve_generation(&foreign_config).expect("reserve foreign unknown");
        assert_eq!(
            publish(&config, temp.path(), unknown),
            Err(GenerationError::InvalidTransition)
        );
        assert_eq!(
            SemanticStore::active_generation(&config).expect("active generation"),
            Some(active)
        );
    }

    #[test]
    fn pending_selection_is_rejected_by_typed_and_relational_guards() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = store_config(&temp);
        let active = SemanticStore::reserve_generation(&config).expect("reserve active");
        publish(&config, temp.path(), active).expect("publish active");
        let pending = SemanticStore::reserve_generation(&config).expect("reserve pending");

        let mut writer = connection::open_writer(config.path()).expect("open writer");
        assert_eq!(
            generation::select_for_test(&mut writer, pending),
            Err(GenerationError::InvalidTransition)
        );
        assert!(generation::select_without_validation_for_test(&mut writer, pending).is_err());
        drop(writer);

        assert_eq!(
            SemanticStore::active_generation(&config).expect("active generation"),
            Some(active)
        );
    }

    #[test]
    fn disabled_lifecycle_returns_before_path_or_sqlite_work() {
        let source_temp = tempfile::tempdir().expect("source temp directory");
        let handle =
            SemanticStore::reserve_generation(&store_config(&source_temp)).expect("reserve source");
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("cache/semantic-store/store.sqlite3");
        let disabled = StoreConfig::new(&path, false);

        assert_eq!(
            SemanticStore::reserve_generation(&disabled),
            Err(GenerationError::Store(StoreStatus::Disabled))
        );
        assert_eq!(
            publish(&disabled, &temp.path().join("missing-workspace"), handle),
            Err(GenerationError::Store(StoreStatus::Disabled))
        );
        let manifest = ManifestFixture::new("disabled", Vec::new());
        assert_eq!(
            SemanticStore::match_active_manifest(
                &disabled,
                manifest.inputs(&temp.path().join("missing-workspace")),
            ),
            Err(GenerationError::Store(StoreStatus::Disabled))
        );
        assert_eq!(
            SemanticStore::active_generation(&disabled),
            Err(GenerationError::Store(StoreStatus::Disabled))
        );
        assert!(!temp.path().join("cache").exists());
        assert!(!path.exists());
    }

    #[test]
    fn preopened_writer_refuses_persistent_reservation_trigger_without_mutation() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = store_config(&temp);
        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);
        let mut writer = connection::open_writer(config.path()).expect("open lifecycle writer");
        let trigger_connection =
            rusqlite::Connection::open(config.path()).expect("open trigger fixture connection");
        trigger_connection
            .execute_batch(
                "CREATE TRIGGER publish_reserved_generation \
                 AFTER INSERT ON generations \
                 BEGIN \
                   UPDATE generations SET status = 'complete' \
                   WHERE generation_id = NEW.generation_id; \
                   INSERT INTO active_generation \
                     (singleton, generation_id, required_status) \
                   VALUES (1, NEW.generation_id, 'complete') \
                   ON CONFLICT(singleton) DO UPDATE SET \
                     generation_id = excluded.generation_id, \
                     required_status = excluded.required_status; \
                 END;",
            )
            .expect("install persistent reservation trigger");
        drop(trigger_connection);
        let before = snapshot(&config);

        assert_eq!(
            generation::reserve(&mut writer),
            Err(GenerationError::Store(StoreStatus::RebuildNeeded(
                StoreRebuildReason::InvalidSchema
            )))
        );
        drop(writer);

        assert_eq!(snapshot(&config), before);
        assert!(before.generations.is_empty());
        assert_eq!(before.selected_generation, None);
    }

    #[test]
    fn successful_rotation_and_unselected_candidate_survive_reopen_exactly() {
        let temp = tempfile::tempdir().expect("temp directory");
        let config = store_config(&temp);
        let first = SemanticStore::reserve_generation(&config).expect("reserve first");
        publish(&config, temp.path(), first).expect("publish first");

        assert_eq!(
            snapshot(&config).generations,
            vec![(first, GenerationStatus::Complete)]
        );
        assert_eq!(snapshot(&config).selected_generation, Some(first));
        assert_eq!(
            SemanticStore::active_generation(&config).expect("reopen first"),
            Some(first)
        );

        let second = SemanticStore::reserve_generation(&config).expect("reserve second");
        publish(&config, temp.path(), second).expect("publish second");
        let unselected = SemanticStore::reserve_generation(&config).expect("reserve unselected");
        let reopened = snapshot(&config);

        assert_eq!(
            reopened.generations,
            vec![
                (first, GenerationStatus::Complete),
                (second, GenerationStatus::Complete),
                (unselected, GenerationStatus::Pending),
            ]
        );
        assert_eq!(reopened.selected_generation, Some(second));
        assert_eq!(
            SemanticStore::active_generation(&config).expect("reopen second"),
            Some(second)
        );
    }

    #[test]
    fn every_publication_failure_preserves_the_previous_selection_after_reopen() {
        for failure_point in PublicationFailurePoint::ALL {
            let temp = tempfile::tempdir().expect("temp directory");
            let config = store_config(&temp);
            let active_manifest = ManifestFixture::new(
                "active",
                vec![source("src/a.ts", Language::TypeScript, "a", 10)],
            );
            let candidate_manifest =
                ManifestFixture::new("candidate", vec![source("src/b.go", Language::Go, "b", 20)]);
            let active = SemanticStore::reserve_generation(&config).expect("reserve active");
            SemanticStore::publish_generation(
                &config,
                active,
                publication(active_manifest.inputs(temp.path())),
            )
            .expect("publish active");
            let candidate = SemanticStore::reserve_generation(&config).expect("reserve candidate");

            assert_eq!(
                publish_with_failure(
                    &config,
                    temp.path(),
                    candidate,
                    &candidate_manifest,
                    failure_point,
                ),
                Err(GenerationError::InjectedFailure(failure_point))
            );

            let reopened = snapshot(&config);
            assert_eq!(
                reopened.generations,
                vec![
                    (active, GenerationStatus::Complete),
                    (candidate, GenerationStatus::Pending),
                ],
                "failure point: {failure_point:?}"
            );
            assert_eq!(
                reopened.selected_generation,
                Some(active),
                "failure point: {failure_point:?}"
            );
            assert_eq!(
                reopened.manifested_generations,
                vec![active],
                "failure point: {failure_point:?}"
            );
            assert_eq!(candidate_projection_rows(&config, candidate), 0);
            assert_eq!(
                reopened.manifest_sources,
                vec![(active, "src/a.ts".to_owned())],
                "failure point: {failure_point:?}"
            );
            assert_eq!(
                SemanticStore::active_generation(&config).expect("reopen active generation"),
                Some(active),
                "failure point: {failure_point:?}"
            );
            assert_eq!(
                SemanticStore::match_active_manifest(&config, active_manifest.inputs(temp.path()),)
                    .expect("reopen active manifest"),
                ManifestMatch::Exact(active),
                "failure point: {failure_point:?}"
            );
        }
    }

    #[test]
    fn every_failed_first_publication_reopens_without_active_truth() {
        for failure_point in PublicationFailurePoint::ALL {
            let temp = tempfile::tempdir().expect("temp directory");
            let config = store_config(&temp);
            let candidate_manifest =
                ManifestFixture::new("candidate", vec![source("src/b.go", Language::Go, "b", 20)]);
            let candidate = SemanticStore::reserve_generation(&config).expect("reserve candidate");

            assert_eq!(
                publish_with_failure(
                    &config,
                    temp.path(),
                    candidate,
                    &candidate_manifest,
                    failure_point,
                ),
                Err(GenerationError::InjectedFailure(failure_point))
            );

            let reopened = snapshot(&config);
            assert_eq!(
                reopened.generations,
                vec![(candidate, GenerationStatus::Pending)],
                "failure point: {failure_point:?}"
            );
            assert_eq!(
                reopened.selected_generation, None,
                "failure point: {failure_point:?}"
            );
            assert!(
                reopened.manifested_generations.is_empty(),
                "failure point: {failure_point:?}"
            );
            assert!(
                reopened.manifest_sources.is_empty(),
                "failure point: {failure_point:?}"
            );
            assert_eq!(candidate_projection_rows(&config, candidate), 0);
            assert_eq!(
                SemanticStore::active_generation(&config).expect("reopen active generation"),
                None,
                "failure point: {failure_point:?}"
            );
        }
    }

    #[test]
    fn malformed_and_future_stores_are_refused_without_mutation() {
        let future_temp = tempfile::tempdir().expect("future temp directory");
        let future_config = store_config(&future_temp);
        std::fs::create_dir_all(
            future_config
                .path()
                .parent()
                .expect("future store directory"),
        )
        .expect("create future store directory");
        install_future_fixture_for_test(future_config.path()).expect("install future fixture");
        let future_before = snapshot(&future_config);
        assert_eq!(
            SemanticStore::active_generation(&future_config),
            Err(GenerationError::Store(StoreStatus::Skipped(
                StoreSkipReason::FutureSchema {
                    found: migrations::CURRENT_SCHEMA_VERSION + 1,
                    supported: migrations::CURRENT_SCHEMA_VERSION,
                }
            )))
        );
        assert_eq!(snapshot(&future_config), future_before);

        let invalid_temp = tempfile::tempdir().expect("invalid temp directory");
        let invalid_config = store_config(&invalid_temp);
        std::fs::create_dir_all(
            invalid_config
                .path()
                .parent()
                .expect("invalid store directory"),
        )
        .expect("create invalid store directory");
        install_invalid_fixture_for_test(invalid_config.path()).expect("install invalid fixture");
        let invalid_before = snapshot(&invalid_config);
        assert_eq!(
            SemanticStore::active_generation(&invalid_config),
            Err(GenerationError::Store(StoreStatus::RebuildNeeded(
                StoreRebuildReason::InvalidSchema
            )))
        );
        assert_eq!(snapshot(&invalid_config), invalid_before);
    }
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
    fn version_zero_case_varied_owned_table_is_preserved_as_invalid_schema() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = store_path(&temp);
        fs::create_dir_all(path.parent().expect("store parent")).expect("create store directory");
        let collision = Connection::open(&path).expect("open collision fixture");
        collision
            .execute_batch(
                "CREATE TABLE \"GENERATIONS\" (value TEXT NOT NULL);\
                 INSERT INTO \"GENERATIONS\" (value) VALUES ('preserve-me');\
                 PRAGMA user_version = 0;",
            )
            .expect("create quoted case-varied collision");
        drop(collision);
        let before = fs::read(&path).expect("read collision fixture");
        let config = StoreConfig::new(&path, true);

        assert_eq!(
            SemanticStore::maintain(&config),
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)
        );
        assert_eq!(
            fs::read(&path).expect("read preserved collision fixture"),
            before
        );
    }

    #[test]
    fn version_one_owned_name_view_is_preserved_as_invalid_schema() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = store_path(&temp);
        fs::create_dir_all(path.parent().expect("store parent")).expect("create store directory");
        connection::install_version_one_fixture_for_test(&path)
            .expect("install exact version-one fixture");
        let collision = Connection::open(&path).expect("open collision fixture");
        collision
            .execute_batch(
                "CREATE VIEW \"ACTIVE_GENERATION\" AS \
                 SELECT value FROM sentinel;",
            )
            .expect("create owned-name view collision");
        drop(collision);
        let before = fs::read(&path).expect("read collision fixture");
        let config = StoreConfig::new(&path, true);

        assert_eq!(
            SemanticStore::maintain(&config),
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)
        );
        assert_eq!(
            fs::read(&path).expect("read preserved collision fixture"),
            before
        );
    }

    #[test]
    fn future_store_is_preserved_and_explicit_rebuild_refuses_it() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = store_path(&temp);
        fs::create_dir_all(path.parent().expect("store parent")).expect("create store directory");
        let future = Connection::open(&path).expect("open future store");
        let future_version = migrations::CURRENT_SCHEMA_VERSION + 1;
        future
            .execute_batch(
                "CREATE TABLE sentinel (value TEXT NOT NULL);\
                 INSERT INTO sentinel (value) VALUES ('future-data');",
            )
            .expect("create future fixture");
        future
            .pragma_update(None, "user_version", future_version)
            .expect("set future version");
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
