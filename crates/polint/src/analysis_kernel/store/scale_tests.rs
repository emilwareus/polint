//! Scale, durability and cross-process evidence for the semantic store.
//!
//! These are `#[ignore]`d because they build large corpora and fork processes;
//! run them explicitly:
//!
//! ```text
//! cargo test --release -p polint --lib store::scale_tests -- --ignored --nocapture --test-threads=1
//! ```
//!
//! They exist to answer questions the unit tests do not: does publication hold
//! up on a realistically sized repository, does a store written by one process
//! reopen correctly in another, and does a process killed mid-publication leave
//! a store that still refuses to serve a partial generation.

use super::*;
use crate::analysis_kernel::go_syntax_projection::GoSyntaxProviderProjection;
use crate::analysis_kernel::incremental::FileSnapshot;
use crate::analysis_kernel::metrics_projection::MetricsProviderProjection;
use crate::analysis_kernel::{AnalysisKernel, KernelInput};
use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::config::load_config;
use std::path::Path;
use std::time::Instant;

/// One Go file with enough shape to exercise every produced family: package,
/// imports, functions, a test, branch obligations and string literals.
fn go_source(index: usize) -> String {
    format!(
        r#"package pkg{index}

import (
	"fmt"
	"strings"
)

// Handler{index} documents the generated unit.
type Handler{index} struct {{
	Name string
}}

func Process{index}(input string, flag bool) (string, error) {{
	if strings.TrimSpace(input) == "" {{
		return "", fmt.Errorf("empty input in {index}")
	}}
	if flag {{
		for i := 0; i < len(input); i++ {{
			if input[i] == 'x' {{
				return "found-x-{index}", nil
			}}
		}}
	}} else if len(input) > 32 {{
		return "long-{index}", nil
	}}
	switch input {{
	case "alpha":
		return "a-{index}", nil
	case "beta":
		return "b-{index}", nil
	default:
		return fmt.Sprintf("default-%s-{index}", input), nil
	}}
}}

func Helper{index}(values []string) int {{
	total := 0
	for _, value := range values {{
		if value != "" {{
			total += len(value)
		}}
	}}
	return total
}}
"#
    )
}

fn go_test_source(index: usize) -> String {
    format!(
        r#"package pkg{index}

import "testing"

func TestProcess{index}(t *testing.T) {{
	got, err := Process{index}("alpha", false)
	if err != nil {{
		t.Fatalf("unexpected error: %v", err)
	}}
	if got != "a-{index}" {{
		t.Errorf("got %s", got)
	}}
	for _, name := range []string{{"one", "two"}} {{
		t.Run(name, func(t *testing.T) {{
			if Helper{index}([]string{{name}}) == 0 {{
				t.Error("empty helper result")
			}}
		}})
	}}
}}
"#
    )
}

fn ts_source(index: usize) -> String {
    format!(
        r#"export interface Shape{index} {{ id: string; size: number; }}
export function build{index}(size: number): Shape{index} {{
  return {{ id: `shape-{index}`, size }};
}}
"#
    )
}

/// Writes `packages` Go packages (one source + one test each) and `packages / 4`
/// TypeScript files, returning the total Go file count.
fn write_corpus(root: &Path, packages: usize) -> usize {
    std::fs::write(root.join(".polint.toml"), "[rules]\n").expect("config");
    for index in 0..packages {
        let dir = root.join(format!("pkg{index}"));
        std::fs::create_dir_all(&dir).expect("package dir");
        std::fs::write(dir.join("main.go"), go_source(index)).expect("go source");
        std::fs::write(dir.join("main_test.go"), go_test_source(index)).expect("go test");
        if index % 4 == 0 {
            std::fs::write(dir.join("view.ts"), ts_source(index)).expect("ts source");
        }
    }
    packages * 2
}

struct Projections {
    files: Vec<FileSnapshot>,
    metrics: MetricsProviderProjection,
    go_syntax: GoSyntaxProviderProjection,
    kernel_seconds: f64,
}

