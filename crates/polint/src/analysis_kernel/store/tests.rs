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
        query_key,
        result_digest: Digest::from_parts(DigestKind::ProviderOutput, "query_result", &[label]),
        precision_tier: PrecisionTier::SetupAware,
        provenance: format!("native:{label}"),
        cache_status: DemandCacheStatus::Computed,
        compute_duration_micros: 10,
    }
}

fn generation_plan_fixture(root: &Path, label: &str) -> commit_plan::StoreCommitPlan {
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
    let validated = ValidatedRunMetadata::from_finalized_run(
        &report,
        AnalysisKernel::provider_manifests(),
        facts,
    )
    .expect("fixture produces a validated handoff");
    commit_plan::StoreCommitPlan::from_validated_run(validated)
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
                from: row.from.clone(),
                to: row.to.clone(),
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
