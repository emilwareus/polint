use super::*;
use crate::analysis_kernel::incremental::{
    Digest, DigestKind, FileSnapshot, RunManifest, RunManifestInputs,
};
use crate::core::Language;

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
        generation::publish(&mut writer, handle, &expected).expect("publish manifest storage");
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
                            [format!("src/{}.ts", "a".repeat(220))],
                        )
                        .expect("tamper aggregate bytes");
                }
            }

            assert_eq!(
                generation::read_manifest(&connection, handle).map(|_| ()),
                Err(GenerationError::InvalidManifest)
            );
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
        SemanticStore::publish_generation(&store_config, handle, baseline.inputs(temp.path()))
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
            manifest.inputs(first_workspace.path()),
        )
        .expect("publish first workspace");
        let candidate =
            SemanticStore::reserve_generation(&store_config).expect("reserve candidate");

        assert_eq!(
            SemanticStore::publish_generation(
                &store_config,
                candidate,
                manifest.inputs(second_workspace.path()),
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

    fn tamper_then_refuse(statement: &str) {
        let temp = tempfile::tempdir().expect("temp directory");
        let store_config = config(&temp);
        let manifest = ManifestFixture::new(
            "config",
            vec![source("src/app.ts", Language::TypeScript, "app", 12)],
        );
        let handle = SemanticStore::reserve_generation(&store_config).expect("reserve generation");
        SemanticStore::publish_generation(&store_config, handle, manifest.inputs(temp.path()))
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
        SemanticStore::publish_generation(config, handle, manifest.inputs(workspace))
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
            manifest.inputs(workspace),
            failure_point,
        )
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
            SemanticStore::publish_generation(&config, active, active_manifest.inputs(temp.path()))
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