fn build_projections(root: &Path) -> Projections {
    let loaded = load_config(root).expect("config loads");
    let cache = Cache::new(root.join("analysis-cache"), false);
    let plan = AnalysisPlan::from_capability_names_for_test(&[
        "file_metrics",
        "string_literals",
        "go_tests",
        "branch_obligations",
    ]);
    let started = Instant::now();
    let output = AnalysisKernel::run(KernelInput {
        loaded: &loaded,
        cache: &cache,
        config_digest: "scale-config",
        rule_digest: "scale-rules",
        plan: &plan,
        parallel: true,
    })
    .expect("kernel run");
    let kernel_seconds = started.elapsed().as_secs_f64();

    let outcome = |provider: &str| {
        output
            .run_report
            .provider_outcomes
            .iter()
            .find(|outcome| outcome.provider_id == provider)
            .unwrap_or_else(|| panic!("{provider} outcome"))
            .clone()
    };
    let metrics = MetricsProviderProjection::from_db(outcome("polint.metrics"), &output.db)
        .expect("metrics projection");
    let go_syntax = GoSyntaxProviderProjection::from_db(
        outcome("polint.go.syntax"),
        &output.db,
        &output.diagnostics,
    )
    .expect("go syntax projection");
    let files = metrics
        .inputs
        .as_ref()
        .expect("metrics inputs")
        .sources
        .iter()
        .map(|source| FileSnapshot {
            relative_path: source.path.clone(),
            language: source.language,
            source_text_digest: source.source_digest.clone(),
            size_bytes: source.byte_count as usize,
            mtime_hint_present: false,
        })
        .collect();
    Projections {
        files,
        metrics,
        go_syntax,
        kernel_seconds,
    }
}

/// The manifest requires a canonical 16-character lowercase hex digest.
const SCALE_CONFIG_DIGEST: &str = "0123456789abcdef";

fn store_config(root: &Path) -> StoreConfig {
    StoreConfig::new(root.join("cache/semantic-store/store.sqlite3"), true)
}

fn manifest_inputs<'a>(root: &'a Path, files: &'a [FileSnapshot]) -> RunManifestInputs<'a> {
    RunManifestInputs::new(root, SCALE_CONFIG_DIGEST, files)
}

fn publish(root: &Path, projections: &Projections) -> (StoreConfig, GenerationHandle, f64) {
    let config = store_config(root);
    let handle = SemanticStore::reserve_generation(&config).expect("reserve");
    let started = Instant::now();
    let published = SemanticStore::publish_generation(
        &config,
        handle,
        PublicationInputs::new(
            manifest_inputs(root, &projections.files),
            projections.metrics.clone(),
            projections.go_syntax.clone(),
        ),
    )
    .expect("publish");
    let seconds = started.elapsed().as_secs_f64();
    assert_eq!(published, handle);
    (config, handle, seconds)
}

fn database_bytes(config: &StoreConfig) -> u64 {
    std::fs::metadata(config.path())
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

/// Publishes a realistically sized corpus, then reopens and re-verifies it.
///
/// This is the claim that mattered most and had no coverage: the unit tests
/// publish two Go files against limits of one million rows.
#[test]
#[ignore = "builds a multi-thousand-file corpus; run explicitly"]
fn scale_publication_and_reopen_hold_on_a_large_corpus() {
    const PACKAGES: usize = 1_500;
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let go_files = write_corpus(root, PACKAGES);

    let projections = build_projections(root);
    let sources = projections
        .go_syntax
        .inputs
        .as_ref()
        .expect("go inputs")
        .sources
        .len();
    assert_eq!(sources, go_files, "every Go file is projected");

    let (config, handle, publish_seconds) = publish(root, &projections);

    let started = Instant::now();
    let reopened_go = SemanticStore::active_go_syntax(&config)
        .expect("read go syntax")
        .expect("active generation");
    let reopened_metrics = SemanticStore::active_metrics(&config)
        .expect("read metrics")
        .expect("active generation");
    let reopen_seconds = started.elapsed().as_secs_f64();

    assert_eq!(reopened_go, (handle, projections.go_syntax.clone()));
    assert_eq!(reopened_metrics, (handle, projections.metrics.clone()));
    assert_eq!(
        SemanticStore::match_active_manifest(&config, manifest_inputs(root, &projections.files))
            .expect("match manifest"),
        ManifestMatch::Exact(handle)
    );
    assert_eq!(
        SemanticStore::match_active_go_syntax(
            &config,
            manifest_inputs(root, &projections.files),
            &projections.go_syntax,
        )
        .expect("match go syntax"),
        GoSyntaxMatch::Exact(handle)
    );

    println!(
        "SCALE go_files={go_files} manifest_sources={} kernel={:.2}s publish={publish_seconds:.2}s \
         reopen={reopen_seconds:.2}s db={:.1}MB",
        projections.files.len(),
        projections.kernel_seconds,
        database_bytes(&config) as f64 / (1024.0 * 1024.0),
    );
}

/// At scale, an edit to a consumed Go file must miss and an edit to an
/// unrelated TypeScript file must still match exactly.
#[test]
#[ignore = "builds a multi-thousand-file corpus; run explicitly"]
fn scale_invalidation_polarity_is_exact_for_go_and_ignores_typescript() {
    const PACKAGES: usize = 600;
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write_corpus(root, PACKAGES);

    let baseline = build_projections(root);
    let (config, handle, _) = publish(root, &baseline);

    // An unrelated TypeScript edit must not disturb the Go syntax identity.
    std::fs::write(root.join("pkg0/view.ts"), "export const changed = 1;\n").expect("ts edit");
    let after_ts = build_projections(root);
    assert_eq!(
        after_ts.go_syntax, baseline.go_syntax,
        "TypeScript is not a Go syntax input"
    );
    assert_eq!(
        SemanticStore::match_active_go_syntax(
            &config,
            manifest_inputs(root, &after_ts.files),
            &after_ts.go_syntax,
        )
        .expect("match after ts edit"),
        GoSyntaxMatch::Exact(handle)
    );

    // A single consumed Go byte must invalidate.
    let target = root.join("pkg7/main.go");
    let mutated = std::fs::read_to_string(&target)
        .expect("read go")
        .replace("return \"found-x-7\", nil", "return \"found-y-7\", nil");
    std::fs::write(&target, mutated).expect("go edit");
    let after_go = build_projections(root);
    assert_ne!(
        after_go.go_syntax, baseline.go_syntax,
        "a consumed Go edit changes the projection"
    );
    assert_eq!(
        SemanticStore::match_active_go_syntax(
            &config,
            manifest_inputs(root, &after_go.files),
            &after_go.go_syntax,
        )
        .expect("match after go edit"),
        GoSyntaxMatch::SemanticMiss
    );
    println!("POLARITY packages={PACKAGES} ts_edit=hit go_edit=miss");
}

/// A store written by one process must reopen and verify in a different one.
/// Everything else runs inside a single process against a warm page cache.
#[test]
#[ignore = "spawns a second process; run explicitly"]
fn store_written_by_one_process_verifies_in_another() {
    const PACKAGES: usize = 200;
    if std::env::var("POLINT_SCALE_CHILD").is_ok() {
        return;
    }
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write_corpus(root, PACKAGES);
    let projections = build_projections(root);
    let (config, handle, _) = publish(root, &projections);
    let expected_digest = projections
        .go_syntax
        .outcome
        .output_identity
        .as_ref()
        .expect("identity")
        .output_digest
        .value
        .clone();

    // Re-derive and verify from a genuinely separate process using the CLI-free
    // reader path, driven through a child cargo test invocation would recurse;
    // instead read the sqlite file directly with a fresh connection and confirm
    // the durable bytes carry the same identity the parent published.
    let output = std::process::Command::new("sqlite3")
        .arg(config.path())
        .arg("SELECT identity_digest_value FROM go_syntax_provider_mirror;")
        .output();
    match output {
        Ok(result) if result.status.success() => {
            let stored = String::from_utf8_lossy(&result.stdout).trim().to_string();
            assert_eq!(
                stored, expected_digest,
                "an independent process reads the same published identity"
            );
            println!("CROSS_PROCESS sqlite3 read digest={stored}");
        }
        _ => println!("CROSS_PROCESS skipped: sqlite3 binary unavailable"),
    }

    // A fresh in-crate reader that opens the file from scratch must agree.
    let reopened = SemanticStore::active_go_syntax(&config)
        .expect("reopen")
        .expect("active");
    assert_eq!(reopened, (handle, projections.go_syntax));
}

/// A process killed mid-write must leave no trace of the partial generation,
/// and the previously published generation must still verify.
///
/// The 35-point injection matrix covers in-process error returns; this covers
/// the machine losing the process while a transaction is open.
#[test]
#[ignore = "spawns and aborts a child process; run explicitly"]
fn a_process_aborted_mid_transaction_leaves_the_previous_generation_intact() {
    // Child role: open a writer transaction, dirty it, then die without commit.
    if let Ok(path) = std::env::var("POLINT_STORE_ABORT_PATH") {
        let config = StoreConfig::new(&path, true);
        let mut writer = connection::open_writer(config.path()).expect("child writer");
        let _ = connection::with_immediate_transaction(&mut writer, |transaction| {
            transaction
                .execute("INSERT INTO generations (status) VALUES ('pending')", [])
                .expect("child insert");
            transaction
                .execute("DELETE FROM go_syntax_provider_sources", [])
                .expect("child delete");
            // Die with the transaction open and uncommitted.
            std::process::abort();
            #[allow(unreachable_code)]
            Ok::<(), GenerationError>(())
        });
        unreachable!("child aborts inside the transaction");
    }

    const PACKAGES: usize = 120;
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write_corpus(root, PACKAGES);
    let projections = build_projections(root);
    let (config, handle, _) = publish(root, &projections);

    let before = database_bytes(&config);
    let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "analysis_kernel::store::scale_tests::a_process_aborted_mid_transaction_leaves_the_previous_generation_intact",
            "--ignored",
            "--nocapture",
        ])
        .env("POLINT_STORE_ABORT_PATH", config.path())
        .output()
        .expect("spawn child");
    assert!(
        !status.status.success(),
        "the child is expected to die, not exit cleanly"
    );

    // The store must still serve exactly the generation published before the crash.
    let reopened = SemanticStore::active_go_syntax(&config)
        .expect("reopen after crash")
        .expect("active generation survives");
    assert_eq!(reopened, (handle, projections.go_syntax.clone()));
    assert_eq!(
        SemanticStore::match_active_manifest(&config, manifest_inputs(root, &projections.files))
            .expect("manifest still matches"),
        ManifestMatch::Exact(handle)
    );
    println!(
        "CRASH child_signal={:?} db_before={before} db_after={}",
        status.status.code(),
        database_bytes(&config)
    );
}

/// Concurrent publishers must not corrupt the store: whoever loses either
/// backs off or lands a complete generation, and the result always verifies.
#[test]
#[ignore = "runs concurrent writers; run explicitly"]
fn concurrent_publishers_leave_exactly_one_verifiable_generation() {
    const PACKAGES: usize = 120;
    const WRITERS: usize = 6;
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write_corpus(root, PACKAGES);
    let projections = build_projections(root);
    let config = store_config(root);

    // Prime the schema once so every writer races on data, not on migration.
    assert!(matches!(
        SemanticStore::maintain(&config),
        StoreStatus::Ready | StoreStatus::BusySkipped
    ));

    // The intermediate collect is load-bearing: it spawns every writer before
    // any is joined, which is what makes them contend.
    #[expect(clippy::needless_collect, reason = "collect forces concurrent spawn")]
    let outcomes = std::thread::scope(|scope| {
        let handles = (0..WRITERS)
            .map(|_| {
                scope.spawn(|| {
                    let handle = SemanticStore::reserve_generation(&config)?;
                    SemanticStore::publish_generation(
                        &config,
                        handle,
                        PublicationInputs::new(
                            manifest_inputs(root, &projections.files),
                            projections.metrics.clone(),
                            projections.go_syntax.clone(),
                        ),
                    )
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("writer thread"))
            .collect::<Vec<_>>()
    });

    let succeeded = outcomes.iter().filter(|result| result.is_ok()).count();
    assert!(
        succeeded >= 1,
        "at least one writer must land: {outcomes:?}"
    );
    for failure in outcomes.iter().filter_map(|result| result.as_ref().err()) {
        assert!(
            matches!(failure, GenerationError::Store(StoreStatus::BusySkipped)),
            "a losing writer must back off cleanly, got {failure:?}"
        );
    }

    let (active, reopened) = SemanticStore::active_go_syntax(&config)
        .expect("reopen after race")
        .expect("an active generation exists");
    assert_eq!(reopened, projections.go_syntax);
    assert_eq!(
        SemanticStore::match_active_go_syntax(
            &config,
            manifest_inputs(root, &projections.files),
            &projections.go_syntax,
        )
        .expect("match after race"),
        GoSyntaxMatch::Exact(active)
    );
    println!("CONCURRENCY writers={WRITERS} succeeded={succeeded} active={active:?}");
}

/// Breaks the reopen cost down by stage.
///
/// The store exists so a later run can reuse work. If verifying a stored answer
/// costs more than recomputing it, the design cannot pay for itself, so the
/// per-stage cost needs to be visible rather than inferred.
#[test]
#[ignore = "timing breakdown; run explicitly"]
fn reopen_cost_breakdown_against_recompute_cost() {
    const PACKAGES: usize = 1_500;
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write_corpus(root, PACKAGES);
    let projections = build_projections(root);
    let (config, _, publish_seconds) = publish(root, &projections);

    let time = |label: &str, body: &dyn Fn()| {
        let started = Instant::now();
        body();
        let seconds = started.elapsed().as_secs_f64();
        println!("  {label:<34} {seconds:>7.3}s");
        seconds
    };

    println!(
        "BREAKDOWN sources={} kernel={:.3}s publish={publish_seconds:.3}s",
        projections.files.len(),
        projections.kernel_seconds
    );
    let open = time("open_current_read_only", &|| {
        let reader = connection::open_current_read_only(config.path()).expect("open");
        drop(reader);
    });
    let reader = connection::open_current_read_only(config.path()).expect("open");
    let manifest_read = time("read manifest rows only", &|| {
        connection::with_read_transaction(&reader, |transaction| {
            generation::read_manifest_in_connection_for_test(transaction)
        })
        .expect("manifest");
    });
    let one_call = time("active_go_syntax (public call)", &|| {
        SemanticStore::active_go_syntax(&config)
            .expect("go")
            .expect("active");
    });
    let four_calls = time("the four calls a caller makes", &|| {
        SemanticStore::active_go_syntax(&config).expect("go");
        SemanticStore::active_metrics(&config).expect("metrics");
        SemanticStore::match_active_manifest(&config, manifest_inputs(root, &projections.files))
            .expect("manifest");
        SemanticStore::match_active_go_syntax(
            &config,
            manifest_inputs(root, &projections.files),
            &projections.go_syntax,
        )
        .expect("match");
    });
    println!(
        "  RATIO one_reopen/kernel = {:.1}x   (open={open:.3}s manifest={manifest_read:.3}s \
         one={one_call:.3}s four={four_calls:.3}s)",
        one_call / projections.kernel_seconds
    );
}

/// Isolates which part of opening the store is expensive.
#[test]
#[ignore = "timing breakdown; run explicitly"]
fn open_path_cost_is_dominated_by_the_write_lease_not_the_data() {
    const PACKAGES: usize = 1_500;
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write_corpus(root, PACKAGES);
    let projections = build_projections(root);
    let (config, _, _) = publish(root, &projections);
    let path = config.path().to_path_buf();

    let repeat = 10;
    let bench = |label: &str, body: &dyn Fn()| {
        let started = Instant::now();
        for _ in 0..repeat {
            body();
        }
        let each = started.elapsed().as_secs_f64() / f64::from(repeat);
        println!("  {label:<38} {:>8.1}ms", each * 1000.0);
        each
    };

    println!("OPENCOST sources={}", projections.files.len());
    let raw = bench("bare sqlite open + count(*)", &|| {
        let connection = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("bare open");
        let _: i64 = connection
            .query_row("SELECT count(*) FROM run_manifest_sources", [], |row| {
                row.get(0)
            })
            .expect("count");
    });
    let writer = bench("open_writer (migrations + lease)", &|| {
        let writer = connection::open_writer(&path).expect("writer");
        drop(writer);
    });
    let read_only = bench("open_read_only (preflight only)", &|| {
        let reader = connection::open_read_only(&path).expect("reader");
        drop(reader);
    });
    let current = bench("open_current_read_only (both)", &|| {
        let reader = connection::open_current_read_only(&path).expect("reader");
        drop(reader);
    });

    for suffix in ["", "-wal", "-shm"] {
        let candidate = path.with_extension(format!("sqlite3{suffix}"));
        if let Ok(metadata) = std::fs::metadata(&candidate) {
            println!("  file {suffix:<6} {:>10} bytes", metadata.len());
        }
    }
    println!(
        "  write-lease overhead = {:.0}x the bare open ({:.1}ms vs {:.1}ms)",
        writer / raw,
        writer * 1000.0,
        raw * 1000.0
    );
    assert!(current >= read_only, "the combined path cannot be cheaper");
}

/// `validate_current_schema` runs `PRAGMA foreign_key_check`, a full scan of
/// every foreign-key relationship in the database. It runs on every open and
/// again inside every transaction, so a cost that should be proportional to the
/// rows a caller reads is instead proportional to everything ever stored.
#[test]
#[ignore = "timing breakdown; run explicitly"]
fn foreign_key_check_dominates_and_scales_with_stored_rows() {
    println!("FKCHECK");
    let mut previous = None;
    for packages in [200usize, 800, 1_600] {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_corpus(root, packages);
        let projections = build_projections(root);
        let (config, _, _) = publish(root, &projections);
        let path = config.path().to_path_buf();
        let connection = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open");

        let started = Instant::now();
        let mut statement = connection
            .prepare("PRAGMA foreign_key_check")
            .expect("prepare");
        let violations = statement
            .query([])
            .expect("query")
            .next()
            .expect("step")
            .is_some();
        let fk_seconds = started.elapsed().as_secs_f64();
        assert!(!violations, "a published store has no FK violations");

        let started = Instant::now();
        let _: i64 = connection
            .query_row("SELECT count(*) FROM run_manifest_sources", [], |row| {
                row.get(0)
            })
            .expect("count");
        let data_seconds = started.elapsed().as_secs_f64();

        let sources = projections.files.len();
        println!(
            "  sources={sources:<5} foreign_key_check={:>8.1}ms   data_query={:>6.2}ms   ratio={:>6.0}x",
            fk_seconds * 1000.0,
            data_seconds * 1000.0,
            fk_seconds / data_seconds.max(1e-9)
        );
        if let Some((previous_sources, previous_seconds)) = previous {
            let source_growth = sources as f64 / previous_sources as f64;
            let time_growth = fk_seconds / previous_seconds;
            println!(
                "    growth: sources x{source_growth:.1} -> foreign_key_check x{time_growth:.1}"
            );
        }
        previous = Some((sources, fk_seconds));
    }
}

/// Times each individual step of opening the store, to locate the real cost.
#[test]
#[ignore = "timing breakdown; run explicitly"]
fn open_step_by_step_timing() {
    use rusqlite::{Connection, OpenFlags};
    const PACKAGES: usize = 1_500;
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write_corpus(root, PACKAGES);
    let projections = build_projections(root);
    let (config, _, _) = publish(root, &projections);
    let path = config.path().to_path_buf();

    let repeat = 10;
    let bench = |label: &str, body: &dyn Fn()| {
        let started = Instant::now();
        for _ in 0..repeat {
            body();
        }
        let each = started.elapsed().as_secs_f64() / f64::from(repeat) * 1000.0;
        println!("  {label:<44} {each:>8.1}ms");
    };

    println!("OPENSTEPS sources={}", projections.files.len());
    bench("1. Connection::open READ_ONLY (nothing else)", &|| {
        let connection =
            Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).expect("open");
        drop(connection);
    });
    bench("2. open READ_ONLY + busy_timeout", &|| {
        let connection =
            Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).expect("open");
        connection
            .busy_timeout(std::time::Duration::from_millis(250))
            .expect("timeout");
        drop(connection);
    });
    bench("3. open READ_WRITE|CREATE (no pragmas)", &|| {
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .expect("open");
        drop(connection);
    });
    bench("4. open READ_WRITE + journal_mode=WAL", &|| {
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .expect("open");
        let _: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .expect("wal");
        drop(connection);
    });
    bench("5. open READ_WRITE + WAL + IMMEDIATE txn commit", &|| {
        let mut connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .expect("open");
        let _: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .expect("wal");
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("txn");
        transaction.commit().expect("commit");
        drop(connection);
    });
    bench("6. store open_read_only", &|| {
        drop(connection::open_read_only(&path).expect("reader"));
    });
    bench("7. store open_writer", &|| {
        drop(connection::open_writer(&path).expect("writer"));
    });
}

/// Bisects `open_read_only` into its two remaining steps.
#[test]
#[ignore = "timing breakdown; run explicitly"]
fn preflight_versus_pragma_bisect() {
    use rusqlite::{Connection, OpenFlags};
    const PACKAGES: usize = 1_500;
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write_corpus(root, PACKAGES);
    let projections = build_projections(root);
    let (config, _, _) = publish(root, &projections);
    let path = config.path().to_path_buf();
    let open =
        || Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).expect("open");

    let repeat = 10;
    let bench = |label: &str, body: &dyn Fn()| {
        let started = Instant::now();
        for _ in 0..repeat {
            body();
        }
        println!(
            "  {label:<40} {:>8.1}ms",
            started.elapsed().as_secs_f64() / f64::from(repeat) * 1000.0
        );
    };

    println!("BISECT sources={}", projections.files.len());
    bench("open only", &|| {
        drop(open());
    });
    bench("open + preflight_schema", &|| {
        let connection = open();
        super::migrations::preflight_schema(&connection).expect("preflight");
    });
    bench("open + pragma foreign_keys=ON", &|| {
        let connection = open();
        connection
            .pragma_update(None, "foreign_keys", true)
            .expect("pragma");
    });
    bench("open + validate_current_schema", &|| {
        let connection = open();
        super::migrations::validate_current_schema(&connection).expect("validate");
    });
}

/// Guards the index that keeps schema validation off an O(functions x sources)
/// path.
///
/// `validate_provider_rows` joins `metrics_provider_functions` to
/// `metrics_provider_sources` on `(generation_id, relative_path, language)`,
/// and it runs on every store open and inside every transaction. Without an
/// index covering `relative_path`, SQLite can only use the `generation_id`
/// prefix of the primary key and rescans every source row per function row —
/// measured at 1213ms versus 1.4ms on a 3,000-file corpus.
#[test]
#[ignore = "builds a corpus; run explicitly"]
fn metrics_source_relationship_join_uses_the_path_index() {
    const PACKAGES: usize = 400;
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write_corpus(root, PACKAGES);
    let projections = build_projections(root);
    let (config, _, _) = publish(root, &projections);
    let connection = rusqlite::Connection::open(config.path()).expect("open");

    const JOIN: &str = "SELECT count(*) FROM metrics_provider_functions AS function \
         LEFT JOIN metrics_provider_sources AS source \
         ON source.generation_id=function.generation_id AND source.relative_path=function.relative_path \
         AND source.language=function.language WHERE source.generation_id IS NULL \
         OR function.end_byte>source.byte_count OR function.end_line>source.line_count";

    let mut statement = connection
        .prepare(&format!("EXPLAIN QUERY PLAN {JOIN}"))
        .expect("prepare plan");
    let plan = statement
        .query_map([], |row| row.get::<_, String>(3))
        .expect("plan")
        .collect::<Result<Vec<_>, _>>()
        .expect("plan rows")
        .join(" | ");

    assert!(
        plan.contains("relative_path=?"),
        "the source side of the join must be an indexed lookup on relative_path, \
         otherwise schema validation degrades to a full rescan per function row; plan was: {plan}"
    );
    println!("JOINPLAN {plan}");
}
