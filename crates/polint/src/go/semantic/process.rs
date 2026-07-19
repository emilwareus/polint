#[cfg(unix)]
use std::collections::VecDeque;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ffi::OsString;
use std::fs;
use std::io::Read;
#[cfg(unix)]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
#[cfg(all(test, target_os = "linux"))]
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
#[cfg(test)]
use std::sync::{Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(not(windows))]
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest as ShaDigest, Sha256};

use crate::go::lifecycle::GoAnalysisConfig;
use crate::go::semantic::protocol::GO_SEMANTIC_SCHEMA;

pub(crate) const GO_SEMANTIC_FRONTEND_ENV: &str = "POLINT_GO_FRONTEND";
const GO_FRONTEND_CACHE_VERSION: &str = "v1";
const GO_ENVIRONMENT_POLICY: &str = "sealed-dependency-snapshot-v7";
const GO_FRONTEND_MAX_SOURCE_FILES: usize = 512;
const GO_FRONTEND_MAX_SOURCE_BYTES: usize = 32 * 1_048_576;
const GO_FRONTEND_MAX_SOURCE_ENTRIES: usize = 4_096;
const GO_FRONTEND_MAX_SOURCE_DIRECTORIES: usize = 512;
const GO_FRONTEND_MAX_SOURCE_DEPTH: usize = 64;
const GO_FRONTEND_MAX_SOURCE_FRONTIER: usize = 512;
const GO_FRONTEND_MAX_EXECUTABLE_BYTES: usize = 64 * 1_048_576;
const GO_FRONTEND_MAX_STALE_STAGING_SCAN: usize = 64;
const GO_STALE_CLEANUP_DIRECTORY_BATCH: usize = 4_096;
const GO_FRONTEND_MAX_STALE_CLEANUP_ENTRIES: usize = 4_096;
const GO_FRONTEND_MAX_STALE_CLEANUP_DEPTH: usize = 64;
const GO_FRONTEND_MAX_PUBLISHED_SOURCES: usize = 32;
const GO_FRONTEND_MAX_PUBLISHED_BUILDS: usize = 32;
const GO_FRONTEND_MAX_PUBLISHED_EXECUTABLES: usize = 32;
const GO_FRONTEND_MAX_PUBLISHED_TOOLCHAINS: usize = 16;
const GO_FRONTEND_MAX_PUBLISHED_SOURCE_BYTES: u64 = 1024 * 1024 * 1024;
const GO_FRONTEND_MAX_PUBLISHED_BUILD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const GO_FRONTEND_MAX_PUBLISHED_EXECUTABLE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const GO_FRONTEND_MAX_PUBLISHED_TOOLCHAIN_BYTES: u64 = 1024 * 1024 * 1024;
const GO_FRONTEND_STALE_STAGING_AGE: std::time::Duration =
    std::time::Duration::from_secs(24 * 60 * 60);
const GO_PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const GO_BUILD_TIMEOUT: Duration = Duration::from_secs(120);
#[cfg(not(windows))]
pub(crate) const GO_OPERATION_TIMEOUT: Duration = Duration::from_secs(150);
#[cfg(windows)]
pub(crate) const GO_OPERATION_TIMEOUT: Duration = Duration::from_secs(300);
const GO_PROBE_STDOUT_BYTES: usize = 64 * 1024;
const GO_PROBE_STDERR_BYTES: usize = 256 * 1024;
const GO_BUILD_STDOUT_BYTES: usize = 256 * 1024;
const GO_BUILD_STDERR_BYTES: usize = 1024 * 1024;
const GO_TOOLCHAIN_MAX_CLOSURE_ENTRIES: usize = 100_000;
const GO_TOOLCHAIN_MAX_CLOSURE_BYTES: u64 = 1024 * 1024 * 1024;
const GO_DEPENDENCY_MAX_ENTRIES: usize = 100_000;
const GO_DEPENDENCY_SOURCE_MAX_ENUMERATED_ENTRIES: usize = 1_000_000;
const GO_DEPENDENCY_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const GO_DEPENDENCY_SOURCE_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const GO_LOCAL_DEPENDENCY_MAX_DEPTH: usize = 256;
const GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS: usize = 64 * 1_048_576;
const GO_LOCAL_REPOSITORY_CACHE_PATH_UNITS: usize = 16 * 1_048_576;
const GO_DEPENDENCY_CLEANUP_MAX_VISITS: usize = (GO_DEPENDENCY_MAX_ENTRIES + 4) * 2;
const GO_DEPENDENCY_CLEANUP_MAX_DEPTH: usize = GO_DEPENDENCY_MAX_ENTRIES + 4;
const GO_DEPENDENCY_MAX_PUBLISHED_SNAPSHOTS: usize = 4;
const GO_DEPENDENCY_MAX_PUBLISHED_MODULE_BYTES: u64 = GO_DEPENDENCY_MAX_BYTES * 4;
const GO_DEPENDENCY_MAX_PUBLISHED_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const GO_DEPENDENCY_MAX_LIFECYCLE_ENTRIES: usize = 1_024;
const GO_DEPENDENCY_STAGE_MARKER_GRACE: Duration = Duration::from_secs(30);
const GO_DEPENDENCY_CAPACITY_RETRY_INITIAL: Duration = Duration::from_millis(25);
const GO_DEPENDENCY_CAPACITY_RETRY_MAX: Duration = Duration::from_millis(250);
const GO_DEPENDENCY_MANIFEST_MAX_BYTES: u64 = 4 * 1024 * 1024;
const GO_DEPENDENCY_COMMAND_STDOUT_BYTES: usize = 16 * 1024 * 1024;
const GO_DEPENDENCY_COMMAND_STDERR_BYTES: usize = 1024 * 1024;
const MIN_SYNTHETIC_GO_WORK_VERSION: &str = "1.24";
const COMMAND_MONITOR_INTERVAL: Duration = Duration::from_millis(1);
const COMMAND_MAX_TRACKED_PROCESSES: usize = 4_096;
const COMMAND_MAX_SCANNED_PROCESSES: usize = 65_536;
const COMMAND_MAX_PROCESS_DESCRIPTORS: usize = 4_096;
const COMMAND_MAX_SCANNED_DESCRIPTORS: usize = 1_048_576;
const COMMAND_MAX_REFRESH_INSPECTIONS: usize = 32_768;
const COMMAND_MAX_REFRESH_METADATA_BYTES: usize = 16 * 1_048_576;
const COMMAND_OWNER_SCAN_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_LOCAL_CERTIFICATION_PATHS: usize = 4_096;
const MAX_LOCAL_CERTIFICATION_PATH_UNITS_TOTAL: usize = 16 * 1_048_576;
const GO_SEMANTIC_PROCESS_CONTAINMENT_SUPPORTED: bool =
    cfg!(any(windows, target_os = "linux", target_os = "macos"));

#[cfg(test)]
static TEST_GO_SEMANTIC_CONCURRENCY: OnceLock<(Mutex<TestGoSemanticConcurrencyState>, Condvar)> =
    OnceLock::new();
#[cfg(all(test, target_os = "linux"))]
static TEST_LINUX_CONTAINMENT_INSPECTION: OnceLock<RwLock<()>> = OnceLock::new();
#[cfg(test)]
thread_local! {
    static TEST_GO_SEMANTIC_SCOPED_PERMIT: std::cell::RefCell<
        Option<TestGoSemanticScopedPermit>,
    > = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
// Functional fixture budgets measure wall time. Run one default-cache semantic
// analysis at a time so concurrent Go frontend CPU and I/O contention is not
// charged to the active fixture; this is also stricter than the shared cache's
// retention accounting bound.
const TEST_GO_SEMANTIC_MAX_CONCURRENCY: usize = 1;
#[cfg(test)]
const TEST_GO_SEMANTIC_SCOPE_WAIT_TIMEOUT: Duration = Duration::from_secs(45 * 60);

#[cfg(test)]
#[derive(Debug)]
struct TestGoSemanticConcurrencyPermit;

#[cfg(test)]
#[derive(Debug)]
struct TestGoSemanticScopedPermit {
    permit: Arc<TestGoSemanticConcurrencyPermit>,
    scope_count: usize,
}

#[cfg(test)]
#[derive(Debug, Default)]
struct TestGoSemanticConcurrencyState {
    active: usize,
    next_waiter: u64,
    waiters: std::collections::VecDeque<u64>,
}

#[cfg(test)]
impl TestGoSemanticConcurrencyState {
    fn enqueue(&mut self) -> Result<u64, GoSemanticProcessError> {
        let waiter = self.next_waiter;
        self.next_waiter = self.next_waiter.checked_add(1).ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "test Go semantic concurrency waiter identifiers are exhausted".to_string(),
            )
        })?;
        self.waiters.push_back(waiter);
        Ok(waiter)
    }

    fn can_admit(&self, waiter: u64) -> bool {
        self.active < TEST_GO_SEMANTIC_MAX_CONCURRENCY && self.waiters.front() == Some(&waiter)
    }

    fn admit(&mut self, waiter: u64) {
        debug_assert!(self.can_admit(waiter));
        let admitted = self.waiters.pop_front();
        debug_assert_eq!(admitted, Some(waiter));
        self.active += 1;
    }

    fn cancel(&mut self, waiter: u64) {
        if let Some(position) = self.waiters.iter().position(|queued| *queued == waiter) {
            self.waiters.remove(position);
        }
    }

    fn release(&mut self) {
        debug_assert!(self.active > 0);
        self.active = self.active.saturating_sub(1);
    }
}

#[cfg(test)]
impl TestGoSemanticConcurrencyPermit {
    fn acquire(deadline: GoOperationDeadline) -> Result<Arc<Self>, GoSemanticProcessError> {
        if let Some(permit) = TEST_GO_SEMANTIC_SCOPED_PERMIT.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|scoped| Arc::clone(&scoped.permit))
        }) {
            return Ok(permit);
        }

        const LABEL: &str = "test Go semantic concurrency coordination";
        let (state, available) = TEST_GO_SEMANTIC_CONCURRENCY.get_or_init(|| {
            (
                Mutex::new(TestGoSemanticConcurrencyState::default()),
                Condvar::new(),
            )
        });
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let waiter = state.enqueue()?;
        while !state.can_admit(waiter) {
            let remaining = match deadline.remaining(LABEL) {
                Ok(remaining) => remaining,
                Err(error) => {
                    state.cancel(waiter);
                    available.notify_all();
                    return Err(error);
                }
            };
            let (next, result) = available
                .wait_timeout(state, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state = next;
            if result.timed_out() && !state.can_admit(waiter) {
                state.cancel(waiter);
                available.notify_all();
                return Err(GoSemanticProcessError::Timeout(format!(
                    "{LABEL} exceeded its execution deadline"
                )));
            }
        }
        state.admit(waiter);
        available.notify_all();
        Ok(Arc::new(Self))
    }
}

/// Holds one shared-cache test slot before fixture runtime measurement begins.
/// Nested frontend preparation on the same thread reuses this permit.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct TestGoSemanticConcurrencyScope {
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

#[cfg(test)]
pub(crate) fn acquire_test_go_semantic_concurrency_scope()
-> Result<TestGoSemanticConcurrencyScope, GoSemanticProcessError> {
    let permit = TestGoSemanticConcurrencyPermit::acquire(GoOperationDeadline::after(
        TEST_GO_SEMANTIC_SCOPE_WAIT_TIMEOUT,
    ))?;
    TEST_GO_SEMANTIC_SCOPED_PERMIT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(scoped) = slot.as_mut() {
            debug_assert!(Arc::ptr_eq(&scoped.permit, &permit));
            scoped.scope_count = scoped.scope_count.checked_add(1).ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "test Go semantic concurrency scope nesting is exhausted".to_string(),
                )
            })?;
        } else {
            *slot = Some(TestGoSemanticScopedPermit {
                permit,
                scope_count: 1,
            });
        }
        Ok::<(), GoSemanticProcessError>(())
    })?;
    Ok(TestGoSemanticConcurrencyScope {
        _not_send: std::marker::PhantomData,
    })
}

#[cfg(test)]
impl Drop for TestGoSemanticConcurrencyScope {
    fn drop(&mut self) {
        TEST_GO_SEMANTIC_SCOPED_PERMIT.with(|slot| {
            let mut slot = slot.borrow_mut();
            let remove = if let Some(scoped) = slot.as_mut() {
                debug_assert!(scoped.scope_count > 0);
                scoped.scope_count = scoped.scope_count.saturating_sub(1);
                scoped.scope_count == 0
            } else {
                debug_assert!(false, "test Go semantic scope lost its permit");
                false
            };
            if remove {
                slot.take();
            }
        });
    }
}

#[cfg(test)]
impl Drop for TestGoSemanticConcurrencyPermit {
    fn drop(&mut self) {
        let Some((state, available)) = TEST_GO_SEMANTIC_CONCURRENCY.get() else {
            return;
        };
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.release();
        available.notify_all();
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
const _: () = assert!(!GO_SEMANTIC_PROCESS_CONTAINMENT_SUPPORTED);

pub(crate) const fn go_semantic_process_containment_supported() -> bool {
    GO_SEMANTIC_PROCESS_CONTAINMENT_SUPPORTED
}

fn unsupported_process_containment_reason() -> String {
    format!(
        "Go semantic analysis is unavailable on {} because bounded process-tree containment is supported only on Linux, macOS, and Windows; syntax-level Go analysis remains available.",
        std::env::consts::OS
    )
}

fn require_go_semantic_process_containment() -> Result<(), GoSemanticProcessError> {
    if go_semantic_process_containment_supported() {
        Ok(())
    } else {
        Err(GoSemanticProcessError::CommandUnavailable(
            unsupported_process_containment_reason(),
        ))
    }
}
const EMBEDDED_GO_FRONTEND_FILES: &[(&str, &str)] = &[
    (
        "go.mod",
        include_str!("../../../go-sidecar/polint-go-frontend/go.mod"),
    ),
    (
        "go.sum",
        include_str!("../../../go-sidecar/polint-go-frontend/go.sum"),
    ),
    (
        "main.go",
        include_str!("../../../go-sidecar/polint-go-frontend/main.go"),
    ),
    (
        "internal/semantic/emit.go",
        include_str!("../../../go-sidecar/polint-go-frontend/internal/semantic/emit.go"),
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GoSemanticProcessError {
    CommandFailed(String),
    CommandUnavailable(String),
    VersionUnsupported(String),
    Timeout(String),
}

impl std::fmt::Display for GoSemanticProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CommandFailed(reason) => write!(f, "{reason}"),
            Self::CommandUnavailable(reason) => write!(f, "{reason}"),
            Self::VersionUnsupported(reason) => write!(f, "{reason}"),
            Self::Timeout(reason) => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for GoSemanticProcessError {}

impl GoSemanticProcessError {
    pub(crate) fn stable_reason(&self) -> &'static str {
        match self {
            Self::CommandFailed(_) => "command_failed",
            Self::CommandUnavailable(_) => "command_unavailable",
            Self::VersionUnsupported(_) => "version_unsupported",
            Self::Timeout(_) => "timeout",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GoOperationDeadline {
    end: Instant,
}

impl GoOperationDeadline {
    pub(crate) fn after(timeout: Duration) -> Self {
        Self {
            end: Instant::now()
                .checked_add(timeout)
                .unwrap_or_else(Instant::now),
        }
    }

    fn cap(self, timeout: Duration) -> Self {
        let stage = Self::after(timeout);
        if self.end <= stage.end { self } else { stage }
    }

    pub(crate) fn min(self, other: Self) -> Self {
        if self.end <= other.end { self } else { other }
    }

    fn check(self, label: &str) -> Result<(), GoSemanticProcessError> {
        if Instant::now() >= self.end {
            return Err(GoSemanticProcessError::Timeout(format!(
                "{label} exceeded its execution deadline"
            )));
        }
        Ok(())
    }

    fn remaining(self, label: &str) -> Result<Duration, GoSemanticProcessError> {
        self.check(label)?;
        Ok(self.end.saturating_duration_since(Instant::now()))
    }

    #[cfg(test)]
    fn at(end: Instant) -> Self {
        Self { end }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BoundedCommandLimits {
    pub(crate) timeout: Duration,
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
    combined_bytes: usize,
}

impl BoundedCommandLimits {
    pub(crate) const fn new(timeout: Duration, stdout_bytes: usize, stderr_bytes: usize) -> Self {
        Self {
            timeout,
            stdout_bytes,
            stderr_bytes,
            combined_bytes: if stdout_bytes > stderr_bytes {
                stdout_bytes
            } else {
                stderr_bytes
            },
        }
    }
}

#[derive(Debug)]
pub(crate) struct BoundedCommandOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
enum OutputStream {
    Stdout,
    Stderr,
}

impl OutputStream {
    const fn label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

#[derive(Debug)]
enum PipeReadError {
    Io(std::io::Error),
    LimitExceeded { limit: usize },
    CombinedLimitExceeded { limit: usize },
    Cancelled,
}

#[derive(Debug)]
struct PipeReadEvent {
    stream: OutputStream,
    result: Result<Vec<u8>, PipeReadError>,
}

pub(crate) fn run_bounded_command(
    command: Command,
    limits: BoundedCommandLimits,
    label: &str,
) -> Result<BoundedCommandOutput, GoSemanticProcessError> {
    run_bounded_command_until(
        command,
        limits,
        GoOperationDeadline::after(limits.timeout),
        label,
    )
}

fn run_bounded_command_until(
    mut command: Command,
    limits: BoundedCommandLimits,
    deadline: GoOperationDeadline,
    label: &str,
) -> Result<BoundedCommandOutput, GoSemanticProcessError> {
    #[cfg(all(test, target_os = "linux"))]
    let (_containment_inspection, deadline) = acquire_test_linux_containment_inspection(deadline);
    require_go_semantic_process_containment()?;
    deadline.check(label)?;
    let stderr_redaction = command_stderr_redaction_policy(&command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_process_group(&mut command);
    let pending_containment =
        PendingProcessContainment::install(&mut command, deadline).map_err(|error| {
            if error.kind() == std::io::ErrorKind::TimedOut {
                GoSemanticProcessError::Timeout(format!(
                    "{label} exceeded its operation deadline during process containment setup: {error}"
                ))
            } else {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to establish {label} process containment: {error}"
                ))
            }
        })?;
    if let Err(error) = deadline.check(label) {
        pending_containment.abort();
        return Err(error);
    }
    let spawned = command.spawn();
    drop(command);
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            pending_containment.abort();
            return Err(if error.kind() == std::io::ErrorKind::NotFound {
                GoSemanticProcessError::CommandUnavailable(format!(
                    "{label} executable was not found."
                ))
            } else {
                GoSemanticProcessError::CommandFailed(format!("failed to start {label}: {error}"))
            });
        }
    };
    let containment = match pending_containment.activate(&child, deadline, label) {
        Ok(containment) => containment,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let (sender, receiver) = mpsc::channel();
    let Some(stdout) = child.stdout.take() else {
        return finish_bounded_command_error(
            GoSemanticProcessError::CommandFailed(format!("failed to capture {label} stdout")),
            child,
            containment,
            None,
        );
    };
    let Some(stderr) = child.stderr.take() else {
        drop(stdout);
        return finish_bounded_command_error(
            GoSemanticProcessError::CommandFailed(format!("failed to capture {label} stderr")),
            child,
            containment,
            None,
        );
    };
    if let Err(error) = configure_pipe_nonblocking(&stdout) {
        drop(stdout);
        drop(stderr);
        return finish_bounded_command_error(
            GoSemanticProcessError::CommandFailed(format!(
                "failed to configure {label} stdout capture: {error}"
            )),
            child,
            containment,
            None,
        );
    }
    if let Err(error) = configure_pipe_nonblocking(&stderr) {
        drop(stdout);
        drop(stderr);
        return finish_bounded_command_error(
            GoSemanticProcessError::CommandFailed(format!(
                "failed to configure {label} stderr capture: {error}"
            )),
            child,
            containment,
            None,
        );
    }
    let cancellation = Arc::new(AtomicBool::new(false));
    let combined_bytes = Arc::new(AtomicUsize::new(0));
    let stdout_reader = match spawn_bounded_reader(
        stdout,
        limits.stdout_bytes,
        limits.combined_bytes,
        OutputStream::Stdout,
        sender.clone(),
        Arc::clone(&cancellation),
        Arc::clone(&combined_bytes),
        deadline,
    ) {
        Ok(reader) => reader,
        Err(error) => {
            drop(stderr);
            return finish_bounded_command_error(
                bounded_reader_setup_error(label, OutputStream::Stdout, error),
                child,
                containment,
                None,
            );
        }
    };
    let stderr_reader = match spawn_bounded_reader(
        stderr,
        limits.stderr_bytes,
        limits.combined_bytes,
        OutputStream::Stderr,
        sender,
        Arc::clone(&cancellation),
        combined_bytes,
        deadline,
    ) {
        Ok(reader) => reader,
        Err(error) => {
            let readers = PipeReaderTasks {
                cancellation,
                tasks: vec![stdout_reader],
            };
            return finish_bounded_command_error(
                bounded_reader_setup_error(label, OutputStream::Stderr, error),
                child,
                containment,
                Some(readers),
            );
        }
    };
    let readers = PipeReaderTasks {
        cancellation,
        tasks: vec![stdout_reader, stderr_reader],
    };

    let mut containment = containment;
    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;
    loop {
        if let Some(error) = containment.poll_failure() {
            return finish_bounded_command_error(
                GoSemanticProcessError::CommandFailed(format!(
                    "{label} process containment failed: {error}"
                )),
                child,
                containment,
                Some(readers),
            );
        }
        if status.is_none() {
            status = match child.try_wait() {
                Ok(status) => status,
                Err(error) => {
                    return finish_bounded_command_error(
                        GoSemanticProcessError::CommandFailed(format!(
                            "failed to poll {label}: {error}"
                        )),
                        child,
                        containment,
                        Some(readers),
                    );
                }
            };
        }
        while let Ok(event) = receiver.try_recv() {
            if let Err(error) = retain_pipe_event(event, &mut stdout, &mut stderr, label) {
                return finish_bounded_command_error(error, child, containment, Some(readers));
            }
        }
        if let (Some(status), Some(stdout), Some(stderr)) =
            (status, stdout.as_mut(), stderr.as_mut())
        {
            let output = BoundedCommandOutput {
                status,
                stdout: std::mem::take(stdout),
                stderr: redact_subprocess_stderr(&std::mem::take(stderr), &stderr_redaction)
                    .into_bytes(),
            };
            return finish_bounded_command_success(output, child, containment, readers);
        }
        if deadline.check(label).is_err() {
            return finish_bounded_command_error(
                GoSemanticProcessError::Timeout(format!("{label} exceeded its execution deadline")),
                child,
                containment,
                Some(readers),
            );
        }
        let remaining = deadline.end.saturating_duration_since(Instant::now());
        let wait = remaining.min(Duration::from_millis(5));
        if let Ok(event) = receiver.recv_timeout(wait)
            && let Err(error) = retain_pipe_event(event, &mut stdout, &mut stderr, label)
        {
            return finish_bounded_command_error(error, child, containment, Some(readers));
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
fn acquire_test_linux_containment_inspection(
    deadline: GoOperationDeadline,
) -> (std::sync::RwLockReadGuard<'static, ()>, GoOperationDeadline) {
    let wait_started = Instant::now();
    let was_expired = wait_started >= deadline.end;
    let guard = TEST_LINUX_CONTAINMENT_INSPECTION
        .get_or_init(|| RwLock::new(()))
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if was_expired {
        return (guard, deadline);
    }
    let waited = wait_started.elapsed();
    let adjusted = GoOperationDeadline {
        end: deadline.end.checked_add(waited).unwrap_or(deadline.end),
    };
    (guard, adjusted)
}

fn bounded_reader_setup_error(
    label: &str,
    stream: OutputStream,
    error: std::io::Error,
) -> GoSemanticProcessError {
    let message = format!("failed to start {label} {} reader: {error}", stream.label());
    if error.kind() == std::io::ErrorKind::TimedOut {
        GoSemanticProcessError::Timeout(message)
    } else {
        GoSemanticProcessError::CommandFailed(message)
    }
}

fn run_bounded_command_with_local_trees_until(
    mut command: Command,
    additional_roots: &[PathBuf],
    limits: BoundedCommandLimits,
    deadline: GoOperationDeadline,
    label: &str,
) -> Result<BoundedCommandOutput, GoSemanticProcessError> {
    require_local_scan_roots_with_exclusions(additional_roots, &[], deadline)?;
    // `Command` inherits the parent cwd when none is set. Resolve that
    // inheritance before certification so the child cannot observe an
    // unchecked path, but do not recursively scan the whole repository:
    // callers provide the exact trees the subprocess can traverse.
    let current_dir = if let Some(path) = command.get_current_dir() {
        validate_local_path_size_until(
            path,
            "Go command working-directory certification",
            deadline,
        )?;
        path.to_path_buf()
    } else {
        std::env::current_dir().map_err(|error| {
            GoSemanticProcessError::CommandUnavailable(format!(
                "failed to resolve {label} working directory before local filesystem certification: {error}"
            ))
        })?
    };
    require_local_existing_path_until(
        &current_dir,
        deadline,
        "Go command working-directory certification",
    )?;
    command.current_dir(go_command_working_directory(&current_dir)?);

    run_bounded_command_until(command, limits, deadline, label)
}

fn collapse_nested_local_roots(
    roots: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<Vec<&Path>, GoSemanticProcessError> {
    const LABEL: &str = "Go semantic local tree certification";

    validate_local_path_batch(roots, LABEL, Some(deadline))?;
    let mut ordered = Vec::new();
    ordered.try_reserve_exact(roots.len()).map_err(|error| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "failed to allocate bounded local-root certification state: {error}"
        ))
    })?;
    for root in roots {
        deadline.check(LABEL)?;
        let depth = root.components().count();
        deadline.check(LABEL)?;
        ordered.push((root.as_path(), depth));
    }
    deadline.check(LABEL)?;
    ordered.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(right.0)));
    deadline.check(LABEL)?;
    let mut collapsed = Vec::<&Path>::new();
    collapsed
        .try_reserve_exact(ordered.len())
        .map_err(|error| {
            GoSemanticProcessError::CommandUnavailable(format!(
                "failed to allocate bounded collapsed-root certification state: {error}"
            ))
        })?;
    for (root, _) in ordered {
        deadline.check(LABEL)?;
        let mut nested = false;
        for parent in &collapsed {
            if root.starts_with(parent) {
                nested = true;
                break;
            }
            deadline.check(LABEL)?;
        }
        if nested {
            continue;
        }
        collapsed.push(root);
    }
    Ok(collapsed)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CommandStderrRedactionPolicy {
    redact_all: bool,
}

const REDACTED_SUBPROCESS_STDERR: &str =
    "[REDACTED: subprocess stderr may contain configured credentials]";

fn command_stderr_redaction_policy(command: &Command) -> CommandStderrRedactionPolicy {
    let mut redact_all = false;
    for (name, value) in command.get_envs() {
        let name = name.to_string_lossy();
        if !matches!(
            name.as_ref(),
            "GOPROXY"
                | "GOSUMDB"
                | "HTTP_PROXY"
                | "HTTPS_PROXY"
                | "http_proxy"
                | "https_proxy"
                | "GOAUTH"
        ) {
            continue;
        }
        let Some(value) = value.and_then(std::ffi::OsStr::to_str) else {
            continue;
        };
        if name == "GOAUTH" {
            let value = value.trim();
            if !value.is_empty() && value != "off" {
                redact_all = true;
            }
            continue;
        }
        if name == "GOSUMDB" {
            for (index, field) in value.split_whitespace().enumerate() {
                if index != 0 || field.contains("://") {
                    redact_all |= endpoint_may_contain_credentials(field);
                }
            }
        } else {
            for endpoint in value.split([',', '|']) {
                redact_all |= endpoint_may_contain_credentials(endpoint);
            }
        }
    }
    CommandStderrRedactionPolicy { redact_all }
}

fn endpoint_may_contain_credentials(endpoint: &str) -> bool {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || matches!(endpoint, "direct" | "off") {
        return false;
    }
    // User-configured file proxies are rejected before command construction.
    // The only retained file endpoint is polint's owner-private module-cache
    // proxy, whose path is not an external credential.
    if has_ascii_case_insensitive_prefix(endpoint, "file:") {
        return false;
    }
    let authority = endpoint
        .find("://")
        .map_or(endpoint, |scheme| &endpoint[scheme + 3..]);
    let authority_end = authority
        .find(|character: char| {
            character.is_ascii_whitespace() || matches!(character, '/' | '?' | '#')
        })
        .unwrap_or(authority.len());
    if authority[..authority_end].contains('@') {
        return true;
    }
    let sensitive_suffix = &authority[authority_end..];
    sensitive_suffix
        .chars()
        .any(|character| !matches!(character, '/' | '?' | '#' | '&' | '='))
}

fn redact_subprocess_stderr(stderr: &[u8], policy: &CommandStderrRedactionPolicy) -> String {
    if stderr.is_empty() {
        return String::new();
    }
    if policy.redact_all {
        return REDACTED_SUBPROCESS_STDERR.to_string();
    }
    let value = String::from_utf8_lossy(stderr).into_owned();
    redact_url_userinfo(&value)
}

fn redact_url_userinfo(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remainder = value;
    while let Some(scheme) = remainder.find("://") {
        let authority_start = scheme + 3;
        output.push_str(&remainder[..authority_start]);
        let authority = &remainder[authority_start..];
        let authority_end = authority
            .find(|character: char| {
                character.is_ascii_whitespace() || matches!(character, '/' | '?' | '#' | ',' | '|')
            })
            .unwrap_or(authority.len());
        if let Some(at) = authority[..authority_end].rfind('@') {
            output.push_str("[REDACTED]@");
            output.push_str(&authority[at + 1..authority_end]);
        } else {
            output.push_str(&authority[..authority_end]);
        }
        remainder = &authority[authority_end..];
    }
    output.push_str(remainder);
    output
}

#[expect(
    clippy::too_many_arguments,
    reason = "Each bounded pipe-reader safety limit and shared cancellation primitive stays explicit at the thread boundary."
)]
fn spawn_bounded_reader(
    reader: impl Read + Send + 'static,
    limit: usize,
    combined_limit: usize,
    stream: OutputStream,
    sender: mpsc::Sender<PipeReadEvent>,
    cancellation: Arc<AtomicBool>,
    combined_bytes: Arc<AtomicUsize>,
    deadline: GoOperationDeadline,
) -> std::io::Result<PipeReaderTask> {
    #[cfg(windows)]
    let (setup_sender, setup_receiver) = mpsc::sync_channel(1);
    let handle = thread::Builder::new()
        .name(format!("polint-command-{}", stream.label()))
        .spawn(move || {
            #[cfg(windows)]
            {
                let canceller =
                    crate::go::semantic::windows::ThreadIoCanceller::for_current_thread();
                let setup_succeeded = canceller.is_ok();
                if setup_sender.send(canceller).is_err() || !setup_succeeded {
                    return;
                }
            }
            let result = read_bounded(
                reader,
                limit,
                combined_limit,
                &cancellation,
                &combined_bytes,
            );
            let _ = sender.send(PipeReadEvent { stream, result });
        })?;
    #[cfg(not(windows))]
    let _ = deadline;
    #[cfg(windows)]
    let remaining = deadline.end.saturating_duration_since(Instant::now());
    #[cfg(windows)]
    let canceller = match setup_receiver.recv_timeout(remaining) {
        Ok(Ok(canceller)) => canceller,
        Ok(Err(error)) => {
            let _ = handle.join();
            return Err(error);
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            drop(handle);
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "Windows {} reader setup exceeded the command deadline",
                    stream.label()
                ),
            ));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = handle.join();
            return Err(std::io::Error::other(
                "Windows command reader setup stopped unexpectedly",
            ));
        }
    };
    Ok(PipeReaderTask {
        handle,
        #[cfg(windows)]
        canceller,
    })
}

fn read_bounded(
    mut reader: impl Read,
    limit: usize,
    combined_limit: usize,
    cancellation: &AtomicBool,
    combined_bytes: &AtomicUsize,
) -> Result<Vec<u8>, PipeReadError> {
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        if cancellation.load(Ordering::Acquire) {
            return Err(PipeReadError::Cancelled);
        }
        let count = match reader.read(&mut buffer) {
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::park_timeout(COMMAND_MONITOR_INTERVAL);
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(PipeReadError::Io(error)),
        };
        if count == 0 {
            return Ok(bytes);
        }
        if count > limit.saturating_sub(bytes.len()) {
            return Err(PipeReadError::LimitExceeded { limit });
        }
        if combined_bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(count)
                    .filter(|next| *next <= combined_limit)
            })
            .is_err()
        {
            return Err(PipeReadError::CombinedLimitExceeded {
                limit: combined_limit,
            });
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
}

fn retain_pipe_event(
    event: PipeReadEvent,
    stdout: &mut Option<Vec<u8>>,
    stderr: &mut Option<Vec<u8>>,
    label: &str,
) -> Result<(), GoSemanticProcessError> {
    let bytes = match event.result {
        Ok(bytes) => bytes,
        Err(PipeReadError::LimitExceeded { limit }) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "{label} {} exceeded the {limit}-byte output limit",
                event.stream.label()
            )));
        }
        Err(PipeReadError::CombinedLimitExceeded { limit }) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "{label} combined stdout and stderr exceeded the {limit}-byte output limit"
            )));
        }
        Err(PipeReadError::Io(error)) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to read {label} {}: {error}",
                event.stream.label()
            )));
        }
        Err(PipeReadError::Cancelled) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "{label} output capture was cancelled unexpectedly"
            )));
        }
    };
    match event.stream {
        OutputStream::Stdout => *stdout = Some(bytes),
        OutputStream::Stderr => *stderr = Some(bytes),
    }
    Ok(())
}

struct PipeReaderTask {
    handle: thread::JoinHandle<()>,
    #[cfg(windows)]
    canceller: crate::go::semantic::windows::ThreadIoCanceller,
}

struct PipeReaderTasks {
    cancellation: Arc<AtomicBool>,
    tasks: Vec<PipeReaderTask>,
}

impl PipeReaderTasks {
    fn cancel(&self) -> Result<(), String> {
        self.cancellation.store(true, Ordering::Release);
        #[cfg(windows)]
        {
            let mut failure = None;
            for task in &self.tasks {
                if let Err(error) = task.canceller.cancel() {
                    failure.get_or_insert_with(|| {
                        format!("failed to cancel a Windows command output reader: {error}")
                    });
                }
            }
            if let Some(error) = failure {
                return Err(error);
            }
        }
        Ok(())
    }

    fn join(self) -> Result<(), String> {
        let mut failure = None;
        for task in self.tasks {
            if task.handle.join().is_err() {
                failure.get_or_insert_with(|| {
                    "a command output reader panicked during cleanup".to_string()
                });
            }
        }
        failure.map_or(Ok(()), Err)
    }

    fn cancel_and_join(self) -> Result<(), String> {
        let cancellation = self.cancel();
        let joined = self.join();
        cancellation.and(joined)
    }
}

fn finish_bounded_command_success(
    output: BoundedCommandOutput,
    mut child: Child,
    containment: ProcessContainment,
    readers: PipeReaderTasks,
) -> Result<BoundedCommandOutput, GoSemanticProcessError> {
    let containment_result = containment.terminate(true);
    let wait_result = child.wait();
    let readers_result = readers.join();
    if let Err(error) = containment_result {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "command process containment cleanup failed: {error}"
        )));
    }
    if let Err(error) = wait_result {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to reap command process: {error}"
        )));
    }
    if let Err(error) = readers_result {
        return Err(GoSemanticProcessError::CommandFailed(error));
    }
    Ok(output)
}

fn finish_bounded_command_error(
    mut error: GoSemanticProcessError,
    mut child: Child,
    containment: ProcessContainment,
    readers: Option<PipeReaderTasks>,
) -> Result<BoundedCommandOutput, GoSemanticProcessError> {
    let containment_result = containment.terminate(true);
    let _ = child.kill();
    let wait_result = child.wait();
    let readers_result = readers.map_or(Ok(()), PipeReaderTasks::cancel_and_join);
    let mut failures = Vec::new();
    if let Err(cleanup_error) = containment_result {
        failures.push(format!(
            "process containment cleanup failed: {cleanup_error}"
        ));
    }
    if let Err(cleanup_error) = wait_result {
        failures.push(format!("process reap failed: {cleanup_error}"));
    }
    if let Err(cleanup_error) = readers_result {
        failures.push(cleanup_error);
    }
    if !failures.is_empty() {
        append_process_error_context(&mut error, &failures.join("; "));
    }
    Err(error)
}

fn append_process_error_context(error: &mut GoSemanticProcessError, context: &str) {
    match error {
        GoSemanticProcessError::CommandFailed(reason)
        | GoSemanticProcessError::CommandUnavailable(reason)
        | GoSemanticProcessError::VersionUnsupported(reason)
        | GoSemanticProcessError::Timeout(reason) => {
            reason.push_str("; ");
            reason.push_str(context);
        }
    }
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn configure_pipe_nonblocking(reader: &impl std::os::fd::AsRawFd) -> std::io::Result<()> {
    let descriptor = reader.as_raw_fd();
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OwnerSentinelIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct OpenOwnerSentinel {
    identity: OwnerSentinelIdentity,
    descriptor: libc::c_int,
}

#[cfg(not(unix))]
fn configure_pipe_nonblocking(_reader: &impl Read) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
enum ContainmentControl {
    Terminate { discover_owner_holders: bool },
}

#[cfg(unix)]
enum ContainmentStatus {
    Ready,
    Failed(String),
}

#[cfg(unix)]
struct PendingProcessContainment {
    control: mpsc::Sender<ContainmentControl>,
    status: mpsc::Receiver<ContainmentStatus>,
    monitor: thread::JoinHandle<Result<(), String>>,
}

#[cfg(unix)]
fn local_filesystem_error_as_io(
    error: crate::go::semantic::local_fs::LocalFilesystemError,
) -> std::io::Error {
    let kind = match &error {
        crate::go::semantic::local_fs::LocalFilesystemError::Inspection { source, .. } => {
            source.kind()
        }
        crate::go::semantic::local_fs::LocalFilesystemError::NonLocal { .. }
        | crate::go::semantic::local_fs::LocalFilesystemError::UnsupportedPlatform { .. } => {
            std::io::ErrorKind::Other
        }
    };
    std::io::Error::new(kind, error.to_string())
}

#[cfg(unix)]
impl PendingProcessContainment {
    #[allow(unsafe_code)]
    fn install(command: &mut Command, deadline: GoOperationDeadline) -> std::io::Result<Self> {
        use std::ffi::CString;
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::net::UnixStream;
        use std::os::unix::process::CommandExt;

        if !go_semantic_process_containment_supported() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                unsupported_process_containment_reason(),
            ));
        }

        let sentinel_root = std::env::temp_dir();
        crate::go::semantic::local_fs::require_local_containing_path_until(
            &sentinel_root,
            deadline.end,
        )
        .map_err(local_filesystem_error_as_io)?;
        let sentinel_directory = tempfile::Builder::new()
            .prefix("polint-containment-")
            .tempdir_in(&sentinel_root)?;
        crate::go::semantic::local_fs::require_local_tree_until(
            sentinel_directory.path(),
            deadline.end,
        )
        .map_err(local_filesystem_error_as_io)?;
        let sentinel_path = CString::new(
            sentinel_directory
                .path()
                .join("owner")
                .as_os_str()
                .as_bytes(),
        )
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "containment sentinel path contained a null byte",
            )
        })?;
        #[cfg(test)]
        let report_invalid_sentinel = command
            .get_envs()
            .any(|(name, _)| name == "POLINT_TEST_INVALID_CONTAINMENT_SENTINEL");
        let (monitor_channel, child_channel) = UnixStream::pair()?;
        let monitor_descriptor = monitor_channel.as_raw_fd();
        unsafe {
            command.pre_exec(move || {
                let descriptor = child_channel.as_raw_fd();
                if monitor_descriptor != descriptor && libc::close(monitor_descriptor) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let sentinel = open_owner_sentinel(&sentinel_path)?;
                #[cfg(test)]
                let sentinel = if report_invalid_sentinel {
                    OpenOwnerSentinel {
                        identity: OwnerSentinelIdentity {
                            device: sentinel.identity.device,
                            inode: sentinel.identity.inode.wrapping_add(1),
                        },
                        descriptor: sentinel.descriptor,
                    }
                } else {
                    sentinel
                };
                write_pre_exec_bytes(descriptor, &libc::getpid().to_ne_bytes())?;
                write_pre_exec_bytes(descriptor, &sentinel.identity.device.to_ne_bytes())?;
                write_pre_exec_bytes(descriptor, &sentinel.identity.inode.to_ne_bytes())?;
                write_pre_exec_bytes(descriptor, &sentinel.descriptor.to_ne_bytes())?;
                let mut release = [0_u8; 1];
                read_pre_exec_bytes(descriptor, &mut release)
            });
        }

        let (control_sender, control_receiver) = mpsc::channel();
        let (status_sender, status_receiver) = mpsc::channel();
        let monitor = thread::Builder::new()
            .name("polint-command-containment".to_string())
            .spawn(move || {
                let _sentinel_directory = sentinel_directory;
                run_process_containment_monitor(
                    monitor_channel,
                    control_receiver,
                    status_sender,
                    deadline,
                )
            })?;
        Ok(Self {
            control: control_sender,
            status: status_receiver,
            monitor,
        })
    }

    fn activate(
        self,
        _child: &Child,
        deadline: GoOperationDeadline,
        label: &str,
    ) -> Result<ProcessContainment, GoSemanticProcessError> {
        let remaining = match deadline.remaining(label) {
            Ok(remaining) => remaining,
            Err(error) => {
                let _ = self.control.send(ContainmentControl::Terminate {
                    discover_owner_holders: true,
                });
                let _ = self.monitor.join();
                return Err(error);
            }
        };
        match self.status.recv_timeout(remaining) {
            Ok(ContainmentStatus::Ready) => {
                if let Err(error) = deadline.check(label) {
                    let _ = self.control.send(ContainmentControl::Terminate {
                        discover_owner_holders: true,
                    });
                    let _ = self.monitor.join();
                    return Err(error);
                }
                Ok(ProcessContainment {
                    control: self.control,
                    status: self.status,
                    monitor: self.monitor,
                })
            }
            Ok(ContainmentStatus::Failed(error)) => {
                let _ = self.control.send(ContainmentControl::Terminate {
                    discover_owner_holders: true,
                });
                let _ = self.monitor.join();
                Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to establish {label} process containment: {error}"
                )))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = self.control.send(ContainmentControl::Terminate {
                    discover_owner_holders: true,
                });
                let _ = self.monitor.join();
                Err(GoSemanticProcessError::Timeout(format!(
                    "{label} exceeded its execution deadline during process containment setup"
                )))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = self.monitor.join();
                deadline.check(label)?;
                Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to establish {label} process containment: containment monitor stopped unexpectedly"
                )))
            }
        }
    }

    fn abort(self) {
        let _ = self.control.send(ContainmentControl::Terminate {
            discover_owner_holders: false,
        });
        let _ = self.monitor.join();
    }
}

#[cfg(windows)]
struct PendingProcessContainment {
    job: crate::go::semantic::windows::KillOnCloseJob,
}

#[cfg(windows)]
impl PendingProcessContainment {
    fn install(command: &mut Command, _deadline: GoOperationDeadline) -> std::io::Result<Self> {
        let job = crate::go::semantic::windows::KillOnCloseJob::new()?;
        crate::go::semantic::windows::configure_suspended_command(command);
        Ok(Self { job })
    }

    fn activate(
        self,
        child: &Child,
        deadline: GoOperationDeadline,
        label: &str,
    ) -> Result<ProcessContainment, GoSemanticProcessError> {
        let terminate_with = |error: GoSemanticProcessError| {
            let _ = self.job.terminate();
            error
        };
        deadline.check(label).map_err(terminate_with)?;
        let suspended = self
            .job
            .assign_suspended_child(child, deadline.end)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::TimedOut {
                    terminate_with(GoSemanticProcessError::Timeout(format!(
                        "{label} exceeded its execution deadline during Windows Job Object containment: {error}"
                    )))
                } else {
                    terminate_with(GoSemanticProcessError::CommandFailed(format!(
                        "failed to establish {label} Windows Job Object containment: {error}"
                    )))
                }
            })?;
        deadline.check(label).map_err(terminate_with)?;
        suspended.resume().map_err(|error| {
            terminate_with(GoSemanticProcessError::CommandFailed(format!(
                "failed to resume contained {label}: {error}"
            )))
        })?;
        Ok(ProcessContainment { job: self.job })
    }

    fn abort(self) {}
}

#[cfg(all(not(unix), not(windows)))]
struct PendingProcessContainment;

#[cfg(all(not(unix), not(windows)))]
impl PendingProcessContainment {
    fn install(_command: &mut Command, _deadline: GoOperationDeadline) -> std::io::Result<Self> {
        Ok(Self)
    }

    fn activate(
        self,
        _child: &Child,
        deadline: GoOperationDeadline,
        label: &str,
    ) -> Result<ProcessContainment, GoSemanticProcessError> {
        deadline.check(label)?;
        Ok(ProcessContainment)
    }

    fn abort(self) {}
}

#[cfg(unix)]
struct ProcessContainment {
    control: mpsc::Sender<ContainmentControl>,
    status: mpsc::Receiver<ContainmentStatus>,
    monitor: thread::JoinHandle<Result<(), String>>,
}

#[cfg(unix)]
impl ProcessContainment {
    fn poll_failure(&mut self) -> Option<String> {
        match self.status.try_recv() {
            Ok(ContainmentStatus::Failed(error)) => Some(error),
            Ok(ContainmentStatus::Ready) | Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some("containment monitor stopped unexpectedly".to_string())
            }
        }
    }

    fn terminate(self, discover_owner_holders: bool) -> Result<(), String> {
        let _ = self.control.send(ContainmentControl::Terminate {
            discover_owner_holders,
        });
        self.monitor
            .join()
            .map_err(|_| "containment monitor panicked".to_string())?
    }
}

#[cfg(windows)]
struct ProcessContainment {
    job: crate::go::semantic::windows::KillOnCloseJob,
}

#[cfg(windows)]
impl ProcessContainment {
    fn poll_failure(&mut self) -> Option<String> {
        None
    }

    fn terminate(self, _discover_owner_holders: bool) -> Result<(), String> {
        self.job
            .terminate()
            .map_err(|error| format!("failed to terminate Windows Job Object: {error}"))
    }
}

#[cfg(all(not(unix), not(windows)))]
struct ProcessContainment;

#[cfg(all(not(unix), not(windows)))]
impl ProcessContainment {
    fn poll_failure(&mut self) -> Option<String> {
        None
    }

    fn terminate(self, _discover_owner_holders: bool) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn open_owner_sentinel(path: &std::ffi::CStr) -> std::io::Result<OpenOwnerSentinel> {
    let descriptor = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW,
            0o600,
        )
    };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(descriptor, metadata.as_mut_ptr()) } != 0 {
        let error = std::io::Error::last_os_error();
        unsafe {
            libc::close(descriptor);
        }
        return Err(error);
    }
    if unsafe { libc::fcntl(descriptor, libc::F_SETFD, 0) } != 0 {
        let error = std::io::Error::last_os_error();
        unsafe {
            libc::close(descriptor);
        }
        return Err(error);
    }
    let metadata = unsafe { metadata.assume_init() };
    #[cfg(target_vendor = "apple")]
    let device = u64::try_from(metadata.st_dev).map_err(|_| {
        unsafe {
            libc::close(descriptor);
        }
        std::io::Error::from_raw_os_error(libc::EOVERFLOW)
    })?;
    #[cfg(not(target_vendor = "apple"))]
    let device = metadata.st_dev;
    let inode = metadata.st_ino;
    Ok(OpenOwnerSentinel {
        identity: OwnerSentinelIdentity { device, inode },
        descriptor,
    })
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn write_pre_exec_bytes(descriptor: libc::c_int, mut bytes: &[u8]) -> std::io::Result<()> {
    while !bytes.is_empty() {
        let written = unsafe { libc::write(descriptor, bytes.as_ptr().cast(), bytes.len()) };
        if written < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if written == 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EPIPE));
        }
        bytes = &bytes[usize::try_from(written).unwrap_or(bytes.len())..];
    }
    Ok(())
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn read_pre_exec_bytes(descriptor: libc::c_int, mut bytes: &mut [u8]) -> std::io::Result<()> {
    while !bytes.is_empty() {
        let read = unsafe { libc::read(descriptor, bytes.as_mut_ptr().cast(), bytes.len()) };
        if read < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if read == 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EPIPE));
        }
        let read = usize::try_from(read).unwrap_or(bytes.len());
        bytes = &mut bytes[read..];
    }
    Ok(())
}

#[cfg(unix)]
fn run_process_containment_monitor(
    mut channel: std::os::unix::net::UnixStream,
    control: mpsc::Receiver<ContainmentControl>,
    status: mpsc::Sender<ContainmentStatus>,
    deadline: GoOperationDeadline,
) -> Result<(), String> {
    containment_setup_deadline_check(deadline)?;
    let remaining = deadline.end.saturating_duration_since(Instant::now());
    channel
        .set_read_timeout(Some(remaining))
        .map_err(|error| format!("failed to bound containment handshake reads: {error}"))?;
    channel
        .set_write_timeout(Some(remaining))
        .map_err(|error| format!("failed to bound containment handshake writes: {error}"))?;
    let mut pid_bytes = [0_u8; std::mem::size_of::<libc::pid_t>()];
    channel
        .read_exact(&mut pid_bytes)
        .map_err(|error| format!("failed to receive contained process id: {error}"))?;
    let pid = libc::pid_t::from_ne_bytes(pid_bytes);
    let mut device_bytes = [0_u8; std::mem::size_of::<u64>()];
    channel
        .read_exact(&mut device_bytes)
        .map_err(|error| format!("failed to receive containment sentinel device: {error}"))?;
    let mut inode_bytes = [0_u8; std::mem::size_of::<u64>()];
    channel
        .read_exact(&mut inode_bytes)
        .map_err(|error| format!("failed to receive containment sentinel inode: {error}"))?;
    let owner_sentinel = OwnerSentinelIdentity {
        device: u64::from_ne_bytes(device_bytes),
        inode: u64::from_ne_bytes(inode_bytes),
    };
    let mut descriptor_bytes = [0_u8; std::mem::size_of::<libc::c_int>()];
    channel
        .read_exact(&mut descriptor_bytes)
        .map_err(|error| format!("failed to receive containment sentinel descriptor: {error}"))?;
    containment_setup_deadline_check(deadline)?;
    let owner_descriptor = libc::c_int::from_ne_bytes(descriptor_bytes);
    let owner_sentinel = resolve_owner_sentinel_identity(pid, owner_descriptor, owner_sentinel)?;
    let mut tracker = ProcessTreeTracker::new(pid, owner_descriptor, owner_sentinel, deadline.end)?;
    containment_setup_deadline_check(deadline)?;
    channel
        .write_all(&[1])
        .map_err(|error| format!("failed to release contained process: {error}"))?;
    let _ = status.send(ContainmentStatus::Ready);

    loop {
        if let Err(error) = tracker.refresh() {
            let cleanup = tracker.terminate(true);
            let error = match cleanup {
                Ok(()) => error,
                Err(cleanup_error) => {
                    format!("{error}; containment cleanup failed: {cleanup_error}")
                }
            };
            let _ = status.send(ContainmentStatus::Failed(error.clone()));
            return Err(error);
        }
        match control.recv_timeout(COMMAND_MONITOR_INTERVAL) {
            Ok(ContainmentControl::Terminate {
                discover_owner_holders,
            }) => return tracker.terminate(discover_owner_holders),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return tracker.terminate(true),
        }
    }
}

#[cfg(unix)]
fn containment_setup_deadline_check(deadline: GoOperationDeadline) -> Result<(), String> {
    if Instant::now() >= deadline.end {
        Err("process containment setup exceeded the command execution deadline".to_string())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ProcessIdentity {
    start_primary: u64,
    start_secondary: u64,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct TrackedProcess {
    pid: libc::pid_t,
    identity: ProcessIdentity,
}

#[cfg(unix)]
struct OwnerSentinelScan {
    deadline: Instant,
    descriptors: usize,
}

#[cfg(unix)]
impl OwnerSentinelScan {
    fn until(deadline: Instant) -> Self {
        Self {
            deadline,
            descriptors: 0,
        }
    }

    #[cfg(test)]
    fn new() -> Self {
        let now = Instant::now();
        Self::until(now.checked_add(COMMAND_OWNER_SCAN_TIMEOUT).unwrap_or(now))
    }

    fn check_deadline(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            Err(format!(
                "ownership-sentinel scan exceeded its {}-millisecond deadline",
                COMMAND_OWNER_SCAN_TIMEOUT.as_millis()
            ))
        } else {
            Ok(())
        }
    }

    fn account_descriptors(&mut self, count: usize) -> Result<(), String> {
        self.check_deadline()?;
        self.descriptors = self
            .descriptors
            .checked_add(count)
            .ok_or_else(|| "ownership-sentinel descriptor count overflowed".to_string())?;
        if self.descriptors > COMMAND_MAX_SCANNED_DESCRIPTORS {
            return Err(format!(
                "ownership-sentinel enumeration exceeded the {COMMAND_MAX_SCANNED_DESCRIPTORS}-descriptor scan limit"
            ));
        }
        Ok(())
    }
}

#[cfg(unix)]
struct ProcessRefreshBudget {
    deadline: Instant,
    inspections: usize,
    metadata_bytes: usize,
}

#[cfg(unix)]
impl ProcessRefreshBudget {
    fn new(deadline: Instant) -> Self {
        Self {
            deadline,
            inspections: 0,
            metadata_bytes: 0,
        }
    }

    fn check(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            Err("process-tree refresh exceeded its execution deadline".to_string())
        } else {
            Ok(())
        }
    }

    fn account_inspections(&mut self, count: usize) -> Result<(), String> {
        self.check()?;
        self.inspections = self
            .inspections
            .checked_add(count)
            .ok_or_else(|| "process-tree inspection count overflowed".to_string())?;
        if self.inspections > COMMAND_MAX_REFRESH_INSPECTIONS {
            return Err(format!(
                "process-tree refresh exceeded the {COMMAND_MAX_REFRESH_INSPECTIONS}-inspection limit"
            ));
        }
        Ok(())
    }

    fn account_metadata_bytes(&mut self, count: usize) -> Result<(), String> {
        self.check()?;
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(count)
            .ok_or_else(|| "process-tree metadata byte count overflowed".to_string())?;
        if self.metadata_bytes > COMMAND_MAX_REFRESH_METADATA_BYTES {
            return Err(format!(
                "process-tree refresh exceeded the {COMMAND_MAX_REFRESH_METADATA_BYTES}-byte metadata limit"
            ));
        }
        Ok(())
    }
}

#[cfg(unix)]
struct ProcessTreeTracker {
    processes: BTreeMap<libc::pid_t, ProcessIdentity>,
    process_group: libc::pid_t,
    root_identity: ProcessIdentity,
    owner_scan_baseline: BTreeMap<libc::pid_t, ProcessIdentity>,
    owner_sentinel: OwnerSentinelIdentity,
    operation_deadline: Instant,
}

#[cfg(unix)]
impl ProcessTreeTracker {
    fn new(
        root: libc::pid_t,
        owner_descriptor: libc::c_int,
        owner_sentinel: OwnerSentinelIdentity,
        operation_deadline: Instant,
    ) -> Result<Self, String> {
        let identity_before = process_identity(root)?
            .ok_or_else(|| format!("contained process {root} exited before it could be tracked"))?;
        if !process_holds_owner_sentinel_descriptor(root, owner_descriptor, owner_sentinel)? {
            return Err(format!(
                "contained process {root} did not retain its ownership sentinel"
            ));
        }
        let identity = process_identity(root)?
            .filter(|identity| *identity == identity_before)
            .ok_or_else(|| {
                format!("contained process {root} changed while ownership was verified")
            })?;
        // The root is still blocked in pre-exec, so an unchanged process that
        // already exists here cannot be one of its descendants. Remember those
        // identities to avoid probing unrelated descriptors during cleanup.
        let owner_scan_baseline = capture_owner_scan_baseline(root, operation_deadline);
        Ok(Self {
            processes: BTreeMap::from([(root, identity)]),
            process_group: root,
            root_identity: identity,
            owner_scan_baseline,
            owner_sentinel,
            operation_deadline,
        })
    }

    fn refresh(&mut self) -> Result<(), String> {
        self.refresh_until(self.operation_deadline)
    }

    fn refresh_until(&mut self, deadline: Instant) -> Result<(), String> {
        let mut budget = ProcessRefreshBudget::new(deadline);
        budget.check()?;
        self.prune_stale_processes(&mut budget)?;
        let mut frontier = self
            .processes
            .iter()
            .map(|(&pid, &identity)| TrackedProcess { pid, identity })
            .collect::<VecDeque<_>>();
        let mut visited = BTreeSet::new();
        while let Some(parent) = frontier.pop_front() {
            budget.account_inspections(1)?;
            if !visited.insert(parent.pid) || !process_matches(parent)? {
                continue;
            }
            let mut candidates = Vec::new();
            for pid in child_process_ids(parent.pid, &mut budget)? {
                if self.processes.contains_key(&pid) {
                    continue;
                }
                budget.account_inspections(1)?;
                if let Some(identity) = process_identity(pid)? {
                    candidates.push((pid, identity));
                }
            }
            if !process_matches(parent)? {
                continue;
            }
            let confirmed = child_process_ids(parent.pid, &mut budget)?
                .into_iter()
                .collect::<BTreeSet<_>>();
            if !process_matches(parent)? {
                continue;
            }
            for (pid, identity) in candidates {
                budget.account_inspections(1)?;
                if !confirmed.contains(&pid)
                    || process_identity(pid)?.is_none_or(|current| current != identity)
                {
                    continue;
                }
                if self.processes.len() >= COMMAND_MAX_TRACKED_PROCESSES {
                    return Err(format!(
                        "contained command exceeded the {COMMAND_MAX_TRACKED_PROCESSES}-process tracking limit"
                    ));
                }
                self.processes.insert(pid, identity);
                frontier.push_back(TrackedProcess { pid, identity });
            }
        }
        Ok(())
    }

    fn prune_stale_processes(&mut self, budget: &mut ProcessRefreshBudget) -> Result<(), String> {
        let tracked = self.tracked_processes();
        for process in tracked {
            budget.account_inspections(1)?;
            if !process_matches(process)? {
                self.processes.remove(&process.pid);
            }
        }
        Ok(())
    }

    fn terminate(&mut self, discover_owner_holders: bool) -> Result<(), String> {
        let mut failure = None;
        let now = Instant::now();
        let cleanup_deadline = now.checked_add(COMMAND_OWNER_SCAN_TIMEOUT).unwrap_or(now);
        let mut owner_scan = OwnerSentinelScan::until(cleanup_deadline);
        let mut stable_post_stop_scans = 0_usize;
        for _ in 0..8 {
            if let Err(error) = self.refresh_until(cleanup_deadline) {
                failure.get_or_insert(error);
            }
            let pre_stop_changed = if discover_owner_holders {
                match self.discover_owner_holders(&mut owner_scan) {
                    Ok(changed) => changed,
                    Err(error) => {
                        failure.get_or_insert(error);
                        false
                    }
                }
            } else {
                false
            };
            for process in self.tracked_processes() {
                if let Err(error) = signal_tracked_process(process, libc::SIGSTOP) {
                    failure.get_or_insert(error);
                }
            }
            if let Err(error) = self.refresh_until(cleanup_deadline) {
                failure.get_or_insert(error);
            }
            let post_stop_changed = if discover_owner_holders {
                match self.discover_owner_holders(&mut owner_scan) {
                    Ok(changed) => changed,
                    Err(error) => {
                        failure.get_or_insert(error);
                        false
                    }
                }
            } else {
                false
            };
            if !pre_stop_changed && !post_stop_changed {
                stable_post_stop_scans = stable_post_stop_scans.saturating_add(1);
            } else {
                stable_post_stop_scans = 0;
            }
            if !discover_owner_holders || stable_post_stop_scans >= 2 {
                break;
            }
        }
        if discover_owner_holders && stable_post_stop_scans < 2 {
            failure.get_or_insert_with(|| {
                "process containment did not converge within its bounded cleanup scan limit"
                    .to_string()
            });
        }
        for process in self.tracked_processes() {
            if let Err(error) = signal_tracked_process(process, libc::SIGSTOP) {
                failure.get_or_insert(error);
            }
        }
        if let Err(error) = self.terminate_owned_process_group() {
            failure.get_or_insert(error);
        }
        for process in self.tracked_processes().into_iter().rev() {
            if let Err(error) = signal_tracked_process(process, libc::SIGKILL) {
                failure.get_or_insert(error);
            }
        }
        if discover_owner_holders
            && let Err(error) =
                self.verify_owner_holders_terminated(&mut owner_scan, cleanup_deadline)
        {
            failure.get_or_insert(error);
        }
        failure.map_or(Ok(()), Err)
    }

    fn terminate_owned_process_group(&self) -> Result<(), String> {
        for process in self.tracked_processes() {
            if !process_matches(process)?
                || process_group_id(process.pid)? != Some(self.process_group)
            {
                continue;
            }
            signal_tracked_process(process, libc::SIGSTOP)?;
            if !process_matches(process)?
                || process_group_id(process.pid)? != Some(self.process_group)
            {
                continue;
            }
            return signal_process_group(self.process_group, libc::SIGKILL);
        }
        Ok(())
    }

    fn discover_owner_holders(&mut self, scan: &mut OwnerSentinelScan) -> Result<bool, String> {
        let mut changed = false;
        for process in processes_holding_owner_sentinel(
            self.owner_sentinel,
            self.root_identity,
            &self.owner_scan_baseline,
            scan,
        )? {
            if self.processes.get(&process.pid) == Some(&process.identity) {
                continue;
            }
            if !self.processes.contains_key(&process.pid)
                && self.processes.len() >= COMMAND_MAX_TRACKED_PROCESSES
            {
                return Err(format!(
                    "contained command exceeded the {COMMAND_MAX_TRACKED_PROCESSES}-process tracking limit"
                ));
            }
            self.processes.insert(process.pid, process.identity);
            changed = true;
        }
        Ok(changed)
    }

    fn verify_owner_holders_terminated(
        &mut self,
        scan: &mut OwnerSentinelScan,
        deadline: Instant,
    ) -> Result<(), String> {
        loop {
            scan.check_deadline()?;
            let holders = processes_holding_owner_sentinel(
                self.owner_sentinel,
                self.root_identity,
                &self.owner_scan_baseline,
                scan,
            )?;
            if holders.is_empty() {
                return Ok(());
            }
            for process in holders {
                if self.processes.get(&process.pid) != Some(&process.identity) {
                    if !self.processes.contains_key(&process.pid)
                        && self.processes.len() >= COMMAND_MAX_TRACKED_PROCESSES
                    {
                        return Err(format!(
                            "contained command exceeded the {COMMAND_MAX_TRACKED_PROCESSES}-process tracking limit"
                        ));
                    }
                    self.processes.insert(process.pid, process.identity);
                }
                signal_tracked_process(process, libc::SIGKILL)?;
            }
            if Instant::now() >= deadline {
                return Err(
                    "ownership-sentinel holders survived the bounded cleanup grace".to_string(),
                );
            }
            thread::park_timeout(COMMAND_MONITOR_INTERVAL);
        }
    }

    fn tracked_processes(&self) -> Vec<TrackedProcess> {
        self.processes
            .iter()
            .map(|(&pid, &identity)| TrackedProcess { pid, identity })
            .collect()
    }
}

#[cfg(unix)]
fn process_matches(process: TrackedProcess) -> Result<bool, String> {
    Ok(process_identity(process.pid)?.is_some_and(|identity| identity == process.identity))
}

#[cfg(unix)]
fn verified_owner_holder_with_scan(
    pid: libc::pid_t,
    sentinel: OwnerSentinelIdentity,
    containment_root: ProcessIdentity,
    owner_scan_baseline: &BTreeMap<libc::pid_t, ProcessIdentity>,
    scan: &mut OwnerSentinelScan,
) -> Result<Option<TrackedProcess>, String> {
    #[cfg(not(target_os = "linux"))]
    let _ = (containment_root, owner_scan_baseline);
    let Some(identity) = process_identity(pid)? else {
        return Ok(None);
    };
    #[cfg(target_os = "linux")]
    let Some(identity) =
        linux_owner_scan_identity(pid, identity, containment_root, owner_scan_baseline)?
    else {
        return Ok(None);
    };
    if !process_holds_owner_sentinel_with_scan(pid, sentinel, scan)? {
        return Ok(None);
    }
    if process_identity(pid)? != Some(identity) {
        return Ok(None);
    }
    Ok(Some(TrackedProcess { pid, identity }))
}

#[cfg(target_os = "linux")]
fn linux_owner_scan_identity(
    pid: libc::pid_t,
    observed: ProcessIdentity,
    containment_root: ProcessIdentity,
    owner_scan_baseline: &BTreeMap<libc::pid_t, ProcessIdentity>,
) -> Result<Option<ProcessIdentity>, String> {
    let mut identity = observed;
    for _ in 0..3 {
        if owner_scan_baseline.get(&pid) == Some(&identity) {
            match process_identity(pid)? {
                None => return Ok(None),
                Some(confirmed) if confirmed == identity => return Ok(None),
                Some(replacement) => identity = replacement,
            }
        }
        // The ownership sentinel follows descriptors inherited by the spawned
        // process tree. An unchanged baseline identity or a process that
        // started strictly before the root cannot be its descendant; equality
        // remains included because Linux start times have clock-tick
        // granularity. Passing the descriptor backward to an existing daemon
        // is outside this containment contract.
        if identity >= containment_root {
            return Ok(Some(identity));
        }
        match process_identity(pid)? {
            None => return Ok(None),
            Some(confirmed) if confirmed == identity => return Ok(None),
            Some(replacement) => identity = replacement,
        }
    }
    Err(format!(
        "process {pid} changed repeatedly while its containment age was verified"
    ))
}

#[cfg(target_os = "linux")]
fn capture_owner_scan_baseline(
    root: libc::pid_t,
    operation_deadline: Instant,
) -> BTreeMap<libc::pid_t, ProcessIdentity> {
    let mut baseline = BTreeMap::new();
    let own_pid = libc::pid_t::try_from(std::process::id()).unwrap_or_default();
    let now = Instant::now();
    let snapshot_deadline = std::cmp::min(
        operation_deadline,
        now.checked_add(COMMAND_OWNER_SCAN_TIMEOUT).unwrap_or(now),
    );
    let Ok(entries) = fs::read_dir("/proc") else {
        return baseline;
    };
    let mut process_count = 0_usize;
    for entry in entries {
        if Instant::now() >= snapshot_deadline || process_count >= COMMAND_MAX_SCANNED_PROCESSES {
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        process_count = process_count.saturating_add(1);
        if pid <= 0 || pid == root || pid == own_pid {
            continue;
        }
        // A partial baseline only reduces an optimization: identities that
        // cannot be sampled remain subject to the normal fail-closed scan.
        if let Ok(Some(identity)) = process_identity(pid) {
            baseline.insert(pid, identity);
        }
    }
    baseline
}

#[cfg(all(unix, not(target_os = "linux")))]
fn capture_owner_scan_baseline(
    _root: libc::pid_t,
    _operation_deadline: Instant,
) -> BTreeMap<libc::pid_t, ProcessIdentity> {
    BTreeMap::new()
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn process_group_id(pid: libc::pid_t) -> Result<Option<libc::pid_t>, String> {
    let process_group = unsafe { libc::getpgid(pid) };
    if process_group >= 0 {
        return Ok(Some(process_group));
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(None)
    } else {
        Err(format!(
            "failed to inspect process group for contained process {pid}: {error}"
        ))
    }
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn signal_process_group(process_group: libc::pid_t, signal: libc::c_int) -> Result<(), String> {
    if process_group <= 0 {
        return Err("contained process group identity was invalid".to_string());
    }
    if unsafe { libc::kill(-process_group, signal) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(format!(
            "failed to signal contained process group {process_group}: {error}"
        ))
    }
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn signal_tracked_process(process: TrackedProcess, signal: libc::c_int) -> Result<(), String> {
    if !process_matches(process)? {
        return Ok(());
    }
    if unsafe { libc::kill(process.pid, signal) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(format!(
            "failed to signal contained process {}: {error}",
            process.pid
        ))
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn process_identity(pid: libc::pid_t) -> Result<Option<ProcessIdentity>, String> {
    let mut information = std::mem::MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let size = std::mem::size_of::<libc::proc_bsdinfo>();
    let size = libc::c_int::try_from(size)
        .map_err(|_| "process identity structure is too large".to_string())?;
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            information.as_mut_ptr().cast(),
            size,
        )
    };
    if read == size {
        let information = unsafe { information.assume_init() };
        return Ok(Some(ProcessIdentity {
            start_primary: information.pbi_start_tvsec,
            start_secondary: information.pbi_start_tvusec,
        }));
    }
    if read <= 0 {
        let error = std::io::Error::last_os_error();
        if matches!(
            error.raw_os_error(),
            Some(libc::ESRCH) | Some(libc::ENOENT) | Some(libc::EPERM) | Some(libc::EACCES)
        ) {
            return Ok(None);
        }
        return Err(format!(
            "failed to inspect contained process {pid}: {error}"
        ));
    }
    Err(format!(
        "received a truncated identity for contained process {pid}"
    ))
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn child_process_ids(
    pid: libc::pid_t,
    budget: &mut ProcessRefreshBudget,
) -> Result<Vec<libc::pid_t>, String> {
    budget.check()?;
    let mut children = vec![0; COMMAND_MAX_TRACKED_PROCESSES];
    let byte_capacity = children
        .len()
        .checked_mul(std::mem::size_of::<libc::pid_t>())
        .and_then(|bytes| libc::c_int::try_from(bytes).ok())
        .ok_or_else(|| "process tracking buffer is too large".to_string())?;
    let count =
        unsafe { libc::proc_listchildpids(pid, children.as_mut_ptr().cast(), byte_capacity) };
    budget.check()?;
    if count < 0 {
        let error = std::io::Error::last_os_error();
        if matches!(error.raw_os_error(), Some(libc::ESRCH) | Some(libc::ENOENT)) {
            return Ok(Vec::new());
        }
        return Err(format!(
            "failed to enumerate children of contained process {pid}: {error}"
        ));
    }
    if usize::try_from(count).unwrap_or(usize::MAX) >= children.len() {
        return Err(format!(
            "contained command exceeded the {COMMAND_MAX_TRACKED_PROCESSES}-process enumeration limit"
        ));
    }
    children.truncate(usize::try_from(count).unwrap_or_default());
    children.retain(|child| *child > 0);
    budget.account_inspections(children.len())?;
    Ok(children)
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct MacProcFileInfo {
    open_flags: u32,
    status: u32,
    offset: libc::off_t,
    file_type: i32,
    guard_flags: u32,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct MacVnodeInfo {
    stat: libc::vinfo_stat,
    kind: i32,
    padding: i32,
    filesystem: libc::fsid_t,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct MacVnodeFdInfo {
    file: MacProcFileInfo,
    vnode: MacVnodeInfo,
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn processes_holding_owner_sentinel(
    sentinel: OwnerSentinelIdentity,
    containment_root: ProcessIdentity,
    owner_scan_baseline: &BTreeMap<libc::pid_t, ProcessIdentity>,
    scan: &mut OwnerSentinelScan,
) -> Result<Vec<TrackedProcess>, String> {
    let mut pids = vec![0; COMMAND_MAX_SCANNED_PROCESSES];
    let byte_capacity = pids
        .len()
        .checked_mul(std::mem::size_of::<libc::pid_t>())
        .and_then(|bytes| libc::c_int::try_from(bytes).ok())
        .ok_or_else(|| "process enumeration buffer is too large".to_string())?;
    let count = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), byte_capacity) };
    if count < 0 {
        return Err(format!(
            "failed to enumerate processes holding the command ownership sentinel: {}",
            std::io::Error::last_os_error()
        ));
    }
    let count = usize::try_from(count).unwrap_or_default();
    if count >= pids.len() {
        return Err(format!(
            "process enumeration exceeded the {COMMAND_MAX_SCANNED_PROCESSES}-process scan limit"
        ));
    }
    pids.truncate(count);
    let mut holders = Vec::new();
    for pid in pids.into_iter().filter(|pid| *pid > 0) {
        scan.check_deadline()?;
        if let Some(process) = verified_owner_holder_with_scan(
            pid,
            sentinel,
            containment_root,
            owner_scan_baseline,
            scan,
        )? {
            holders.push(process);
            if holders.len() >= COMMAND_MAX_TRACKED_PROCESSES {
                return Err(format!(
                    "ownership-sentinel enumeration exceeded the {COMMAND_MAX_TRACKED_PROCESSES}-process limit"
                ));
            }
        }
    }
    Ok(holders)
}

#[cfg(target_os = "macos")]
fn resolve_owner_sentinel_identity(
    _pid: libc::pid_t,
    _descriptor: libc::c_int,
    reported: OwnerSentinelIdentity,
) -> Result<OwnerSentinelIdentity, String> {
    Ok(reported)
}

#[cfg(target_os = "macos")]
fn process_holds_owner_sentinel_descriptor(
    pid: libc::pid_t,
    descriptor: libc::c_int,
    sentinel: OwnerSentinelIdentity,
) -> Result<bool, String> {
    mac_descriptor_holds_owner_sentinel(pid, descriptor, sentinel)
}

#[cfg(all(target_os = "macos", test))]
fn process_holds_owner_sentinel(
    pid: libc::pid_t,
    sentinel: OwnerSentinelIdentity,
) -> Result<bool, String> {
    let mut scan = OwnerSentinelScan::new();
    process_holds_owner_sentinel_with_scan(pid, sentinel, &mut scan)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn process_holds_owner_sentinel_with_scan(
    pid: libc::pid_t,
    sentinel: OwnerSentinelIdentity,
    scan: &mut OwnerSentinelScan,
) -> Result<bool, String> {
    let mut descriptors = vec![
        libc::proc_fdinfo {
            proc_fd: 0,
            proc_fdtype: 0,
        };
        COMMAND_MAX_PROCESS_DESCRIPTORS
    ];
    let descriptor_bytes = descriptors
        .len()
        .checked_mul(std::mem::size_of::<libc::proc_fdinfo>())
        .and_then(|bytes| libc::c_int::try_from(bytes).ok())
        .ok_or_else(|| "file descriptor enumeration buffer is too large".to_string())?;
    let bytes = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDLISTFDS,
            0,
            descriptors.as_mut_ptr().cast(),
            descriptor_bytes,
        )
    };
    if bytes <= 0 {
        let error = std::io::Error::last_os_error();
        if matches!(
            error.raw_os_error(),
            Some(libc::ESRCH)
                | Some(libc::ENOENT)
                | Some(libc::EBADF)
                | Some(libc::EPERM)
                | Some(libc::EACCES)
        ) {
            return Ok(false);
        }
        return Err(format!(
            "failed to enumerate descriptors for contained process {pid}: {error}"
        ));
    }
    let count =
        usize::try_from(bytes).unwrap_or_default() / std::mem::size_of::<libc::proc_fdinfo>();
    if count >= descriptors.len() {
        return Err(format!(
            "contained process {pid} exceeded the {COMMAND_MAX_PROCESS_DESCRIPTORS}-descriptor ownership scan limit"
        ));
    }
    descriptors.truncate(count);
    scan.account_descriptors(count)?;
    for descriptor in descriptors
        .into_iter()
        .filter(|descriptor| descriptor.proc_fdtype == libc::PROX_FDTYPE_VNODE as u32)
    {
        scan.check_deadline()?;
        if mac_descriptor_holds_owner_sentinel(pid, descriptor.proc_fd, sentinel)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn mac_descriptor_holds_owner_sentinel(
    pid: libc::pid_t,
    descriptor: libc::c_int,
    sentinel: OwnerSentinelIdentity,
) -> Result<bool, String> {
    const PROC_PIDFDVNODEINFO: libc::c_int = 1;

    if descriptor < 0 {
        return Ok(false);
    }
    let mut information = std::mem::MaybeUninit::<MacVnodeFdInfo>::uninit();
    let size = libc::c_int::try_from(std::mem::size_of::<MacVnodeFdInfo>())
        .map_err(|_| "vnode information structure is too large".to_string())?;
    let read = unsafe {
        libc::proc_pidfdinfo(
            pid,
            descriptor,
            PROC_PIDFDVNODEINFO,
            information.as_mut_ptr().cast(),
            size,
        )
    };
    if read == size {
        let information = unsafe { information.assume_init() };
        return Ok(u64::from(information.vnode.stat.vst_dev) == sentinel.device
            && information.vnode.stat.vst_ino == sentinel.inode);
    }
    if read <= 0 {
        let error = std::io::Error::last_os_error();
        if matches!(
            error.raw_os_error(),
            Some(libc::ESRCH)
                | Some(libc::ENOENT)
                | Some(libc::EBADF)
                | Some(libc::EPERM)
                | Some(libc::EACCES)
        ) {
            return Ok(false);
        }
        return Err(format!(
            "failed to inspect process {pid} descriptor {descriptor}: {error}"
        ));
    }
    Err(format!(
        "received truncated vnode metadata for process {pid} descriptor {descriptor}"
    ))
}

#[cfg(target_os = "linux")]
fn process_identity(pid: libc::pid_t) -> Result<Option<ProcessIdentity>, String> {
    let Some(bytes) = read_proc_file_bounded(
        &Path::new("/proc").join(pid.to_string()).join("stat"),
        16 * 1024,
    )?
    else {
        return Ok(None);
    };
    parse_linux_process_stat_identity(pid, &bytes)
}

#[cfg(target_os = "linux")]
fn parse_linux_process_stat_identity(
    pid: libc::pid_t,
    bytes: &[u8],
) -> Result<Option<ProcessIdentity>, String> {
    let value = std::str::from_utf8(bytes)
        .map_err(|error| format!("contained process {pid} stat was not UTF-8: {error}"))?;
    let fields = value
        .rsplit_once(") ")
        .ok_or_else(|| format!("contained process {pid} stat was malformed"))?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let state = fields
        .first()
        .ok_or_else(|| format!("contained process {pid} stat omitted its state"))?;
    // A terminal single-thread task has released its descriptors, but procfs
    // can retain the task entry until its parent reaps it and deny `fdinfo`
    // access during that interval. A zombie thread-group leader can still
    // have live sibling threads sharing its file table, so keep that case in
    // the fail-closed descriptor scan.
    let terminal = match *state {
        "X" | "x" => true,
        "Z" => {
            fields
                .get(17)
                .ok_or_else(|| format!("contained process {pid} stat omitted its thread count"))?
                .parse::<u64>()
                .map_err(|error| {
                    format!("contained process {pid} thread count was invalid: {error}")
                })?
                <= 1
        }
        _ => false,
    };
    if terminal {
        return Ok(None);
    }
    let start = fields
        .get(19)
        .ok_or_else(|| format!("contained process {pid} stat omitted its start time"))?
        .parse::<u64>()
        .map_err(|error| format!("contained process {pid} start time was invalid: {error}"))?;
    Ok(Some(ProcessIdentity {
        start_primary: start,
        start_secondary: 0,
    }))
}

#[cfg(target_os = "linux")]
fn child_process_ids(
    pid: libc::pid_t,
    budget: &mut ProcessRefreshBudget,
) -> Result<Vec<libc::pid_t>, String> {
    budget.check()?;
    let tasks_path = Path::new("/proc").join(pid.to_string()).join("task");
    let mut tasks = match fs::read_dir(&tasks_path) {
        Ok(tasks) => tasks,
        Err(error) if proc_entry_vanished(&error) => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "failed to enumerate threads for contained process {pid}: {error}"
            ));
        }
    };
    let mut task_count = 0_usize;
    let mut children = BTreeSet::new();
    loop {
        budget.check()?;
        let task = tasks.next();
        budget.check()?;
        let Some(task) = task else {
            break;
        };
        budget.account_inspections(1)?;
        let task = match task {
            Ok(task) => task,
            Err(error) if proc_entry_vanished(&error) => break,
            Err(error) => {
                return Err(format!(
                    "failed to enumerate threads for contained process {pid}: {error}"
                ));
            }
        };
        let Some(task_id) = task
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        task_count = task_count.saturating_add(1);
        if task_count > COMMAND_MAX_TRACKED_PROCESSES {
            return Err(format!(
                "contained process {pid} exceeded the {COMMAND_MAX_TRACKED_PROCESSES}-thread enumeration limit"
            ));
        }
        let path = tasks_path.join(task_id.to_string()).join("children");
        let Some(bytes) = read_proc_file_bounded(&path, 64 * 1024)? else {
            continue;
        };
        budget.account_metadata_bytes(bytes.len())?;
        let value = std::str::from_utf8(&bytes)
            .map_err(|error| format!("contained process {pid} children were not UTF-8: {error}"))?;
        for child in value.split_whitespace() {
            let child = child.parse::<libc::pid_t>().map_err(|error| {
                format!("contained process {pid} had an invalid child pid: {error}")
            })?;
            budget.account_inspections(1)?;
            children.insert(child);
            if children.len() > COMMAND_MAX_TRACKED_PROCESSES {
                return Err(format!(
                    "contained process {pid} exceeded the {COMMAND_MAX_TRACKED_PROCESSES}-child enumeration limit"
                ));
            }
        }
    }
    Ok(children.into_iter().collect())
}

#[cfg(target_os = "linux")]
fn processes_holding_owner_sentinel(
    sentinel: OwnerSentinelIdentity,
    containment_root: ProcessIdentity,
    owner_scan_baseline: &BTreeMap<libc::pid_t, ProcessIdentity>,
    scan: &mut OwnerSentinelScan,
) -> Result<Vec<TrackedProcess>, String> {
    let own_pid = libc::pid_t::try_from(std::process::id()).unwrap_or_default();
    let entries = fs::read_dir("/proc").map_err(|error| {
        format!("failed to enumerate processes holding the command ownership sentinel: {error}")
    })?;
    let mut holders = Vec::new();
    let mut process_count = 0_usize;
    for entry in entries {
        scan.check_deadline()?;
        let entry = entry.map_err(|error| {
            format!("failed to enumerate a process ownership candidate: {error}")
        })?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        process_count = process_count.saturating_add(1);
        if process_count > COMMAND_MAX_SCANNED_PROCESSES {
            return Err(format!(
                "process enumeration exceeded the {COMMAND_MAX_SCANNED_PROCESSES}-process scan limit"
            ));
        }
        if pid <= 0 || pid == own_pid {
            continue;
        }
        if let Some(process) = verified_owner_holder_with_scan(
            pid,
            sentinel,
            containment_root,
            owner_scan_baseline,
            scan,
        )? {
            holders.push(process);
            if holders.len() >= COMMAND_MAX_TRACKED_PROCESSES {
                return Err(format!(
                    "ownership-sentinel enumeration exceeded the {COMMAND_MAX_TRACKED_PROCESSES}-process limit"
                ));
            }
        }
    }
    Ok(holders)
}

#[cfg(target_os = "linux")]
fn resolve_owner_sentinel_identity(
    pid: libc::pid_t,
    descriptor: libc::c_int,
    reported: OwnerSentinelIdentity,
) -> Result<OwnerSentinelIdentity, String> {
    let actual = linux_descriptor_sentinel_identity(pid, descriptor)?.ok_or_else(|| {
        format!("contained process {pid} did not retain its ownership sentinel descriptor")
    })?;
    if actual.inode != reported.inode {
        return Err(format!(
            "contained process {pid} reported an invalid ownership sentinel identity"
        ));
    }
    Ok(actual)
}

#[cfg(target_os = "linux")]
fn process_holds_owner_sentinel_descriptor(
    pid: libc::pid_t,
    descriptor: libc::c_int,
    sentinel: OwnerSentinelIdentity,
) -> Result<bool, String> {
    Ok(linux_descriptor_sentinel_identity(pid, descriptor)? == Some(sentinel))
}

#[cfg(all(target_os = "linux", test))]
fn process_holds_owner_sentinel(
    pid: libc::pid_t,
    sentinel: OwnerSentinelIdentity,
) -> Result<bool, String> {
    let mut scan = OwnerSentinelScan::new();
    process_holds_owner_sentinel_with_scan(pid, sentinel, &mut scan)
}

#[cfg(target_os = "linux")]
fn process_holds_owner_sentinel_with_scan(
    pid: libc::pid_t,
    sentinel: OwnerSentinelIdentity,
    scan: &mut OwnerSentinelScan,
) -> Result<bool, String> {
    let mut descriptors =
        match fs::read_dir(Path::new("/proc").join(pid.to_string()).join("fdinfo")) {
            Ok(descriptors) => descriptors,
            Err(error) if proc_entry_vanished(&error) => {
                return Ok(false);
            }
            Err(error) => {
                return Err(format!(
                    "failed to enumerate descriptors for process {pid}: {error}"
                ));
            }
        };
    let mut descriptor_count = 0_usize;
    loop {
        scan.check_deadline()?;
        let descriptor = descriptors.next();
        scan.check_deadline()?;
        let Some(descriptor) = descriptor else {
            break;
        };
        let descriptor = match descriptor {
            Ok(descriptor) => descriptor,
            Err(error) if proc_entry_vanished(&error) => return Ok(false),
            Err(error) => {
                return Err(format!(
                    "failed to enumerate descriptors for process {pid}: {error}"
                ));
            }
        };
        let Some(descriptor) = descriptor
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<libc::c_int>().ok())
        else {
            continue;
        };
        descriptor_count = descriptor_count.saturating_add(1);
        if descriptor_count > COMMAND_MAX_PROCESS_DESCRIPTORS {
            return Err(format!(
                "contained process {pid} exceeded the {COMMAND_MAX_PROCESS_DESCRIPTORS}-descriptor ownership scan limit"
            ));
        }
        scan.account_descriptors(1)?;
        if linux_descriptor_sentinel_identity(pid, descriptor)? == Some(sentinel) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(target_os = "linux")]
fn linux_descriptor_sentinel_identity(
    pid: libc::pid_t,
    descriptor: libc::c_int,
) -> Result<Option<OwnerSentinelIdentity>, String> {
    if descriptor < 0 {
        return Ok(None);
    }
    let path = Path::new("/proc")
        .join(pid.to_string())
        .join("fdinfo")
        .join(descriptor.to_string());
    let Some(bytes) = read_proc_file_bounded(&path, 4 * 1024)? else {
        return Ok(None);
    };
    parse_linux_fdinfo_sentinel_identity(&bytes).map(Some)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn parse_linux_fdinfo_sentinel_identity(bytes: &[u8]) -> Result<OwnerSentinelIdentity, String> {
    let value = std::str::from_utf8(bytes)
        .map_err(|error| format!("process descriptor metadata was not UTF-8: {error}"))?;
    let mut mount_id = None;
    let mut inode = None;
    for line in value.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        match name {
            "mnt_id" => {
                mount_id = Some(value.trim().parse::<u64>().map_err(|error| {
                    format!("process descriptor mount id was invalid: {error}")
                })?);
            }
            "ino" => {
                inode =
                    Some(value.trim().parse::<u64>().map_err(|error| {
                        format!("process descriptor inode was invalid: {error}")
                    })?);
            }
            _ => {}
        }
    }
    Ok(OwnerSentinelIdentity {
        device: mount_id
            .ok_or_else(|| "process descriptor metadata omitted its mount id".to_string())?,
        inode: inode.ok_or_else(|| "process descriptor metadata omitted its inode".to_string())?,
    })
}

#[cfg(target_os = "linux")]
fn read_proc_file_bounded(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if proc_entry_vanished(&error) => {
            return Ok(None);
        }
        Err(error) => {
            return Err(format!(
                "failed to open process metadata `{}`: {error}",
                path.display()
            ));
        }
    };
    read_proc_reader_bounded(path, file, limit)
}

#[cfg(target_os = "linux")]
fn read_proc_reader_bounded(
    path: &Path,
    reader: impl Read,
    limit: u64,
) -> Result<Option<Vec<u8>>, String> {
    let mut bytes = Vec::new();
    match reader.take(limit.saturating_add(1)).read_to_end(&mut bytes) {
        Ok(_) => {}
        Err(error) if proc_entry_vanished(&error) => {
            // procfs entries can disappear after open when the process exits,
            // so a successful open does not guarantee that its first read can
            // still observe the sampled process.
            return Ok(None);
        }
        Err(error) => {
            return Err(format!(
                "failed to read process metadata `{}`: {error}",
                path.display()
            ));
        }
    }
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(format!(
            "process metadata `{}` exceeded its {limit}-byte limit",
            path.display()
        ));
    }
    Ok(Some(bytes))
}

#[cfg(target_os = "linux")]
fn proc_entry_vanished(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound || error.raw_os_error() == Some(libc::ESRCH)
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn process_identity(_pid: libc::pid_t) -> Result<Option<ProcessIdentity>, String> {
    Err(unsupported_process_containment_reason())
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn child_process_ids(
    _pid: libc::pid_t,
    budget: &mut ProcessRefreshBudget,
) -> Result<Vec<libc::pid_t>, String> {
    budget.check()?;
    Err(unsupported_process_containment_reason())
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn processes_holding_owner_sentinel(
    _sentinel: OwnerSentinelIdentity,
    _containment_root: ProcessIdentity,
    _owner_scan_baseline: &BTreeMap<libc::pid_t, ProcessIdentity>,
    _scan: &mut OwnerSentinelScan,
) -> Result<Vec<TrackedProcess>, String> {
    Err(unsupported_process_containment_reason())
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn resolve_owner_sentinel_identity(
    _pid: libc::pid_t,
    _descriptor: libc::c_int,
    _reported: OwnerSentinelIdentity,
) -> Result<OwnerSentinelIdentity, String> {
    Err(unsupported_process_containment_reason())
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn process_holds_owner_sentinel_descriptor(
    _pid: libc::pid_t,
    _descriptor: libc::c_int,
    _sentinel: OwnerSentinelIdentity,
) -> Result<bool, String> {
    Err(unsupported_process_containment_reason())
}

#[cfg(all(unix, test, not(any(target_os = "linux", target_os = "macos"))))]
fn process_holds_owner_sentinel(
    _pid: libc::pid_t,
    _sentinel: OwnerSentinelIdentity,
) -> Result<bool, String> {
    Err(unsupported_process_containment_reason())
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn process_holds_owner_sentinel_with_scan(
    _pid: libc::pid_t,
    _sentinel: OwnerSentinelIdentity,
    scan: &mut OwnerSentinelScan,
) -> Result<bool, String> {
    scan.check_deadline()?;
    Err(unsupported_process_containment_reason())
}

#[cfg(unix)]
fn configure_child_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_child_process_group(_command: &mut Command) {}

#[derive(Debug, Clone)]
pub(crate) enum GoSemanticCommand {
    Binary(PathBuf),
    SourceDir(PathBuf),
    Embedded,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedGoSemanticFrontend {
    executable: PathBuf,
    executable_digest: String,
    source_digest: Option<String>,
    toolchain: Arc<PreparedGoToolchain>,
    dependency_snapshot: Arc<GoDependencySnapshot>,
    environment_policy: &'static str,
    operation_deadline: GoOperationDeadline,
    #[cfg(windows)]
    cache_guard: Option<crate::go::semantic::windows::PinnedDirectoryGuard>,
    #[cfg(test)]
    _test_concurrency_permit: Option<Arc<TestGoSemanticConcurrencyPermit>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GoHostTarget {
    os: String,
    arch: String,
}

impl GoHostTarget {
    fn current_process() -> Result<Self, GoSemanticProcessError> {
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            "linux" => "linux",
            "windows" => "windows",
            other => {
                return Err(GoSemanticProcessError::CommandUnavailable(format!(
                    "unsupported host operating system for Go semantic analysis: {other}"
                )));
            }
        };
        let arch = match std::env::consts::ARCH {
            "x86" => "386",
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            other => {
                return Err(GoSemanticProcessError::CommandUnavailable(format!(
                    "unsupported host architecture for Go semantic analysis: {other}"
                )));
            }
        };
        Ok(Self {
            os: os.to_string(),
            arch: arch.to_string(),
        })
    }

    fn parse_label(value: &str) -> Option<Self> {
        let (os, arch) = value.trim().split_once('/')?;
        if os.is_empty() || arch.is_empty() {
            return None;
        }
        Some(Self {
            os: os.to_string(),
            arch: arch.to_string(),
        })
    }

    fn label(&self) -> String {
        format!("{}/{}", self.os, self.arch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GoToolchain {
    executable: PathBuf,
    executable_digest: String,
    canonical_selection: PathBuf,
    version: String,
    host_target: GoHostTarget,
    goroot: PathBuf,
    closure: GoToolchainClosure,
}

#[derive(Debug, Clone)]
struct PreparedGoToolchain {
    executable: PathBuf,
    executable_digest: String,
    canonical_selection: PathBuf,
    version: String,
    host_target: GoHostTarget,
    goroot: PathBuf,
    runtime_search_path: OsString,
    closure: GoToolchainClosure,
    environment: CertifiedGoEnvironment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GoToolchainClosure {
    digest: String,
    content_digest: String,
    metadata_digest: String,
    root_metadata_digest: String,
    entry_count: usize,
    byte_count: u64,
    delegated_tool_count: usize,
}

#[derive(Debug, Clone)]
struct GoDependencySnapshot {
    snapshots_root: PathBuf,
    snapshot_root: PathBuf,
    module_cache_root: PathBuf,
    workspace_path: Option<PathBuf>,
    workspace_digest: String,
    workspace_closure: Option<DependencyClosure>,
    content_digest: String,
    module_content_digest: String,
    metadata_digest: String,
    module_root_metadata_digest: String,
    entry_count: usize,
    byte_count: u64,
    local_dependencies_digest: String,
    local_inputs: Option<Arc<LocalDependencyInputs>>,
    analysis_roots: Vec<PathBuf>,
    _lease: Option<Arc<DependencySnapshotLease>>,
}

#[derive(Clone, PartialEq, Eq)]
struct CertifiedGoEnvironment {
    variables: BTreeMap<String, OsString>,
    digest: String,
}

impl std::fmt::Debug for CertifiedGoEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertifiedGoEnvironment")
            .field("variable_names", &self.variables.keys().collect::<Vec<_>>())
            .field("digest", &self.digest)
            .finish()
    }
}

impl CertifiedGoEnvironment {
    fn capture(cache_root: &Path, offline: bool) -> Result<Self, GoSemanticProcessError> {
        let state_root = ensure_private_subdirectory(cache_root, Path::new("go-state"))?;
        let default_gopath = ensure_private_subdirectory(&state_root, Path::new("gopath"))?;
        let default_build_cache =
            ensure_private_subdirectory(&state_root, Path::new("build-cache"))?;
        let temp = ensure_private_subdirectory(&state_root, Path::new("tmp"))?;

        let mut variables = BTreeMap::new();
        let gopath = default_gopath.as_os_str().to_os_string();
        let default_module_cache = default_gopath.join("pkg/mod");
        if offline {
            // Strict offline mode is deliberately independent of ambient
            // network and module-cache policy. Discard those variables without
            // reading or validating them because none can reach a child.
            variables.insert("GOPROXY".to_string(), OsString::from("off"));
            variables.insert("GOPRIVATE".to_string(), OsString::new());
            variables.insert("GONOPROXY".to_string(), OsString::new());
            variables.insert("GONOSUMDB".to_string(), OsString::new());
            variables.insert("GOSUMDB".to_string(), OsString::from("off"));
            variables.insert("GOINSECURE".to_string(), OsString::new());
        } else {
            let goprivate = environment_value_or("GOPRIVATE", "");
            let configured_goproxy = normalize_goproxy_environment(&environment_value_or(
                "GOPROXY",
                "https://proxy.golang.org",
            ))?;
            let configured_gosumdb = environment_value_or("GOSUMDB", "sum.golang.org");
            validate_gosumdb_environment(&configured_gosumdb)?;
            variables.insert("GOPROXY".to_string(), configured_goproxy);
            variables.insert("GOPRIVATE".to_string(), goprivate.clone());
            variables.insert(
                "GONOPROXY".to_string(),
                std::env::var_os("GONOPROXY").unwrap_or_else(|| goprivate.clone()),
            );
            variables.insert(
                "GONOSUMDB".to_string(),
                std::env::var_os("GONOSUMDB").unwrap_or(goprivate),
            );
            variables.insert("GOSUMDB".to_string(), configured_gosumdb);
            variables.insert(
                "GOINSECURE".to_string(),
                environment_value_or("GOINSECURE", ""),
            );
        }
        variables.insert("GOPATH".to_string(), gopath);
        variables.insert(
            "GOMODCACHE".to_string(),
            default_module_cache.into_os_string(),
        );
        variables.insert("GOCACHE".to_string(), default_build_cache.into_os_string());
        variables.insert("GOVCS".to_string(), OsString::from("off"));
        variables.insert("GOAUTH".to_string(), OsString::from("off"));
        variables.insert("GOTELEMETRY".to_string(), OsString::from("off"));
        variables.insert("TMPDIR".to_string(), temp.as_os_str().to_os_string());
        variables.insert("TMP".to_string(), temp.as_os_str().to_os_string());
        variables.insert("TEMP".to_string(), temp.into_os_string());
        if !offline {
            for name in ["HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"] {
                if let Some(value) = std::env::var_os(name) {
                    validate_network_proxy_environment(name, &value)?;
                    variables.insert(name.to_string(), value);
                }
            }
            for name in ["NO_PROXY", "no_proxy"] {
                let Some(value) = std::env::var_os(name) else {
                    continue;
                };
                variables.insert(name.to_string(), value);
            }
        }

        let digest = certified_environment_digest(&variables);
        Ok(Self { variables, digest })
    }

    #[cfg(test)]
    fn for_test() -> Self {
        let variables = BTreeMap::from([
            ("GOPROXY".to_string(), OsString::from("off")),
            ("GOVCS".to_string(), OsString::from("off")),
        ]);
        let digest = certified_environment_digest(&variables);
        Self { variables, digest }
    }
}

fn unicode_environment_value<'a>(
    name: &str,
    value: &'a std::ffi::OsStr,
) -> Result<&'a str, GoSemanticProcessError> {
    value.to_str().ok_or_else(|| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "{name} must be valid Unicode before it can enter the sealed Go environment."
        ))
    })
}

fn has_ascii_case_insensitive_prefix(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

fn validate_goproxy_environment(value: &std::ffi::OsStr) -> Result<(), GoSemanticProcessError> {
    normalize_goproxy_environment(value).map(|_| ())
}

fn normalize_goproxy_environment(
    value: &std::ffi::OsStr,
) -> Result<OsString, GoSemanticProcessError> {
    let value = unicode_environment_value("GOPROXY", value)?;
    let mut normalized = String::new();
    let mut remaining = value;
    let mut separator_after_previous = None;
    let mut endpoint_count = 0_usize;
    loop {
        let (raw_endpoint, separator, rest) = match remaining.find([',', '|']) {
            Some(index) => (
                &remaining[..index],
                remaining.as_bytes().get(index).copied().map(char::from),
                &remaining[index + 1..],
            ),
            None => (remaining, None, ""),
        };
        remaining = rest;
        let endpoint = raw_endpoint.trim();
        if endpoint.is_empty() {
            if separator.is_none() {
                break;
            }
            continue;
        }
        if has_ascii_case_insensitive_prefix(endpoint, "file:") {
            return Err(GoSemanticProcessError::CommandUnavailable(
                "configured GOPROXY file endpoints are not accepted; local file proxies must use polint's certified dependency inputs."
                    .to_string(),
            ));
        }
        let endpoint = if matches!(endpoint, "direct" | "off")
            || has_ascii_case_insensitive_prefix(endpoint, "https://")
            || has_ascii_case_insensitive_prefix(endpoint, "http://")
        {
            endpoint.to_string()
        } else if endpoint.contains(['.', ':', '/'])
            && !endpoint.contains(":/")
            && !looks_like_absolute_proxy_path(endpoint)
        {
            format!("https://{endpoint}")
        } else {
            return Err(GoSemanticProcessError::CommandUnavailable(
                "GOPROXY contains an unsupported or malformed proxy endpoint.".to_string(),
            ));
        };
        if endpoint_count != 0 {
            normalized.push(separator_after_previous.unwrap_or(','));
        }
        normalized.push_str(&endpoint);
        endpoint_count = endpoint_count.saturating_add(1);
        separator_after_previous = separator;
        // Go treats both built-ins as terminal. Dropping unreachable suffixes
        // keeps the sealed value equivalent while avoiding retained secrets.
        if matches!(endpoint.as_str(), "direct" | "off") || separator.is_none() {
            break;
        }
    }
    if endpoint_count == 0 {
        return Err(GoSemanticProcessError::CommandUnavailable(
            "GOPROXY contains no usable proxy endpoint.".to_string(),
        ));
    }
    Ok(OsString::from(normalized))
}

fn looks_like_absolute_proxy_path(value: &str) -> bool {
    Path::new(value).is_absolute()
        || value.starts_with('/')
        || value.starts_with('\\')
        || (value.as_bytes().get(1) == Some(&b':')
            && value
                .as_bytes()
                .get(2)
                .is_some_and(|byte| matches!(byte, b'/' | b'\\')))
}

fn validate_gosumdb_environment(value: &std::ffi::OsStr) -> Result<(), GoSemanticProcessError> {
    let value = unicode_environment_value("GOSUMDB", value)?;
    let mut fields = value.split_whitespace();
    let Some(name_or_key) = fields.next() else {
        return Err(GoSemanticProcessError::CommandUnavailable(
            "GOSUMDB must not be empty.".to_string(),
        ));
    };
    let alternate = fields.next();
    if fields.next().is_some() || (name_or_key == "off" && alternate.is_some()) {
        return Err(GoSemanticProcessError::CommandUnavailable(
            "GOSUMDB contains an unsupported or malformed alternate URL.".to_string(),
        ));
    }
    let Some(alternate) = alternate else {
        return Ok(());
    };
    if has_ascii_case_insensitive_prefix(alternate, "file:") {
        return Err(GoSemanticProcessError::CommandUnavailable(
            "configured GOSUMDB file endpoints are not accepted.".to_string(),
        ));
    }
    if !has_ascii_case_insensitive_prefix(alternate, "https://")
        && !has_ascii_case_insensitive_prefix(alternate, "http://")
    {
        return Err(GoSemanticProcessError::CommandUnavailable(
            "GOSUMDB contains an unsupported or malformed alternate URL.".to_string(),
        ));
    }
    Ok(())
}

fn validate_network_proxy_environment(
    name: &str,
    value: &std::ffi::OsStr,
) -> Result<(), GoSemanticProcessError> {
    let value = unicode_environment_value(name, value)?;
    let value = value.trim();
    if has_ascii_case_insensitive_prefix(value, "file:") {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "{name} file endpoints are not accepted in the sealed Go environment."
        )));
    }
    if value.contains("://")
        && !has_ascii_case_insensitive_prefix(value, "https://")
        && !has_ascii_case_insensitive_prefix(value, "http://")
    {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "{name} contains an unsupported proxy URL scheme."
        )));
    }
    if looks_like_absolute_proxy_path(value) {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "{name} must not name a local filesystem path."
        )));
    }
    Ok(())
}

fn environment_value_or(name: &str, default: &str) -> OsString {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OsString::from(default))
}

fn certified_environment_digest(variables: &BTreeMap<String, OsString>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"polint-go-certified-environment-v1");
    for (name, value) in variables {
        hash_length_prefixed(&mut hasher, name.as_bytes());
        hash_length_prefixed(&mut hasher, &os_string_bytes(value));
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(unix)]
fn os_string_bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    value.as_bytes().to_vec()
}

#[cfg(windows)]
fn os_string_bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    value.encode_wide().flat_map(u16::to_le_bytes).collect()
}

#[cfg(all(not(unix), not(windows)))]
fn os_string_bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    value.to_string_lossy().as_bytes().to_vec()
}

#[cfg(unix)]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(all(not(unix), not(windows)))]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DependencyClosure {
    content_digest: String,
    metadata_digest: String,
    root_metadata_digest: String,
    entry_count: usize,
    byte_count: u64,
}

fn dependency_closure_metadata_matches(
    current: &DependencyClosure,
    expected: &DependencyClosure,
) -> bool {
    current.metadata_digest == expected.metadata_digest
        && current.root_metadata_digest == expected.root_metadata_digest
        && current.entry_count == expected.entry_count
        && current.byte_count == expected.byte_count
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DependencySnapshotPayloadStamp {
    request_key: String,
    workspace_work_digest: Option<String>,
    workspace_final_digest: Option<String>,
    module_content_digest: String,
    module_entry_count: usize,
    module_byte_count: u64,
}

#[derive(Debug)]
struct VerifiedDependencySnapshotPayload {
    stamp: DependencySnapshotPayloadStamp,
    module_closure: DependencyClosure,
}

#[derive(Debug)]
struct DependencySnapshotLease {
    _file: fs::File,
}

#[derive(Debug)]
struct DependencySnapshotReservation {
    path: Option<PathBuf>,
    file: Option<fs::File>,
}

impl DependencySnapshotLease {
    fn acquire(
        snapshots_root: &Path,
        request_key: &str,
        deadline: GoOperationDeadline,
    ) -> Result<Arc<Self>, GoSemanticProcessError> {
        if !is_dependency_snapshot_key(request_key) {
            return Err(GoSemanticProcessError::CommandFailed(
                "Go dependency snapshot lease key is invalid.".to_string(),
            ));
        }
        let control = ensure_private_subdirectory(snapshots_root, Path::new("control"))?;
        let leases = ensure_private_subdirectory(&control, Path::new("leases"))?;
        let path = dependency_lease_path(&leases, request_key)?;
        let file = open_dependency_lock_file(&path)?;
        lock_dependency_file_shared_until(&file, deadline)?;
        file.set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::now()))
            .map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to refresh Go dependency snapshot lease `{}`: {error}",
                    path.display()
                ))
            })?;
        Ok(Arc::new(Self { _file: file }))
    }
}

impl DependencySnapshotReservation {
    fn create(snapshots_root: &Path, request_key: &str) -> Result<Self, GoSemanticProcessError> {
        let control = ensure_private_subdirectory(snapshots_root, Path::new("control"))?;
        let reservations = ensure_private_subdirectory(&control, Path::new("reservations"))?;
        let reservation = tempfile::Builder::new()
            .prefix(&format!(".reservation-{request_key}-"))
            .make_in(&reservations, create_dependency_reservation_file)
            .map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to reserve Go dependency snapshot capacity: {error}"
                ))
            })?;
        let (file, path) = reservation.keep().map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to persist Go dependency snapshot reservation: {}",
                error.error
            ))
        })?;
        file.try_lock().map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to lock Go dependency snapshot reservation: {error}"
            ))
        })?;
        Ok(Self {
            path: Some(path),
            file: Some(file),
        })
    }

    fn release(mut self) -> Result<(), GoSemanticProcessError> {
        self.remove()
    }

    fn remove(&mut self) -> Result<(), GoSemanticProcessError> {
        let Some(path) = self.path.take() else {
            return Ok(());
        };
        // Windows reservation handles intentionally deny delete sharing so
        // stale-cleanup cannot unlink a live lock. Release our own handle
        // before removing its name.
        drop(self.file.take());
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                self.path = Some(path.clone());
                Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to release Go dependency snapshot reservation `{}`: {error}",
                    path.display()
                )))
            }
        }
    }
}

impl Drop for DependencySnapshotReservation {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

fn is_dependency_snapshot_key(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn dependency_lease_path(
    leases_root: &Path,
    request_key: &str,
) -> Result<PathBuf, GoSemanticProcessError> {
    if !is_dependency_snapshot_key(request_key) {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go dependency snapshot lease key is invalid.".to_string(),
        ));
    }
    // Stable lock slots bound lifecycle metadata while preserving lock identity.
    // A collision only delays eviction; it can never permit an in-use snapshot
    // to be retired.
    Ok(leases_root.join(format!("slot-{}.lock", &request_key[..2])))
}

#[cfg(unix)]
fn open_dependency_lock_file(path: &Path) -> Result<fs::File, GoSemanticProcessError> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to open Go dependency snapshot lock `{}`: {error}",
                path.display()
            ))
        })?;
    let metadata = file.metadata().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go dependency snapshot lock `{}`: {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file()
        || metadata.uid() != effective_user_id()
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go dependency snapshot lock `{}` is not an owner-only regular file.",
            path.display()
        )));
    }
    Ok(file)
}

#[cfg(unix)]
fn create_dependency_reservation_file(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != effective_user_id()
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "dependency reservation is not an owner-only regular file",
        ));
    }
    Ok(file)
}

#[cfg(unix)]
fn open_existing_dependency_lock_file(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != effective_user_id()
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "dependency reservation is not an owner-only regular file",
        ));
    }
    Ok(file)
}

#[cfg(windows)]
fn open_dependency_lock_file(path: &Path) -> Result<fs::File, GoSemanticProcessError> {
    crate::go::semantic::windows::open_private_lock_file(path).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to open Go dependency snapshot lock `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(windows)]
fn create_dependency_reservation_file(path: &Path) -> std::io::Result<fs::File> {
    crate::go::semantic::windows::create_private_lock_file(path)
}

#[cfg(windows)]
fn open_existing_dependency_lock_file(path: &Path) -> std::io::Result<fs::File> {
    crate::go::semantic::windows::open_existing_private_lock_file(path)
}

#[cfg(all(not(unix), not(windows)))]
fn open_dependency_lock_file(_path: &Path) -> Result<fs::File, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "Go dependency snapshot locking is unavailable on this platform.".to_string(),
    ))
}

#[cfg(all(not(unix), not(windows)))]
fn create_dependency_reservation_file(_path: &Path) -> std::io::Result<fs::File> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "private Go dependency reservation creation is unavailable on this platform",
    ))
}

#[cfg(all(not(unix), not(windows)))]
fn open_existing_dependency_lock_file(_path: &Path) -> std::io::Result<fs::File> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Go dependency snapshot locking is unavailable on this platform.",
    ))
}

fn try_lock_would_block(error: &fs::TryLockError) -> bool {
    matches!(error, fs::TryLockError::WouldBlock)
}

fn lock_dependency_file_shared_until(
    file: &fs::File,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    loop {
        deadline.check("Go dependency snapshot lease acquisition")?;
        match file.try_lock_shared() {
            Ok(()) => return Ok(()),
            Err(error) if try_lock_would_block(&error) => {
                thread::sleep(COMMAND_MONITOR_INTERVAL);
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to acquire shared Go dependency snapshot lease: {error}"
                )));
            }
        }
    }
}

fn lock_dependency_file_exclusive_until(
    file: &fs::File,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    loop {
        deadline.check("Go dependency snapshot lifecycle lock acquisition")?;
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(error) if try_lock_would_block(&error) => {
                thread::sleep(COMMAND_MONITOR_INTERVAL);
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to acquire exclusive Go dependency snapshot lifecycle lock: {error}"
                )));
            }
        }
    }
}

#[derive(Debug)]
enum DependencySnapshotAvailability {
    Existing,
    Reserved(DependencySnapshotReservation),
}

#[derive(Debug)]
struct RetainedDependencySnapshot {
    key: String,
    path: PathBuf,
    module_byte_count: u64,
    last_used: std::time::SystemTime,
}

fn dependency_lifecycle_lock_until(
    snapshots_root: &Path,
    deadline: GoOperationDeadline,
) -> Result<fs::File, GoSemanticProcessError> {
    let control = ensure_private_subdirectory(snapshots_root, Path::new("control"))?;
    let file = open_dependency_lock_file(&control.join("lifecycle.lock"))?;
    lock_dependency_file_exclusive_until(&file, deadline)?;
    Ok(file)
}

fn acquire_or_reserve_dependency_snapshot_until(
    snapshots_root: &Path,
    request_key: &str,
    current_staging: &Path,
    deadline: GoOperationDeadline,
) -> Result<(Arc<DependencySnapshotLease>, DependencySnapshotAvailability), GoSemanticProcessError>
{
    let mut cleaned_staging = false;
    let mut retry_interval = GO_DEPENDENCY_CAPACITY_RETRY_INITIAL;
    loop {
        let lifecycle = dependency_lifecycle_lock_until(snapshots_root, deadline)?;
        let destination = snapshots_root.join(request_key);
        match fs::symlink_metadata(&destination) {
            Ok(_) => {
                let lease =
                    DependencySnapshotLease::acquire(snapshots_root, request_key, deadline)?;
                return Ok((lease, DependencySnapshotAvailability::Existing));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go dependency snapshot destination `{}`: {error}",
                    destination.display()
                )));
            }
        }
        let active_reservations =
            cleanup_and_count_dependency_reservations_until(snapshots_root, deadline)?;
        if !cleaned_staging {
            cleanup_and_count_orphaned_dependency_staging_until(
                snapshots_root,
                current_staging,
                deadline,
            )?;
            cleaned_staging = true;
        }
        // A liveness-marked stage exists before its request key is known and
        // before admission. Reservations, rather than those waiting stages,
        // are the single capacity accounting source for active populations.
        let capacity_available = enforce_dependency_snapshot_retention_until(
            snapshots_root,
            Some(request_key),
            active_reservations.saturating_add(1),
            deadline,
        )?;
        if !capacity_available {
            // Finalization also needs the lifecycle lock. Release it before
            // waiting so an active analysis can publish or retire its slot,
            // then recheck both the destination and capacity under a new lock.
            drop(lifecycle);
            let remaining = deadline.remaining("Go dependency snapshot capacity wait")?;
            thread::sleep(retry_interval.min(remaining));
            retry_interval = retry_interval
                .saturating_mul(2)
                .min(GO_DEPENDENCY_CAPACITY_RETRY_MAX);
            continue;
        }
        let reservation = DependencySnapshotReservation::create(snapshots_root, request_key)?;
        let lease = DependencySnapshotLease::acquire(snapshots_root, request_key, deadline)?;
        return Ok((lease, DependencySnapshotAvailability::Reserved(reservation)));
    }
}

fn finalize_dependency_snapshot_until(
    snapshots_root: &Path,
    request_key: &str,
    staging: &mut StagingDirectory,
    reservation: DependencySnapshotReservation,
    has_workspace: bool,
    deadline: GoOperationDeadline,
) -> Result<bool, GoSemanticProcessError> {
    let _lifecycle = dependency_lifecycle_lock_until(snapshots_root, deadline)?;
    staging.release_dependency_liveness()?;
    seal_dependency_snapshot_envelope(staging.path(), has_workspace, deadline)?;
    let destination = snapshots_root.join(request_key);
    let published = match fs::rename(staging.path(), &destination) {
        Ok(()) => {
            #[cfg(unix)]
            seal_dependency_envelope_path(&destination, true)?;
            staging.mark_published();
            true
        }
        Err(_) if fs::symlink_metadata(&destination).is_ok() => false,
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to publish sealed Go dependency snapshot `{}`: {error}",
                destination.display()
            )));
        }
    };
    reservation.release()?;
    Ok(published)
}

fn cleanup_and_count_dependency_reservations_until(
    snapshots_root: &Path,
    deadline: GoOperationDeadline,
) -> Result<usize, GoSemanticProcessError> {
    let control = ensure_private_subdirectory(snapshots_root, Path::new("control"))?;
    let reservations = ensure_private_subdirectory(&control, Path::new("reservations"))?;
    let entries = fs::read_dir(&reservations).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to enumerate Go dependency snapshot reservations: {error}"
        ))
    })?;
    let mut active = 0_usize;
    let mut inspected = 0_usize;
    for entry in entries {
        deadline.check("Go dependency snapshot reservation cleanup")?;
        inspected = inspected.saturating_add(1);
        if inspected > GO_DEPENDENCY_MAX_LIFECYCLE_ENTRIES {
            return Err(GoSemanticProcessError::CommandUnavailable(
                "Go dependency snapshot reservation limit was exceeded; retry after active analyses finish."
                    .to_string(),
            ));
        }
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go dependency snapshot reservation: {error}"
            ))
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(suffix) = name.strip_prefix(".reservation-") else {
            continue;
        };
        let Some(_key) = suffix
            .get(..64)
            .filter(|key| is_dependency_snapshot_key(key))
        else {
            continue;
        };
        let reservation = match open_existing_dependency_lock_file(&entry.path()) {
            Ok(reservation) => reservation,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to open Go dependency snapshot reservation: {error}"
                )));
            }
        };
        match reservation.try_lock() {
            Ok(()) => {
                drop(reservation);
                match fs::remove_file(entry.path()) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "failed to remove stale Go dependency snapshot reservation: {error}"
                        )));
                    }
                }
            }
            Err(error) if try_lock_would_block(&error) => {
                active = active.saturating_add(1);
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go dependency snapshot reservation lease: {error}"
                )));
            }
        }
    }
    Ok(active)
}

fn cleanup_and_count_orphaned_dependency_staging_until(
    snapshots_root: &Path,
    current_staging: &Path,
    deadline: GoOperationDeadline,
) -> Result<usize, GoSemanticProcessError> {
    let staging = ensure_private_subdirectory(snapshots_root, Path::new("staging"))?;
    let entries = fs::read_dir(&staging).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to enumerate Go dependency staging directories: {error}"
        ))
    })?;
    let mut retained = 0_usize;
    let mut inspected = 0_usize;
    for entry in entries {
        deadline.check("orphaned Go dependency staging cleanup")?;
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect a Go dependency staging directory: {error}"
            ))
        })?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !name.starts_with(".dependency-") {
            continue;
        }
        inspected = inspected.saturating_add(1);
        if inspected > GO_DEPENDENCY_MAX_LIFECYCLE_ENTRIES {
            return Err(GoSemanticProcessError::CommandUnavailable(
                "orphaned Go dependency staging limit was exceeded.".to_string(),
            ));
        }
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go dependency staging directory `{}`: {error}",
                    entry.path().display()
                )));
            }
        };
        if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go dependency staging directory `{}` is unsafe.",
                entry.path().display()
            )));
        }
        if entry.path() == current_staging {
            continue;
        }
        let liveness_path = entry.path().join(".liveness");
        let mut liveness_present = false;
        match open_existing_dependency_lock_file(&liveness_path) {
            Ok(liveness) => match liveness.try_lock() {
                Ok(()) => {
                    liveness_present = true;
                    drop(liveness);
                }
                Err(error) if try_lock_would_block(&error) => {
                    retained = retained.saturating_add(1);
                    continue;
                }
                Err(error) => {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "failed to inspect Go dependency staging liveness: {error}"
                    )));
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to open Go dependency staging liveness file: {error}"
                )));
            }
        }
        if !liveness_present
            && metadata.modified().is_ok_and(|modified| {
                modified.elapsed().unwrap_or_default() < GO_DEPENDENCY_STAGE_MARKER_GRACE
            })
        {
            retained = retained.saturating_add(1);
            continue;
        }
        match remove_directory_tree_with_limits(
            &entry.path(),
            GO_DEPENDENCY_CLEANUP_MAX_VISITS,
            GO_DEPENDENCY_CLEANUP_MAX_DEPTH,
            deadline,
        ) {
            Ok(true) => {}
            Ok(false) => retained = retained.saturating_add(1),
            Err(error @ GoSemanticProcessError::Timeout(_)) => return Err(error),
            Err(_) => retained = retained.saturating_add(1),
        }
    }
    Ok(retained)
}

fn collect_retained_dependency_snapshots_until(
    snapshots_root: &Path,
    deadline: GoOperationDeadline,
) -> Result<Vec<RetainedDependencySnapshot>, GoSemanticProcessError> {
    let control = ensure_private_subdirectory(snapshots_root, Path::new("control"))?;
    let leases = ensure_private_subdirectory(&control, Path::new("leases"))?;
    let entries = fs::read_dir(snapshots_root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to enumerate published Go dependency snapshots: {error}"
        ))
    })?;
    let mut retained = Vec::new();
    for entry in entries {
        deadline.check("Go dependency snapshot retention scan")?;
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect a published Go dependency snapshot: {error}"
            ))
        })?;
        let name = entry.file_name();
        let Some(key) = name
            .to_str()
            .filter(|name| is_dependency_snapshot_key(name))
        else {
            continue;
        };
        if retained.len() >= GO_DEPENDENCY_MAX_LIFECYCLE_ENTRIES {
            return Err(GoSemanticProcessError::CommandUnavailable(
                "Go dependency snapshot retention index exceeded its safety limit.".to_string(),
            ));
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect published Go dependency snapshot `{key}`: {error}"
            ))
        })?;
        if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "published Go dependency snapshot `{key}` is not a direct regular directory."
            )));
        }
        let payload =
            read_regular_file_no_follow_until(&entry.path().join("payload.json"), deadline)
                .ok()
                .and_then(|bytes| {
                    serde_json::from_slice::<DependencySnapshotPayloadStamp>(&bytes).ok()
                });
        let module_byte_count = payload
            .filter(|payload| payload.request_key == key)
            .map_or(GO_DEPENDENCY_MAX_BYTES, |payload| payload.module_byte_count);
        let lease_path = dependency_lease_path(&leases, key)?;
        let last_used = fs::symlink_metadata(&lease_path)
            .and_then(|metadata| metadata.modified())
            .or_else(|_| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        retained.push(RetainedDependencySnapshot {
            key: key.to_string(),
            path: entry.path(),
            module_byte_count,
            last_used,
        });
    }
    Ok(retained)
}

fn enforce_dependency_snapshot_retention_until(
    snapshots_root: &Path,
    keep_key: Option<&str>,
    reserved_slots: usize,
    deadline: GoOperationDeadline,
) -> Result<bool, GoSemanticProcessError> {
    let mut retained = collect_retained_dependency_snapshots_until(snapshots_root, deadline)?;
    retained.sort_by(|left, right| {
        left.last_used
            .cmp(&right.last_used)
            .then_with(|| left.key.cmp(&right.key))
    });
    let mut retained_count = retained.len();
    let mut retained_bytes = retained.iter().fold(0_u64, |total, snapshot| {
        total.saturating_add(snapshot.module_byte_count)
    });
    let reserved_bytes = u64::try_from(reserved_slots)
        .unwrap_or(u64::MAX)
        .saturating_mul(GO_DEPENDENCY_MAX_BYTES);
    let now = std::time::SystemTime::now();
    for snapshot in retained {
        deadline.check("Go dependency snapshot retention")?;
        let expired = now.duration_since(snapshot.last_used).unwrap_or_default()
            >= GO_DEPENDENCY_MAX_PUBLISHED_AGE;
        let over_count =
            retained_count.saturating_add(reserved_slots) > GO_DEPENDENCY_MAX_PUBLISHED_SNAPSHOTS;
        let over_bytes = retained_bytes.saturating_add(reserved_bytes)
            > GO_DEPENDENCY_MAX_PUBLISHED_MODULE_BYTES;
        if !expired && !over_count && !over_bytes {
            break;
        }
        if keep_key == Some(snapshot.key.as_str()) {
            continue;
        }
        if retire_dependency_snapshot_until(snapshots_root, &snapshot, deadline)? {
            retained_count = retained_count.saturating_sub(1);
            retained_bytes = retained_bytes.saturating_sub(snapshot.module_byte_count);
        }
    }
    if retained_count.saturating_add(reserved_slots) > GO_DEPENDENCY_MAX_PUBLISHED_SNAPSHOTS
        || retained_bytes.saturating_add(reserved_bytes) > GO_DEPENDENCY_MAX_PUBLISHED_MODULE_BYTES
    {
        return Ok(false);
    }
    Ok(true)
}

fn retire_dependency_snapshot_until(
    snapshots_root: &Path,
    snapshot: &RetainedDependencySnapshot,
    deadline: GoOperationDeadline,
) -> Result<bool, GoSemanticProcessError> {
    let control = ensure_private_subdirectory(snapshots_root, Path::new("control"))?;
    let leases = ensure_private_subdirectory(&control, Path::new("leases"))?;
    let lease_path = dependency_lease_path(&leases, &snapshot.key)?;
    let lease = open_dependency_lock_file(&lease_path)?;
    match lease.try_lock() {
        Ok(()) => {}
        Err(error) if try_lock_would_block(&error) => return Ok(false),
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to acquire Go dependency snapshot retirement lease: {error}"
            )));
        }
    }
    let metadata = fs::symlink_metadata(&snapshot.path).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to re-inspect Go dependency snapshot before retirement: {error}"
        ))
    })?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go dependency snapshot changed before retirement.".to_string(),
        ));
    }
    let staging = ensure_private_subdirectory(snapshots_root, Path::new("staging"))?;
    let quarantine = tempfile::Builder::new()
        .prefix(&format!(".dependency-retired-{}-", snapshot.key))
        .suffix(".abandoned")
        .tempdir_in(&staging)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to allocate retired Go dependency quarantine: {error}"
            ))
        })?
        .keep();
    let retired = quarantine.join("payload");
    move_dependency_snapshot_to_quarantine(
        &snapshot.path,
        &retired,
        &metadata,
        "retire Go dependency snapshot",
    )?;
    let removed = remove_directory_tree_with_limits(
        &quarantine,
        GO_DEPENDENCY_CLEANUP_MAX_VISITS,
        GO_DEPENDENCY_CLEANUP_MAX_DEPTH,
        deadline,
    );
    Ok(matches!(removed, Ok(true)))
}

fn quarantine_corrupt_dependency_snapshot_until(
    snapshots_root: &Path,
    request_key: &str,
    observed_destination_identity: Option<&str>,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    let destination = snapshots_root.join(request_key);
    loop {
        deadline.check("corrupt Go dependency snapshot quarantine")?;
        if try_quarantine_corrupt_dependency_snapshot_until(
            snapshots_root,
            request_key,
            &destination,
            observed_destination_identity,
            deadline,
        )? {
            return Ok(());
        }
        deadline.check("corrupt Go dependency snapshot quarantine")?;
        thread::sleep(COMMAND_MONITOR_INTERVAL);
    }
}

fn try_quarantine_corrupt_dependency_snapshot_until(
    snapshots_root: &Path,
    request_key: &str,
    destination: &Path,
    observed_destination_identity: Option<&str>,
    deadline: GoOperationDeadline,
) -> Result<bool, GoSemanticProcessError> {
    let quarantine_to_remove = {
        let lifecycle = dependency_lifecycle_lock_until(snapshots_root, deadline)?;
        let control = ensure_private_subdirectory(snapshots_root, Path::new("control"))?;
        let leases = ensure_private_subdirectory(&control, Path::new("leases"))?;
        let lease = open_dependency_lock_file(&dependency_lease_path(&leases, request_key)?)?;
        match lease.try_lock() {
            Ok(()) => {
                let current_identity =
                    dependency_snapshot_destination_identity_until(destination, deadline)?;
                if current_identity.as_deref() != observed_destination_identity {
                    return Ok(true);
                }
                if current_identity.is_none() {
                    return Ok(true);
                }

                let staging = ensure_private_subdirectory(snapshots_root, Path::new("staging"))?;
                let quarantine = tempfile::Builder::new()
                    .prefix(&format!(".dependency-corrupt-{request_key}-"))
                    .suffix(".abandoned")
                    .tempdir_in(&staging)
                    .map_err(|error| {
                        GoSemanticProcessError::CommandFailed(format!(
                            "failed to allocate corrupt Go dependency quarantine: {error}"
                        ))
                    })?
                    .keep();
                let quarantined = quarantine.join("payload");
                let metadata = match fs::symlink_metadata(destination) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
                    Err(error) => {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "failed to re-inspect corrupt Go dependency snapshot: {error}"
                        )));
                    }
                };
                if !move_dependency_snapshot_to_quarantine(
                    destination,
                    &quarantined,
                    &metadata,
                    "quarantine corrupt Go dependency snapshot",
                )? {
                    return Ok(true);
                }
                Some(quarantine)
            }
            Err(error) if try_lock_would_block(&error) => {
                drop(lease);
                drop(lifecycle);
                return Ok(false);
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to acquire corrupt Go dependency snapshot quarantine lease: {error}"
                )));
            }
        }
    };
    if let Some(quarantine) = quarantine_to_remove {
        let _ = remove_directory_tree_with_limits(
            &quarantine,
            GO_DEPENDENCY_CLEANUP_MAX_VISITS,
            GO_DEPENDENCY_CLEANUP_MAX_DEPTH,
            deadline,
        );
    }
    Ok(true)
}

fn move_dependency_snapshot_to_quarantine(
    source: &Path,
    destination: &Path,
    metadata: &fs::Metadata,
    action: &str,
) -> Result<bool, GoSemanticProcessError> {
    if metadata.is_dir() && !metadata_is_link_or_reparse(metadata) {
        return move_direct_dependency_snapshot_directory_to_quarantine(
            source,
            destination,
            metadata,
            action,
        );
    }
    match fs::rename(source, destination) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to {action}: {error}"
        ))),
    }
}

#[cfg(unix)]
struct MovableDependencySnapshotRoot {
    directory: fs::File,
    reseal_on_drop: bool,
}

#[cfg(unix)]
impl MovableDependencySnapshotRoot {
    fn open(path: &Path, expected_metadata: &fs::Metadata) -> Result<Self, GoSemanticProcessError> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

        let directory = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(path)
            .map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to anchor sealed Go dependency snapshot for retirement: {error}"
                ))
            })?;
        let current_metadata = directory.metadata().map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect anchored Go dependency snapshot for retirement: {error}"
            ))
        })?;
        if !current_metadata.is_dir()
            || current_metadata.dev() != expected_metadata.dev()
            || current_metadata.ino() != expected_metadata.ino()
        {
            return Err(GoSemanticProcessError::CommandFailed(
                "Go dependency snapshot changed before retirement.".to_string(),
            ));
        }

        // Darwin requires owner write permission on a sealed directory before
        // it can be renamed. Only the envelope is reopened; children remain
        // sealed. The descriptor pins the checked inode for failure resealing.
        directory
            .set_permissions(fs::Permissions::from_mode(0o700))
            .map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to reopen sealed Go dependency snapshot for retirement: {error}"
                ))
            })?;
        Ok(Self {
            directory,
            reseal_on_drop: true,
        })
    }

    fn keep_open(self) {
        let mut this = self;
        this.reseal_on_drop = false;
    }

    fn reseal(&mut self) -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        self.directory
            .set_permissions(fs::Permissions::from_mode(0o500))?;
        self.reseal_on_drop = false;
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for MovableDependencySnapshotRoot {
    fn drop(&mut self) {
        if self.reseal_on_drop {
            use std::os::unix::fs::PermissionsExt;

            let _ = self
                .directory
                .set_permissions(fs::Permissions::from_mode(0o500));
        }
    }
}

#[cfg(unix)]
fn move_direct_dependency_snapshot_directory_to_quarantine(
    source: &Path,
    destination: &Path,
    metadata: &fs::Metadata,
    action: &str,
) -> Result<bool, GoSemanticProcessError> {
    let mut root = MovableDependencySnapshotRoot::open(source, metadata)?;
    match fs::rename(source, destination) {
        Ok(()) => {
            root.keep_open();
            Ok(true)
        }
        Err(error) => {
            let source_is_absent = error.kind() == std::io::ErrorKind::NotFound
                && matches!(
                    fs::symlink_metadata(source),
                    Err(inspect_error)
                        if inspect_error.kind() == std::io::ErrorKind::NotFound
                );
            if let Err(reseal_error) = root.reseal() {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to {action}: {error}; the anchored source also could not be resealed: {reseal_error}"
                )));
            }
            if source_is_absent {
                Ok(false)
            } else {
                Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to {action}: {error}"
                )))
            }
        }
    }
}

#[cfg(windows)]
fn move_direct_dependency_snapshot_directory_to_quarantine(
    source: &Path,
    destination: &Path,
    _metadata: &fs::Metadata,
    action: &str,
) -> Result<bool, GoSemanticProcessError> {
    crate::go::semantic::windows::make_private_path_writable(source, true).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to reopen sealed Go dependency snapshot for retirement: {error}"
        ))
    })?;
    match fs::rename(source, destination) {
        Ok(()) => Ok(true),
        Err(error) => {
            let source_is_absent = error.kind() == std::io::ErrorKind::NotFound
                && matches!(
                    fs::symlink_metadata(source),
                    Err(inspect_error)
                        if inspect_error.kind() == std::io::ErrorKind::NotFound
                );
            if !source_is_absent
                && let Err(reseal_error) = seal_dependency_envelope_path(source, true)
            {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to {action}: {error}; the source also could not be resealed: {reseal_error}"
                )));
            }
            if source_is_absent {
                Ok(false)
            } else {
                Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to {action}: {error}"
                )))
            }
        }
    }
}

#[cfg(all(not(unix), not(windows)))]
fn move_direct_dependency_snapshot_directory_to_quarantine(
    _source: &Path,
    _destination: &Path,
    _metadata: &fs::Metadata,
    _action: &str,
) -> Result<bool, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "sealed Go dependency snapshot retirement is unavailable on this platform.".to_string(),
    ))
}

fn dependency_snapshot_destination_identity_until(
    destination: &Path,
    deadline: GoOperationDeadline,
) -> Result<Option<String>, GoSemanticProcessError> {
    validate_local_path_size_until(
        destination,
        "Go dependency snapshot destination identity capture",
        deadline,
    )?;
    #[cfg(windows)]
    {
        let destination = destination.to_path_buf();
        run_windows_file_io_certification(
            deadline,
            "Go dependency snapshot destination identity capture",
            move || {
                deadline.check("Go dependency snapshot destination identity capture")?;
                let parent = destination.parent().ok_or_else(|| {
                    GoSemanticProcessError::CommandFailed(
                        "Go dependency snapshot destination has no parent directory.".to_string(),
                    )
                })?;
                let certified_scope =
                    crate::go::semantic::windows::certified_local_directory_until(
                        parent,
                        deadline.end,
                    )
                    .map_err(|error| {
                        windows_file_io_error(
                            error,
                            format!(
                                "failed to certify Go dependency snapshots root `{}`",
                                parent.display()
                            ),
                        )
                    })?;
                let identity = match certified_scope
                    .direct_child_identity_allow_reparse(&destination)
                {
                    Ok(identity) => identity,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => {
                        return Err(windows_file_io_error(
                            error,
                            format!(
                                "failed to capture poisoned Go dependency destination `{}`",
                                destination.display()
                            ),
                        ));
                    }
                };
                let mut hasher = Sha256::new();
                hasher.update(b"polint-go-dependency-destination-identity-v1");
                hash_windows_file_identity(&mut hasher, identity, 0);
                Ok(Some(format!("{:x}", hasher.finalize())))
            },
        )
    }
    #[cfg(not(windows))]
    dependency_snapshot_destination_identity_inner(destination, deadline)
}

#[cfg(not(windows))]
fn dependency_snapshot_destination_identity_inner(
    destination: &Path,
    deadline: GoOperationDeadline,
) -> Result<Option<String>, GoSemanticProcessError> {
    deadline.check("Go dependency snapshot destination identity capture")?;
    let metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go dependency snapshot destination `{}`: {error}",
                destination.display()
            )));
        }
    };
    let mut hasher = Sha256::new();
    hasher.update(b"polint-go-dependency-destination-identity-v1");
    hasher.update([if metadata_is_link_or_reparse(&metadata) {
        0
    } else if metadata.is_dir() {
        1
    } else if metadata.is_file() {
        2
    } else {
        3
    }]);
    hash_toolchain_metadata(&mut hasher, destination, &metadata)?;
    Ok(Some(format!("{:x}", hasher.finalize())))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalDependencyInputs {
    root: PathBuf,
    config: GoAnalysisConfig,
    repository_cache_relatives: Vec<PathBuf>,
    digest: String,
    path_content_digest: String,
    verification_digest: String,
    selected_directories: Vec<PathBuf>,
    files: Vec<PathBuf>,
    module_count: usize,
    package_count: usize,
    entry_count: usize,
    file_count: usize,
    byte_count: u64,
}

#[derive(Debug, PartialEq, Eq)]
struct LocalDependencyPathSeal {
    content_digest: String,
    metadata_digest: String,
    entry_count: usize,
    byte_count: u64,
}

fn configure_dependency_execution_environment(
    command: &mut Command,
    snapshot: &GoDependencySnapshot,
) -> Result<(), GoSemanticProcessError> {
    let module_cache_root = go_command_path(&snapshot.module_cache_root)?;
    let workspace_path = snapshot
        .workspace_path
        .as_deref()
        .map(go_command_path)
        .transpose()?
        .unwrap_or_else(|| PathBuf::from("off"));
    command
        .env("GOMODCACHE", module_cache_root)
        .env("GOPROXY", "off")
        .env("GOSUMDB", "off")
        .env("GOVCS", "off")
        .env("GOAUTH", "off")
        .env("GOPRIVATE", "")
        .env("GONOPROXY", "")
        .env("GONOSUMDB", "*")
        .env("GOFLAGS", "-mod=readonly");
    command.env("POLINT_GO_WORKSPACE", workspace_path);
    Ok(())
}

fn prepare_dependency_snapshot(
    cache_root: &Path,
    toolchain: &PreparedGoToolchain,
    analysis: Option<(&Path, &GoAnalysisConfig)>,
    repository_cache_roots: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<GoDependencySnapshot, GoSemanticProcessError> {
    deadline.check("Go dependency snapshot preparation")?;
    require_local_existing_path_until(cache_root, deadline, "Go dependency cache certification")?;
    let snapshots_root =
        ensure_private_subdirectory(cache_root, Path::new("dependency-snapshots"))?;
    require_local_existing_path_until(
        &snapshots_root,
        deadline,
        "Go dependency snapshots-root certification",
    )?;
    let staging_root = ensure_private_subdirectory(&snapshots_root, Path::new("staging"))?;
    let mut staging = {
        // Dependency orphan cleanup uses this same lifecycle lock. Holding it
        // across publication of the directory and its locked liveness marker
        // prevents another process from observing a markerless live stage.
        let _lifecycle = dependency_lifecycle_lock_until(&snapshots_root, deadline)?;
        StagingDirectory::create_dependency_until(&staging_root, ".dependency-", deadline)?
    };
    let module_cache = staging.path().join("modules");
    create_private_directory(&module_cache)?;

    let (private_workspace, request_key, initial_population_inputs) = if let Some((root, config)) =
        analysis
    {
        require_local_existing_path_until(
            root,
            deadline,
            "Go dependency analysis-root certification",
        )?;
        let workspace = private_go_workspace(staging.path(), toolchain, root, config, deadline)?;
        workspace.verify_replacement_manifest_inputs(root, deadline)?;
        let manifest_digest =
            dependency_manifest_digest(root, config, workspace.analysis_roots(), deadline)?;
        let source_digest = dependency_population_source_digest(
            root,
            workspace.analysis_roots(),
            repository_cache_roots,
            deadline,
        )?;
        let request_key = dependency_snapshot_request_key(
            toolchain,
            root,
            config,
            &workspace,
            &manifest_digest,
            &source_digest,
            deadline,
        )?;
        (
            Some(workspace),
            request_key,
            Some(DependencyPopulationInputs {
                manifest_digest,
                source_digest,
            }),
        )
    } else {
        (None, empty_dependency_snapshot_request_key(toolchain), None)
    };
    let analysis_roots = private_workspace
        .as_ref()
        .map_or_else(Vec::new, |workspace| workspace.analysis_roots().to_vec());
    let expected_workspace_work_digest = private_workspace
        .as_ref()
        .and_then(PrivateGoWorkspace::path)
        .map(|path| private_workspace_work_digest(path, deadline))
        .transpose()?;
    let destination = snapshots_root.join(&request_key);
    let (initial_lease, initial_availability) = acquire_or_reserve_dependency_snapshot_until(
        &snapshots_root,
        &request_key,
        staging.path(),
        deadline,
    )?;
    let mut lease = Some(initial_lease);
    let mut availability = initial_availability;
    let mut repaired_corrupt_snapshot = false;
    let mut populated_local_inputs = None;

    let verified = loop {
        match availability {
            DependencySnapshotAvailability::Existing => {
                let observed_destination_identity =
                    dependency_snapshot_destination_identity_until(&destination, deadline)?;
                match verify_dependency_snapshot_payload(
                    &destination,
                    &request_key,
                    expected_workspace_work_digest.as_deref(),
                    deadline,
                ) {
                    Ok(verified) => {
                        staging.discard_dependency_until(&snapshots_root, deadline)?;
                        break verified;
                    }
                    Err(error)
                        if !repaired_corrupt_snapshot
                            && !matches!(error, GoSemanticProcessError::Timeout(_)) =>
                    {
                        drop(lease.take());
                        quarantine_corrupt_dependency_snapshot_until(
                            &snapshots_root,
                            &request_key,
                            observed_destination_identity.as_deref(),
                            deadline,
                        )?;
                        let (next_lease, next_availability) =
                            acquire_or_reserve_dependency_snapshot_until(
                                &snapshots_root,
                                &request_key,
                                staging.path(),
                                deadline,
                            )?;
                        lease = Some(next_lease);
                        availability = next_availability;
                        repaired_corrupt_snapshot = true;
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
            DependencySnapshotAvailability::Reserved(reservation) => {
                let prepared_workspace = match (
                    analysis,
                    private_workspace.as_ref(),
                    initial_population_inputs.as_ref(),
                ) {
                    (Some((root, config)), Some(workspace), Some(population_inputs)) => {
                        Some(populate_dependency_snapshot(
                            &module_cache,
                            toolchain,
                            root,
                            config,
                            workspace,
                            repository_cache_roots,
                            population_inputs,
                            deadline,
                        )?)
                    }
                    (None, None, None) => None,
                    _ => {
                        return Err(GoSemanticProcessError::CommandFailed(
                            "Go dependency request workspace state is inconsistent.".to_string(),
                        ));
                    }
                };
                seal_dependency_tree(&module_cache, deadline)?;
                let module_closure = capture_dependency_closure(&module_cache, deadline)?;
                let payload = DependencySnapshotPayloadStamp {
                    request_key: request_key.clone(),
                    workspace_work_digest: expected_workspace_work_digest.clone(),
                    workspace_final_digest: prepared_workspace
                        .as_ref()
                        .map(|workspace| workspace.digest.clone()),
                    module_content_digest: module_closure.content_digest.clone(),
                    module_entry_count: module_closure.entry_count,
                    module_byte_count: module_closure.byte_count,
                };
                let payload_bytes = serde_json::to_vec(&payload).map_err(|error| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to encode sealed Go dependency snapshot payload: {error}"
                    ))
                })?;
                write_new_private_file(
                    &staging.path().join("request"),
                    request_key.as_bytes(),
                    false,
                )?;
                write_new_private_file(
                    &staging.path().join("payload.json"),
                    &payload_bytes,
                    false,
                )?;
                let published_ours = finalize_dependency_snapshot_until(
                    &snapshots_root,
                    &request_key,
                    &mut staging,
                    reservation,
                    prepared_workspace.is_some(),
                    deadline,
                )?;
                if published_ours && let Some(workspace) = prepared_workspace {
                    let published_workspace = destination.join(&workspace.relative_path);
                    if private_workspace_digest(&published_workspace, deadline)? != workspace.digest
                    {
                        return Err(GoSemanticProcessError::CommandFailed(
                            "published private Go workspace failed content verification."
                                .to_string(),
                        ));
                    }
                    populated_local_inputs = Some(workspace.local_inputs);
                }
                if published_ours {
                    break VerifiedDependencySnapshotPayload {
                        stamp: payload,
                        module_closure,
                    };
                } else {
                    staging.discard_dependency_until(&snapshots_root, deadline)?;
                    break verify_dependency_snapshot_payload(
                        &destination,
                        &request_key,
                        expected_workspace_work_digest.as_deref(),
                        deadline,
                    )?;
                }
            }
        }
    };
    let request_stamp = read_regular_file_no_follow_until(&destination.join("request"), deadline)?;
    if request_stamp != request_key.as_bytes() {
        return Err(GoSemanticProcessError::CommandFailed(
            "sealed Go dependency snapshot request binding failed verification.".to_string(),
        ));
    }

    let module_cache_root = destination.join("modules");
    let workspace_path = analysis.map(|_| destination.join("workspace/go.work"));
    let workspace_digest = workspace_path.as_deref().map_or_else(
        || Ok(security_digest_bytes(b"polint-go-private-workspace-off-v1")),
        |path| private_workspace_digest(path, deadline),
    )?;
    let workspace_closure = workspace_path
        .as_deref()
        .map(|path| {
            let directory = path.parent().ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "private Go workspace has no parent directory.".to_string(),
                )
            })?;
            capture_dependency_closure(directory, deadline)
        })
        .transpose()?;
    let local_inputs = match (analysis, populated_local_inputs, private_workspace.as_ref()) {
        (Some(_), Some(inputs), Some(_)) => Some(inputs),
        (Some((root, config)), None, Some(workspace)) => {
            workspace.verify_replacement_manifest_inputs(root, deadline)?;
            let inputs = capture_local_dependency_inputs(
                &module_cache_root,
                workspace_path.as_deref(),
                toolchain,
                root,
                config,
                repository_cache_roots,
                &analysis_roots,
                GoPackageListingMode::Verify,
                deadline,
            )?;
            workspace.verify_replacement_manifest_inputs(root, deadline)?;
            Some(inputs)
        }
        (None, None, None) => None,
        _ => {
            return Err(GoSemanticProcessError::CommandFailed(
                "Go dependency request local-input state is inconsistent.".to_string(),
            ));
        }
    }
    .map(Arc::new);
    let local_dependencies_digest = local_inputs.as_ref().map_or_else(
        || security_digest_bytes(b"polint-go-local-inputs-empty-v1"),
        |inputs| inputs.digest.clone(),
    );
    Ok(GoDependencySnapshot {
        snapshots_root,
        snapshot_root: destination,
        module_cache_root,
        workspace_path,
        workspace_digest,
        workspace_closure,
        content_digest: dependency_snapshot_content_digest(&verified.stamp),
        module_content_digest: verified.stamp.module_content_digest,
        metadata_digest: verified.module_closure.metadata_digest,
        module_root_metadata_digest: verified.module_closure.root_metadata_digest,
        entry_count: verified.module_closure.entry_count,
        byte_count: verified.module_closure.byte_count,
        local_dependencies_digest,
        local_inputs,
        analysis_roots,
        _lease: lease,
    })
}

fn verify_dependency_snapshot(
    snapshot: &GoDependencySnapshot,
    _toolchain: &PreparedGoToolchain,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    let current = capture_dependency_closure(&snapshot.module_cache_root, deadline)?;
    if current.content_digest != snapshot.module_content_digest
        || current.metadata_digest != snapshot.metadata_digest
        || current.entry_count != snapshot.entry_count
        || current.byte_count != snapshot.byte_count
    {
        return Err(GoSemanticProcessError::CommandFailed(
            "sealed Go dependency snapshot changed after preparation.".to_string(),
        ));
    }
    match (
        snapshot.workspace_path.as_deref(),
        snapshot.workspace_closure.as_ref(),
    ) {
        (Some(path), Some(expected_closure)) => {
            if private_workspace_digest(path, deadline)? != snapshot.workspace_digest {
                return Err(GoSemanticProcessError::CommandFailed(
                    "private Go workspace changed after preparation.".to_string(),
                ));
            }
            let workspace_root = path.parent().ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "private Go workspace has no parent directory.".to_string(),
                )
            })?;
            if workspace_root != snapshot.snapshot_root.join("workspace") {
                return Err(GoSemanticProcessError::CommandFailed(
                    "private Go workspace escaped its prepared snapshot.".to_string(),
                ));
            }
            let current_closure = capture_dependency_closure(workspace_root, deadline)?;
            if !dependency_closure_metadata_matches(&current_closure, expected_closure) {
                return Err(GoSemanticProcessError::CommandFailed(
                    "private Go workspace closure changed after preparation.".to_string(),
                ));
            }
        }
        (None, None)
            if snapshot.workspace_digest
                == security_digest_bytes(b"polint-go-private-workspace-off-v1") => {}
        _ => {
            return Err(GoSemanticProcessError::CommandFailed(
                "private Go workspace identity is inconsistent.".to_string(),
            ));
        }
    }
    if let Some(expected) = &snapshot.local_inputs {
        verify_local_dependency_inputs(expected, deadline)?;
    }
    Ok(())
}

fn verify_dependency_snapshot_binding(
    snapshot: &Arc<GoDependencySnapshot>,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    #[cfg(windows)]
    {
        if crate::go::semantic::windows::cancellable_file_io_pass_is_active() {
            return verify_dependency_snapshot_binding_on_active_windows_worker(snapshot, deadline);
        }
        const LABEL: &str = "Go dependency snapshot binding verification";
        deadline.check(LABEL)?;
        let snapshot = Arc::clone(snapshot);
        deadline.check(LABEL)?;
        run_windows_file_io_certification(deadline, LABEL, move || {
            verify_dependency_snapshot_binding_on_active_windows_worker(&snapshot, deadline)
        })
    }
    #[cfg(not(windows))]
    verify_dependency_snapshot_binding_inner(snapshot.as_ref(), deadline)
}

#[cfg(windows)]
fn verify_dependency_snapshot_binding_on_active_windows_worker(
    snapshot: &GoDependencySnapshot,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    let certified_scope = crate::go::semantic::windows::certified_local_tree_until(
        &snapshot.snapshot_root,
        &[],
        deadline.end,
    )
    .map_err(|error| {
        windows_file_io_error(
            error,
            format!(
                "failed to recertify Go dependency snapshot `{}`",
                snapshot.snapshot_root.display()
            ),
        )
    })?;
    verify_dependency_snapshot_binding_inner(snapshot, deadline, &certified_scope)
}

fn verify_dependency_snapshot_binding_inner(
    snapshot: &GoDependencySnapshot,
    deadline: GoOperationDeadline,
    #[cfg(windows)] certified_scope: &crate::go::semantic::windows::CertifiedLocalTree,
) -> Result<(), GoSemanticProcessError> {
    deadline.check("Go dependency snapshot binding verification")?;
    let snapshots_root = fs::canonicalize(&snapshot.snapshots_root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize Go dependency snapshots root: {error}"
        ))
    })?;
    let lexical_metadata = fs::symlink_metadata(&snapshot.snapshot_root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect prepared Go dependency snapshot: {error}"
        ))
    })?;
    if metadata_is_link_or_reparse(&lexical_metadata) || !lexical_metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(
            "prepared Go dependency snapshot is not a direct regular directory.".to_string(),
        ));
    }
    let snapshot_root = fs::canonicalize(&snapshot.snapshot_root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize prepared Go dependency snapshot: {error}"
        ))
    })?;
    if snapshot_root.parent() != Some(snapshots_root.as_path()) {
        return Err(GoSemanticProcessError::CommandFailed(
            "prepared Go dependency snapshot escaped its cache root.".to_string(),
        ));
    }
    let canonical_metadata = fs::symlink_metadata(&snapshot_root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to re-inspect prepared Go dependency snapshot: {error}"
        ))
    })?;
    if !dependency_metadata_matches(&lexical_metadata, &canonical_metadata) {
        return Err(GoSemanticProcessError::CommandFailed(
            "prepared Go dependency snapshot changed during binding verification.".to_string(),
        ));
    }
    #[cfg(windows)]
    {
        let scoped_root = certified_scope.root();
        let scoped_metadata = fs::symlink_metadata(scoped_root).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect the scoped Go dependency snapshot root: {error}"
            ))
        })?;
        if !dependency_metadata_matches(&canonical_metadata, &scoped_metadata) {
            return Err(GoSemanticProcessError::CommandFailed(
                "prepared Go dependency snapshot changed during scoped binding verification."
                    .to_string(),
            ));
        }
        certified_dependency_entry_state(certified_scope, scoped_root, &scoped_metadata)?;
    }
    #[cfg(not(windows))]
    verify_dependency_entry_permissions(&snapshot_root, &canonical_metadata)?;

    // The Go child opens the whole module cache, not just its root. Re-certify
    // every nested entry's stable identity and sealed access before and after
    // execution. This remains O(entries): file bytes were authenticated during
    // preparation and are not reread here.
    if snapshot.module_cache_root != snapshot.snapshot_root.join("modules") {
        return Err(GoSemanticProcessError::CommandFailed(
            "prepared Go module cache escaped its dependency snapshot.".to_string(),
        ));
    }
    #[cfg(windows)]
    let current_module_closure = capture_dependency_closure_inner(
        &certified_scope.root().join("modules"),
        false,
        deadline,
        certified_scope,
    )?;
    #[cfg(not(windows))]
    let current_module_closure =
        capture_dependency_closure_inner(&snapshot.module_cache_root, false, deadline)?;
    if current_module_closure.metadata_digest != snapshot.metadata_digest
        || current_module_closure.root_metadata_digest != snapshot.module_root_metadata_digest
        || current_module_closure.entry_count != snapshot.entry_count
        || current_module_closure.byte_count != snapshot.byte_count
    {
        return Err(GoSemanticProcessError::CommandFailed(
            "prepared Go module snapshot closure changed after preparation.".to_string(),
        ));
    }

    let request_key = snapshot
        .snapshot_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| is_dependency_snapshot_key(name))
        .ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "prepared Go dependency snapshot request key is invalid.".to_string(),
            )
        })?;
    let request =
        read_regular_file_no_follow_until(&snapshot.snapshot_root.join("request"), deadline)?;
    if request != request_key.as_bytes() {
        return Err(GoSemanticProcessError::CommandFailed(
            "prepared Go dependency snapshot request binding changed.".to_string(),
        ));
    }
    let payload =
        read_regular_file_no_follow_until(&snapshot.snapshot_root.join("payload.json"), deadline)?;
    let payload: DependencySnapshotPayloadStamp =
        serde_json::from_slice(&payload).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to decode prepared Go dependency snapshot payload: {error}"
            ))
        })?;
    if payload.request_key != request_key
        || payload.module_content_digest != snapshot.module_content_digest
        || payload.module_entry_count != snapshot.entry_count
        || payload.module_byte_count != snapshot.byte_count
        || dependency_snapshot_content_digest(&payload) != snapshot.content_digest
    {
        return Err(GoSemanticProcessError::CommandFailed(
            "prepared Go dependency snapshot payload changed after preparation.".to_string(),
        ));
    }
    match (
        snapshot.workspace_path.as_deref(),
        snapshot.workspace_closure.as_ref(),
    ) {
        (Some(path), Some(expected_closure)) => {
            if private_workspace_digest(path, deadline)? != snapshot.workspace_digest {
                return Err(GoSemanticProcessError::CommandFailed(
                    "private Go workspace changed after preparation.".to_string(),
                ));
            }
            let workspace_root = path.parent().ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "private Go workspace has no parent directory.".to_string(),
                )
            })?;
            if workspace_root != snapshot.snapshot_root.join("workspace") {
                return Err(GoSemanticProcessError::CommandFailed(
                    "private Go workspace escaped its prepared snapshot.".to_string(),
                ));
            }
            #[cfg(windows)]
            let current_closure = capture_dependency_closure_inner(
                &certified_scope.root().join("workspace"),
                false,
                deadline,
                certified_scope,
            )?;
            #[cfg(not(windows))]
            let current_closure =
                capture_dependency_closure_inner(workspace_root, false, deadline)?;
            if !dependency_closure_metadata_matches(&current_closure, expected_closure) {
                return Err(GoSemanticProcessError::CommandFailed(
                    "private Go workspace closure changed after preparation.".to_string(),
                ));
            }
        }
        (None, None)
            if snapshot.workspace_digest
                == security_digest_bytes(b"polint-go-private-workspace-off-v1") => {}
        _ => {
            return Err(GoSemanticProcessError::CommandFailed(
                "private Go workspace identity is inconsistent.".to_string(),
            ));
        }
    }
    if let Some(expected) = &snapshot.local_inputs {
        verify_local_dependency_inputs(expected, deadline)?;
    }
    Ok(())
}

fn verify_local_dependency_inputs(
    expected: &Arc<LocalDependencyInputs>,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    #[cfg(windows)]
    let current = {
        const LABEL: &str = "local Go dependency verification";
        if crate::go::semantic::windows::cancellable_file_io_pass_is_active() {
            capture_local_dependency_path_seal(
                &expected.root,
                &expected.repository_cache_relatives,
                &expected.selected_directories,
                &expected.files,
                Some(&expected.path_content_digest),
                deadline,
            )?
        } else {
            deadline.check(LABEL)?;
            let expected = Arc::clone(expected);
            deadline.check(LABEL)?;
            run_windows_file_io_certification(deadline, LABEL, move || {
                capture_local_dependency_path_seal(
                    &expected.root,
                    &expected.repository_cache_relatives,
                    &expected.selected_directories,
                    &expected.files,
                    Some(&expected.path_content_digest),
                    deadline,
                )
            })?
        }
    };
    #[cfg(not(windows))]
    let current = capture_local_dependency_path_seal(
        &expected.root,
        &expected.repository_cache_relatives,
        &expected.selected_directories,
        &expected.files,
        Some(&expected.path_content_digest),
        deadline,
    )?;
    if current.metadata_digest != expected.verification_digest
        || current.entry_count != expected.entry_count
        || current.byte_count != expected.byte_count
    {
        return Err(GoSemanticProcessError::CommandFailed(
            "local Go dependency inputs changed after preparation.".to_string(),
        ));
    }
    Ok(())
}

fn capture_published_dependency_closure(
    snapshots_root: &Path,
    snapshot_root: &Path,
    deadline: GoOperationDeadline,
) -> Result<DependencyClosure, GoSemanticProcessError> {
    deadline.check("published Go dependency snapshot verification")?;
    require_local_existing_path_until(
        snapshots_root,
        deadline,
        "published Go dependency snapshot verification",
    )?;
    require_local_existing_path_until(
        snapshot_root,
        deadline,
        "published Go dependency snapshot verification",
    )?;
    let canonical_snapshots_root = fs::canonicalize(snapshots_root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize Go dependency snapshots root `{}`: {error}",
            snapshots_root.display()
        ))
    })?;
    let lexical_metadata = fs::symlink_metadata(snapshot_root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect published Go dependency snapshot `{}`: {error}",
            snapshot_root.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&lexical_metadata) || !lexical_metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(
            "published Go dependency snapshot must be a direct regular directory.".to_string(),
        ));
    }
    let canonical_snapshot_root = fs::canonicalize(snapshot_root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize published Go dependency snapshot `{}`: {error}",
            snapshot_root.display()
        ))
    })?;
    if canonical_snapshot_root.parent() != Some(canonical_snapshots_root.as_path()) {
        return Err(GoSemanticProcessError::CommandFailed(
            "published Go dependency snapshot escaped its snapshots root.".to_string(),
        ));
    }
    let canonical_metadata = fs::symlink_metadata(&canonical_snapshot_root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to re-inspect published Go dependency snapshot `{}`: {error}",
            snapshot_root.display()
        ))
    })?;
    if !dependency_metadata_matches(&lexical_metadata, &canonical_metadata) {
        return Err(GoSemanticProcessError::CommandFailed(
            "published Go dependency snapshot changed during certification.".to_string(),
        ));
    }
    capture_dependency_closure(&canonical_snapshot_root, deadline)
}

fn capture_dependency_closure(
    root: &Path,
    deadline: GoOperationDeadline,
) -> Result<DependencyClosure, GoSemanticProcessError> {
    validate_local_path_size_until(root, "Go dependency snapshot certification", deadline)?;
    #[cfg(windows)]
    {
        let root = root.to_path_buf();
        run_windows_file_io_certification(
            deadline,
            "Go dependency snapshot certification",
            move || {
                let certified_scope = crate::go::semantic::windows::certified_local_tree_until(
                    &root,
                    &[],
                    deadline.end,
                )
                .map_err(|error| {
                    windows_file_io_error(
                        error,
                        format!(
                            "failed to certify Go dependency snapshot `{}`",
                            root.display()
                        ),
                    )
                })?;
                capture_dependency_closure_inner(
                    certified_scope.root(),
                    true,
                    deadline,
                    &certified_scope,
                )
            },
        )
    }
    #[cfg(not(windows))]
    capture_dependency_closure_inner(root, true, deadline)
}

fn push_bounded_directory_entry(
    entries: &mut Vec<fs::DirEntry>,
    entry: fs::DirEntry,
    processed_entries: usize,
    maximum_entries: usize,
    skipped_entry_allowance: usize,
    label: &str,
) -> Result<(), GoSemanticProcessError> {
    let collection_limit = maximum_entries.saturating_add(skipped_entry_allowance);
    let prospective_entries = processed_entries
        .checked_add(entries.len())
        .and_then(|entries| entries.checked_add(1));
    if prospective_entries.is_none_or(|entries| entries > collection_limit) {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "{label} contains more than {maximum_entries} entries."
        )));
    }
    entries.push(entry);
    Ok(())
}

fn capture_dependency_closure_inner(
    root: &Path,
    hash_contents: bool,
    deadline: GoOperationDeadline,
    #[cfg(windows)] certified_scope: &crate::go::semantic::windows::CertifiedLocalTree,
) -> Result<DependencyClosure, GoSemanticProcessError> {
    deadline.check("Go dependency snapshot verification")?;
    #[cfg(not(windows))]
    require_local_scan_root(root, deadline)?;
    let lexical_metadata = fs::symlink_metadata(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go dependency snapshot `{}`: {error}",
            root.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&lexical_metadata) || !lexical_metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go dependency snapshot root must be a regular directory.".to_string(),
        ));
    }
    #[cfg(windows)]
    let root = root.to_path_buf();
    #[cfg(not(windows))]
    let root = fs::canonicalize(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize Go dependency snapshot `{}`: {error}",
            root.display()
        ))
    })?;
    let root_metadata = fs::symlink_metadata(&root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go dependency snapshot `{}`: {error}",
            root.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&root_metadata) || !root_metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go dependency snapshot root must be a regular directory.".to_string(),
        ));
    }
    if !dependency_metadata_matches(&lexical_metadata, &root_metadata) {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go dependency snapshot root changed during certification.".to_string(),
        ));
    }
    #[cfg(windows)]
    let (root_identity, root_access) =
        certified_dependency_entry_state(certified_scope, &root, &root_metadata)?;
    #[cfg(not(windows))]
    verify_dependency_entry_permissions(&root, &root_metadata)?;

    let mut content_hasher = Sha256::new();
    content_hasher.update(b"polint-go-dependency-content-v1");
    let mut metadata_hasher = Sha256::new();
    metadata_hasher.update(b"polint-go-dependency-metadata-v1");
    #[cfg(windows)]
    hash_windows_file_identity(&mut metadata_hasher, root_identity, root_access);
    #[cfg(not(windows))]
    hash_toolchain_metadata(&mut metadata_hasher, &root, &root_metadata)?;
    let mut root_metadata_hasher = Sha256::new();
    root_metadata_hasher.update(b"polint-go-dependency-root-metadata-v1");
    #[cfg(windows)]
    hash_windows_file_identity(&mut root_metadata_hasher, root_identity, root_access);
    #[cfg(not(windows))]
    hash_toolchain_metadata(&mut root_metadata_hasher, &root, &root_metadata)?;
    let mut frontier = vec![root.clone()];
    let mut entry_count = 0_usize;
    let mut byte_count = 0_u64;

    while let Some(directory) = frontier.pop() {
        deadline.check("Go dependency snapshot verification")?;
        #[cfg(not(windows))]
        let canonical = fs::canonicalize(&directory).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to canonicalize Go dependency snapshot directory `{}`: {error}",
                directory.display()
            ))
        })?;
        #[cfg(windows)]
        let canonical = directory.clone();
        if !canonical.starts_with(&root) {
            return Err(GoSemanticProcessError::CommandFailed(
                "Go dependency snapshot directory escaped its root.".to_string(),
            ));
        }
        let iterator = fs::read_dir(&directory).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to enumerate Go dependency snapshot `{}`: {error}",
                directory.display()
            ))
        })?;
        let mut entries = Vec::new();
        for entry in iterator {
            deadline.check("Go dependency snapshot verification")?;
            let entry = entry.map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to enumerate Go dependency snapshot entry: {error}"
                ))
            })?;
            push_bounded_directory_entry(
                &mut entries,
                entry,
                entry_count,
                GO_DEPENDENCY_MAX_ENTRIES,
                0,
                "Go dependency snapshot",
            )?;
        }
        entries.sort_by_key(fs::DirEntry::file_name);
        let mut child_directories = Vec::new();
        for entry in entries {
            deadline.check("Go dependency snapshot verification")?;
            entry_count = entry_count.saturating_add(1);
            if entry_count > GO_DEPENDENCY_MAX_ENTRIES {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go dependency snapshot contains more than {GO_DEPENDENCY_MAX_ENTRIES} entries."
                )));
            }
            let path = entry.path();
            let relative = path.strip_prefix(&root).map_err(|_| {
                GoSemanticProcessError::CommandFailed(
                    "Go dependency snapshot entry escaped its root.".to_string(),
                )
            })?;
            let relative_bytes = os_string_bytes(relative.as_os_str());
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go dependency snapshot entry `{}`: {error}",
                    path.display()
                ))
            })?;
            if metadata_is_link_or_reparse(&metadata) {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go dependency snapshot entry `{}` must not be a symlink.",
                    path.display()
                )));
            }
            #[cfg(windows)]
            let (identity, access) =
                certified_dependency_entry_state(certified_scope, &path, &metadata)?;
            #[cfg(not(windows))]
            verify_dependency_entry_permissions(&path, &metadata)?;
            if hash_contents {
                hash_length_prefixed(&mut content_hasher, &relative_bytes);
            }
            hash_length_prefixed(&mut metadata_hasher, &relative_bytes);
            #[cfg(windows)]
            hash_windows_file_identity(&mut metadata_hasher, identity, access);
            #[cfg(not(windows))]
            hash_toolchain_metadata(&mut metadata_hasher, &path, &metadata)?;
            if metadata.is_dir() {
                if hash_contents {
                    content_hasher.update(b"directory");
                }
                child_directories.push(path);
            } else if metadata.is_file() {
                if hash_contents {
                    content_hasher.update(b"file");
                    content_hasher.update(metadata.len().to_le_bytes());
                }
                byte_count = byte_count.checked_add(metadata.len()).ok_or_else(|| {
                    GoSemanticProcessError::CommandFailed(
                        "Go dependency snapshot byte count overflowed.".to_string(),
                    )
                })?;
                if byte_count > GO_DEPENDENCY_MAX_BYTES {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "Go dependency snapshot exceeds the {GO_DEPENDENCY_MAX_BYTES}-byte limit."
                    )));
                }
                if hash_contents {
                    #[cfg(windows)]
                    hash_dependency_file(
                        certified_scope,
                        &path,
                        &metadata,
                        &mut content_hasher,
                        deadline,
                    )?;
                    #[cfg(not(windows))]
                    hash_dependency_file(&path, &metadata, &mut content_hasher, deadline)?;
                }
            } else {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go dependency snapshot entry `{}` is not a regular file or directory.",
                    path.display()
                )));
            }
        }
        child_directories.reverse();
        frontier.extend(child_directories);
    }

    Ok(DependencyClosure {
        content_digest: format!("{:x}", content_hasher.finalize()),
        metadata_digest: format!("{:x}", metadata_hasher.finalize()),
        root_metadata_digest: format!("{:x}", root_metadata_hasher.finalize()),
        entry_count,
        byte_count,
    })
}

#[cfg(unix)]
fn verify_dependency_entry_permissions(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let expected_mode = if metadata.is_dir() { 0o500 } else { 0o400 };
    if metadata.uid() != effective_user_id()
        || metadata.permissions().mode() & 0o777 != expected_mode
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go dependency snapshot entry `{}` is not sealed for its owner.",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn certified_dependency_entry_state(
    certified_scope: &crate::go::semantic::windows::CertifiedLocalTree,
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(crate::go::semantic::windows::WindowsFileIdentity, u32), GoSemanticProcessError> {
    let (identity, access) = certified_scope
        .private_state(path, metadata.is_dir())
        .map_err(|error| {
            windows_file_io_error(
                error,
                format!(
                    "failed to capture sealed Go dependency state for `{}`",
                    path.display()
                ),
            )
        })?;
    if !windows_metadata_matches_identity(metadata, identity)
        || access != crate::go::semantic::windows::WindowsPrivateAccess::Sealed
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go dependency snapshot entry `{}` changed or is not sealed for its owner.",
            path.display()
        )));
    }
    Ok((identity, access.projection()))
}

#[cfg(all(not(unix), not(windows)))]
fn verify_dependency_entry_permissions(
    _path: &Path,
    _metadata: &fs::Metadata,
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "sealed Go dependency snapshots are unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn hash_dependency_file(
    path: &Path,
    expected: &fs::Metadata,
    hasher: &mut Sha256,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to open Go dependency snapshot file `{}`: {error}",
                path.display()
            ))
        })?;
    let opened = file.metadata().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect opened Go dependency snapshot file `{}`: {error}",
            path.display()
        ))
    })?;
    if !dependency_metadata_matches(expected, &opened) {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go dependency snapshot file `{}` changed while it was opened.",
            path.display()
        )));
    }
    let mut read_bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        deadline.check("Go dependency snapshot file hashing")?;
        let count = file.read(&mut buffer).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to read Go dependency snapshot file `{}`: {error}",
                path.display()
            ))
        })?;
        if count == 0 {
            break;
        }
        read_bytes = read_bytes.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
        if read_bytes > expected.len() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go dependency snapshot file `{}` grew while it was hashed.",
                path.display()
            )));
        }
        hasher.update(&buffer[..count]);
    }
    let after = file.metadata().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to re-inspect Go dependency snapshot file `{}`: {error}",
            path.display()
        ))
    })?;
    if read_bytes != expected.len() || !dependency_metadata_matches(expected, &after) {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go dependency snapshot file `{}` changed while it was hashed.",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn hash_dependency_file(
    certified_scope: &crate::go::semantic::windows::CertifiedLocalTree,
    path: &Path,
    expected: &fs::Metadata,
    hasher: &mut Sha256,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    deadline.check("Go dependency snapshot file hashing")?;
    let file = certified_scope
        .open_regular_no_follow(path)
        .map_err(|error| {
            windows_file_io_error(
                error,
                format!(
                    "failed to open Go dependency snapshot file `{}` securely",
                    path.display()
                ),
            )
        })?;
    if !windows_metadata_matches_identity(expected, file.identity()) {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go dependency snapshot file `{}` changed while it was opened.",
            path.display()
        )));
    }
    let bytes_read = file
        .hash_into_until(hasher, expected.len(), deadline.end)
        .map_err(|error| {
            let context = format!(
                "failed to hash Go dependency snapshot file `{}` securely: {error}",
                path.display()
            );
            windows_file_io_error(error, context)
        })?;
    deadline.check("Go dependency snapshot file hashing")?;
    if bytes_read != expected.len() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go dependency snapshot file `{}` changed while it was hashed.",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn hash_dependency_file(
    _path: &Path,
    _expected: &fs::Metadata,
    _hasher: &mut Sha256,
    _deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "sealed Go dependency snapshots are unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn dependency_metadata_matches(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(windows)]
fn dependency_metadata_matches(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    left.file_attributes() == right.file_attributes()
        && left.creation_time() == right.creation_time()
        && left.last_write_time() == right.last_write_time()
        && left.file_size() == right.file_size()
        && left.is_dir() == right.is_dir()
        && left.is_file() == right.is_file()
}

#[cfg(windows)]
fn windows_metadata_matches_identity(
    metadata: &fs::Metadata,
    identity: crate::go::semantic::windows::WindowsFileIdentity,
) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_attributes() == identity.attributes
        && metadata.creation_time() == u64::try_from(identity.creation_time).unwrap_or(u64::MAX)
        && metadata.last_write_time() == u64::try_from(identity.last_write_time).unwrap_or(u64::MAX)
        && metadata.file_size() == identity.size
        && metadata.is_dir() == identity.directory
}

#[cfg(all(not(unix), not(windows)))]
fn dependency_metadata_matches(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.permissions().readonly() == right.permissions().readonly()
        && left.modified().ok() == right.modified().ok()
}

#[cfg(unix)]
fn seal_dependency_tree(
    root: &Path,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    let mut frontier = vec![root.to_path_buf()];
    let mut directories = Vec::new();
    let mut entries_seen = 0_usize;
    while let Some(directory) = frontier.pop() {
        deadline.check("Go dependency snapshot sealing")?;
        directories.push(directory.clone());
        let entries = fs::read_dir(&directory).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to enumerate Go dependency snapshot during sealing: {error}"
            ))
        })?;
        for entry in entries {
            deadline.check("Go dependency snapshot sealing")?;
            entries_seen = entries_seen.saturating_add(1);
            if entries_seen > GO_DEPENDENCY_MAX_ENTRIES {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go dependency snapshot contains more than {GO_DEPENDENCY_MAX_ENTRIES} entries."
                )));
            }
            let entry = entry.map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go dependency snapshot during sealing: {error}"
                ))
            })?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go dependency snapshot entry during sealing: {error}"
                ))
            })?;
            if metadata_is_link_or_reparse(&metadata) {
                return Err(GoSemanticProcessError::CommandFailed(
                    "Go dependency snapshot must not contain symlinks.".to_string(),
                ));
            }
            if metadata.is_dir() {
                frontier.push(entry.path());
            } else if metadata.is_file() {
                fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o400)).map_err(
                    |error| {
                        GoSemanticProcessError::CommandFailed(format!(
                            "failed to seal Go dependency snapshot file: {error}"
                        ))
                    },
                )?;
            } else {
                return Err(GoSemanticProcessError::CommandFailed(
                    "Go dependency snapshot contains a special file.".to_string(),
                ));
            }
        }
    }
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        deadline.check("Go dependency snapshot sealing")?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o500)).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to seal Go dependency snapshot directory: {error}"
            ))
        })?;
    }
    Ok(())
}

#[cfg(windows)]
fn seal_dependency_tree(
    root: &Path,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    let mut frontier = vec![root.to_path_buf()];
    let mut directories = Vec::new();
    let mut entries_seen = 0_usize;
    while let Some(directory) = frontier.pop() {
        deadline.check("Go dependency snapshot sealing")?;
        directories.push(directory.clone());
        let entries = fs::read_dir(&directory).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to enumerate Go dependency snapshot during sealing: {error}"
            ))
        })?;
        for entry in entries {
            deadline.check("Go dependency snapshot sealing")?;
            entries_seen = entries_seen.saturating_add(1);
            if entries_seen > GO_DEPENDENCY_MAX_ENTRIES {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go dependency snapshot contains more than {GO_DEPENDENCY_MAX_ENTRIES} entries."
                )));
            }
            let entry = entry.map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go dependency snapshot during sealing: {error}"
                ))
            })?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go dependency snapshot entry during sealing: {error}"
                ))
            })?;
            if metadata_is_link_or_reparse(&metadata) {
                return Err(GoSemanticProcessError::CommandFailed(
                    "Go dependency snapshot must not contain reparse points.".to_string(),
                ));
            }
            if metadata.is_dir() {
                frontier.push(path);
            } else if metadata.is_file() {
                crate::go::semantic::windows::seal_private_path(&path, false).map_err(|error| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to seal Go dependency snapshot file `{}`: {error}",
                        path.display()
                    ))
                })?;
            } else {
                return Err(GoSemanticProcessError::CommandFailed(
                    "Go dependency snapshot contains a special file.".to_string(),
                ));
            }
        }
    }
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        deadline.check("Go dependency snapshot sealing")?;
        crate::go::semantic::windows::seal_private_path(&directory, true).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to seal Go dependency snapshot directory `{}`: {error}",
                directory.display()
            ))
        })?;
    }
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn seal_dependency_tree(
    _root: &Path,
    _deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "sealed Go dependency snapshots are unavailable on this platform.".to_string(),
    ))
}

fn seal_dependency_snapshot_envelope(
    root: &Path,
    has_workspace: bool,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    deadline.check("Go dependency snapshot envelope sealing")?;
    let mut expected = BTreeSet::from([
        OsString::from("modules"),
        OsString::from("payload.json"),
        OsString::from("request"),
    ]);
    if has_workspace {
        expected.insert(OsString::from("workspace"));
    }
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to enumerate Go dependency snapshot envelope `{}`: {error}",
            root.display()
        ))
    })? {
        deadline.check("Go dependency snapshot envelope sealing")?;
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go dependency snapshot envelope entry: {error}"
            ))
        })?;
        actual.insert(entry.file_name());
    }
    if actual != expected {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go dependency snapshot envelope contains unexpected entries.".to_string(),
        ));
    }
    for name in ["request", "payload.json"] {
        let path = root.join(name);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go dependency snapshot envelope file `{}`: {error}",
                path.display()
            ))
        })?;
        if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go dependency snapshot envelope file `{}` is unsafe.",
                path.display()
            )));
        }
        seal_dependency_envelope_path(&path, false)?;
    }
    for name in ["modules", "workspace"]
        .into_iter()
        .take(if has_workspace { 2 } else { 1 })
    {
        let path = root.join(name);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect sealed Go dependency snapshot subtree `{}`: {error}",
                path.display()
            ))
        })?;
        if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "sealed Go dependency snapshot subtree `{}` is unsafe.",
                path.display()
            )));
        }
    }
    #[cfg(unix)]
    {
        // macOS refuses to rename a directory after its owner write bit is
        // removed. The lifecycle lock keeps the destination private while the
        // root is renamed and immediately sealed; every child is already
        // sealed at this point.
        Ok(())
    }
    #[cfg(not(unix))]
    seal_dependency_envelope_path(root, true)
}

#[cfg(unix)]
fn seal_dependency_envelope_path(
    path: &Path,
    directory: bool,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if directory { 0o500 } else { 0o400 }),
    )
    .map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to seal Go dependency snapshot envelope `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(windows)]
fn seal_dependency_envelope_path(
    path: &Path,
    directory: bool,
) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::windows::seal_private_path(path, directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to seal Go dependency snapshot envelope `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(all(not(unix), not(windows)))]
fn seal_dependency_envelope_path(
    _path: &Path,
    _directory: bool,
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "Go dependency snapshot envelope sealing is unavailable on this platform.".to_string(),
    ))
}

fn require_local_scan_root(
    path: &Path,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    require_local_scan_root_with_exclusions(path, &[], deadline)
}

#[cfg(not(windows))]
fn require_local_dependency_tree_mounts(
    path: &Path,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::local_fs::require_local_tree_mounts_until(path, deadline.end).map_err(
        |error| {
            if matches!(
                &error,
                crate::go::semantic::local_fs::LocalFilesystemError::Inspection { source, .. }
                    if source.kind() == std::io::ErrorKind::TimedOut
            ) {
                return GoSemanticProcessError::Timeout(format!(
                    "Go semantic local tree certification exceeded its operation deadline: {error}"
                ));
            }
            GoSemanticProcessError::CommandUnavailable(format!(
                "Go semantic analysis requires a local filesystem boundary: {error}"
            ))
        },
    )
}

#[cfg(windows)]
fn require_local_dependency_tree_mounts(
    path: &Path,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    deadline.check("Go semantic local tree certification")?;
    crate::go::semantic::windows::require_local_fixed_volume(path).map_err(|error| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "Go semantic analysis requires a local filesystem boundary: {error}"
        ))
    })?;
    deadline.check("Go semantic local tree certification")
}

fn require_local_scan_roots_with_exclusions(
    roots: &[PathBuf],
    exclusions: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    validate_local_path_batch(
        exclusions,
        "Go semantic local tree exclusions",
        Some(deadline),
    )?;
    for root in collapse_nested_local_roots(roots, deadline)? {
        deadline.check("Go semantic local tree certification")?;
        let nested_exclusions = exclusions
            .iter()
            .filter(|exclusion| exclusion.as_path() != root && exclusion.starts_with(root))
            .cloned()
            .collect::<Vec<_>>();
        require_local_scan_root_with_exclusions(root, &nested_exclusions, deadline)?;
    }
    Ok(())
}

fn require_local_go_scan_roots_with_scope(
    roots: &[PathBuf],
    exclusions: &[PathBuf],
    inclusions: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    validate_local_path_batch(
        exclusions,
        "Go semantic local tree exclusions",
        Some(deadline),
    )?;
    validate_local_path_batch(
        inclusions,
        "Go semantic local tree inclusions",
        Some(deadline),
    )?;
    for root in collapse_nested_local_roots(roots, deadline)? {
        deadline.check("Go semantic local tree certification")?;
        let nested_exclusions = exclusions
            .iter()
            .filter(|exclusion| exclusion.as_path() != root && exclusion.starts_with(root))
            .cloned()
            .collect::<Vec<_>>();
        let nested_inclusions = inclusions
            .iter()
            .filter(|inclusion| inclusion.starts_with(root))
            .cloned()
            .collect::<Vec<_>>();
        #[cfg(windows)]
        let result = crate::go::semantic::local_fs::require_local_tree_with_scope_until(
            root,
            &nested_exclusions,
            &nested_inclusions,
            deadline.end,
        );
        #[cfg(not(windows))]
        let result = {
            let _ = (&nested_exclusions, &nested_inclusions);
            crate::go::semantic::local_fs::require_local_tree_until(root, deadline.end)
        };
        result.map_err(|error| {
            if matches!(
                &error,
                crate::go::semantic::local_fs::LocalFilesystemError::Inspection { source, .. }
                    if source.kind() == std::io::ErrorKind::TimedOut
            ) {
                return GoSemanticProcessError::Timeout(format!(
                    "Go semantic local tree certification exceeded its operation deadline: {error}"
                ));
            }
            GoSemanticProcessError::CommandUnavailable(format!(
                "Go semantic analysis requires a local filesystem boundary: {error}"
            ))
        })?;
    }
    Ok(())
}

fn require_local_scan_root_with_exclusions(
    path: &Path,
    exclusions: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    #[cfg(windows)]
    let result = crate::go::semantic::local_fs::require_local_tree_with_exclusions_until(
        path,
        exclusions,
        deadline.end,
    );
    #[cfg(not(windows))]
    let result = {
        let _ = exclusions;
        crate::go::semantic::local_fs::require_local_tree_until(path, deadline.end)
    };
    result.map_err(|error| {
        if matches!(
            &error,
            crate::go::semantic::local_fs::LocalFilesystemError::Inspection { source, .. }
                if source.kind() == std::io::ErrorKind::TimedOut
        ) {
            return GoSemanticProcessError::Timeout(format!(
                "Go semantic local tree certification exceeded its operation deadline: {error}"
            ));
        }
        GoSemanticProcessError::CommandUnavailable(format!(
            "Go semantic analysis requires a local filesystem boundary: {error}"
        ))
    })
}

fn require_local_existing_path(path: &Path) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::local_fs::require_local_containing_path(path).map_err(|error| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "Go semantic analysis requires a local filesystem boundary: {error}"
        ))
    })
}

fn validate_local_path_size(
    path: &Path,
    label: &'static str,
) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::local_fs::validate_path_size(path).map_err(|error| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "{label} rejected an oversized filesystem path: {error}"
        ))
    })
}

fn validate_local_path_size_until(
    path: &Path,
    label: &'static str,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    deadline.check(label)?;
    validate_local_path_size(path, label)?;
    deadline.check(label)
}

fn validate_local_path_batch(
    paths: &[PathBuf],
    label: &'static str,
    deadline: Option<GoOperationDeadline>,
) -> Result<(), GoSemanticProcessError> {
    validate_local_path_batch_with_limits(
        paths,
        label,
        deadline,
        MAX_LOCAL_CERTIFICATION_PATHS,
        MAX_LOCAL_CERTIFICATION_PATH_UNITS_TOTAL,
    )
}

fn validate_local_path_batch_with_limits(
    paths: &[PathBuf],
    label: &'static str,
    deadline: Option<GoOperationDeadline>,
    maximum_paths: usize,
    maximum_units: usize,
) -> Result<(), GoSemanticProcessError> {
    validate_local_path_batches_with_limits(&[paths], label, deadline, maximum_paths, maximum_units)
}

fn validate_local_path_batches_with_limits(
    batches: &[&[PathBuf]],
    label: &'static str,
    deadline: Option<GoOperationDeadline>,
    maximum_paths: usize,
    maximum_units: usize,
) -> Result<(), GoSemanticProcessError> {
    if let Some(deadline) = deadline {
        deadline.check(label)?;
    }
    let path_count = batches.iter().try_fold(0_usize, |count, paths| {
        count.checked_add(paths.len()).ok_or_else(|| {
            GoSemanticProcessError::CommandUnavailable(format!(
                "{label} path-count accounting overflowed"
            ))
        })
    })?;
    if path_count > maximum_paths {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "{label} exceeded the {maximum_paths}-path certification limit"
        )));
    }
    let mut total_units = 0_usize;
    for paths in batches {
        for path in *paths {
            if let Some(deadline) = deadline {
                deadline.check(label)?;
            }
            validate_local_path_size(path, label)?;
            total_units = total_units
                .checked_add(local_path_storage_units(path))
                .ok_or_else(|| {
                    GoSemanticProcessError::CommandUnavailable(format!(
                        "{label} path-size accounting overflowed"
                    ))
                })?;
            if total_units > maximum_units {
                return Err(GoSemanticProcessError::CommandUnavailable(format!(
                    "{label} exceeded the {maximum_units}-unit aggregate path limit"
                )));
            }
            if let Some(deadline) = deadline {
                deadline.check(label)?;
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn clone_bounded_path_batch_until(
    paths: &[PathBuf],
    label: &'static str,
    deadline: GoOperationDeadline,
) -> Result<Vec<PathBuf>, GoSemanticProcessError> {
    deadline.check(label)?;
    let mut owned = Vec::new();
    owned.try_reserve_exact(paths.len()).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to allocate bounded path ownership for {label}: {error}"
        ))
    })?;
    for path in paths {
        deadline.check(label)?;
        owned.push(path.clone());
        deadline.check(label)?;
    }
    Ok(owned)
}

#[cfg(unix)]
fn local_path_storage_units(path: &Path) -> usize {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().len()
}

#[cfg(windows)]
fn local_path_storage_units(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().count()
}

#[cfg(not(any(unix, windows)))]
fn local_path_storage_units(path: &Path) -> usize {
    path.components().count()
}

fn require_local_existing_paths_until(
    paths: &[PathBuf],
    deadline: GoOperationDeadline,
    label: &'static str,
) -> Result<(), GoSemanticProcessError> {
    validate_local_path_batch(paths, label, Some(deadline))?;
    #[cfg(windows)]
    {
        if crate::go::semantic::windows::cancellable_file_io_pass_is_active() {
            for path in paths {
                deadline.check(label)?;
                require_local_existing_path(path)?;
                deadline.check(label)?;
            }
            return Ok(());
        }
        deadline.check(label)?;
        let paths = clone_bounded_path_batch_until(paths, label, deadline)?;
        deadline.check(label)?;
        run_windows_file_io_certification(deadline, label, move || {
            for path in paths {
                require_local_existing_path(&path)?;
                deadline.check(label)?;
            }
            Ok(())
        })
    }
    #[cfg(not(windows))]
    {
        for path in paths {
            deadline.check(label)?;
            crate::go::semantic::local_fs::require_local_containing_path_until(path, deadline.end)
                .map_err(|error| {
                    if matches!(
                        &error,
                        crate::go::semantic::local_fs::LocalFilesystemError::Inspection {
                            source,
                            ..
                        } if source.kind() == std::io::ErrorKind::TimedOut
                    ) {
                        return GoSemanticProcessError::Timeout(format!(
                            "{label} exceeded its operation deadline: {error}"
                        ));
                    }
                    GoSemanticProcessError::CommandUnavailable(format!(
                        "Go semantic analysis requires a local filesystem boundary: {error}"
                    ))
                })?;
            deadline.check(label)?;
        }
        Ok(())
    }
}

fn require_local_existing_path_until(
    path: &Path,
    deadline: GoOperationDeadline,
    label: &'static str,
) -> Result<(), GoSemanticProcessError> {
    validate_local_path_size_until(path, label, deadline)?;
    require_local_existing_paths_until(std::slice::from_ref(&path.to_path_buf()), deadline, label)
}

fn require_local_creation_root(path: &Path) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::local_fs::require_local_filesystem_for_creation(path).map_err(|error| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "Go semantic analysis requires a local filesystem boundary: {error}"
        ))
    })
}

fn require_local_creation_root_until(
    path: &Path,
    deadline: GoOperationDeadline,
    label: &'static str,
) -> Result<(), GoSemanticProcessError> {
    validate_local_path_size_until(path, label, deadline)?;
    #[cfg(windows)]
    {
        if crate::go::semantic::windows::cancellable_file_io_pass_is_active() {
            require_local_creation_root(path)?;
            return deadline.check(label);
        }
        let path = path.to_path_buf();
        run_windows_file_io_certification(deadline, label, move || {
            require_local_creation_root(&path)?;
            deadline.check(label)
        })
    }
    #[cfg(not(windows))]
    {
        deadline.check(label)?;
        crate::go::semantic::local_fs::require_local_filesystem_for_creation_until(
            path,
            deadline.end,
        )
        .map_err(|error| {
            if matches!(
                &error,
                crate::go::semantic::local_fs::LocalFilesystemError::Inspection {
                    source,
                    ..
                } if source.kind() == std::io::ErrorKind::TimedOut
            ) {
                return GoSemanticProcessError::Timeout(format!(
                    "{label} exceeded its operation deadline: {error}"
                ));
            }
            GoSemanticProcessError::CommandUnavailable(format!(
                "Go semantic analysis requires a local filesystem boundary: {error}"
            ))
        })?;
        deadline.check(label)
    }
}

struct PrivateGoWorkspace {
    path: Option<PathBuf>,
    main_modules: Vec<PathBuf>,
    replacement_manifest_digest: String,
    analysis_roots: Vec<PathBuf>,
}

impl PrivateGoWorkspace {
    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn analysis_roots(&self) -> &[PathBuf] {
        &self.analysis_roots
    }

    fn verify_replacement_manifest_inputs(
        &self,
        root: &Path,
        deadline: GoOperationDeadline,
    ) -> Result<(), GoSemanticProcessError> {
        let observed = main_module_replacement_manifest_digest(root, &self.main_modules, deadline)?;
        if observed != self.replacement_manifest_digest {
            return Err(GoSemanticProcessError::CommandFailed(
                "Go module replacement inputs changed while the private workspace was prepared."
                    .to_string(),
            ));
        }
        Ok(())
    }
}

fn private_go_workspace(
    private_root: &Path,
    toolchain: &PreparedGoToolchain,
    root: &Path,
    config: &GoAnalysisConfig,
    deadline: GoOperationDeadline,
) -> Result<PrivateGoWorkspace, GoSemanticProcessError> {
    deadline.check("private Go workspace preparation")?;
    require_local_existing_path_until(root, deadline, "private Go workspace preparation")?;
    let checked_in_path = root.join("go.work");
    require_local_creation_root_until(
        &checked_in_path,
        deadline,
        "private Go workspace selection",
    )?;
    deadline.check("private Go workspace selection")?;
    let workspace_directory = private_root.join("workspace");
    create_private_directory(&workspace_directory)?;
    let path = workspace_directory.join("go.work");
    let checked_in_bytes = match fs::symlink_metadata(&checked_in_path) {
        Ok(metadata) => {
            if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
                return Err(GoSemanticProcessError::CommandUnavailable(format!(
                    "checked-in Go workspace `{}` must be a direct regular file.",
                    checked_in_path.display()
                )));
            }
            require_local_existing_path_until(
                &checked_in_path,
                deadline,
                "private Go workspace selection",
            )?;
            Some(read_regular_file_no_follow_until(
                &checked_in_path,
                deadline,
            )?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect checked-in Go workspace `{}`: {error}",
                checked_in_path.display()
            )));
        }
    };
    let mut selected_path = checked_in_bytes
        .as_ref()
        .map_or_else(|| path.clone(), |_| checked_in_path.clone());
    let bytes = match checked_in_bytes {
        Some(bytes) => bytes,
        None => synthetic_private_go_workspace(root, &config.module_roots, deadline)?,
    };
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > GO_DEPENDENCY_MANIFEST_MAX_BYTES {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go workspace `{}` exceeds the {}-byte dependency manifest limit.",
            selected_path.display(),
            GO_DEPENDENCY_MANIFEST_MAX_BYTES
        )));
    }
    deadline.check("private Go workspace preparation")?;
    write_new_private_mutable_file(&path, &bytes)?;
    let mut normalized =
        normalize_private_go_workspace(toolchain, root, &selected_path, &path, deadline)?;
    if selected_path == checked_in_path {
        let canonical_root = fs::canonicalize(root).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to canonicalize Go analysis root: {error}"
            ))
        })?;
        let configured_roots = config
            .module_roots
            .iter()
            .map(|module_root| {
                certified_workspace_local_path(
                    &canonical_root,
                    module_root,
                    &canonical_root,
                    true,
                    deadline,
                )
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let checked_in_covers_configured = configured_roots
            .iter()
            .all(|module_root| normalized.main_modules.binary_search(module_root).is_ok());
        if checked_in_covers_configured {
            copy_checked_in_workspace_sum(&selected_path, &path, deadline)?;
        } else {
            let synthetic = synthetic_private_go_workspace(root, &config.module_roots, deadline)?;
            overwrite_private_mutable_file(&path, &synthetic)?;
            selected_path = path.clone();
            normalized =
                normalize_private_go_workspace(toolchain, root, &selected_path, &path, deadline)?;
        }
    }
    let replacements = module_local_replacement_roots(
        toolchain,
        root,
        &normalized.main_modules,
        &workspace_directory,
        deadline,
    )?;
    normalized.analysis_roots.extend(replacements.roots);
    normalized.analysis_roots.sort();
    normalized.analysis_roots.dedup();
    Ok(PrivateGoWorkspace {
        path: Some(path),
        main_modules: normalized.main_modules,
        replacement_manifest_digest: replacements.manifest_digest,
        analysis_roots: normalized.analysis_roots,
    })
}

fn copy_checked_in_workspace_sum(
    selected_path: &Path,
    private_path: &Path,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    let mut selected_sum_value = selected_path.as_os_str().to_os_string();
    selected_sum_value.push(".sum");
    let selected_sum_path = PathBuf::from(selected_sum_value);
    require_local_creation_root_until(
        &selected_sum_path,
        deadline,
        "checked-in Go workspace sum selection",
    )?;
    deadline.check("checked-in Go workspace sum selection")?;
    let metadata = match fs::symlink_metadata(&selected_sum_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go workspace sum `{}`: {error}",
                selected_sum_path.display()
            )));
        }
    };
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go workspace sum `{}` must be a regular file.",
            selected_sum_path.display()
        )));
    }
    require_local_existing_path_until(
        &selected_sum_path,
        deadline,
        "checked-in Go workspace sum selection",
    )?;
    let sum_bytes = read_regular_file_no_follow_until(&selected_sum_path, deadline)?;
    if u64::try_from(sum_bytes.len()).unwrap_or(u64::MAX) > GO_DEPENDENCY_MANIFEST_MAX_BYTES {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go workspace sum `{}` exceeds the {}-byte dependency manifest limit.",
            selected_sum_path.display(),
            GO_DEPENDENCY_MANIFEST_MAX_BYTES
        )));
    }
    let mut private_sum_value = private_path.as_os_str().to_os_string();
    private_sum_value.push(".sum");
    write_new_private_mutable_file(&PathBuf::from(private_sum_value), &sum_bytes)
}

fn synthetic_private_go_workspace(
    root: &Path,
    module_roots: &[String],
    deadline: GoOperationDeadline,
) -> Result<Vec<u8>, GoSemanticProcessError> {
    deadline.check("synthetic private Go workspace preparation")?;
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize Go analysis root: {error}"
        ))
    })?;
    let mut version = MIN_SYNTHETIC_GO_WORK_VERSION.to_string();
    let mut uses = BTreeSet::new();
    for module_root in module_roots {
        deadline.check("synthetic private Go workspace preparation")?;
        let module = certified_workspace_local_path(
            &canonical_root,
            module_root,
            &canonical_root,
            true,
            deadline,
        )?;
        let go_mod = module.join("go.mod");
        let metadata = fs::symlink_metadata(&go_mod).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go module manifest `{}`: {error}",
                go_mod.display()
            ))
        })?;
        if metadata.len() > GO_DEPENDENCY_MANIFEST_MAX_BYTES {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go module manifest `{}` exceeds the {}-byte limit.",
                go_mod.display(),
                GO_DEPENDENCY_MANIFEST_MAX_BYTES
            )));
        }
        let manifest = read_regular_file_no_follow_until(&go_mod, deadline)?;
        if let Some(module_version) = go_mod_version_directive(&manifest)?
            && go_version_is_greater(&module_version, &version)
        {
            version = module_version;
        }
        uses.insert(module);
    }
    let mut workspace = format!("go {version}\n\nuse (\n");
    for module in uses {
        workspace.push('\t');
        workspace.push_str(&quote_go_workspace_path(&module)?);
        workspace.push('\n');
    }
    workspace.push_str(")\n");
    Ok(workspace.into_bytes())
}

fn go_mod_version_directive(bytes: &[u8]) -> Result<Option<String>, GoSemanticProcessError> {
    let source = std::str::from_utf8(bytes).map_err(|_| {
        GoSemanticProcessError::CommandFailed(
            "Go module manifest must be valid UTF-8 before workspace preparation.".to_string(),
        )
    })?;
    for raw in source.lines() {
        let line = raw
            .split_once("//")
            .map_or(raw, |(before, _)| before)
            .trim();
        let mut fields = line.split_ascii_whitespace();
        if fields.next() == Some("go") {
            return Ok(fields.next().map(str::to_string));
        }
    }
    Ok(None)
}

fn go_version_is_greater(left: &str, right: &str) -> bool {
    let mut left = left
        .trim_start_matches("go")
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0));
    let mut right = right
        .trim_start_matches("go")
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0));
    loop {
        match (left.next(), right.next()) {
            (Some(left), Some(right)) if left != right => return left > right,
            (Some(left), None) if left != 0 => return true,
            (None, Some(right)) if right != 0 => return false,
            (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => {}
            (None, None) => return false,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GoWorkEditJson {
    #[serde(default)]
    go: String,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    r#use: Vec<GoWorkEditUse>,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    replace: Vec<GoWorkEditReplace>,
}

fn null_as_default_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GoWorkEditUse {
    disk_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GoWorkEditReplace {
    old: GoWorkEditModule,
    new: GoWorkEditModule,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GoWorkEditModule {
    path: String,
    #[serde(default)]
    version: String,
}

#[derive(Debug)]
struct NormalizedPrivateGoWorkspace {
    main_modules: Vec<PathBuf>,
    analysis_roots: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GoModEditJson {
    #[serde(default, deserialize_with = "null_as_default_vec")]
    replace: Vec<GoWorkEditReplace>,
}

fn normalize_private_go_workspace(
    toolchain: &PreparedGoToolchain,
    root: &Path,
    selected_path: &Path,
    private_path: &Path,
    deadline: GoOperationDeadline,
) -> Result<NormalizedPrivateGoWorkspace, GoSemanticProcessError> {
    deadline.check("private Go workspace normalization")?;
    let mut inspect = Command::new(&toolchain.executable);
    configure_go_environment(&mut inspect, toolchain);
    inspect
        .arg("work")
        .arg("edit")
        .arg("-json")
        // The bytes were copied with no-follow reads. Parse that private copy
        // so the subprocess never reopens the mutable checked-in workspace;
        // relative directives are still resolved against `selected_path` below.
        .arg(private_path)
        .env("GOWORK", "off")
        .env("GOPROXY", "off")
        .env("GOSUMDB", "off")
        .env("GOFLAGS", "");
    let private_directory = private_path.parent().ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(
            "private Go workspace has no parent directory.".to_string(),
        )
    })?;
    let private_tree = private_directory.to_path_buf();
    let output = run_prepared_go_command_with_local_trees_until(
        toolchain,
        inspect,
        std::slice::from_ref(&private_tree),
        BoundedCommandLimits::new(
            GO_OPERATION_TIMEOUT,
            GO_DEPENDENCY_COMMAND_STDOUT_BYTES,
            GO_DEPENDENCY_COMMAND_STDERR_BYTES,
        ),
        deadline,
        "private Go workspace parsing",
    )?;
    if !output.status.success() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "private Go workspace parsing failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let workspace: GoWorkEditJson = serde_json::from_slice(&output.stdout).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to decode parsed Go workspace: {error}"
        ))
    })?;
    deadline.check("private Go workspace parsing")?;
    if workspace.go.is_empty() {
        return Err(GoSemanticProcessError::CommandFailed(
            "parsed Go workspace has no Go version directive.".to_string(),
        ));
    }
    let root = fs::canonicalize(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize Go analysis root: {error}"
        ))
    })?;
    let selected_directory = selected_path.parent().unwrap_or(&root);
    let original = read_regular_file_no_follow_until(private_path, deadline)?;
    let auxiliary = parse_workspace_auxiliary_directives(&original, deadline)?;
    let mut expected_uses = BTreeSet::new();
    for entry in &workspace.r#use {
        deadline.check("private Go workspace normalization")?;
        let absolute = certified_workspace_local_path(
            selected_directory,
            &entry.disk_path,
            &root,
            true,
            deadline,
        )?;
        expected_uses.insert(absolute.clone());
    }
    let mut expected_replacements = BTreeSet::new();
    for replacement in &workspace.replace {
        deadline.check("private Go workspace normalization")?;
        let old = go_module_spec(&replacement.old);
        let new = if replacement.new.version.is_empty() {
            certified_workspace_local_path(
                selected_directory,
                &replacement.new.path,
                &root,
                true,
                deadline,
            )?
            .to_string_lossy()
            .into_owned()
        } else {
            go_module_spec(&replacement.new)
        };
        expected_replacements.insert((old, new));
    }
    let mut normalized = format!("go {}\n", workspace.go);
    if let Some(toolchain) = &auxiliary.toolchain {
        normalized.push_str("\ntoolchain ");
        normalized.push_str(toolchain);
        normalized.push('\n');
    }
    if !auxiliary.godebug.is_empty() {
        normalized.push_str("\ngodebug (\n");
        for setting in &auxiliary.godebug {
            normalized.push('\t');
            normalized.push_str(setting);
            normalized.push('\n');
        }
        normalized.push_str(")\n");
    }
    normalized.push_str("\nuse (\n");
    for module in &expected_uses {
        normalized.push('\t');
        normalized.push_str(&quote_go_workspace_path(module)?);
        normalized.push('\n');
    }
    normalized.push_str(")\n");
    for (old, new) in &expected_replacements {
        normalized.push_str("\nreplace ");
        normalized.push_str(old);
        normalized.push_str(" => ");
        if Path::new(new).is_absolute() {
            normalized.push_str(&quote_go_workspace_path(Path::new(new))?);
        } else {
            normalized.push_str(new);
        }
        normalized.push('\n');
    }
    overwrite_private_mutable_file(private_path, normalized.as_bytes())?;
    let rewritten = inspect_private_go_workspace(toolchain, private_path, deadline)?;
    if rewritten.go != workspace.go {
        return Err(GoSemanticProcessError::CommandFailed(
            "rewritten private Go workspace changed its Go version directive.".to_string(),
        ));
    }
    let rewritten_uses = rewritten
        .r#use
        .iter()
        .map(|entry| {
            certified_workspace_local_path(
                private_directory,
                &entry.disk_path,
                &root,
                true,
                deadline,
            )
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if rewritten_uses != expected_uses {
        return Err(GoSemanticProcessError::CommandFailed(
            "rewritten private Go workspace changed its certified module roots.".to_string(),
        ));
    }
    let rewritten_replacements = rewritten
        .replace
        .iter()
        .map(|replacement| {
            let old = go_module_spec(&replacement.old);
            let new = if replacement.new.version.is_empty() {
                certified_workspace_local_path(
                    private_directory,
                    &replacement.new.path,
                    &root,
                    true,
                    deadline,
                )?
                .to_string_lossy()
                .into_owned()
            } else {
                go_module_spec(&replacement.new)
            };
            Ok((old, new))
        })
        .collect::<Result<BTreeSet<_>, GoSemanticProcessError>>()?;
    if rewritten_replacements != expected_replacements {
        return Err(GoSemanticProcessError::CommandFailed(
            "rewritten private Go workspace changed its certified replacements.".to_string(),
        ));
    }
    let rewritten_bytes = read_regular_file_no_follow_until(private_path, deadline)?;
    let rewritten_auxiliary = parse_workspace_auxiliary_directives(&rewritten_bytes, deadline)?;
    if rewritten_auxiliary != auxiliary {
        return Err(GoSemanticProcessError::CommandFailed(
            "rewritten private Go workspace changed its toolchain or godebug directives."
                .to_string(),
        ));
    }
    let main_modules = expected_uses.iter().cloned().collect::<Vec<_>>();
    let mut analysis_roots = expected_uses.into_iter().collect::<Vec<_>>();
    analysis_roots.extend(
        expected_replacements
            .into_iter()
            .filter_map(|(_, replacement)| {
                let path = PathBuf::from(replacement);
                path.is_absolute().then_some(path)
            }),
    );
    analysis_roots.sort();
    analysis_roots.dedup();
    Ok(NormalizedPrivateGoWorkspace {
        main_modules,
        analysis_roots,
    })
}

struct LocalReplacementRoots {
    roots: Vec<PathBuf>,
    manifest_digest: String,
}

fn read_main_module_manifest(
    module_root: &Path,
    deadline: GoOperationDeadline,
) -> Result<(PathBuf, Vec<u8>), GoSemanticProcessError> {
    const LABEL: &str = "private Go module manifest parsing";

    let go_mod = module_root.join("go.mod");
    require_local_existing_path_until(&go_mod, deadline, LABEL)?;
    let metadata = fs::symlink_metadata(&go_mod).map_err(|error| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "failed to inspect Go module manifest `{}`: {error}",
            go_mod.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "Go module manifest `{}` must be a direct regular file.",
            go_mod.display()
        )));
    }
    if metadata.len() > GO_DEPENDENCY_MANIFEST_MAX_BYTES {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go module manifest `{}` exceeds the {}-byte limit.",
            go_mod.display(),
            GO_DEPENDENCY_MANIFEST_MAX_BYTES
        )));
    }
    require_local_existing_path_until(&go_mod, deadline, LABEL)?;
    let bytes = read_regular_file_no_follow_until(&go_mod, deadline)?;
    Ok((go_mod, bytes))
}

fn hash_main_module_manifest(
    hasher: &mut Sha256,
    canonical_root: &Path,
    go_mod: &Path,
    bytes: &[u8],
) -> Result<(), GoSemanticProcessError> {
    let relative = go_mod.strip_prefix(canonical_root).map_err(|_| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "Go module manifest `{}` escaped the analysis root.",
            go_mod.display()
        ))
    })?;
    hash_length_prefixed(hasher, &os_string_bytes(relative.as_os_str()));
    hash_length_prefixed(hasher, bytes);
    Ok(())
}

fn main_module_replacement_manifest_digest(
    root: &Path,
    main_modules: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<String, GoSemanticProcessError> {
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize Go analysis root: {error}"
        ))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(b"polint-go-main-module-replacement-inputs-v1");
    for module_root in main_modules {
        deadline.check("Go module replacement input verification")?;
        let (go_mod, bytes) = read_main_module_manifest(module_root, deadline)?;
        hash_main_module_manifest(&mut hasher, &canonical_root, &go_mod, &bytes)?;
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn module_local_replacement_roots(
    toolchain: &PreparedGoToolchain,
    root: &Path,
    main_modules: &[PathBuf],
    private_workspace_directory: &Path,
    deadline: GoOperationDeadline,
) -> Result<LocalReplacementRoots, GoSemanticProcessError> {
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize Go analysis root: {error}"
        ))
    })?;
    let private_manifests = private_workspace_directory.join("module-manifests");
    create_private_directory(&private_manifests)?;
    verify_go_toolchain_binding_until(toolchain, deadline)?;
    let parsed = (|| {
        let mut replacement_roots = BTreeSet::new();
        let mut manifest_hasher = Sha256::new();
        manifest_hasher.update(b"polint-go-main-module-replacement-inputs-v1");

        for (index, module_root) in main_modules.iter().enumerate() {
            deadline.check("private Go module manifest parsing")?;
            let (go_mod, bytes) = read_main_module_manifest(module_root, deadline)?;
            hash_main_module_manifest(&mut manifest_hasher, &canonical_root, &go_mod, &bytes)?;
            let private_manifest = private_manifests.join(format!("module-{index}.mod"));
            write_new_private_mutable_file(&private_manifest, &bytes)?;

            let mut command = Command::new(&toolchain.executable);
            configure_go_environment(&mut command, toolchain);
            command
                .arg("mod")
                .arg("edit")
                .arg("-json")
                .arg(&private_manifest)
                .env("GOWORK", "off")
                .env("GOPROXY", "off")
                .env("GOSUMDB", "off")
                .env("GOFLAGS", "");
            let output = run_bounded_command_with_local_trees_until(
                command,
                std::slice::from_ref(&private_manifests),
                BoundedCommandLimits::new(
                    GO_OPERATION_TIMEOUT,
                    GO_DEPENDENCY_COMMAND_STDOUT_BYTES,
                    GO_DEPENDENCY_COMMAND_STDERR_BYTES,
                ),
                deadline,
                "private Go module manifest parsing",
            )?;
            if !output.status.success() {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "private Go module manifest parsing failed for `{}`: {}",
                    go_mod.display(),
                    String::from_utf8_lossy(&output.stderr).trim()
                )));
            }
            let manifest: GoModEditJson =
                serde_json::from_slice(&output.stdout).map_err(|error| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to decode parsed Go module manifest `{}`: {error}",
                        go_mod.display()
                    ))
                })?;
            fs::remove_file(&private_manifest).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to remove private Go module manifest scratch file `{}`: {error}",
                    private_manifest.display()
                ))
            })?;
            for replacement in manifest.replace {
                deadline.check("Go module local replacement certification")?;
                if replacement.new.version.is_empty() {
                    replacement_roots.insert(certified_workspace_local_path(
                        module_root,
                        &replacement.new.path,
                        &canonical_root,
                        true,
                        deadline,
                    )?);
                }
            }
        }
        fs::remove_dir(&private_manifests).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to remove private Go module manifest scratch directory `{}`: {error}",
                private_manifests.display()
            ))
        })?;
        Ok(LocalReplacementRoots {
            roots: replacement_roots.into_iter().collect(),
            manifest_digest: format!("{:x}", manifest_hasher.finalize()),
        })
    })();
    let binding = verify_go_toolchain_binding_until(toolchain, deadline);
    binding?;
    parsed
}

fn inspect_private_go_workspace(
    toolchain: &PreparedGoToolchain,
    path: &Path,
    deadline: GoOperationDeadline,
) -> Result<GoWorkEditJson, GoSemanticProcessError> {
    let mut command = Command::new(&toolchain.executable);
    configure_go_environment(&mut command, toolchain);
    command
        .arg("work")
        .arg("edit")
        .arg("-json")
        .arg(path)
        .env("GOWORK", "off")
        .env("GOPROXY", "off")
        .env("GOSUMDB", "off")
        .env("GOFLAGS", "");
    let local_root = path.parent().map(Path::to_path_buf).ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(
            "private Go workspace has no parent directory.".to_string(),
        )
    })?;
    let output = run_prepared_go_command_with_local_trees_until(
        toolchain,
        command,
        std::slice::from_ref(&local_root),
        BoundedCommandLimits::new(
            GO_OPERATION_TIMEOUT,
            GO_DEPENDENCY_COMMAND_STDOUT_BYTES,
            GO_DEPENDENCY_COMMAND_STDERR_BYTES,
        ),
        deadline,
        "rewritten private Go workspace verification",
    )?;
    if !output.status.success() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "rewritten private Go workspace verification failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let workspace = serde_json::from_slice(&output.stdout).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to decode rewritten private Go workspace: {error}"
        ))
    })?;
    deadline.check("rewritten private Go workspace verification")?;
    Ok(workspace)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct WorkspaceAuxiliaryDirectives {
    toolchain: Option<String>,
    godebug: Vec<String>,
}

fn parse_workspace_auxiliary_directives(
    bytes: &[u8],
    deadline: GoOperationDeadline,
) -> Result<WorkspaceAuxiliaryDirectives, GoSemanticProcessError> {
    let source = std::str::from_utf8(bytes).map_err(|_| {
        GoSemanticProcessError::CommandFailed(
            "Go workspace must be valid UTF-8 before normalization.".to_string(),
        )
    })?;
    let mut result = WorkspaceAuxiliaryDirectives::default();
    let mut godebug_block = false;
    for raw in source.lines() {
        deadline.check("Go workspace auxiliary directive parsing")?;
        let line = raw
            .split_once("//")
            .map_or(raw, |(before, _)| before)
            .trim();
        if line.is_empty() {
            continue;
        }
        if godebug_block {
            let (entry, closes_block) = line
                .split_once(')')
                .map_or((line, false), |(before, _)| (before.trim(), true));
            if !entry.is_empty() {
                result.godebug.push(entry.to_string());
            }
            if closes_block {
                godebug_block = false;
            }
            continue;
        }
        let mut directive = line.splitn(2, char::is_whitespace);
        let name = directive.next().unwrap_or_default();
        let value = directive.next().map(str::trim).unwrap_or_default();
        if name == "toolchain" && !value.is_empty() {
            result.toolchain = Some(value.to_string());
        } else if name == "godebug" && !value.is_empty() {
            if let Some(block) = value.strip_prefix('(') {
                let (entry, closes_block) = block
                    .split_once(')')
                    .map_or((block.trim(), false), |(before, _)| (before.trim(), true));
                if !entry.is_empty() {
                    result.godebug.push(entry.to_string());
                }
                godebug_block = !closes_block;
            } else {
                result.godebug.push(value.to_string());
            }
        }
    }
    if godebug_block {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go workspace has an unterminated godebug block.".to_string(),
        ));
    }
    Ok(result)
}

fn quote_go_workspace_path(path: &Path) -> Result<String, GoSemanticProcessError> {
    let command_path = go_command_path(path)?;
    let text = command_path.to_str().ok_or_else(|| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "local Go workspace path `{}` is not representable as Unicode.",
            path.display()
        ))
    })?;
    serde_json::to_string(text).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to encode a local Go workspace path: {error}"
        ))
    })
}

#[cfg(not(windows))]
pub(super) fn go_command_path(path: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    Ok(path.to_path_buf())
}

#[cfg(windows)]
pub(super) fn go_command_path(path: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    crate::go::semantic::windows::go_command_path(path).map_err(|error| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "local path `{}` is not representable at the Go command boundary: {error}",
            path.display()
        ))
    })
}

#[cfg(not(windows))]
pub(super) fn go_command_working_directory(path: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    Ok(path.to_path_buf())
}

#[cfg(windows)]
pub(super) fn go_command_working_directory(path: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    crate::go::semantic::windows::go_command_working_directory(path).map_err(|error| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "local path `{}` is not usable as a Go command working directory: {error}",
            path.display()
        ))
    })
}

fn go_module_spec(module: &GoWorkEditModule) -> String {
    if module.version.is_empty() {
        module.path.clone()
    } else {
        format!("{}@{}", module.path, module.version)
    }
}

fn certified_workspace_local_path(
    workspace_directory: &Path,
    raw: &str,
    root: &Path,
    require_go_mod: bool,
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    let path = Path::new(raw);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_directory.join(path)
    };
    let path = lexically_normalized_absolute_path(&path).ok_or_else(|| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "local Go workspace path `{}` has invalid lexical components.",
            path.display()
        ))
    })?;
    require_local_existing_path_until(&path, deadline, "local Go workspace path certification")?;
    let path = fs::canonicalize(&path).map_err(|error| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "failed to resolve local Go workspace path `{}`: {error}",
            path.display()
        ))
    })?;
    if !path.starts_with(root) {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "Go workspace contains an external local path `{}`.",
            path.display()
        )));
    }
    require_local_existing_path_until(&path, deadline, "local Go workspace path certification")?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "failed to inspect local Go workspace path `{}`: {error}",
            path.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "local Go workspace path `{}` must resolve to a regular directory.",
            path.display()
        )));
    }
    if require_go_mod {
        let go_mod = path.join("go.mod");
        require_local_existing_path_until(
            &go_mod,
            deadline,
            "local Go workspace path certification",
        )?;
        let metadata = fs::symlink_metadata(&go_mod).map_err(|error| {
            GoSemanticProcessError::CommandUnavailable(format!(
                "Go workspace module `{}` has no safe go.mod: {error}",
                path.display()
            ))
        })?;
        if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
            return Err(GoSemanticProcessError::CommandUnavailable(format!(
                "Go workspace module `{}` has no safe regular go.mod.",
                path.display()
            )));
        }
    }
    Ok(path)
}

fn lexically_normalized_absolute_path(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::absolute(path).ok()?
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            std::path::Component::Normal(part) => normalized.push(part),
        }
    }
    Some(normalized)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Dependency population keeps every sealed cache path, analysis input, and absolute deadline explicit."
)]
fn populate_dependency_snapshot(
    module_cache: &Path,
    toolchain: &PreparedGoToolchain,
    root: &Path,
    config: &GoAnalysisConfig,
    workspace: &PrivateGoWorkspace,
    repository_cache_roots: &[PathBuf],
    before: &DependencyPopulationInputs,
    deadline: GoOperationDeadline,
) -> Result<PreparedWorkspace, GoSemanticProcessError> {
    let proxy = dependency_population_proxy(toolchain, config.offline, deadline)?;
    workspace.verify_replacement_manifest_inputs(root, deadline)?;
    let local_inputs = capture_local_dependency_inputs(
        module_cache,
        workspace.path(),
        toolchain,
        root,
        config,
        repository_cache_roots,
        workspace.analysis_roots(),
        GoPackageListingMode::Populate(proxy),
        deadline,
    )?;
    workspace.verify_replacement_manifest_inputs(root, deadline)?;
    let after = DependencyPopulationInputs {
        manifest_digest: dependency_manifest_digest(
            root,
            config,
            workspace.analysis_roots(),
            deadline,
        )?,
        source_digest: dependency_population_source_digest(
            root,
            workspace.analysis_roots(),
            repository_cache_roots,
            deadline,
        )?,
    };
    if before != &after {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go dependency inputs changed while the private snapshot was populated.".to_string(),
        ));
    }
    let path = workspace.path().ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(
            "private Go workspace path is unavailable after dependency population.".to_string(),
        )
    })?;
    let directory = path.parent().ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(
            "private Go workspace has no parent directory.".to_string(),
        )
    })?;
    seal_dependency_tree(directory, deadline)?;
    let digest = private_workspace_digest(path, deadline)?;
    Ok(PreparedWorkspace {
        relative_path: PathBuf::from("workspace/go.work"),
        digest,
        local_inputs,
    })
}

#[derive(Debug)]
struct PreparedWorkspace {
    relative_path: PathBuf,
    digest: String,
    local_inputs: LocalDependencyInputs,
}

#[derive(Debug, Eq, PartialEq)]
struct DependencyPopulationInputs {
    manifest_digest: String,
    source_digest: String,
}

fn private_workspace_digest(
    path: &Path,
    deadline: GoOperationDeadline,
) -> Result<String, GoSemanticProcessError> {
    let mut hasher = Sha256::new();
    hasher.update(b"polint-go-private-workspace-v1");
    let work = read_regular_file_no_follow_until(path, deadline)?;
    hash_length_prefixed(&mut hasher, &work);
    let mut sum_value = path.as_os_str().to_os_string();
    sum_value.push(".sum");
    let sum_path = PathBuf::from(sum_value);
    match fs::symlink_metadata(&sum_path) {
        Ok(metadata) if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "private Go workspace sum `{}` must be a regular file.",
                sum_path.display()
            )));
        }
        Ok(_) => {
            let sum = read_regular_file_no_follow_until(&sum_path, deadline)?;
            hash_length_prefixed(&mut hasher, &sum);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            hasher.update(b"missing-sum");
        }
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect private Go workspace sum `{}`: {error}",
                sum_path.display()
            )));
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn private_workspace_work_digest(
    path: &Path,
    deadline: GoOperationDeadline,
) -> Result<String, GoSemanticProcessError> {
    let mut hasher = Sha256::new();
    hasher.update(b"polint-go-private-workspace-work-v1");
    hash_length_prefixed(
        &mut hasher,
        &read_regular_file_no_follow_until(path, deadline)?,
    );
    Ok(format!("{:x}", hasher.finalize()))
}

fn verify_dependency_snapshot_payload(
    destination: &Path,
    request_key: &str,
    expected_workspace_work_digest: Option<&str>,
    deadline: GoOperationDeadline,
) -> Result<VerifiedDependencySnapshotPayload, GoSemanticProcessError> {
    let request = read_regular_file_no_follow_until(&destination.join("request"), deadline)?;
    if request != request_key.as_bytes() {
        return Err(GoSemanticProcessError::CommandFailed(
            "sealed Go dependency snapshot request binding failed verification.".to_string(),
        ));
    }
    let payload = read_regular_file_no_follow_until(&destination.join("payload.json"), deadline)?;
    let payload: DependencySnapshotPayloadStamp =
        serde_json::from_slice(&payload).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to decode sealed Go dependency snapshot payload: {error}"
            ))
        })?;
    if payload.request_key != request_key
        || payload.workspace_work_digest.as_deref() != expected_workspace_work_digest
    {
        return Err(GoSemanticProcessError::CommandFailed(
            "sealed Go dependency snapshot payload does not match the current request.".to_string(),
        ));
    }
    match expected_workspace_work_digest {
        Some(expected) => {
            let workspace = destination.join("workspace/go.work");
            if private_workspace_work_digest(&workspace, deadline)? != expected {
                return Err(GoSemanticProcessError::CommandFailed(
                    "sealed Go dependency snapshot workspace changed after publication."
                        .to_string(),
                ));
            }
            let Some(expected_final) = payload.workspace_final_digest.as_deref() else {
                return Err(GoSemanticProcessError::CommandFailed(
                    "sealed Go dependency snapshot is missing its final workspace binding."
                        .to_string(),
                ));
            };
            if private_workspace_digest(&workspace, deadline)? != expected_final {
                return Err(GoSemanticProcessError::CommandFailed(
                    "sealed Go dependency snapshot workspace payload failed verification."
                        .to_string(),
                ));
            }
        }
        None if payload.workspace_final_digest.is_some() => {
            return Err(GoSemanticProcessError::CommandFailed(
                "empty Go dependency snapshot unexpectedly contains a workspace binding."
                    .to_string(),
            ));
        }
        None => {}
    }
    let module_closure = capture_dependency_closure(&destination.join("modules"), deadline)?;
    if module_closure.content_digest != payload.module_content_digest
        || module_closure.entry_count != payload.module_entry_count
        || module_closure.byte_count != payload.module_byte_count
    {
        return Err(GoSemanticProcessError::CommandFailed(
            "sealed Go dependency module-cache payload failed verification.".to_string(),
        ));
    }
    Ok(VerifiedDependencySnapshotPayload {
        stamp: payload,
        module_closure,
    })
}

fn dependency_snapshot_content_digest(payload: &DependencySnapshotPayloadStamp) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"polint-go-dependency-payload-content-v2");
    hash_length_prefixed(&mut hasher, payload.module_content_digest.as_bytes());
    hasher.update(
        u64::try_from(payload.module_entry_count)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(payload.module_byte_count.to_le_bytes());
    format!("{:x}", hasher.finalize())
}

fn empty_dependency_snapshot_request_key(toolchain: &PreparedGoToolchain) -> String {
    security_digest_strings(&[
        "polint-go-empty-dependency-request-v1".to_string(),
        toolchain.executable_digest.clone(),
        toolchain.closure.content_digest.clone(),
        GO_ENVIRONMENT_POLICY.to_string(),
    ])
}

fn dependency_manifest_digest(
    root: &Path,
    config: &GoAnalysisConfig,
    analysis_roots: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<String, GoSemanticProcessError> {
    let mut paths = vec![root.join("go.work"), root.join("go.work.sum")];
    for module_root in &config.module_roots {
        let module_root = root.join(module_root);
        paths.push(module_root.join("go.mod"));
        paths.push(module_root.join("go.sum"));
    }
    for analysis_root in analysis_roots {
        paths.push(analysis_root.join("go.mod"));
        paths.push(analysis_root.join("go.sum"));
    }
    paths.sort();
    paths.dedup();
    let mut hasher = Sha256::new();
    hasher.update(b"polint-go-dependency-manifests-v1");
    for path in paths {
        deadline.check("Go dependency manifest certification")?;
        let relative = path.strip_prefix(root).unwrap_or(&path);
        hash_length_prefixed(&mut hasher, &os_string_bytes(relative.as_os_str()));
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "Go dependency manifest `{}` must be a regular file.",
                        path.display()
                    )));
                }
                if metadata.len() > GO_DEPENDENCY_MANIFEST_MAX_BYTES {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "Go dependency manifest `{}` exceeds the {}-byte limit.",
                        path.display(),
                        GO_DEPENDENCY_MANIFEST_MAX_BYTES
                    )));
                }
                let bytes = read_regular_file_no_follow_until(&path, deadline)?;
                deadline.check("Go dependency manifest certification")?;
                hash_length_prefixed(&mut hasher, &bytes);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                hasher.update(b"missing");
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go dependency manifest `{}`: {error}",
                    path.display()
                )));
            }
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn dependency_snapshot_request_key(
    toolchain: &PreparedGoToolchain,
    root: &Path,
    config: &GoAnalysisConfig,
    workspace: &PrivateGoWorkspace,
    manifest_digest: &str,
    source_digest: &str,
    deadline: GoOperationDeadline,
) -> Result<String, GoSemanticProcessError> {
    let workspace_path = workspace.path().ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(
            "private Go workspace path is unavailable while binding dependency inputs.".to_string(),
        )
    })?;
    let workspace_digest = private_workspace_digest(workspace_path, deadline)?;
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize Go analysis root for dependency binding: {error}"
        ))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(b"polint-go-dependency-request-v2");
    hash_length_prefixed(&mut hasher, &os_string_bytes(canonical_root.as_os_str()));
    hash_length_prefixed(&mut hasher, manifest_digest.as_bytes());
    hash_length_prefixed(&mut hasher, source_digest.as_bytes());
    hash_length_prefixed(&mut hasher, workspace_digest.as_bytes());
    hash_go_analysis_semantic_config(&mut hasher, config);
    hash_length_prefixed(&mut hasher, toolchain.version.as_bytes());
    hash_length_prefixed(&mut hasher, toolchain.executable_digest.as_bytes());
    hash_length_prefixed(&mut hasher, toolchain.closure.content_digest.as_bytes());
    hash_length_prefixed(&mut hasher, GO_ENVIRONMENT_POLICY.as_bytes());
    // Acquisition policy stays outside the reusable payload key, so an online
    // run can populate the private per-user cache for the same offline analysis.
    // Package selection is bound because population follows the package graph
    // that the semantic frontend will actually load.
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Debug)]
struct DependencyPopulationProxy {
    value: OsString,
    local_root: Option<PathBuf>,
}

#[derive(Debug)]
enum GoPackageListingMode {
    Populate(DependencyPopulationProxy),
    Verify,
}

impl DependencyPopulationProxy {
    fn into_frontend_build_inputs(self, source_dir: &Path) -> (OsString, Vec<PathBuf>) {
        let mut local_trees = vec![source_dir.to_path_buf()];
        local_trees.extend(self.local_root);
        (self.value, local_trees)
    }
}

fn dependency_population_proxy(
    toolchain: &PreparedGoToolchain,
    offline: bool,
    deadline: GoOperationDeadline,
) -> Result<DependencyPopulationProxy, GoSemanticProcessError> {
    if offline {
        return Ok(DependencyPopulationProxy {
            value: OsString::from("off"),
            local_root: None,
        });
    }
    let configured = toolchain.environment.variables.get("GOPROXY");
    if let Some(configured) = configured {
        validate_goproxy_environment(configured)?;
    }
    let configured = configured
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty() && *value != "off");
    let local_root = certified_ambient_module_download_cache(deadline)?;
    compose_dependency_population_proxy(configured, local_root)
}

fn compose_dependency_population_proxy(
    configured: Option<&str>,
    local_root: Option<PathBuf>,
) -> Result<DependencyPopulationProxy, GoSemanticProcessError> {
    let local = local_root.as_deref().map(file_proxy_url).transpose()?;
    let value = match (local, configured) {
        (Some(local), Some(configured)) => format!("{local}|{configured}"),
        (Some(local), None) => local,
        (None, Some(configured)) => configured.to_string(),
        (None, None) => "off".to_string(),
    };
    Ok(DependencyPopulationProxy {
        value: OsString::from(value),
        local_root,
    })
}

fn ambient_module_download_cache() -> Option<PathBuf> {
    let module_cache = std::env::var_os("GOMODCACHE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("GOPATH")
                .and_then(|value| std::env::split_paths(&value).next())
                .map(|path| path.join("pkg/mod"))
        })
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join("go/pkg/mod")))?;
    Some(module_cache.join("cache/download"))
}

fn certified_ambient_module_download_cache(
    deadline: GoOperationDeadline,
) -> Result<Option<PathBuf>, GoSemanticProcessError> {
    let Some(download) = ambient_module_download_cache() else {
        return Ok(None);
    };
    certify_ambient_module_download_cache_candidate(download, deadline)
}

fn certify_ambient_module_download_cache_candidate(
    download: PathBuf,
    deadline: GoOperationDeadline,
) -> Result<Option<PathBuf>, GoSemanticProcessError> {
    deadline.check("ambient Go module cache certification")?;
    if require_local_creation_root_until(
        &download,
        deadline,
        "ambient Go module cache certification",
    )
    .is_err()
    {
        return Ok(None);
    }
    deadline.check("ambient Go module cache certification")?;
    let metadata = match fs::symlink_metadata(&download) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Ok(None);
    }
    if require_local_scan_root(&download, deadline).is_err() {
        return Ok(None);
    }
    deadline.check("ambient Go module cache certification")?;
    Ok(Some(download))
}

#[cfg(unix)]
fn file_proxy_url(path: &Path) -> Result<String, GoSemanticProcessError> {
    use std::os::unix::ffi::OsStrExt;

    let canonical = fs::canonicalize(path).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize local Go module proxy `{}`: {error}",
            path.display()
        ))
    })?;
    let mut value = String::from("file://");
    for byte in canonical.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            value.push(char::from(*byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut value, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    Ok(value)
}

#[cfg(windows)]
fn file_proxy_url(path: &Path) -> Result<String, GoSemanticProcessError> {
    crate::go::semantic::windows::file_url(path).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to construct local Go module proxy URL for `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(all(not(unix), not(windows)))]
fn file_proxy_url(_path: &Path) -> Result<String, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "local Go module file proxies are unavailable on this platform.".to_string(),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ListedGoPackage {
    #[serde(default)]
    import_path: String,
    dir: Option<PathBuf>,
    module: Option<ListedGoPackageModule>,
    #[serde(default)]
    go_files: Vec<String>,
    #[serde(default)]
    cgo_files: Vec<String>,
    #[serde(default)]
    ignored_go_files: Vec<String>,
    #[serde(default)]
    invalid_go_files: Vec<String>,
    #[serde(default)]
    c_files: Vec<String>,
    #[serde(default)]
    cxx_files: Vec<String>,
    #[serde(default)]
    m_files: Vec<String>,
    #[serde(default)]
    h_files: Vec<String>,
    #[serde(default)]
    f_files: Vec<String>,
    #[serde(default)]
    s_files: Vec<String>,
    #[serde(default)]
    swig_files: Vec<String>,
    #[serde(default)]
    swig_cxx_files: Vec<String>,
    #[serde(default)]
    syso_files: Vec<String>,
    #[serde(default)]
    test_go_files: Vec<String>,
    #[serde(default)]
    x_test_go_files: Vec<String>,
    #[serde(default)]
    ignored_other_files: Vec<String>,
    #[serde(default)]
    embed_files: Vec<String>,
    #[serde(default)]
    test_embed_files: Vec<String>,
    #[serde(default)]
    x_test_embed_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ListedGoPackageModule {
    #[serde(default)]
    path: String,
    #[serde(default)]
    main: bool,
    #[serde(default)]
    version: String,
    dir: Option<PathBuf>,
    go_mod: Option<PathBuf>,
    #[serde(default)]
    go_version: String,
    #[serde(default)]
    toolchain: String,
    replace: Option<Box<ListedGoPackageModule>>,
}

#[derive(Debug, Eq, PartialEq)]
struct ListedGoPackageModuleStamp {
    path: String,
    version: String,
    go_version: String,
    toolchain: String,
    main: bool,
    replaced: bool,
    selected_path: String,
    selected_version: String,
    location: ListedGoPackageModuleLocation,
}

#[derive(Debug, Eq, PartialEq)]
enum ListedGoPackageModuleLocation {
    Local(Vec<u8>),
    SealedCache,
}

impl ListedGoPackage {
    fn source_files(&self) -> impl Iterator<Item = &str> {
        self.go_files
            .iter()
            .chain(&self.cgo_files)
            .chain(&self.ignored_go_files)
            .chain(&self.invalid_go_files)
            .chain(&self.c_files)
            .chain(&self.cxx_files)
            .chain(&self.m_files)
            .chain(&self.h_files)
            .chain(&self.f_files)
            .chain(&self.s_files)
            .chain(&self.swig_files)
            .chain(&self.swig_cxx_files)
            .chain(&self.syso_files)
            .chain(&self.test_go_files)
            .chain(&self.x_test_go_files)
            .chain(&self.ignored_other_files)
            .chain(&self.embed_files)
            .chain(&self.test_embed_files)
            .chain(&self.x_test_embed_files)
            .map(String::as_str)
    }
}

fn certify_listed_package_module(
    module: &ListedGoPackageModule,
    root: &Path,
    module_cache: &Path,
    repository_cache_roots: &[PathBuf],
    files: &mut BTreeSet<PathBuf>,
) -> Result<ListedGoPackageModuleStamp, GoSemanticProcessError> {
    let selected = module.replace.as_deref().unwrap_or(module);
    if module.path.is_empty() || selected.path.is_empty() {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go dependency package graph reported a module without an identity.".to_string(),
        ));
    }
    let selected_directory = selected.dir.as_deref().ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(format!(
            "Go dependency package graph reported module `{}` without a source directory.",
            module.path
        ))
    })?;
    let directory = canonical_dependency_input_path(selected_directory, true)?;
    let local = module.main || selected.version.is_empty();
    let location = if local {
        if !directory.starts_with(root) {
            return Err(GoSemanticProcessError::CommandUnavailable(format!(
                "Go dependency graph contains an external local module `{}`.",
                directory.display()
            )));
        }
        reject_repository_cache_overlap(&directory, repository_cache_roots, "local Go module")?;
        let go_mod = selected
            .go_mod
            .clone()
            .unwrap_or_else(|| directory.join("go.mod"));
        insert_existing_local_input(&go_mod, root, files)?;
        insert_existing_local_input(&go_mod.with_file_name("go.sum"), root, files)?;
        ListedGoPackageModuleLocation::Local(os_string_bytes(
            directory
                .strip_prefix(root)
                .unwrap_or(&directory)
                .as_os_str(),
        ))
    } else {
        if !directory.starts_with(module_cache) {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "versioned Go dependency `{}` escaped the private module snapshot.",
                directory.display()
            )));
        }
        ListedGoPackageModuleLocation::SealedCache
    };
    Ok(ListedGoPackageModuleStamp {
        path: module.path.clone(),
        version: module.version.clone(),
        go_version: module.go_version.clone(),
        toolchain: module.toolchain.clone(),
        main: module.main,
        replaced: module.replace.is_some(),
        selected_path: selected.path.clone(),
        selected_version: selected.version.clone(),
        location,
    })
}

fn go_module_manifest_inputs(
    analysis_roots: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<Vec<PathBuf>, GoSemanticProcessError> {
    const LABEL: &str = "Go module manifest input certification";

    validate_local_path_batch(analysis_roots, LABEL, Some(deadline))?;
    let mut manifests = Vec::new();
    for root in analysis_roots {
        deadline.check(LABEL)?;
        for (name, required) in [("go.mod", true), ("go.sum", false)] {
            let path = root.join(name);
            if required {
                require_local_existing_path_until(&path, deadline, LABEL)?;
            } else {
                require_local_creation_root_until(&path, deadline, LABEL)?;
            }
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if !required && error.kind() == std::io::ErrorKind::NotFound => {
                    continue;
                }
                Err(error) => {
                    return Err(GoSemanticProcessError::CommandUnavailable(format!(
                        "failed to inspect Go module manifest input `{}`: {error}",
                        path.display()
                    )));
                }
            };
            if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
                return Err(GoSemanticProcessError::CommandUnavailable(format!(
                    "Go module manifest input `{}` must be a direct regular file.",
                    path.display()
                )));
            }
            require_local_existing_path_until(&path, deadline, LABEL)?;
            manifests.push(path);
        }
    }
    manifests.sort();
    manifests.dedup();
    Ok(manifests)
}

fn certified_repository_cache_relatives(
    root: &Path,
    repository_cache_roots: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<Vec<PathBuf>, GoSemanticProcessError> {
    let mut relatives = Vec::new();
    for cache_root in repository_cache_roots {
        require_local_creation_root_until(
            cache_root,
            deadline,
            "repository cache-root certification",
        )?;
        let Ok(metadata) = fs::symlink_metadata(cache_root) else {
            continue;
        };
        if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
            continue;
        }
        let Ok(cache_root) = fs::canonicalize(cache_root) else {
            continue;
        };
        let Ok(relative) = cache_root.strip_prefix(root) else {
            continue;
        };
        if !relative.as_os_str().is_empty() {
            relatives.push(relative.to_path_buf());
        }
    }
    relatives.sort();
    relatives.dedup();
    Ok(relatives)
}

fn dependency_population_source_digest(
    root: &Path,
    analysis_roots: &[PathBuf],
    repository_cache_roots: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<String, GoSemanticProcessError> {
    const LABEL: &str = "Go dependency population source binding";

    validate_local_path_size_until(root, LABEL, deadline)?;
    validate_local_path_batch(analysis_roots, LABEL, Some(deadline))?;
    validate_local_path_batch(repository_cache_roots, LABEL, Some(deadline))?;
    #[cfg(windows)]
    {
        let root = root.to_path_buf();
        let analysis_roots = analysis_roots.to_vec();
        let repository_cache_roots = repository_cache_roots.to_vec();
        return run_windows_file_io_certification(deadline, LABEL, move || {
            dependency_population_source_digest_inner(
                &root,
                &analysis_roots,
                &repository_cache_roots,
                deadline,
            )
        });
    }
    #[cfg(not(windows))]
    dependency_population_source_digest_inner(
        root,
        analysis_roots,
        repository_cache_roots,
        deadline,
    )
}

fn dependency_population_source_digest_inner(
    root: &Path,
    analysis_roots: &[PathBuf],
    repository_cache_roots: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<String, GoSemanticProcessError> {
    const LABEL: &str = "Go dependency population source binding";

    deadline.check(LABEL)?;
    require_local_existing_path_until(root, deadline, LABEL)?;
    let root = fs::canonicalize(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize Go analysis root for source binding: {error}"
        ))
    })?;
    let repository_cache_relatives =
        certified_repository_cache_relatives(&root, repository_cache_roots, deadline)?;
    let repository_cache_roots = repository_cache_relatives
        .iter()
        .map(|relative| root.join(relative))
        .collect::<Vec<_>>();
    let mut certified_analysis_roots = Vec::new();
    certified_analysis_roots
        .try_reserve_exact(analysis_roots.len())
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to allocate bounded Go dependency source-root state: {error}"
            ))
        })?;
    for analysis_root in analysis_roots {
        deadline.check(LABEL)?;
        let analysis_root = canonical_dependency_input_path(analysis_root, true)?;
        if !analysis_root.starts_with(&root) {
            return Err(GoSemanticProcessError::CommandUnavailable(format!(
                "Go dependency source root `{}` escaped `{}`.",
                analysis_root.display(),
                root.display()
            )));
        }
        reject_repository_cache_overlap(
            &analysis_root,
            &repository_cache_roots,
            "Go dependency source root",
        )?;
        certified_analysis_roots.push(analysis_root);
    }
    certified_analysis_roots.sort();
    certified_analysis_roots.dedup();
    let scan_roots = collapse_nested_local_roots(&certified_analysis_roots, deadline)?;
    for scan_root in &scan_roots {
        require_local_dependency_tree_mounts(scan_root, deadline)?;
    }

    // Package patterns may explicitly select directories that Go wildcard
    // expansion normally ignores, and repo-local replacements can live outside
    // configured module roots. Bind every local Go source and module/workspace
    // manifest that could alter the package graph; repository caches are the
    // only traversal exclusions.
    let mut frontier = scan_roots
        .into_iter()
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    let mut files = BTreeSet::new();
    let mut entry_count = 0_usize;
    let mut retained_path_units = 0_usize;
    for scan_root in &frontier {
        retain_dependency_population_path(
            scan_root,
            &mut entry_count,
            &mut retained_path_units,
            deadline,
        )?;
    }
    let mut enumerated_count = 0_usize;
    while let Some(directory) = frontier.pop() {
        deadline.check(LABEL)?;
        if repository_cache_roots
            .iter()
            .any(|cache_root| directory.starts_with(cache_root))
        {
            continue;
        }
        let depth = directory
            .strip_prefix(&root)
            .map(|relative| relative.components().count())
            .unwrap_or(GO_LOCAL_DEPENDENCY_MAX_DEPTH.saturating_add(1));
        if depth > GO_LOCAL_DEPENDENCY_MAX_DEPTH {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go dependency population source depth exceeds {GO_LOCAL_DEPENDENCY_MAX_DEPTH}."
            )));
        }
        let entries = fs::read_dir(&directory).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to enumerate Go dependency source directory `{}`: {error}",
                directory.display()
            ))
        })?;
        for entry in entries {
            deadline.check(LABEL)?;
            enumerated_count = enumerated_count.checked_add(1).ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "Go dependency source enumeration accounting overflowed.".to_string(),
                )
            })?;
            if enumerated_count > GO_DEPENDENCY_SOURCE_MAX_ENUMERATED_ENTRIES {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go dependency source enumeration contains more than {GO_DEPENDENCY_SOURCE_MAX_ENUMERATED_ENTRIES} entries."
                )));
            }
            let entry = entry.map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to enumerate a Go dependency source entry under `{}`: {error}",
                    directory.display()
                ))
            })?;
            let path = entry.path();
            if repository_cache_roots
                .iter()
                .any(|cache_root| path.starts_with(cache_root))
            {
                continue;
            }
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go dependency source entry `{}`: {error}",
                    path.display()
                ))
            })?;
            if metadata_is_link_or_reparse(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                retain_dependency_population_path(
                    &path,
                    &mut entry_count,
                    &mut retained_path_units,
                    deadline,
                )?;
                frontier.push(path);
            } else if metadata.is_file() && is_dependency_population_input(&path) {
                retain_dependency_population_path(
                    &path,
                    &mut entry_count,
                    &mut retained_path_units,
                    deadline,
                )?;
                files.insert(path);
            }
        }
    }

    let mut hasher = Sha256::new();
    hasher.update(b"polint-go-dependency-package-inputs-v1");
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to initialize Go dependency source parser: {error}"
            ))
        })?;
    let mut byte_count = 0_u64;
    for path in files {
        deadline.check(LABEL)?;
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go dependency source `{}`: {error}",
                path.display()
            ))
        })?;
        if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
            return Err(GoSemanticProcessError::CommandUnavailable(format!(
                "Go dependency source `{}` must remain a direct regular file.",
                path.display()
            )));
        }
        byte_count = byte_count.checked_add(metadata.len()).ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "Go dependency source byte accounting overflowed.".to_string(),
            )
        })?;
        if byte_count > GO_DEPENDENCY_MAX_BYTES {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go dependency source graph exceeds the {GO_DEPENDENCY_MAX_BYTES}-byte limit."
            )));
        }
        let relative = path.strip_prefix(&root).map_err(|_| {
            GoSemanticProcessError::CommandFailed(format!(
                "Go dependency source `{}` escaped its analysis root.",
                path.display()
            ))
        })?;
        hash_length_prefixed(&mut hasher, &os_string_bytes(relative.as_os_str()));
        if path.extension().and_then(|extension| extension.to_str()) == Some("go") {
            if metadata.len() > GO_DEPENDENCY_SOURCE_MAX_FILE_BYTES {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go dependency source `{}` exceeds the {GO_DEPENDENCY_SOURCE_MAX_FILE_BYTES}-byte per-file limit.",
                    path.display()
                )));
            }
            let bytes = read_regular_file_no_follow_until(&path, deadline)?;
            hash_go_dependency_source_signature(&mut hasher, &mut parser, &bytes, deadline)?;
        } else {
            let bytes = read_regular_file_no_follow_until(&path, deadline)?;
            hasher.update(b"manifest");
            hash_length_prefixed(&mut hasher, &bytes);
        }
    }
    deadline.check(LABEL)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn retain_dependency_population_path(
    path: &Path,
    entry_count: &mut usize,
    retained_path_units: &mut usize,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    const LABEL: &str = "Go dependency population source binding";

    validate_local_path_size_until(path, LABEL, deadline)?;
    *entry_count = entry_count.checked_add(1).ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(
            "Go dependency source entry accounting overflowed.".to_string(),
        )
    })?;
    if *entry_count > GO_DEPENDENCY_MAX_ENTRIES {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go dependency source graph contains more than {GO_DEPENDENCY_MAX_ENTRIES} directories and package-driving files."
        )));
    }
    *retained_path_units = retained_path_units
        .checked_add(local_path_storage_units(path))
        .ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "Go dependency source path-state accounting overflowed.".to_string(),
            )
        })?;
    if *retained_path_units > GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go dependency source path state exceeds {GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS} storage units."
        )));
    }
    Ok(())
}

fn hash_go_dependency_source_signature(
    hasher: &mut Sha256,
    parser: &mut tree_sitter::Parser,
    source: &[u8],
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    hasher.update(b"go-source-dependency-signature-v1");
    let mut progress = |_state: &tree_sitter::ParseState| {
        if Instant::now() >= deadline.end {
            std::ops::ControlFlow::Break(())
        } else {
            std::ops::ControlFlow::Continue(())
        }
    };
    let options = tree_sitter::ParseOptions::new().progress_callback(&mut progress);
    let mut read = |offset: usize, _point: tree_sitter::Point| &source[offset..];
    let tree = parser
        .parse_with_options(&mut read, None, Some(options))
        .ok_or_else(|| {
            if Instant::now() >= deadline.end {
                GoSemanticProcessError::Timeout(
                    "Go dependency source parsing exceeded its operation deadline.".to_string(),
                )
            } else {
                GoSemanticProcessError::CommandFailed(
                    "tree-sitter returned no Go dependency source parse tree.".to_string(),
                )
            }
        })?;
    deadline.check("Go dependency source parsing")?;
    let root = tree.root_node();
    if root.has_error() {
        hasher.update(b"unparsed-source");
        hash_length_prefixed(hasher, source);
        return Ok(());
    }
    let package = (0..root.named_child_count())
        .filter_map(|index| root.named_child(index as u32))
        .find(|node| node.kind() == "package_clause");
    let Some(package) = package else {
        hasher.update(b"unparsed-source");
        hash_length_prefixed(hasher, source);
        return Ok(());
    };

    let prefix = source[..package.start_byte()]
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap_or(&source[..package.start_byte()]);
    for line in prefix.split(|byte| *byte == b'\n') {
        let start = line
            .iter()
            .position(|byte| !byte.is_ascii_whitespace())
            .unwrap_or(line.len());
        let end = line
            .iter()
            .rposition(|byte| !byte.is_ascii_whitespace())
            .map_or(start, |index| index.saturating_add(1));
        let line = &line[start..end];
        if line.starts_with(b"//go:build") || line.starts_with(b"// +build") {
            hasher.update(b"build-constraint");
            hash_length_prefixed(hasher, line);
        }
    }
    hasher.update(b"package-clause");
    hash_length_prefixed(
        hasher,
        source
            .get(package.start_byte()..package.end_byte())
            .unwrap_or_default(),
    );
    for index in 0..root.named_child_count() {
        let Some(node) = root.named_child(index as u32) else {
            continue;
        };
        if node.kind() != "import_declaration" {
            continue;
        }
        hasher.update(b"import-declaration");
        hash_length_prefixed(
            hasher,
            source
                .get(node.start_byte()..node.end_byte())
                .unwrap_or_default(),
        );
    }
    Ok(())
}

fn is_dependency_population_input(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("go.mod" | "go.sum" | "go.work" | "go.work.sum")
    ) || path.extension().and_then(|extension| extension.to_str()) == Some("go")
}

#[expect(
    clippy::too_many_arguments,
    reason = "Local dependency certification keeps every trust root, selected scope, and absolute deadline explicit."
)]
fn capture_local_dependency_inputs(
    module_cache: &Path,
    workspace_path: Option<&Path>,
    toolchain: &PreparedGoToolchain,
    root: &Path,
    config: &GoAnalysisConfig,
    repository_cache_roots: &[PathBuf],
    analysis_roots: &[PathBuf],
    listing_mode: GoPackageListingMode,
    deadline: GoOperationDeadline,
) -> Result<LocalDependencyInputs, GoSemanticProcessError> {
    const LABEL: &str = "local Go dependency input certification";

    validate_local_path_size_until(module_cache, LABEL, deadline)?;
    if let Some(workspace_path) = workspace_path {
        validate_local_path_size_until(workspace_path, LABEL, deadline)?;
    }
    validate_local_path_size_until(root, LABEL, deadline)?;
    validate_local_path_batch(repository_cache_roots, LABEL, Some(deadline))?;
    validate_local_path_batch(analysis_roots, LABEL, Some(deadline))?;
    #[cfg(windows)]
    {
        let module_cache = module_cache.to_path_buf();
        let workspace_path = workspace_path.map(Path::to_path_buf);
        let toolchain = toolchain.clone();
        let root = root.to_path_buf();
        let config = config.clone();
        let repository_cache_roots = repository_cache_roots.to_vec();
        let analysis_roots = analysis_roots.to_vec();
        run_windows_file_io_certification(deadline, LABEL, move || {
            capture_local_dependency_inputs_inner(
                &module_cache,
                workspace_path.as_deref(),
                &toolchain,
                &root,
                &config,
                &repository_cache_roots,
                &analysis_roots,
                listing_mode,
                deadline,
            )
        })
    }
    #[cfg(not(windows))]
    capture_local_dependency_inputs_inner(
        module_cache,
        workspace_path,
        toolchain,
        root,
        config,
        repository_cache_roots,
        analysis_roots,
        listing_mode,
        deadline,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "The inner certification boundary mirrors the already-validated trust roots and selected scope without ambient state."
)]
fn capture_local_dependency_inputs_inner(
    module_cache: &Path,
    workspace_path: Option<&Path>,
    toolchain: &PreparedGoToolchain,
    root: &Path,
    config: &GoAnalysisConfig,
    repository_cache_roots: &[PathBuf],
    analysis_roots: &[PathBuf],
    listing_mode: GoPackageListingMode,
    deadline: GoOperationDeadline,
) -> Result<LocalDependencyInputs, GoSemanticProcessError> {
    deadline.check("local Go dependency input certification")?;
    require_local_existing_path_until(root, deadline, "local Go dependency input certification")?;
    let root = fs::canonicalize(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize Go analysis root: {error}"
        ))
    })?;
    let repository_cache_relatives =
        certified_repository_cache_relatives(&root, repository_cache_roots, deadline)?;
    let repository_cache_roots = repository_cache_relatives
        .iter()
        .map(|relative| root.join(relative))
        .collect::<Vec<_>>();
    for analysis_root in analysis_roots {
        let analysis_root = canonical_dependency_input_path(analysis_root, true)?;
        reject_repository_cache_overlap(
            &analysis_root,
            &repository_cache_roots,
            "Go analysis root",
        )?;
    }
    // Cache exclusions are traversal boundaries for dependency capture, not
    // permission to let the Go subprocess inspect an uncertified tree. Scan
    // both sides before the first graph-discovery command so explicit imports
    // beneath dot-prefixed cache paths cannot bypass local-volume checks.
    require_local_scan_roots_with_exclusions(analysis_roots, &repository_cache_roots, deadline)?;
    require_local_scan_roots_with_exclusions(&repository_cache_roots, &[], deadline)?;
    let command_root = go_command_working_directory(&root)?;
    let workspace_value = workspace_path
        .map(go_command_path)
        .transpose()?
        .unwrap_or_else(|| PathBuf::from("off"));
    let module_cache_value = go_command_path(module_cache)?;
    // Full workspace-graph commands can fetch a version that a local `use`
    // module shadows. Follow the analyzer's package graph so only source that
    // can participate in this configured analysis enters the sealed snapshot.
    let (proxy, recursive_roots, goflags, sumdb_off, label) = match listing_mode {
        GoPackageListingMode::Populate(proxy) => (
            proxy.value,
            proxy.local_root.into_iter().collect::<Vec<_>>(),
            "-mod=readonly",
            config.offline,
            "Go dependency snapshot package population",
        ),
        GoPackageListingMode::Verify => (
            OsString::from("off"),
            Vec::new(),
            "-mod=readonly",
            true,
            "local Go dependency input listing",
        ),
    };
    let mut command = Command::new(&toolchain.executable);
    configure_go_environment(&mut command, toolchain);
    command
        .arg("list")
        .arg("-deps")
        .arg(
            "-json=ImportPath,Dir,Module,GoFiles,CgoFiles,IgnoredGoFiles,InvalidGoFiles,CFiles,CXXFiles,MFiles,HFiles,FFiles,SFiles,SwigFiles,SwigCXXFiles,SysoFiles,TestGoFiles,XTestGoFiles,IgnoredOtherFiles,EmbedFiles,TestEmbedFiles,XTestEmbedFiles",
        )
        .current_dir(command_root)
        .env("GOWORK", workspace_value)
        .env("GOMODCACHE", module_cache_value)
        .env("GOPROXY", proxy)
        .env("GOVCS", "off")
        .env("GOAUTH", "off")
        .env("GONOPROXY", "none")
        .env("GOFLAGS", goflags);
    if sumdb_off {
        command.env("GOSUMDB", "off");
    }
    if config.include_tests {
        command.arg("-test");
    }
    if !config.build_tags.is_empty() {
        command.arg(format!("-tags={}", config.build_tags.join(",")));
    }
    command.args(config.rooted_package_patterns());
    let manifest_inputs = go_module_manifest_inputs(analysis_roots, deadline)?;
    let output = run_prepared_go_command_with_local_scope_until(
        toolchain,
        command,
        &manifest_inputs,
        &recursive_roots,
        BoundedCommandLimits::new(
            GO_OPERATION_TIMEOUT,
            GO_DEPENDENCY_COMMAND_STDOUT_BYTES,
            GO_DEPENDENCY_COMMAND_STDERR_BYTES,
        ),
        deadline,
        label,
    )?;
    if !output.status.success() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "{label} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let mut hasher = Sha256::new();
    hasher.update(b"polint-go-local-inputs-v2");
    hash_go_analysis_semantic_config(&mut hasher, config);
    let module_cache = fs::canonicalize(module_cache).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize private Go module cache: {error}"
        ))
    })?;
    let mut directories = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut modules = BTreeMap::new();
    let listed_packages =
        serde_json::Deserializer::from_slice(&output.stdout).into_iter::<ListedGoPackage>();
    for package in listed_packages {
        deadline.check("local Go dependency module graph certification")?;
        let package = package.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to decode local Go dependency package graph: {error}"
            ))
        })?;
        let Some(module) = package.module.as_ref() else {
            continue;
        };
        let stamp = certify_listed_package_module(
            module,
            &root,
            &module_cache,
            &repository_cache_roots,
            &mut files,
        )?;
        let key = (stamp.path.clone(), stamp.version.clone());
        if let Some(existing) = modules.get(&key) {
            if existing != &stamp {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go dependency package graph reported inconsistent metadata for `{}@{}`.",
                    stamp.path, stamp.version
                )));
            }
        } else {
            modules.insert(key, stamp);
        }
        if modules.len() > GO_DEPENDENCY_MAX_ENTRIES {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency graph contains more than {GO_DEPENDENCY_MAX_ENTRIES} modules."
            )));
        }
    }
    let module_count = modules.len();
    for module in modules.into_values() {
        hasher.update(b"module");
        hash_length_prefixed(&mut hasher, module.path.as_bytes());
        hash_length_prefixed(&mut hasher, module.version.as_bytes());
        hash_length_prefixed(&mut hasher, module.go_version.as_bytes());
        hash_length_prefixed(&mut hasher, module.toolchain.as_bytes());
        hasher.update([u8::from(module.main), u8::from(module.replaced)]);
        hash_length_prefixed(&mut hasher, module.selected_path.as_bytes());
        hash_length_prefixed(&mut hasher, module.selected_version.as_bytes());
        match module.location {
            ListedGoPackageModuleLocation::Local(relative) => {
                hasher.update(b"local-module");
                hash_length_prefixed(&mut hasher, &relative);
            }
            ListedGoPackageModuleLocation::SealedCache => {
                hasher.update(b"sealed-module-cache");
            }
        }
    }
    let packages =
        serde_json::Deserializer::from_slice(&output.stdout).into_iter::<ListedGoPackage>();
    let mut package_count = 0_usize;
    for package in packages {
        deadline.check("local Go dependency input certification")?;
        let package = package.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to decode local Go dependency package listing: {error}"
            ))
        })?;
        let Some(directory) = package.dir.as_deref() else {
            continue;
        };
        let directory = canonical_dependency_input_path(directory, true)?;
        if !directory.starts_with(&root) {
            continue;
        }
        reject_repository_cache_overlap(&directory, &repository_cache_roots, "local Go package")?;
        package_count = package_count.saturating_add(1);
        if package_count > GO_DEPENDENCY_MAX_ENTRIES {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency listing contains more than {GO_DEPENDENCY_MAX_ENTRIES} packages."
            )));
        }
        hash_length_prefixed(&mut hasher, package.import_path.as_bytes());
        hash_length_prefixed(
            &mut hasher,
            &os_string_bytes(
                directory
                    .strip_prefix(&root)
                    .unwrap_or(&directory)
                    .as_os_str(),
            ),
        );
        directories.insert(directory.clone());
        for source in package.source_files() {
            deadline.check("local Go dependency input certification")?;
            let source = Path::new(source);
            let source = if source.is_absolute() {
                source.to_path_buf()
            } else {
                directory.join(source)
            };
            let source = canonical_local_input_path(&source, &root, false)?;
            if source.starts_with(&root) {
                reject_repository_cache_overlap(
                    &source,
                    &repository_cache_roots,
                    "local Go package input",
                )?;
                files.insert(source);
            }
        }
        if let Some(module) = package.module.as_ref() {
            let selected = module.replace.as_deref().unwrap_or(module);
            if let Some(module_directory) = selected.dir.as_deref() {
                let module_directory = canonical_dependency_input_path(module_directory, true)?;
                if module_directory.starts_with(&root) {
                    directories.insert(module_directory);
                }
            }
            if let Some(go_mod) = selected.go_mod.as_deref() {
                insert_existing_local_input(go_mod, &root, &mut files)?;
                insert_existing_local_input(&go_mod.with_file_name("go.sum"), &root, &mut files)?;
            }
        }
    }
    for manifest in [root.join("go.work"), root.join("go.work.sum")] {
        insert_existing_local_input(&manifest, &root, &mut files)?;
    }
    for module_root in &config.module_roots {
        let module_root = root.join(module_root);
        insert_existing_local_input(&module_root.join("go.mod"), &root, &mut files)?;
        insert_existing_local_input(&module_root.join("go.sum"), &root, &mut files)?;
    }
    let selected_directories = directories.into_iter().collect::<Vec<_>>();
    let files = files.into_iter().collect::<Vec<_>>();
    reject_selected_local_inputs_in_repository_cache(
        &selected_directories,
        &files,
        &repository_cache_roots,
    )?;
    let path_seal = capture_local_dependency_path_seal(
        &root,
        &repository_cache_relatives,
        &selected_directories,
        &files,
        None,
        deadline,
    )?;
    hasher.update(b"local-path-content");
    hash_length_prefixed(&mut hasher, path_seal.content_digest.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    let file_count = files.len();
    Ok(LocalDependencyInputs {
        root,
        config: config.clone(),
        repository_cache_relatives,
        digest,
        path_content_digest: path_seal.content_digest,
        verification_digest: path_seal.metadata_digest,
        selected_directories,
        files,
        module_count,
        package_count,
        entry_count: path_seal.entry_count,
        file_count,
        byte_count: path_seal.byte_count,
    })
}

fn capture_local_dependency_path_seal(
    root: &Path,
    repository_cache_relatives: &[PathBuf],
    selected_directories: &[PathBuf],
    files: &[PathBuf],
    known_content_digest: Option<&str>,
    deadline: GoOperationDeadline,
) -> Result<LocalDependencyPathSeal, GoSemanticProcessError> {
    capture_local_dependency_path_seal_with_entry_limit(
        root,
        repository_cache_relatives,
        selected_directories,
        files,
        known_content_digest,
        GO_DEPENDENCY_MAX_ENTRIES,
        deadline,
    )
}

struct LocalDependencyDirectoryScope {
    content: BTreeSet<PathBuf>,
    traversal: BTreeSet<PathBuf>,
}

fn local_dependency_directory_scope_until(
    root: &Path,
    selected_directories: &[PathBuf],
    files: &[PathBuf],
    maximum_entries: usize,
    deadline: GoOperationDeadline,
) -> Result<LocalDependencyDirectoryScope, GoSemanticProcessError> {
    const LABEL: &str = "local Go dependency directory-scope certification";

    let selected_entries = selected_directories
        .len()
        .checked_add(files.len())
        .ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "local Go dependency selection count overflowed.".to_string(),
            )
        })?;
    if selected_entries > maximum_entries {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "local Go dependency selection contains more than {maximum_entries} entries."
        )));
    }
    validate_local_path_size_until(root, LABEL, deadline)?;
    validate_local_path_batches_with_limits(
        &[selected_directories, files],
        LABEL,
        Some(deadline),
        maximum_entries,
        GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS,
    )?;

    let maximum_retained_paths = maximum_entries
        .checked_mul(2)
        .and_then(|maximum| maximum.checked_add(2))
        .ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "local Go dependency retained-path limit overflowed.".to_string(),
            )
        })?;
    let mut retained_paths = 0_usize;
    let mut retained_units = 0_usize;
    let mut content = BTreeSet::new();
    for directory in selected_directories {
        validate_local_dependency_scope_path(root, directory, deadline)?;
        insert_local_dependency_scope_path(
            &mut content,
            directory,
            &mut retained_paths,
            &mut retained_units,
            maximum_retained_paths,
            deadline,
        )?;
    }
    for file in files {
        if !file.starts_with(root) {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency input `{}` is outside `{}`.",
                file.display(),
                root.display()
            )));
        }
        let Some(parent) = file.parent() else {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency input `{}` has no parent directory.",
                file.display()
            )));
        };
        validate_local_dependency_scope_path(root, parent, deadline)?;
        insert_local_dependency_scope_path(
            &mut content,
            parent,
            &mut retained_paths,
            &mut retained_units,
            maximum_retained_paths,
            deadline,
        )?;
    }

    let mut traversal = BTreeSet::new();
    insert_local_dependency_scope_path(
        &mut traversal,
        root,
        &mut retained_paths,
        &mut retained_units,
        maximum_retained_paths,
        deadline,
    )?;
    for directory in &content {
        for ancestor in directory.ancestors() {
            deadline.check(LABEL)?;
            if !ancestor.starts_with(root) {
                break;
            }
            validate_local_dependency_scope_path(root, ancestor, deadline)?;
            insert_local_dependency_scope_path(
                &mut traversal,
                ancestor,
                &mut retained_paths,
                &mut retained_units,
                maximum_retained_paths,
                deadline,
            )?;
        }
    }

    Ok(LocalDependencyDirectoryScope { content, traversal })
}

fn validate_local_dependency_scope_path(
    root: &Path,
    path: &Path,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    const LABEL: &str = "local Go dependency directory-scope certification";

    deadline.check(LABEL)?;
    let relative = path.strip_prefix(root).map_err(|_| {
        GoSemanticProcessError::CommandFailed(format!(
            "local Go dependency path `{}` is outside `{}`.",
            path.display(),
            root.display()
        ))
    })?;
    let depth = relative.components().count();
    deadline.check(LABEL)?;
    if depth > GO_LOCAL_DEPENDENCY_MAX_DEPTH {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "local Go dependency directory depth exceeds {GO_LOCAL_DEPENDENCY_MAX_DEPTH}."
        )));
    }
    Ok(())
}

fn insert_local_dependency_scope_path(
    paths: &mut BTreeSet<PathBuf>,
    path: &Path,
    retained_paths: &mut usize,
    retained_units: &mut usize,
    maximum_paths: usize,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    const LABEL: &str = "local Go dependency directory-scope certification";

    deadline.check(LABEL)?;
    if paths.contains(path) {
        deadline.check(LABEL)?;
        return Ok(());
    }
    *retained_paths = retained_paths.checked_add(1).ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(
            "local Go dependency path-state count overflowed.".to_string(),
        )
    })?;
    if *retained_paths > maximum_paths {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "local Go dependency path state exceeds {maximum_paths} retained paths."
        )));
    }
    *retained_units = retained_units
        .checked_add(local_path_storage_units(path))
        .ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "local Go dependency path-state byte accounting overflowed.".to_string(),
            )
        })?;
    if *retained_units > GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "local Go dependency path state exceeds the {GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS}-unit limit."
        )));
    }
    paths.insert(path.to_path_buf());
    deadline.check(LABEL)
}

fn local_repository_cache_roots_until(
    root: &Path,
    relatives: &[PathBuf],
    deadline: GoOperationDeadline,
) -> Result<Vec<PathBuf>, GoSemanticProcessError> {
    const LABEL: &str = "local Go repository-cache path certification";

    validate_local_path_batch(relatives, LABEL, Some(deadline))?;
    let mut roots = Vec::new();
    roots.try_reserve_exact(relatives.len()).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to allocate bounded repository-cache path state: {error}"
        ))
    })?;
    let mut retained_units = 0_usize;
    for relative in relatives {
        deadline.check(LABEL)?;
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "repository-cache path `{}` is not repository-relative.",
                relative.display()
            )));
        }
        let path = root.join(relative);
        validate_local_path_size_until(&path, LABEL, deadline)?;
        retained_units = retained_units
            .checked_add(local_path_storage_units(&path))
            .ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "repository-cache path accounting overflowed.".to_string(),
                )
            })?;
        if retained_units > GO_LOCAL_REPOSITORY_CACHE_PATH_UNITS {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "repository-cache paths exceed the {GO_LOCAL_REPOSITORY_CACHE_PATH_UNITS}-unit limit."
            )));
        }
        roots.push(path);
    }
    deadline.check(LABEL)?;
    roots.sort_unstable_by(|left, right| compare_local_path_components(left, right));
    deadline.check(LABEL)?;
    roots.dedup_by(|left, right| {
        compare_local_path_components(left, right) == std::cmp::Ordering::Equal
    });
    Ok(roots)
}

fn compare_local_path_components(left: &Path, right: &Path) -> std::cmp::Ordering {
    left.components()
        .map(|component| component.as_os_str())
        .cmp(right.components().map(|component| component.as_os_str()))
}

fn sorted_local_paths_contain_exact(paths: &[PathBuf], path: &Path) -> bool {
    paths
        .binary_search_by(|candidate| compare_local_path_components(candidate, path))
        .is_ok()
}

fn sorted_local_paths_contain_descendant(paths: &[PathBuf], path: &Path) -> bool {
    let index =
        paths.partition_point(|candidate| compare_local_path_components(candidate, path).is_lt());
    paths
        .get(index)
        .is_some_and(|candidate| candidate.starts_with(path))
}

fn sorted_local_paths_contain_ancestor(paths: &[PathBuf], path: &Path, root: &Path) -> bool {
    path.ancestors()
        .take_while(|ancestor| ancestor.starts_with(root))
        .any(|ancestor| sorted_local_paths_contain_exact(paths, ancestor))
}

fn capture_local_dependency_path_seal_with_entry_limit(
    root: &Path,
    repository_cache_relatives: &[PathBuf],
    selected_directories: &[PathBuf],
    files: &[PathBuf],
    known_content_digest: Option<&str>,
    maximum_entries: usize,
    deadline: GoOperationDeadline,
) -> Result<LocalDependencyPathSeal, GoSemanticProcessError> {
    const LABEL: &str = "local Go dependency path certification";

    let directory_scope = local_dependency_directory_scope_until(
        root,
        selected_directories,
        files,
        maximum_entries,
        deadline,
    )?;
    let repository_cache_roots =
        local_repository_cache_roots_until(root, repository_cache_relatives, deadline)?;
    #[cfg(windows)]
    {
        if crate::go::semantic::windows::cancellable_file_io_pass_is_active() {
            return capture_local_dependency_path_seal_on_active_windows_worker(
                root,
                repository_cache_relatives,
                &repository_cache_roots,
                files,
                &directory_scope,
                known_content_digest,
                maximum_entries,
                deadline,
            );
        }
        deadline.check(LABEL)?;
        let root = root.to_path_buf();
        deadline.check(LABEL)?;
        let repository_cache_relatives =
            clone_bounded_path_batch_until(repository_cache_relatives, LABEL, deadline)?;
        let files = clone_bounded_path_batch_until(files, LABEL, deadline)?;
        let known_content_digest = known_content_digest.map(str::to_string);
        deadline.check(LABEL)?;
        return run_windows_file_io_certification(deadline, LABEL, move || {
            capture_local_dependency_path_seal_on_active_windows_worker(
                &root,
                &repository_cache_relatives,
                &repository_cache_roots,
                &files,
                &directory_scope,
                known_content_digest.as_deref(),
                maximum_entries,
                deadline,
            )
        });
    }
    #[cfg(not(windows))]
    {
        // Mount locality must be re-established before this seal performs its
        // first dependency-entry metadata lookup or directory read.
        // Preparation-time locality is not durable across mount changes.
        require_local_dependency_tree_mounts(root, deadline)?;
        capture_local_dependency_path_seal_inner(
            root,
            repository_cache_relatives,
            &repository_cache_roots,
            files,
            &directory_scope,
            known_content_digest,
            maximum_entries,
            deadline,
        )
    }
}

#[cfg(windows)]
fn capture_local_dependency_path_seal_on_active_windows_worker(
    root: &Path,
    repository_cache_relatives: &[PathBuf],
    repository_cache_roots: &[PathBuf],
    files: &[PathBuf],
    directory_scope: &LocalDependencyDirectoryScope,
    known_content_digest: Option<&str>,
    maximum_entries: usize,
    deadline: GoOperationDeadline,
) -> Result<LocalDependencyPathSeal, GoSemanticProcessError> {
    const LABEL: &str = "local Go dependency path certification";

    deadline.check(LABEL)?;
    let mut inclusions = Vec::new();
    inclusions
        .try_reserve_exact(directory_scope.traversal.len())
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to allocate bounded local-scope inclusion state: {error}"
            ))
        })?;
    for inclusion in &directory_scope.traversal {
        deadline.check(LABEL)?;
        inclusions.push(inclusion.clone());
    }
    deadline.check(LABEL)?;
    let certified_scope = crate::go::semantic::windows::certified_local_tree_with_scope_until(
        root,
        repository_cache_roots,
        &inclusions,
        deadline.end,
    )
    .map_err(|error| {
        windows_file_io_error(
            error,
            format!(
                "failed to certify local Go dependency paths under `{}`",
                root.display()
            ),
        )
    })?;
    capture_local_dependency_path_seal_inner(
        root,
        repository_cache_relatives,
        repository_cache_roots,
        files,
        directory_scope,
        known_content_digest,
        maximum_entries,
        deadline,
        &certified_scope,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "Path sealing explicitly binds exclusions, selected files, directory scope, content identity, limits, and deadline."
)]
fn capture_local_dependency_path_seal_inner(
    root: &Path,
    repository_cache_relatives: &[PathBuf],
    repository_cache_roots: &[PathBuf],
    files: &[PathBuf],
    directory_scope: &LocalDependencyDirectoryScope,
    known_content_digest: Option<&str>,
    maximum_entries: usize,
    deadline: GoOperationDeadline,
    #[cfg(windows)] certified_scope: &crate::go::semantic::windows::CertifiedLocalTree,
) -> Result<LocalDependencyPathSeal, GoSemanticProcessError> {
    // This seal provides bounded best-effort concurrent-change detection under
    // polint's no-concurrent-local-mutation contract. It does not pin the
    // repository tree; doing that would require analyzing a private immutable
    // source snapshot rather than reopening repository paths in the Go child.
    let mut content_hasher = known_content_digest.is_none().then(|| {
        let mut hasher = Sha256::new();
        hasher.update(b"polint-go-local-path-content-v2");
        hasher
    });
    let mut metadata_hasher = Sha256::new();
    metadata_hasher.update(b"polint-go-local-path-verification-v2");
    for relative in repository_cache_relatives {
        metadata_hasher.update(b"repository-cache-exclusion");
        hash_length_prefixed(&mut metadata_hasher, &os_string_bytes(relative.as_os_str()));
    }

    let maximum_directory_state_paths = maximum_entries
        .checked_mul(2)
        .and_then(|maximum| maximum.checked_add(2))
        .ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "local Go dependency directory-state limit overflowed.".to_string(),
            )
        })?;
    let mut entry_count = 0_usize;
    let content_directories = &directory_scope.content;
    let traversal_directories = &directory_scope.traversal;
    deadline.check("local Go dependency directory certification")?;
    let mut frontier = Vec::new();
    frontier
        .try_reserve_exact(traversal_directories.len())
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to allocate bounded local Go dependency frontier: {error}"
            ))
        })?;
    let mut frontier_path_units = 0_usize;
    for directory in traversal_directories {
        deadline.check("local Go dependency directory certification")?;
        let depth = directory
            .strip_prefix(root)
            .map(|relative| relative.components().count())
            .unwrap_or(GO_LOCAL_DEPENDENCY_MAX_DEPTH.saturating_add(1));
        frontier_path_units = frontier_path_units
            .checked_add(local_path_storage_units(directory))
            .ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "local Go dependency frontier path accounting overflowed.".to_string(),
                )
            })?;
        if frontier_path_units > GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency frontier exceeds the {GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS}-unit path limit."
            )));
        }
        frontier.push((directory.clone(), depth));
    }
    deadline.check("local Go dependency directory certification")?;
    frontier.sort_by(|left, right| left.0.cmp(&right.0));
    deadline.check("local Go dependency directory certification")?;
    frontier.dedup_by(|left, right| left.0 == right.0);
    if frontier.len() > maximum_directory_state_paths {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "local Go dependency frontier exceeds the {maximum_directory_state_paths}-path limit."
        )));
    }
    frontier.reverse();
    deadline.check("local Go dependency directory certification")?;
    let mut visited = HashSet::new();
    let mut visited_path_units = 0_usize;
    while let Some((directory, depth)) = frontier.pop() {
        frontier_path_units = frontier_path_units
            .checked_sub(local_path_storage_units(&directory))
            .ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "local Go dependency frontier path accounting underflowed.".to_string(),
                )
            })?;
        deadline.check("local Go dependency directory certification")?;
        if depth > GO_LOCAL_DEPENDENCY_MAX_DEPTH {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency directory depth exceeds {GO_LOCAL_DEPENDENCY_MAX_DEPTH}."
            )));
        }
        if sorted_local_paths_contain_ancestor(repository_cache_roots, &directory, root)
            || visited.contains(&directory)
        {
            continue;
        }
        if visited.len() >= maximum_directory_state_paths {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency visited paths exceed the {maximum_directory_state_paths}-path limit."
            )));
        }
        visited_path_units = visited_path_units
            .checked_add(local_path_storage_units(&directory))
            .ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "local Go dependency visited-path accounting overflowed.".to_string(),
                )
            })?;
        if visited_path_units > GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency visited paths exceed the {GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS}-unit limit."
            )));
        }
        visited.try_reserve(1).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to grow the bounded local Go dependency visited-path index: {error}"
            ))
        })?;
        let directory_metadata = fs::symlink_metadata(&directory).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect local Go dependency directory `{}`: {error}",
                directory.display()
            ))
        })?;
        if metadata_is_link_or_reparse(&directory_metadata) || !directory_metadata.is_dir() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency directory `{}` must be a direct regular directory.",
                directory.display()
            )));
        }
        let relative = directory.strip_prefix(root).unwrap_or(&directory);
        let relative_bytes = os_string_bytes(relative.as_os_str());
        metadata_hasher.update(b"directory");
        hash_length_prefixed(&mut metadata_hasher, &relative_bytes);
        #[cfg(windows)]
        hash_local_scoped_metadata(
            &mut metadata_hasher,
            certified_scope,
            &directory,
            &directory_metadata,
        )?;
        #[cfg(not(windows))]
        hash_toolchain_metadata(&mut metadata_hasher, &directory, &directory_metadata)?;
        let children = fs::read_dir(&directory).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to enumerate local Go dependency directory `{}`: {error}",
                directory.display()
            ))
        })?;
        let directory_is_selected = content_directories.contains(&directory);
        // Cache roots consume the separately bounded scope budget and are
        // omitted from the dependency entry count. Allowing the whole bounded
        // cache-root count here avoids a per-directory linear prefix scan.
        let cache_boundary_allowance = repository_cache_roots.len();
        let mut entries = Vec::new();
        for entry in children {
            deadline.check("local Go dependency directory certification")?;
            let entry = entry.map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to enumerate a local Go dependency directory entry: {error}"
                ))
            })?;
            let name = entry.file_name();
            let entry_path = entry.path();
            if name
                .to_str()
                .is_some_and(|name| name.starts_with(".polint-dependency-"))
            {
                continue;
            }
            let file_type = entry.file_type().map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect local Go dependency entry type `{}`: {error}",
                    entry_path.display()
                ))
            })?;
            let cache_boundary_or_ancestor =
                sorted_local_paths_contain_descendant(repository_cache_roots, &entry_path);
            // `traversal_directories` is ancestor-closed, so a descendant
            // selection exists exactly when this entry is itself indexed.
            let selected_boundary_or_ancestor = traversal_directories.contains(&entry_path);
            let relevant = if cache_boundary_or_ancestor {
                true
            } else if file_type.is_dir() {
                !is_go_universe_ignored_directory(&name) || selected_boundary_or_ancestor
            } else {
                directory_is_selected || is_go_universe_marker_file(&name)
            };
            if !relevant {
                continue;
            }
            push_bounded_directory_entry(
                &mut entries,
                entry,
                entry_count,
                maximum_entries,
                cache_boundary_allowance,
                "local Go dependency directories",
            )?;
        }
        deadline.check("local Go dependency directory certification")?;
        entries.sort_by_key(fs::DirEntry::file_name);
        deadline.check("local Go dependency directory certification")?;
        let mut child_directories = Vec::new();
        for entry in entries {
            deadline.check("local Go dependency directory certification")?;
            let entry_path = entry.path();
            if sorted_local_paths_contain_exact(repository_cache_roots, &entry_path) {
                let metadata = fs::symlink_metadata(&entry_path).map_err(|error| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to inspect repository-cache exclusion `{}`: {error}",
                        entry_path.display()
                    ))
                })?;
                if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "repository-cache exclusion `{}` must remain a direct directory.",
                        entry_path.display()
                    )));
                }
                continue;
            }
            entry_count = entry_count.checked_add(1).ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "local Go dependency entry-count accounting overflowed.".to_string(),
                )
            })?;
            if entry_count > maximum_entries {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "local Go dependency directories contain more than {maximum_entries} entries."
                )));
            }
            let metadata = fs::symlink_metadata(&entry_path).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect local Go dependency entry `{}`: {error}",
                    entry_path.display()
                ))
            })?;
            let kind = if metadata.is_dir() {
                b"directory".as_slice()
            } else if metadata.is_file() {
                b"file".as_slice()
            } else if metadata_is_link_or_reparse(&metadata) {
                b"symlink".as_slice()
            } else {
                b"special".as_slice()
            };
            let relative = entry_path.strip_prefix(root).unwrap_or(&entry_path);
            hash_length_prefixed(&mut metadata_hasher, &os_string_bytes(relative.as_os_str()));
            metadata_hasher.update(kind);
            #[cfg(windows)]
            hash_local_scoped_metadata(
                &mut metadata_hasher,
                certified_scope,
                &entry_path,
                &metadata,
            )?;
            #[cfg(not(windows))]
            hash_toolchain_metadata(&mut metadata_hasher, &entry_path, &metadata)?;
            if metadata.is_dir() && !metadata_is_link_or_reparse(&metadata) {
                frontier_path_units = frontier_path_units
                    .checked_add(local_path_storage_units(&entry_path))
                    .ok_or_else(|| {
                        GoSemanticProcessError::CommandFailed(
                            "local Go dependency frontier path accounting overflowed.".to_string(),
                        )
                    })?;
                if frontier_path_units > GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "local Go dependency frontier exceeds the {GO_LOCAL_DEPENDENCY_MAX_PATH_STATE_UNITS}-unit path limit."
                    )));
                }
                child_directories.try_reserve(1).map_err(|error| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to grow the bounded local Go dependency frontier: {error}"
                    ))
                })?;
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    GoSemanticProcessError::CommandFailed(
                        "local Go dependency depth accounting overflowed.".to_string(),
                    )
                })?;
                child_directories.push((entry_path, child_depth));
            }
        }
        deadline.check("local Go dependency directory certification")?;
        child_directories.reverse();
        let next_frontier_count = frontier
            .len()
            .checked_add(child_directories.len())
            .ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "local Go dependency frontier count accounting overflowed.".to_string(),
                )
            })?;
        if next_frontier_count > maximum_directory_state_paths {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency frontier exceeds the {maximum_directory_state_paths}-path limit."
            )));
        }
        frontier
            .try_reserve(child_directories.len())
            .map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to grow the bounded local Go dependency frontier: {error}"
                ))
            })?;
        frontier.extend(child_directories);
        deadline.check("local Go dependency directory certification")?;
        if !visited.insert(directory) {
            return Err(GoSemanticProcessError::CommandFailed(
                "local Go dependency visited-path index lost uniqueness.".to_string(),
            ));
        }
    }

    let mut byte_count = 0_u64;
    for path in files {
        deadline.check("local Go dependency file certification")?;
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect local Go dependency input `{}`: {error}",
                path.display()
            ))
        })?;
        if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency input `{}` must be a regular file.",
                path.display()
            )));
        }
        byte_count = byte_count.checked_add(metadata.len()).ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "local Go dependency byte count overflowed.".to_string(),
            )
        })?;
        if byte_count > GO_DEPENDENCY_MAX_BYTES {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "local Go dependency inputs exceed the {GO_DEPENDENCY_MAX_BYTES}-byte limit."
            )));
        }
        let relative = path.strip_prefix(root).unwrap_or(path);
        let relative_bytes = os_string_bytes(relative.as_os_str());
        if let Some(content_hasher) = content_hasher.as_mut() {
            content_hasher.update(b"file");
            hash_length_prefixed(content_hasher, &relative_bytes);
            content_hasher.update(metadata.len().to_le_bytes());
            #[cfg(windows)]
            hash_dependency_file(certified_scope, path, &metadata, content_hasher, deadline)?;
            #[cfg(not(windows))]
            hash_dependency_file(path, &metadata, content_hasher, deadline)?;
        }
        metadata_hasher.update(b"file");
        hash_length_prefixed(&mut metadata_hasher, &relative_bytes);
        #[cfg(windows)]
        hash_local_scoped_metadata(&mut metadata_hasher, certified_scope, path, &metadata)?;
        #[cfg(not(windows))]
        hash_toolchain_metadata(&mut metadata_hasher, path, &metadata)?;
    }
    let content_digest = match (known_content_digest, content_hasher) {
        (Some(digest), None) => digest.to_string(),
        (None, Some(hasher)) => format!("{:x}", hasher.finalize()),
        _ => unreachable!("local dependency content capture mode is inconsistent"),
    };
    hash_length_prefixed(&mut metadata_hasher, content_digest.as_bytes());
    Ok(LocalDependencyPathSeal {
        content_digest,
        metadata_digest: format!("{:x}", metadata_hasher.finalize()),
        entry_count,
        byte_count,
    })
}

fn hash_go_analysis_semantic_config(hasher: &mut Sha256, config: &GoAnalysisConfig) {
    for values in [
        &config.module_roots,
        &config.package_patterns,
        &config.build_tags,
        &config.files_without_module_root,
    ] {
        hasher.update(
            u64::try_from(values.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for value in values {
            hash_length_prefixed(hasher, value.as_bytes());
        }
    }
    hasher.update([u8::from(config.include_tests)]);
}

fn is_go_universe_ignored_directory(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    name == "testdata" || name.starts_with('.') || name.starts_with('_')
}

fn is_go_universe_marker_file(name: &std::ffi::OsStr) -> bool {
    let path = Path::new(name);
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("go.mod" | "go.work")
    ) || path.extension().and_then(|extension| extension.to_str()) == Some("go")
}

fn canonical_local_input_path(
    path: &Path,
    root: &Path,
    expect_directory: bool,
) -> Result<PathBuf, GoSemanticProcessError> {
    let canonical = canonical_dependency_input_path(path, expect_directory)?;
    if !canonical.starts_with(root) {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "local Go dependency path `{}` escaped the analysis root.",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn reject_repository_cache_overlap(
    path: &Path,
    repository_cache_roots: &[PathBuf],
    label: &str,
) -> Result<(), GoSemanticProcessError> {
    if let Some(cache_root) = repository_cache_roots
        .iter()
        .find(|cache_root| path.starts_with(cache_root))
    {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "{label} `{}` overlaps repository-cache exclusion `{}`; move the cache outside selected Go inputs.",
            path.display(),
            cache_root.display()
        )));
    }
    Ok(())
}

fn reject_selected_local_inputs_in_repository_cache(
    selected_directories: &[PathBuf],
    files: &[PathBuf],
    repository_cache_roots: &[PathBuf],
) -> Result<(), GoSemanticProcessError> {
    for directory in selected_directories {
        reject_repository_cache_overlap(
            directory,
            repository_cache_roots,
            "selected local Go directory",
        )?;
    }
    for file in files {
        reject_repository_cache_overlap(file, repository_cache_roots, "selected local Go input")?;
    }
    Ok(())
}

fn canonical_dependency_input_path(
    path: &Path,
    expect_directory: bool,
) -> Result<PathBuf, GoSemanticProcessError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect local Go dependency path `{}`: {error}",
            path.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&metadata)
        || (expect_directory && !metadata.is_dir())
        || (!expect_directory && !metadata.is_file())
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "local Go dependency path `{}` has an unsupported file type.",
            path.display()
        )));
    }
    fs::canonicalize(path).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize local Go dependency path `{}`: {error}",
            path.display()
        ))
    })
}

fn insert_existing_local_input(
    path: &Path,
    root: &Path,
    files: &mut BTreeSet<PathBuf>,
) -> Result<(), GoSemanticProcessError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            files.insert(canonical_local_input_path(path, root, false)?);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect local Go dependency manifest `{}`: {error}",
                path.display()
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct FrontendSourceFile {
    relative_path: PathBuf,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct FrontendSourceSnapshot {
    digest: String,
    files: Vec<FrontendSourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrontendBuildProvenance {
    source_digest: String,
    toolchain_version: String,
    toolchain_executable_digest: String,
    toolchain_content_digest: String,
    host_target: GoHostTarget,
    environment_policy: &'static str,
}

impl FrontendBuildProvenance {
    fn cache_key(&self) -> String {
        security_digest_strings(&[
            format!("source_digest={}", self.source_digest),
            format!("toolchain_version={}", self.toolchain_version),
            format!(
                "toolchain_executable_digest={}",
                self.toolchain_executable_digest
            ),
            format!("toolchain_content_digest={}", self.toolchain_content_digest),
            format!("host_target={}", self.host_target.label()),
            format!("environment_policy={}", self.environment_policy),
        ])
    }

    fn stamp(&self, executable_digest: &str) -> String {
        format!(
            "source_digest={}\ntoolchain_version={}\ntoolchain_executable_digest={}\ntoolchain_content_digest={}\nhost_target={}\nenvironment_policy={}\nexecutable_digest={}\n",
            self.source_digest,
            self.toolchain_version,
            self.toolchain_executable_digest,
            self.toolchain_content_digest,
            self.host_target.label(),
            self.environment_policy,
            executable_digest
        )
    }
}

impl PreparedGoSemanticFrontend {
    #[cfg(test)]
    fn with_test_concurrency_permit(
        mut self,
        permit: Arc<TestGoSemanticConcurrencyPermit>,
    ) -> Self {
        self._test_concurrency_permit = Some(permit);
        self
    }

    pub(crate) fn certified_analysis_root(&self) -> Option<&Path> {
        self.dependency_snapshot
            .local_inputs
            .as_deref()
            .map(|inputs| inputs.root.as_path())
    }

    pub(crate) fn prepare() -> Result<Self, GoSemanticProcessError> {
        Self::prepare_until(GoOperationDeadline::after(GO_OPERATION_TIMEOUT))
    }

    pub(crate) fn prepare_until(
        deadline: GoOperationDeadline,
    ) -> Result<Self, GoSemanticProcessError> {
        #[cfg(test)]
        let test_concurrency_permit = TestGoSemanticConcurrencyPermit::acquire(deadline)?;
        #[cfg(windows)]
        let prepared = {
            run_windows_file_io_certification(
                deadline,
                "complete default Go semantic frontend preparation",
                move || {
                    require_go_semantic_process_containment()?;
                    let cache_root = default_frontend_cache_root_until(deadline)?;
                    Self::prepare_with_cache_root_until(&cache_root, deadline)
                },
            )?
        };

        #[cfg(not(windows))]
        let prepared = {
            require_go_semantic_process_containment()?;
            let cache_root = default_frontend_cache_root_until(deadline)?;
            Self::prepare_with_cache_root_until(&cache_root, deadline)?
        };
        #[cfg(test)]
        let prepared = prepared.with_test_concurrency_permit(test_concurrency_permit);
        Ok(prepared)
    }

    pub(crate) fn prepare_for_analysis_until(
        root: &Path,
        config: &GoAnalysisConfig,
        repository_cache_roots: &[PathBuf],
        deadline: GoOperationDeadline,
    ) -> Result<Self, GoSemanticProcessError> {
        const LABEL: &str = "complete Go semantic analysis frontend preparation";

        validate_local_path_size_until(root, LABEL, deadline)?;
        validate_local_path_batch(repository_cache_roots, LABEL, Some(deadline))?;
        #[cfg(test)]
        let test_concurrency_permit = TestGoSemanticConcurrencyPermit::acquire(deadline)?;
        #[cfg(windows)]
        let prepared = {
            let root = root.to_path_buf();
            let config = config.clone();
            let repository_cache_roots = repository_cache_roots.to_vec();
            run_windows_file_io_certification(deadline, LABEL, move || {
                require_go_semantic_process_containment()?;
                let cache_root = default_frontend_cache_root_until(deadline)?;
                Self::prepare_with_cache_root_for_analysis(
                    &cache_root,
                    Some((&root, &config)),
                    &repository_cache_roots,
                    deadline,
                )
            })?
        };
        #[cfg(not(windows))]
        let prepared = {
            require_go_semantic_process_containment()?;
            let cache_root = default_frontend_cache_root_until(deadline)?;
            Self::prepare_with_cache_root_for_analysis(
                &cache_root,
                Some((root, config)),
                repository_cache_roots,
                deadline,
            )?
        };
        #[cfg(test)]
        let prepared = prepared.with_test_concurrency_permit(test_concurrency_permit);
        Ok(prepared)
    }

    fn prepare_with_cache_root(cache_root: &Path) -> Result<Self, GoSemanticProcessError> {
        Self::prepare_with_cache_root_until(
            cache_root,
            GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
        )
    }

    fn prepare_with_cache_root_until(
        cache_root: &Path,
        deadline: GoOperationDeadline,
    ) -> Result<Self, GoSemanticProcessError> {
        Self::prepare_with_cache_root_for_analysis(cache_root, None, &[], deadline)
    }

    fn prepare_with_cache_root_for_analysis(
        cache_root: &Path,
        analysis: Option<(&Path, &GoAnalysisConfig)>,
        repository_cache_roots: &[PathBuf],
        deadline: GoOperationDeadline,
    ) -> Result<Self, GoSemanticProcessError> {
        const LABEL: &str = "complete Go semantic frontend preparation";

        validate_local_path_size_until(cache_root, LABEL, deadline)?;
        if let Some((root, _)) = analysis {
            validate_local_path_size_until(root, LABEL, deadline)?;
        }
        validate_local_path_batch(repository_cache_roots, LABEL, Some(deadline))?;
        #[cfg(windows)]
        let prepared = {
            let cache_root = cache_root.to_path_buf();
            let analysis = analysis.map(|(root, config)| (root.to_path_buf(), config.clone()));
            let repository_cache_roots = repository_cache_roots.to_vec();
            run_windows_file_io_certification(deadline, LABEL, move || {
                Self::prepare_with_cache_root_for_analysis_inner(
                    &cache_root,
                    analysis
                        .as_ref()
                        .map(|(root, config)| (root.as_path(), config)),
                    &repository_cache_roots,
                    deadline,
                )
            })?
        };
        #[cfg(not(windows))]
        let prepared = Self::prepare_with_cache_root_for_analysis_inner(
            cache_root,
            analysis,
            repository_cache_roots,
            deadline,
        )?;
        Ok(prepared)
    }

    fn prepare_with_cache_root_for_analysis_inner(
        cache_root: &Path,
        analysis: Option<(&Path, &GoAnalysisConfig)>,
        repository_cache_roots: &[PathBuf],
        deadline: GoOperationDeadline,
    ) -> Result<Self, GoSemanticProcessError> {
        require_go_semantic_process_containment()?;
        deadline.check("Go semantic frontend preparation")?;
        let cache_root = initialize_private_cache_root_until(cache_root, deadline)?;
        #[cfg(windows)]
        let cache_guard = crate::go::semantic::windows::PinnedDirectoryGuard::open(&cache_root)
            .map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to pin private Go semantic frontend cache `{}`: {error}",
                    cache_root.display()
                ))
            })?;
        let offline = analysis.is_some_and(|(_, config)| config.offline);
        let toolchain = prepare_go_toolchain_until(
            &cache_root,
            local_go_toolchain_until(deadline)?,
            offline,
            deadline,
        )?;
        let prepared = match resolve_go_semantic_frontend_in_until(&cache_root, deadline)? {
            GoSemanticCommand::Binary(executable) => {
                let dependency_snapshot = prepare_dependency_snapshot(
                    &cache_root,
                    &toolchain,
                    analysis,
                    repository_cache_roots,
                    deadline,
                )?;
                prepare_binary_frontend_with_snapshot(
                    &cache_root,
                    &executable,
                    toolchain,
                    dependency_snapshot,
                    deadline,
                )
            }
            GoSemanticCommand::SourceDir(source_dir) => {
                let source = capture_source_snapshot_until(&source_dir, deadline)?;
                Self::prepare_source_frontend_with_snapshot(
                    &cache_root,
                    source,
                    toolchain,
                    analysis,
                    repository_cache_roots,
                    offline,
                    deadline,
                )
            }
            GoSemanticCommand::Embedded => Self::prepare_source_frontend_with_snapshot(
                &cache_root,
                embedded_source_snapshot(),
                toolchain,
                analysis,
                repository_cache_roots,
                offline,
                deadline,
            ),
        }?;
        #[cfg(windows)]
        let prepared = {
            let mut prepared = prepared;
            cache_guard.verify_path_binding().map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "private Go semantic frontend cache path changed during preparation: {error}"
                ))
            })?;
            prepared.cache_guard = Some(cache_guard);
            prepared
        };
        Ok(prepared)
    }

    fn prepare_source_frontend_with_snapshot(
        cache_root: &Path,
        source: FrontendSourceSnapshot,
        toolchain: PreparedGoToolchain,
        analysis: Option<(&Path, &GoAnalysisConfig)>,
        repository_cache_roots: &[PathBuf],
        offline: bool,
        deadline: GoOperationDeadline,
    ) -> Result<Self, GoSemanticProcessError> {
        ensure_go_toolchain_supported(&toolchain.version)?;
        let source_dir = materialize_source_snapshot_until(cache_root, &source, deadline)?;
        let provenance = FrontendBuildProvenance {
            source_digest: source.digest.clone(),
            toolchain_version: toolchain.version.clone(),
            toolchain_executable_digest: toolchain.executable_digest.clone(),
            toolchain_content_digest: toolchain.closure.content_digest.clone(),
            host_target: toolchain.host_target.clone(),
            environment_policy: GO_ENVIRONMENT_POLICY,
        };
        let built = ensure_frontend_binary_until(
            cache_root,
            &source_dir,
            &provenance,
            &toolchain,
            offline,
            deadline,
        )?;
        let bytes = read_regular_file_no_follow_until(&built, deadline)?;
        let executable_digest = security_digest_bytes(&bytes);
        let executable = seal_executable_until(cache_root, &bytes, &executable_digest, deadline)?;
        let dependency_snapshot = prepare_dependency_snapshot(
            cache_root,
            &toolchain,
            analysis,
            repository_cache_roots,
            deadline,
        )?;
        Ok(Self {
            executable,
            executable_digest,
            source_digest: Some(source.digest),
            toolchain: Arc::new(toolchain),
            dependency_snapshot: Arc::new(dependency_snapshot),
            environment_policy: GO_ENVIRONMENT_POLICY,
            operation_deadline: deadline,
            #[cfg(windows)]
            cache_guard: None,
            #[cfg(test)]
            _test_concurrency_permit: None,
        })
    }

    pub(crate) fn command(&self, root: &Path) -> Result<Command, GoSemanticProcessError> {
        let command_root = go_command_working_directory(root)?;
        let mut command = Command::new(&self.executable);
        command.current_dir(command_root);
        configure_go_environment(&mut command, &self.toolchain);
        configure_dependency_execution_environment(&mut command, &self.dependency_snapshot)?;
        Ok(command)
    }

    pub(crate) fn run_command(
        &self,
        command: Command,
        limits: BoundedCommandLimits,
        label: &str,
    ) -> Result<BoundedCommandOutput, GoSemanticProcessError> {
        self.run_command_with_deadline(command, limits, label)
            .map(|(output, _)| output)
    }

    pub(crate) fn run_command_with_deadline(
        &self,
        command: Command,
        limits: BoundedCommandLimits,
        label: &str,
    ) -> Result<(BoundedCommandOutput, Instant), GoSemanticProcessError> {
        let deadline = self
            .operation_deadline
            .min(GoOperationDeadline::after(limits.timeout));
        verify_prepared_runtime_bindings_until(self, deadline)?;
        let (repository_cache_roots, local_scope_inclusions) =
            if let Some(inputs) = self.dependency_snapshot.local_inputs.as_deref() {
                let repository_cache_roots = local_repository_cache_roots_until(
                    &inputs.root,
                    &inputs.repository_cache_relatives,
                    deadline,
                )?;
                let scope = local_dependency_directory_scope_until(
                    &inputs.root,
                    &inputs.selected_directories,
                    &inputs.files,
                    GO_DEPENDENCY_MAX_ENTRIES,
                    deadline,
                )?;
                let mut inclusions = Vec::new();
                inclusions
                    .try_reserve_exact(scope.traversal.len())
                    .map_err(|error| {
                        GoSemanticProcessError::CommandFailed(format!(
                            "failed to allocate bounded Go command scope state: {error}"
                        ))
                    })?;
                for inclusion in scope.traversal {
                    deadline.check("Go command local-scope certification")?;
                    inclusions.push(inclusion);
                }
                (repository_cache_roots, inclusions)
            } else {
                validate_local_path_batch(
                    &self.dependency_snapshot.analysis_roots,
                    "Go command local-scope certification",
                    Some(deadline),
                )?;
                (Vec::new(), self.dependency_snapshot.analysis_roots.clone())
            };
        require_local_go_scan_roots_with_scope(
            &self.dependency_snapshot.analysis_roots,
            &repository_cache_roots,
            &local_scope_inclusions,
            deadline,
        )?;
        let output = run_prepared_go_command_with_local_trees_until(
            &self.toolchain,
            command,
            &[],
            limits,
            deadline,
            label,
        )?;
        deadline.check(label)?;
        verify_prepared_runtime_bindings_until(self, deadline)?;
        Ok((output, deadline.end))
    }

    pub(crate) fn identity_parts(&self) -> Vec<String> {
        vec![
            format!("executable_digest={}", self.executable_digest),
            format!(
                "source_digest={}",
                self.source_digest.as_deref().unwrap_or("prebuilt")
            ),
            format!("toolchain_version={}", self.toolchain.version),
            format!(
                "toolchain_executable_digest={}",
                self.toolchain.executable_digest
            ),
            format!(
                "toolchain_content_digest={}",
                self.toolchain.closure.content_digest
            ),
            format!(
                "dependency_snapshot_digest={}",
                self.dependency_snapshot.content_digest
            ),
            format!(
                "local_dependencies_digest={}",
                self.dependency_snapshot.local_dependencies_digest
            ),
            format!("host_target={}", self.toolchain.host_target.label()),
            format!("environment_policy={}", self.environment_policy),
        ]
    }

    pub(crate) fn identity_digest(&self) -> String {
        let parts = self.identity_parts();
        security_digest_strings(&parts)
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        executable_digest: &str,
        source_digest: Option<&str>,
        toolchain_version: Option<&str>,
    ) -> Self {
        let version = toolchain_version.unwrap_or("go-test").to_string();
        Self {
            executable: PathBuf::from("polint-go-frontend-test"),
            executable_digest: executable_digest.to_string(),
            source_digest: source_digest.map(str::to_string),
            toolchain: Arc::new(PreparedGoToolchain {
                executable: PathBuf::from("go-test"),
                executable_digest: format!("go-executable-{version}"),
                canonical_selection: PathBuf::from("/test/bin/go"),
                version: version.clone(),
                host_target: GoHostTarget {
                    os: "test".to_string(),
                    arch: "test".to_string(),
                },
                goroot: PathBuf::from("/test/go"),
                runtime_search_path: OsString::from("/sealed/toolchain:/test/bin"),
                closure: GoToolchainClosure {
                    digest: format!("go-closure-{version}"),
                    content_digest: format!("go-content-{version}"),
                    metadata_digest: format!("go-metadata-{version}"),
                    root_metadata_digest: format!("go-root-metadata-{version}"),
                    entry_count: 1,
                    byte_count: 1,
                    delegated_tool_count: 1,
                },
                environment: CertifiedGoEnvironment::for_test(),
            }),
            dependency_snapshot: Arc::new(GoDependencySnapshot {
                snapshots_root: PathBuf::from("/test/dependency-snapshots"),
                snapshot_root: PathBuf::from("/test/dependency-snapshots/snapshot"),
                module_cache_root: PathBuf::from(if cfg!(windows) {
                    r"C:\test\module-cache"
                } else {
                    "/test/module-cache"
                }),
                workspace_path: None,
                workspace_digest: security_digest_bytes(b"polint-go-private-workspace-off-v1"),
                workspace_closure: None,
                content_digest: "test-dependencies".to_string(),
                module_content_digest: "test-dependency-modules".to_string(),
                metadata_digest: "test-dependency-metadata".to_string(),
                module_root_metadata_digest: "test-dependency-root-metadata".to_string(),
                entry_count: 0,
                byte_count: 0,
                local_dependencies_digest: security_digest_bytes(
                    b"polint-go-local-inputs-empty-v1",
                ),
                local_inputs: None,
                analysis_roots: Vec::new(),
                _lease: None,
            }),
            environment_policy: GO_ENVIRONMENT_POLICY,
            operation_deadline: GoOperationDeadline::after(Duration::from_secs(60 * 60)),
            #[cfg(windows)]
            cache_guard: None,
            _test_concurrency_permit: None,
        }
    }
}

fn verify_prepared_runtime_bindings_until(
    frontend: &PreparedGoSemanticFrontend,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    #[cfg(windows)]
    {
        const LABEL: &str = "Go semantic runtime filesystem binding verification";
        deadline.check(LABEL)?;
        let frontend = frontend.clone();
        deadline.check(LABEL)?;
        return run_windows_file_io_certification(deadline, LABEL, move || {
            verify_prepared_runtime_bindings_inner(&frontend, deadline)
        });
    }
    #[cfg(not(windows))]
    verify_prepared_runtime_bindings_inner(frontend, deadline)
}

fn verify_prepared_runtime_bindings_inner(
    frontend: &PreparedGoSemanticFrontend,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    verify_prepared_cache_binding(frontend)?;
    verify_prepared_frontend_executable_until(frontend, deadline)?;
    verify_dependency_snapshot_binding(&frontend.dependency_snapshot, deadline)
}

fn prepare_binary_frontend(
    cache_root: &Path,
    executable: &Path,
    toolchain: PreparedGoToolchain,
) -> Result<PreparedGoSemanticFrontend, GoSemanticProcessError> {
    let deadline = GoOperationDeadline::after(GO_OPERATION_TIMEOUT);
    let dependency_snapshot =
        prepare_dependency_snapshot(cache_root, &toolchain, None, &[], deadline)?;
    prepare_binary_frontend_with_snapshot(
        cache_root,
        executable,
        toolchain,
        dependency_snapshot,
        deadline,
    )
}

fn prepare_binary_frontend_with_snapshot(
    cache_root: &Path,
    executable: &Path,
    toolchain: PreparedGoToolchain,
    dependency_snapshot: GoDependencySnapshot,
    operation_deadline: GoOperationDeadline,
) -> Result<PreparedGoSemanticFrontend, GoSemanticProcessError> {
    operation_deadline.check("Go semantic frontend binary preparation")?;
    require_local_existing_path_until(
        executable,
        operation_deadline,
        "Go semantic frontend binary preparation",
    )?;
    let bytes = read_regular_file_no_follow_until(executable, operation_deadline)?;
    let executable_digest = security_digest_bytes(&bytes);
    let executable =
        seal_executable_until(cache_root, &bytes, &executable_digest, operation_deadline)?;
    Ok(PreparedGoSemanticFrontend {
        executable,
        executable_digest,
        source_digest: None,
        toolchain: Arc::new(toolchain),
        dependency_snapshot: Arc::new(dependency_snapshot),
        environment_policy: GO_ENVIRONMENT_POLICY,
        operation_deadline,
        #[cfg(windows)]
        cache_guard: None,
        #[cfg(test)]
        _test_concurrency_permit: None,
    })
}

#[cfg(windows)]
fn verify_prepared_cache_binding(
    frontend: &PreparedGoSemanticFrontend,
) -> Result<(), GoSemanticProcessError> {
    let guard = frontend.cache_guard.as_ref().ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(
            "prepared Go semantic frontend is missing its pinned cache root.".to_string(),
        )
    })?;
    guard.verify_path_binding().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "private Go semantic frontend cache path no longer names its pinned directory: {error}"
        ))
    })
}

#[cfg(not(windows))]
fn verify_prepared_cache_binding(
    _frontend: &PreparedGoSemanticFrontend,
) -> Result<(), GoSemanticProcessError> {
    Ok(())
}

fn verify_prepared_frontend_executable(
    frontend: &PreparedGoSemanticFrontend,
) -> Result<(), GoSemanticProcessError> {
    verify_prepared_frontend_executable_until(
        frontend,
        GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
    )
}

fn verify_prepared_frontend_executable_until(
    frontend: &PreparedGoSemanticFrontend,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    deadline.check("sealed Go semantic frontend verification")?;
    require_local_existing_path_until(
        &frontend.executable,
        deadline,
        "sealed Go semantic frontend verification",
    )?;
    let digest = security_digest_bytes(&read_regular_file_no_follow_until(
        &frontend.executable,
        deadline,
    )?);
    deadline.check("sealed Go semantic frontend verification")?;
    if digest != frontend.executable_digest {
        return Err(GoSemanticProcessError::CommandFailed(
            "sealed Go semantic frontend changed after preparation.".to_string(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) enum GoSemanticToolPreparation {
    NotInvoked {
        reason: String,
    },
    SetupMissing {
        reason: String,
        process_error: Option<GoSemanticProcessError>,
    },
    Ready(PreparedGoSemanticFrontend),
}

impl GoSemanticToolPreparation {
    pub(crate) fn not_invoked(reason: impl Into<String>) -> Self {
        Self::NotInvoked {
            reason: reason.into(),
        }
    }

    pub(crate) fn setup_missing(reason: impl Into<String>) -> Self {
        Self::SetupMissing {
            reason: reason.into(),
            process_error: None,
        }
    }

    pub(crate) fn prepare(
        root: &Path,
        config: &GoAnalysisConfig,
        repository_cache_roots: &[PathBuf],
    ) -> Self {
        if let Err(error) = require_go_semantic_process_containment() {
            return Self::SetupMissing {
                reason: error.stable_reason().to_string(),
                process_error: Some(error),
            };
        }
        match PreparedGoSemanticFrontend::prepare_for_analysis_until(
            root,
            config,
            repository_cache_roots,
            GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
        ) {
            Ok(frontend) => Self::Ready(frontend),
            Err(error) => Self::SetupMissing {
                reason: error.stable_reason().to_string(),
                process_error: Some(error),
            },
        }
    }
}

pub(crate) fn resolve_go_semantic_frontend() -> Result<GoSemanticCommand, GoSemanticProcessError> {
    require_go_semantic_process_containment()?;
    let deadline = GoOperationDeadline::after(GO_OPERATION_TIMEOUT);
    let cache_root = default_frontend_cache_root_until(deadline)?;
    let cache_root = initialize_private_cache_root_until(&cache_root, deadline)?;
    resolve_go_semantic_frontend_in_until(&cache_root, deadline)
}

fn resolve_go_semantic_frontend_in(
    cache_root: &Path,
) -> Result<GoSemanticCommand, GoSemanticProcessError> {
    resolve_go_semantic_frontend_in_until(
        cache_root,
        GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
    )
}

fn resolve_go_semantic_frontend_in_until(
    _cache_root: &Path,
    deadline: GoOperationDeadline,
) -> Result<GoSemanticCommand, GoSemanticProcessError> {
    deadline.check("Go semantic frontend resolution")?;
    if let Ok(path) = std::env::var(GO_SEMANTIC_FRONTEND_ENV)
        && !path.trim().is_empty()
    {
        let path = PathBuf::from(path);
        return command_for_path_until(path, deadline);
    }
    if let Some(path) = installed_frontend_binary(deadline)? {
        return Ok(GoSemanticCommand::Binary(path));
    }
    #[cfg(windows)]
    {
        Ok(GoSemanticCommand::Embedded)
    }
    #[cfg(not(windows))]
    {
        let embedded = embedded_source_snapshot();
        materialize_source_snapshot_until(_cache_root, &embedded, deadline)
            .map(GoSemanticCommand::SourceDir)
    }
}

pub(crate) fn command_for_path(path: PathBuf) -> Result<GoSemanticCommand, GoSemanticProcessError> {
    command_for_path_until(path, GoOperationDeadline::after(GO_OPERATION_TIMEOUT))
}

fn command_for_path_until(
    path: PathBuf,
    deadline: GoOperationDeadline,
) -> Result<GoSemanticCommand, GoSemanticProcessError> {
    require_local_existing_path_until(&path, deadline, "Go semantic frontend override resolution")?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go semantic frontend `{}`: {error}",
            path.display()
        ))
    })?;
    if metadata.is_file() {
        return Ok(GoSemanticCommand::Binary(path));
    }
    #[cfg(windows)]
    if metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandUnavailable(
            "source-directory Go semantic frontend overrides are unavailable on Windows; use a prebuilt binary override"
                .to_string(),
        ));
    }
    #[cfg(not(windows))]
    if metadata.is_dir() {
        let manifest_path = path.join("go.mod");
        require_local_creation_root_until(
            &manifest_path,
            deadline,
            "Go semantic frontend override resolution",
        )?;
        if fs::symlink_metadata(manifest_path).is_ok_and(|manifest| manifest.is_file()) {
            return Ok(GoSemanticCommand::SourceDir(path));
        }
    }
    Err(GoSemanticProcessError::CommandFailed(format!(
        "{GO_SEMANTIC_FRONTEND_ENV} must point to a polint-go-frontend binary or source directory."
    )))
}

fn installed_frontend_binary(
    deadline: GoOperationDeadline,
) -> Result<Option<PathBuf>, GoSemanticProcessError> {
    deadline.check("installed Go semantic frontend resolution")?;
    let executable = std::env::current_exe().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to resolve current executable: {error}"
        ))
    })?;
    let Some(directory) = executable.parent() else {
        return Ok(None);
    };
    installed_frontend_binary_in(directory, deadline)
}

fn installed_frontend_binary_in(
    directory: &Path,
    deadline: GoOperationDeadline,
) -> Result<Option<PathBuf>, GoSemanticProcessError> {
    deadline.check("installed Go semantic frontend resolution")?;
    require_local_existing_path_until(
        directory,
        deadline,
        "installed Go semantic frontend resolution",
    )?;
    deadline.check("installed Go semantic frontend resolution")?;
    let candidate = directory.join(frontend_binary_name());
    require_local_creation_root_until(
        &candidate,
        deadline,
        "installed Go semantic frontend resolution",
    )?;
    let metadata = match fs::symlink_metadata(&candidate) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect installed Go semantic frontend `{}`: {error}",
                candidate.display()
            )));
        }
    };
    deadline.check("installed Go semantic frontend resolution")?;
    if metadata_is_link_or_reparse(&metadata) {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "installed Go semantic frontend `{}` must not be a symlink.",
            candidate.display()
        )));
    }
    if !metadata.is_file() {
        return Ok(None);
    }
    require_local_existing_path_until(
        &candidate,
        deadline,
        "installed Go semantic frontend resolution",
    )?;
    deadline.check("installed Go semantic frontend resolution")?;
    Ok(Some(candidate))
}

fn frontend_binary_name() -> &'static str {
    if cfg!(windows) {
        "polint-go-frontend.exe"
    } else {
        "polint-go-frontend"
    }
}

#[cfg(unix)]
fn default_frontend_cache_root() -> Result<PathBuf, GoSemanticProcessError> {
    default_frontend_cache_root_until(GoOperationDeadline::after(GO_OPERATION_TIMEOUT))
}

#[cfg(unix)]
fn default_frontend_cache_root_until(
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    deadline.check("Go semantic frontend cache resolution")?;
    let home = std::env::var_os("HOME").ok_or_else(|| {
        GoSemanticProcessError::CommandUnavailable(
            "HOME is required for the private Go semantic frontend cache.".to_string(),
        )
    })?;
    let home = PathBuf::from(home);
    require_local_existing_path_until(&home, deadline, "Go semantic frontend cache resolution")?;
    deadline.check("Go semantic frontend cache resolution")?;
    let home = home.canonicalize().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to resolve the user home for the Go semantic frontend cache: {error}"
        ))
    })?;
    Ok(home
        .join(".cache")
        .join("polint")
        .join("go-frontend")
        .join(GO_FRONTEND_CACHE_VERSION))
}

#[cfg(windows)]
fn default_frontend_cache_root() -> Result<PathBuf, GoSemanticProcessError> {
    default_frontend_cache_root_until(GoOperationDeadline::after(GO_OPERATION_TIMEOUT))
}

#[cfg(windows)]
fn default_frontend_cache_root_until(
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    deadline.check("Go semantic frontend cache resolution")?;
    let local_app_data = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        GoSemanticProcessError::CommandUnavailable(
            "LOCALAPPDATA is required for the private Go semantic frontend cache.".to_string(),
        )
    })?;
    let local_app_data = PathBuf::from(local_app_data);
    validate_local_path_size_until(
        &local_app_data,
        "Go semantic frontend cache resolution",
        deadline,
    )?;
    if !local_app_data.is_absolute() {
        return Err(GoSemanticProcessError::CommandFailed(
            "LOCALAPPDATA must be an absolute path for the private Go semantic frontend cache."
                .to_string(),
        ));
    }
    let cache_root = local_app_data
        .join("polint")
        .join("go-frontend")
        .join(GO_FRONTEND_CACHE_VERSION);
    validate_local_path_size_until(
        &cache_root,
        "Go semantic frontend cache resolution",
        deadline,
    )?;
    Ok(cache_root)
}

#[cfg(all(not(unix), not(windows)))]
fn default_frontend_cache_root() -> Result<PathBuf, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(all(not(unix), not(windows)))]
fn default_frontend_cache_root_until(
    _deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    default_frontend_cache_root()
}

fn initialize_private_cache_root_until(
    root: &Path,
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    deadline.check("Go semantic frontend cache initialization")?;
    require_local_creation_root_until(root, deadline, "Go semantic frontend cache initialization")?;
    deadline.check("Go semantic frontend cache initialization")?;
    ensure_private_cache_root(root)?;
    deadline.check("Go semantic frontend cache initialization")?;
    let canonical = canonical_private_cache_root(root)?;
    require_local_existing_path_until(
        &canonical,
        deadline,
        "Go semantic frontend cache initialization",
    )?;
    deadline.check("Go semantic frontend cache initialization")?;
    Ok(canonical)
}

#[cfg(unix)]
fn ensure_private_cache_root(root: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    let root = normalized_private_cache_root(root)?;
    let root = root.as_path();
    validate_private_cache_ancestors(root)?;
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata_is_link_or_reparse(&metadata) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache root `{}` must not be a symlink.",
                root.display()
            )));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache root `{}` is not a directory.",
                root.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(root).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to create private Go semantic frontend cache `{}`: {error}",
                    root.display()
                ))
            })?;
        }
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go semantic frontend cache root `{}`: {error}",
                root.display()
            )));
        }
    }

    let metadata = fs::symlink_metadata(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go semantic frontend cache root `{}`: {error}",
            root.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend cache root `{}` is unsafe.",
            root.display()
        )));
    }
    if metadata.uid() != effective_user_id() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend cache root `{}` is not owned by the current user.",
            root.display()
        )));
    }
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to restrict Go semantic frontend cache root `{}`: {error}",
            root.display()
        ))
    })?;
    let mode = fs::symlink_metadata(root)
        .map_err(|_| {
            GoSemanticProcessError::CommandFailed(
                "Go semantic frontend cache root became unavailable.".to_string(),
            )
        })?
        .mode();
    if mode & 0o077 != 0 {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend cache root `{}` is accessible by another user.",
            root.display()
        )));
    }
    validate_private_cache_ancestors(root)?;
    Ok(())
}

#[cfg(unix)]
fn canonical_private_cache_root(root: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    let normalized = normalized_private_cache_root(root)?;
    let canonical = normalized.canonicalize().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize private Go semantic frontend cache `{}`: {error}",
            root.display()
        ))
    })?;
    validate_private_cache_ancestors(&canonical)?;
    Ok(canonical)
}

#[cfg(unix)]
fn normalized_private_cache_root(root: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    use std::os::unix::fs::MetadataExt;

    if !root.is_absolute() {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go semantic frontend cache root must be absolute.".to_string(),
        ));
    }
    if root.components().any(|component| {
        !matches!(
            component,
            std::path::Component::RootDir | std::path::Component::Normal(_)
        )
    }) {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go semantic frontend cache root must not contain relative path components."
                .to_string(),
        ));
    }

    let mut current = PathBuf::new();
    for component in root.components() {
        current.push(component.as_os_str());
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go semantic frontend cache ancestor `{}`: {error}",
                    current.display()
                )));
            }
        };
        if metadata_is_link_or_reparse(&metadata) && (current == root || metadata.uid() != 0) {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache ancestor `{}` must not be a symlink.",
                current.display()
            )));
        }
    }

    let mut existing = root;
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name() else {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "failed to find an existing ancestor for private Go semantic frontend cache `{}`.",
                        root.display()
                    )));
                };
                missing.push(name.to_os_string());
                existing = existing.parent().ok_or_else(|| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to find an existing ancestor for private Go semantic frontend cache `{}`.",
                        root.display()
                    ))
                })?;
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect private Go semantic frontend cache `{}`: {error}",
                    existing.display()
                )));
            }
        }
    }
    let mut normalized = existing.canonicalize().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize private Go semantic frontend cache ancestor `{}`: {error}",
            existing.display()
        ))
    })?;
    for component in missing.into_iter().rev() {
        normalized.push(component);
    }
    validate_private_cache_ancestors(&normalized)?;
    Ok(normalized)
}

#[cfg(unix)]
fn validate_private_cache_ancestors(root: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::MetadataExt;

    if !root.is_absolute() {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go semantic frontend cache root must be absolute.".to_string(),
        ));
    }
    let mut current = PathBuf::new();
    let mut missing_ancestor = false;
    for component in root.components() {
        current.push(component.as_os_str());
        if missing_ancestor {
            continue;
        }
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing_ancestor = true;
                continue;
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go semantic frontend cache ancestor `{}`: {error}",
                    current.display()
                )));
            }
        };
        if metadata_is_link_or_reparse(&metadata) {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache ancestor `{}` must not be a symlink.",
                current.display()
            )));
        }
        if !metadata.is_dir() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache ancestor `{}` is not a directory.",
                current.display()
            )));
        }
        let mode = metadata.mode();
        let owner_can_replace =
            metadata.uid() != 0 && metadata.uid() != effective_user_id() && mode & 0o200 != 0;
        let shared_writable_without_sticky = mode & 0o022 != 0 && mode & 0o1000 == 0;
        if owner_can_replace || shared_writable_without_sticky {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache ancestor `{}` is mutable by another user.",
                current.display()
            )));
        }
    }
    Ok(())
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn effective_user_id() -> u32 {
    // SAFETY: `geteuid` takes no arguments and has no memory-safety preconditions.
    unsafe { libc::geteuid() }
}

#[cfg(windows)]
fn ensure_private_cache_root(root: &Path) -> Result<(), GoSemanticProcessError> {
    if !root.is_absolute()
        || root.components().any(|component| {
            !matches!(
                component,
                std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
                    | std::path::Component::Normal(_)
            )
        })
    {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go semantic frontend cache root must be an absolute normalized Windows path."
                .to_string(),
        ));
    }

    let mut existing = root;
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(metadata) => {
                if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "Go semantic frontend cache ancestor `{}` is unsafe.",
                        existing.display()
                    )));
                }
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = existing.file_name().ok_or_else(|| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to find an existing ancestor for private Go semantic frontend cache `{}`.",
                        root.display()
                    ))
                })?;
                missing.push(name.to_os_string());
                existing = existing.parent().ok_or_else(|| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to find an existing ancestor for private Go semantic frontend cache `{}`.",
                        root.display()
                    ))
                })?;
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect private Go semantic frontend cache `{}`: {error}",
                    existing.display()
                )));
            }
        }
    }
    crate::go::semantic::windows::require_local_fixed_volume(existing).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "private Go semantic frontend cache ancestor `{}` is unsafe: {error}",
            existing.display()
        ))
    })?;
    let mut current = existing.canonicalize().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize private Go semantic frontend cache ancestor `{}`: {error}",
            existing.display()
        ))
    })?;
    for component in missing.into_iter().rev() {
        current.push(component);
        crate::go::semantic::windows::create_private_directory(&current).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to create private Go semantic frontend cache directory `{}`: {error}",
                current.display()
            ))
        })?;
    }
    crate::go::semantic::windows::make_private_path_writable(&current, true).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to restrict private Go semantic frontend cache `{}`: {error}",
            current.display()
        ))
    })?;
    crate::go::semantic::windows::verify_private_path(&current, true, false).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "private Go semantic frontend cache `{}` failed DACL verification: {error}",
            current.display()
        ))
    })
}

#[cfg(windows)]
fn canonical_private_cache_root(root: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    let canonical = root.canonicalize().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize private Go semantic frontend cache `{}`: {error}",
            root.display()
        ))
    })?;
    crate::go::semantic::windows::verify_private_path(&canonical, true, false).map_err(
        |error| {
            GoSemanticProcessError::CommandFailed(format!(
                "private Go semantic frontend cache `{}` failed DACL verification: {error}",
                canonical.display()
            ))
        },
    )?;
    Ok(canonical)
}

#[cfg(all(not(unix), not(windows)))]
fn ensure_private_cache_root(_root: &Path) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(all(not(unix), not(windows)))]
fn canonical_private_cache_root(_root: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

fn ensure_private_subdirectory(
    root: &Path,
    relative_path: &Path,
) -> Result<PathBuf, GoSemanticProcessError> {
    let mut current = root.to_path_buf();
    for component in relative_path.components() {
        let std::path::Component::Normal(segment) = component else {
            return Err(GoSemanticProcessError::CommandFailed(
                "invalid Go semantic frontend cache path.".to_string(),
            ));
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go semantic frontend cache directory `{}` is unsafe.",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                create_private_directory(&current)?;
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go semantic frontend cache directory `{}`: {error}",
                    current.display()
                )));
            }
        }
        #[cfg(windows)]
        crate::go::semantic::windows::verify_private_path(&current, true, false).map_err(
            |error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "Go semantic frontend cache directory `{}` failed DACL verification: {error}",
                    current.display()
                ))
            },
        )?;
    }
    Ok(current)
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    match builder.create(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|_| {
                GoSemanticProcessError::CommandFailed(format!(
                    "Go semantic frontend cache directory `{}` became unavailable.",
                    path.display()
                ))
            })?;
            if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go semantic frontend cache directory `{}` is unsafe.",
                    path.display()
                )));
            }
            Ok(())
        }
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to create Go semantic frontend cache directory `{}`: {error}",
            path.display()
        ))),
    }
}

#[cfg(windows)]
fn create_private_directory(path: &Path) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::windows::create_private_directory(path).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to create private Go semantic frontend cache directory `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(all(not(unix), not(windows)))]
fn create_private_directory(_path: &Path) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[derive(Debug)]
struct StagingDirectory {
    path: PathBuf,
    cleanup: bool,
    dependency_liveness: Option<fs::File>,
}

impl StagingDirectory {
    fn create(parent: &Path, prefix: &str) -> Result<Self, GoSemanticProcessError> {
        Self::create_until(
            parent,
            prefix,
            GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
        )
    }

    fn create_until(
        parent: &Path,
        prefix: &str,
        deadline: GoOperationDeadline,
    ) -> Result<Self, GoSemanticProcessError> {
        Self::create_with_cleanup_limits_until(
            parent,
            prefix,
            GO_FRONTEND_MAX_STALE_CLEANUP_ENTRIES,
            GO_FRONTEND_MAX_STALE_CLEANUP_DEPTH,
            deadline,
        )
    }

    fn create_dependency_until(
        parent: &Path,
        prefix: &str,
        deadline: GoOperationDeadline,
    ) -> Result<Self, GoSemanticProcessError> {
        let mut staging = Self::create_with_cleanup_limits_until(
            parent,
            prefix,
            GO_DEPENDENCY_CLEANUP_MAX_VISITS,
            GO_DEPENDENCY_CLEANUP_MAX_DEPTH,
            deadline,
        )?;
        let liveness_path = staging.path.join(".liveness");
        write_new_private_mutable_file(&liveness_path, b"")?;
        let liveness = open_existing_dependency_lock_file(&liveness_path).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to open Go dependency staging liveness lock `{}`: {error}",
                liveness_path.display()
            ))
        })?;
        liveness.try_lock().map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to lock Go dependency staging liveness file: {error}"
            ))
        })?;
        staging.dependency_liveness = Some(liveness);
        Ok(staging)
    }

    fn create_with_cleanup_limits_until(
        parent: &Path,
        prefix: &str,
        max_visits: usize,
        max_depth: usize,
        deadline: GoOperationDeadline,
    ) -> Result<Self, GoSemanticProcessError> {
        deadline.check("Go semantic staging allocation")?;
        cleanup_stale_staging_directories_with_limits_until(
            parent,
            prefix,
            GO_FRONTEND_STALE_STAGING_AGE,
            max_visits,
            max_depth,
            deadline,
        )?;
        deadline.check("Go semantic staging allocation")?;
        let directory = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(parent)
            .map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to allocate private Go semantic frontend staging directory: {error}"
                ))
            })?;
        #[cfg(windows)]
        crate::go::semantic::windows::make_private_path_writable(directory.path(), true).map_err(
            |error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to restrict private Go semantic frontend staging directory `{}`: {error}",
                    directory.path().display()
                ))
            },
        )?;
        Ok(Self {
            path: directory.keep(),
            cleanup: true,
            dependency_liveness: None,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn mark_published(&mut self) {
        self.cleanup = false;
    }

    fn release_dependency_liveness(&mut self) -> Result<(), GoSemanticProcessError> {
        let Some(liveness) = self.dependency_liveness.take() else {
            return Ok(());
        };
        drop(liveness);
        let path = self.path.join(".liveness");
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to remove Go dependency staging liveness file `{}`: {error}",
                path.display()
            ))),
        }
    }

    fn discard_dependency_until(
        mut self,
        snapshots_root: &Path,
        deadline: GoOperationDeadline,
    ) -> Result<(), GoSemanticProcessError> {
        let _lifecycle = dependency_lifecycle_lock_until(snapshots_root, deadline)?;
        self.release_dependency_liveness()?;
        if !remove_directory_tree_with_limits(
            &self.path,
            GO_DEPENDENCY_CLEANUP_MAX_VISITS,
            GO_DEPENDENCY_CLEANUP_MAX_DEPTH,
            deadline,
        )? {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to remove unused Go dependency staging directory `{}` within snapshot cleanup bounds.",
                self.path.display()
            )));
        }
        self.cleanup = false;
        Ok(())
    }
}

fn cleanup_stale_staging_directories(
    parent: &Path,
    prefix: &str,
    minimum_age: std::time::Duration,
) -> Result<(), GoSemanticProcessError> {
    cleanup_stale_staging_directories_until(
        parent,
        prefix,
        minimum_age,
        GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
    )
}

fn cleanup_stale_staging_directories_until(
    parent: &Path,
    prefix: &str,
    minimum_age: std::time::Duration,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    cleanup_stale_staging_directories_with_limits_until(
        parent,
        prefix,
        minimum_age,
        GO_FRONTEND_MAX_STALE_CLEANUP_ENTRIES,
        GO_FRONTEND_MAX_STALE_CLEANUP_DEPTH,
        deadline,
    )
}

fn cleanup_stale_staging_directories_with_limits_until(
    parent: &Path,
    prefix: &str,
    minimum_age: std::time::Duration,
    max_visits: usize,
    max_depth: usize,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    deadline.check("stale Go semantic staging cleanup")?;
    let Ok(entries) = fs::read_dir(parent) else {
        return Ok(());
    };
    let mut matching_entries = 0_usize;
    for entry in entries.flatten() {
        deadline.check("stale Go semantic staging cleanup")?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !name.starts_with(prefix) {
            continue;
        }
        if matching_entries >= GO_FRONTEND_MAX_STALE_STAGING_SCAN {
            break;
        }
        matching_entries = matching_entries.saturating_add(1);
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
            continue;
        }
        let dependency_staging = prefix == ".dependency-";
        let mut dependency_liveness_present = false;
        if dependency_staging {
            let liveness_path = entry.path().join(".liveness");
            match open_existing_dependency_lock_file(&liveness_path) {
                Ok(liveness) => {
                    dependency_liveness_present = true;
                    match liveness.try_lock() {
                        Ok(()) => drop(liveness),
                        Err(error) if try_lock_would_block(&error) => continue,
                        Err(error) => {
                            return Err(GoSemanticProcessError::CommandFailed(format!(
                                "failed to inspect Go dependency staging liveness: {error}"
                            )));
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "failed to open Go dependency staging liveness file: {error}"
                    )));
                }
            }
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let elapsed = modified.elapsed().unwrap_or_default();
        if dependency_staging
            && !dependency_liveness_present
            && elapsed < GO_DEPENDENCY_STAGE_MARKER_GRACE
        {
            continue;
        }
        let abandoned = dependency_staging || name.ends_with(".abandoned");
        if !abandoned && elapsed < minimum_age {
            continue;
        }
        match remove_directory_tree_with_limits(&entry.path(), max_visits, max_depth, deadline) {
            Ok(_) => {}
            Err(error @ GoSemanticProcessError::Timeout(_)) => return Err(error),
            Err(_) => {}
        }
    }
    Ok(())
}

#[cfg(unix)]
fn remove_directory_tree_with_limits(
    root: &Path,
    max_visits: usize,
    max_depth: usize,
    deadline: GoOperationDeadline,
) -> Result<bool, GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    enum Visit {
        Enter(PathBuf, usize),
        Exit(PathBuf),
    }

    let mut frontier = vec![Visit::Enter(root.to_path_buf(), 0)];
    let mut inspected = 0_usize;
    while let Some(visit) = frontier.pop() {
        deadline.check("stale Go semantic staging cleanup")?;
        inspected = inspected.saturating_add(1);
        if inspected > max_visits {
            return Ok(false);
        }
        match visit {
            Visit::Enter(path, depth) => {
                if depth > max_depth {
                    return Ok(false);
                }
                let metadata = match fs::symlink_metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "failed to inspect stale Go semantic staging entry `{}`: {error}",
                            path.display()
                        )));
                    }
                };
                if metadata.is_dir() && !metadata_is_link_or_reparse(&metadata) {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(
                        |error| {
                            GoSemanticProcessError::CommandFailed(format!(
                                "failed to reopen stale Go semantic staging directory `{}`: {error}",
                                path.display()
                            ))
                        },
                    )?;
                    let mut entries = fs::read_dir(&path).map_err(|error| {
                        GoSemanticProcessError::CommandFailed(format!(
                            "failed to enumerate stale Go semantic staging directory `{}`: {error}",
                            path.display()
                        ))
                    })?;
                    let child_limit =
                        GO_STALE_CLEANUP_DIRECTORY_BATCH.min(max_visits.saturating_sub(inspected));
                    let mut children = Vec::with_capacity(child_limit);
                    for _ in 0..child_limit {
                        deadline.check("stale Go semantic staging cleanup")?;
                        let Some(entry) = entries.next() else {
                            break;
                        };
                        children.push(
                            entry.map_err(|error| {
                                    GoSemanticProcessError::CommandFailed(format!(
                                        "failed to enumerate stale Go semantic staging entry: {error}"
                                    ))
                                })?
                                .path(),
                        );
                    }
                    let completely_enumerated = entries.next().is_none();
                    if completely_enumerated {
                        frontier.push(Visit::Exit(path));
                    } else {
                        frontier.push(Visit::Enter(path, depth));
                    }
                    children.sort();
                    frontier.extend(
                        children
                            .into_iter()
                            .rev()
                            .map(|child| Visit::Enter(child, depth.saturating_add(1))),
                    );
                } else {
                    fs::remove_file(&path).map_err(|error| {
                        GoSemanticProcessError::CommandFailed(format!(
                            "failed to remove stale Go semantic staging entry `{}`: {error}",
                            path.display()
                        ))
                    })?;
                }
            }
            Visit::Exit(path) => match fs::remove_dir(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "failed to remove stale Go semantic staging directory `{}`: {error}",
                        path.display()
                    )));
                }
            },
        }
    }
    Ok(true)
}

#[cfg(windows)]
fn remove_directory_tree_with_limits(
    root: &Path,
    max_visits: usize,
    max_depth: usize,
    deadline: GoOperationDeadline,
) -> Result<bool, GoSemanticProcessError> {
    enum Visit {
        Enter(PathBuf, usize),
        Exit(PathBuf),
    }

    let mut frontier = vec![Visit::Enter(root.to_path_buf(), 0)];
    let mut inspected = 0_usize;
    while let Some(visit) = frontier.pop() {
        deadline.check("stale Go semantic staging cleanup")?;
        inspected = inspected.saturating_add(1);
        if inspected > max_visits {
            return Ok(false);
        }
        match visit {
            Visit::Enter(path, depth) => {
                if depth > max_depth {
                    return Ok(false);
                }
                let metadata = match fs::symlink_metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "failed to inspect stale Go semantic staging entry `{}`: {error}",
                            path.display()
                        )));
                    }
                };
                if metadata_is_link_or_reparse(&metadata) {
                    return Ok(false);
                }
                if metadata.is_dir() {
                    crate::go::semantic::windows::make_private_path_writable(&path, true).map_err(
                        |error| {
                            GoSemanticProcessError::CommandFailed(format!(
                                "failed to reopen stale Go semantic staging directory `{}`: {error}",
                                path.display()
                            ))
                        },
                    )?;
                    let mut entries = fs::read_dir(&path).map_err(|error| {
                        GoSemanticProcessError::CommandFailed(format!(
                            "failed to enumerate stale Go semantic staging directory `{}`: {error}",
                            path.display()
                        ))
                    })?;
                    let child_limit =
                        GO_STALE_CLEANUP_DIRECTORY_BATCH.min(max_visits.saturating_sub(inspected));
                    let mut children = Vec::with_capacity(child_limit);
                    for _ in 0..child_limit {
                        deadline.check("stale Go semantic staging cleanup")?;
                        let Some(entry) = entries.next() else {
                            break;
                        };
                        children.push(
                            entry.map_err(|error| {
                                    GoSemanticProcessError::CommandFailed(format!(
                                        "failed to enumerate stale Go semantic staging entry: {error}"
                                    ))
                                })?
                                .path(),
                        );
                    }
                    let completely_enumerated = entries.next().is_none();
                    if completely_enumerated {
                        frontier.push(Visit::Exit(path));
                    } else {
                        frontier.push(Visit::Enter(path, depth));
                    }
                    children.sort();
                    frontier.extend(
                        children
                            .into_iter()
                            .rev()
                            .map(|child| Visit::Enter(child, depth.saturating_add(1))),
                    );
                } else if metadata.is_file() {
                    crate::go::semantic::windows::make_private_path_writable(&path, false)
                        .map_err(|error| {
                            GoSemanticProcessError::CommandFailed(format!(
                                "failed to reopen stale Go semantic staging file `{}`: {error}",
                                path.display()
                            ))
                        })?;
                    fs::remove_file(&path).map_err(|error| {
                        GoSemanticProcessError::CommandFailed(format!(
                            "failed to remove stale Go semantic staging entry `{}`: {error}",
                            path.display()
                        ))
                    })?;
                } else {
                    return Ok(false);
                }
            }
            Visit::Exit(path) => match fs::remove_dir(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "failed to remove stale Go semantic staging directory `{}`: {error}",
                        path.display()
                    )));
                }
            },
        }
    }
    Ok(true)
}

#[cfg(all(not(unix), not(windows)))]
fn remove_directory_tree_with_limits(
    _root: &Path,
    _max_visits: usize,
    _max_depth: usize,
    _deadline: GoOperationDeadline,
) -> Result<bool, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "bounded stale staging cleanup is unavailable on this platform.".to_string(),
    ))
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.cleanup {
            return;
        }
        if self.dependency_liveness.is_some() {
            // Dependency stages are already recognizable by their dedicated
            // prefix, and orphan cleanup coordinates through the liveness
            // lock. Leave the marker in place so the handle can close before
            // a later lifecycle pass removes the tree on every platform.
            return;
        }
        let mut quarantine = self.path.as_os_str().to_os_string();
        quarantine.push(".abandoned");
        let _ = fs::rename(&self.path, PathBuf::from(quarantine));
    }
}

pub(crate) fn embedded_frontend_hash() -> String {
    embedded_source_snapshot().digest
}

pub(crate) fn frontend_digest(
    frontend: &GoSemanticCommand,
) -> Result<String, GoSemanticProcessError> {
    match frontend {
        GoSemanticCommand::Binary(path) => {
            let bytes = read_regular_file_no_follow(path)?;
            Ok(security_digest_bytes(&bytes))
        }
        GoSemanticCommand::SourceDir(path) => Ok(capture_source_snapshot(path)?.digest),
        GoSemanticCommand::Embedded => Ok(embedded_frontend_hash()),
    }
}

fn embedded_source_snapshot() -> FrontendSourceSnapshot {
    let mut files = EMBEDDED_GO_FRONTEND_FILES
        .iter()
        .map(|(relative_path, contents)| FrontendSourceFile {
            relative_path: PathBuf::from(relative_path),
            bytes: contents.as_bytes().to_vec(),
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let digest = source_snapshot_digest(&files);
    FrontendSourceSnapshot { digest, files }
}

#[cfg(unix)]
fn capture_source_snapshot(path: &Path) -> Result<FrontendSourceSnapshot, GoSemanticProcessError> {
    capture_source_snapshot_until(path, GoOperationDeadline::after(GO_OPERATION_TIMEOUT))
}

#[cfg(unix)]
fn capture_source_snapshot_until(
    path: &Path,
    deadline: GoOperationDeadline,
) -> Result<FrontendSourceSnapshot, GoSemanticProcessError> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::OpenOptionsExt;

    deadline.check("Go semantic frontend source capture")?;
    require_local_scan_root(path, deadline)?;
    let root = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to anchor Go semantic frontend source directory `{}`: {error}",
                path.display()
            ))
        })?;
    if !root
        .metadata()
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go semantic frontend source directory `{}`: {error}",
                path.display()
            ))
        })?
        .is_dir()
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend source `{}` is not a directory.",
            path.display()
        )));
    }

    let mut frontier = vec![(OwnedFd::from(root), PathBuf::new(), 0_usize)];
    let mut visited_entries = 0_usize;
    let mut visited_directories = 1_usize;
    let mut captured = Vec::new();
    let mut total_bytes = 0_usize;

    while let Some((directory, relative_directory, depth)) = frontier.pop() {
        deadline.check("Go semantic frontend source capture")?;
        let mut names = directory_entry_names(&directory, &mut visited_entries, deadline)?;
        names.sort();
        for name in names.into_iter().rev() {
            deadline.check("Go semantic frontend source capture")?;
            let relative_path = relative_directory.join(&name);
            let name = CString::new(name.as_bytes()).map_err(|_| {
                GoSemanticProcessError::CommandFailed(
                    "Go semantic frontend source contains an invalid path.".to_string(),
                )
            })?;
            let directory_flags =
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY;
            match open_frontend_at(directory.as_raw_fd(), &name, directory_flags) {
                Ok(child) => {
                    if skip_frontend_digest_dir(&relative_path) {
                        continue;
                    }
                    let child_depth = depth.saturating_add(1);
                    if child_depth > GO_FRONTEND_MAX_SOURCE_DEPTH {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "Go semantic frontend source exceeds traversal depth {GO_FRONTEND_MAX_SOURCE_DEPTH}."
                        )));
                    }
                    visited_directories = visited_directories.checked_add(1).ok_or_else(|| {
                        GoSemanticProcessError::CommandFailed(
                            "Go semantic frontend source directory count overflowed.".to_string(),
                        )
                    })?;
                    if visited_directories > GO_FRONTEND_MAX_SOURCE_DIRECTORIES {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "Go semantic frontend source contains more than {GO_FRONTEND_MAX_SOURCE_DIRECTORIES} directories."
                        )));
                    }
                    frontier.push((child, relative_path, child_depth));
                    if frontier.len() > GO_FRONTEND_MAX_SOURCE_FRONTIER {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "Go semantic frontend source traversal frontier exceeds {GO_FRONTEND_MAX_SOURCE_FRONTIER} directories."
                        )));
                    }
                }
                Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {}
                Err(error) if error.raw_os_error() == Some(libc::ENOTDIR) => {
                    if !is_frontend_digest_source(&relative_path) {
                        continue;
                    }
                    if captured.len() >= GO_FRONTEND_MAX_SOURCE_FILES {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "Go semantic frontend source contains more than {GO_FRONTEND_MAX_SOURCE_FILES} files."
                        )));
                    }
                    let file = open_frontend_at(
                        directory.as_raw_fd(),
                        &name,
                        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
                    )
                    .map_err(|error| {
                        GoSemanticProcessError::CommandFailed(format!(
                            "failed to open Go semantic frontend source `{}`: {error}",
                            relative_path.display()
                        ))
                    })?;
                    let remaining = GO_FRONTEND_MAX_SOURCE_BYTES.saturating_sub(total_bytes);
                    let bytes = read_open_frontend_source(
                        fs::File::from(file),
                        &relative_path,
                        remaining,
                        deadline,
                    )?;
                    total_bytes = total_bytes.checked_add(bytes.len()).ok_or_else(|| {
                        GoSemanticProcessError::CommandFailed(
                            "Go semantic frontend source size overflowed.".to_string(),
                        )
                    })?;
                    captured.push(FrontendSourceFile {
                        relative_path,
                        bytes,
                    });
                }
                Err(error) => {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "failed to inspect Go semantic frontend source `{}`: {error}",
                        relative_path.display()
                    )));
                }
            }
        }
    }
    captured.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if !captured
        .iter()
        .any(|file| file.relative_path == Path::new("go.mod"))
    {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go semantic frontend source snapshot has no go.mod.".to_string(),
        ));
    }
    let digest = source_snapshot_digest(&captured);
    Ok(FrontendSourceSnapshot {
        digest,
        files: captured,
    })
}

#[cfg(not(unix))]
fn capture_source_snapshot(_path: &Path) -> Result<FrontendSourceSnapshot, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "secure Go semantic frontend source traversal is unavailable on this platform.".to_string(),
    ))
}

#[cfg(not(unix))]
fn capture_source_snapshot_until(
    _path: &Path,
    _deadline: GoOperationDeadline,
) -> Result<FrontendSourceSnapshot, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "secure Go semantic frontend source traversal is unavailable on this platform.".to_string(),
    ))
}

fn source_snapshot_digest(files: &[FrontendSourceFile]) -> String {
    let mut parts = vec![GO_SEMANTIC_SCHEMA.to_string()];
    parts.extend(files.iter().map(|file| {
        format!(
            "{}:{}",
            file.relative_path.to_string_lossy().replace('\\', "/"),
            security_digest_bytes(&file.bytes)
        )
    }));
    security_digest_strings(&parts)
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn directory_entry_names(
    directory: &std::os::fd::OwnedFd,
    visited_entries: &mut usize,
    deadline: GoOperationDeadline,
) -> Result<Vec<std::ffi::OsString>, GoSemanticProcessError> {
    use std::ffi::{CStr, OsString};
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStringExt;

    let duplicated = unsafe { libc::dup(directory.as_raw_fd()) };
    if duplicated < 0 {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to duplicate Go semantic frontend source directory handle: {}",
            std::io::Error::last_os_error()
        )));
    }
    let stream = unsafe { libc::fdopendir(duplicated) };
    if stream.is_null() {
        let error = std::io::Error::last_os_error();
        unsafe { libc::close(duplicated) };
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to enumerate Go semantic frontend source directory: {error}"
        )));
    }

    struct DirectoryStream(*mut libc::DIR);
    impl Drop for DirectoryStream {
        fn drop(&mut self) {
            unsafe { libc::closedir(self.0) };
        }
    }

    let stream = DirectoryStream(stream);
    let errno = readdir_errno_pointer()?;
    let mut names = Vec::new();
    loop {
        deadline.check("Go semantic frontend source enumeration")?;
        unsafe { *errno = 0 };
        let entry = unsafe { libc::readdir(stream.0) };
        let Some(entry) = classify_readdir_result(entry, unsafe { *errno })? else {
            break;
        };
        let entry = entry.as_ptr();
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if matches!(name, b"." | b"..") {
            continue;
        }
        *visited_entries = visited_entries.checked_add(1).ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "Go semantic frontend source entry count overflowed.".to_string(),
            )
        })?;
        if *visited_entries > GO_FRONTEND_MAX_SOURCE_ENTRIES {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend source contains more than {GO_FRONTEND_MAX_SOURCE_ENTRIES} entries."
            )));
        }
        names.push(OsString::from_vec(name.to_vec()));
    }
    Ok(names)
}

#[cfg(unix)]
fn classify_readdir_result(
    entry: *mut libc::dirent,
    errno: libc::c_int,
) -> Result<Option<std::ptr::NonNull<libc::dirent>>, GoSemanticProcessError> {
    if let Some(entry) = std::ptr::NonNull::new(entry) {
        return Ok(Some(entry));
    }
    if errno == 0 {
        return Ok(None);
    }
    Err(GoSemanticProcessError::CommandFailed(format!(
        "failed to enumerate Go semantic frontend source directory: {}",
        std::io::Error::from_raw_os_error(errno)
    )))
}

#[cfg(any(target_os = "linux", target_os = "dragonfly"))]
#[allow(unsafe_code)]
fn readdir_errno_pointer() -> Result<*mut libc::c_int, GoSemanticProcessError> {
    unsafe extern "C" {
        fn __errno_location() -> *mut libc::c_int;
    }

    let pointer = unsafe { __errno_location() };
    validate_errno_pointer(pointer)
}

#[cfg(any(target_vendor = "apple", target_os = "freebsd"))]
#[allow(unsafe_code)]
fn readdir_errno_pointer() -> Result<*mut libc::c_int, GoSemanticProcessError> {
    let pointer = unsafe { libc::__error() };
    validate_errno_pointer(pointer)
}

#[cfg(any(target_os = "android", target_os = "netbsd", target_os = "openbsd"))]
#[allow(unsafe_code)]
fn readdir_errno_pointer() -> Result<*mut libc::c_int, GoSemanticProcessError> {
    let pointer = unsafe { libc::__errno() };
    validate_errno_pointer(pointer)
}

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
#[allow(unsafe_code)]
fn readdir_errno_pointer() -> Result<*mut libc::c_int, GoSemanticProcessError> {
    let pointer = unsafe { libc::___errno() };
    validate_errno_pointer(pointer)
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "android",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
        target_os = "illumos"
    ))
))]
fn readdir_errno_pointer() -> Result<*mut libc::c_int, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(format!(
        "Go semantic frontend source enumeration is unavailable on {} because readdir errors cannot be authenticated on this host.",
        std::env::consts::OS
    )))
}

#[cfg(unix)]
fn validate_errno_pointer(
    pointer: *mut libc::c_int,
) -> Result<*mut libc::c_int, GoSemanticProcessError> {
    if pointer.is_null() {
        Err(GoSemanticProcessError::CommandFailed(
            "failed to access the thread-local error state for Go semantic frontend source enumeration."
                .to_string(),
        ))
    } else {
        Ok(pointer)
    }
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn open_frontend_at(
    directory: std::os::fd::RawFd,
    name: &std::ffi::CStr,
    flags: libc::c_int,
) -> std::io::Result<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;

    let descriptor = unsafe { libc::openat(directory, name.as_ptr(), flags) };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(descriptor) })
}

#[cfg(unix)]
fn read_open_frontend_source(
    mut file: fs::File,
    relative_path: &Path,
    remaining_bytes: usize,
    deadline: GoOperationDeadline,
) -> Result<Vec<u8>, GoSemanticProcessError> {
    let metadata = file.metadata().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go semantic frontend source `{}`: {error}",
            relative_path.display()
        ))
    })?;
    let file_len = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if !metadata.is_file() || file_len > remaining_bytes {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend source exceeds {GO_FRONTEND_MAX_SOURCE_BYTES} bytes."
        )));
    }
    let mut bytes = Vec::with_capacity(file_len);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        deadline.check("Go semantic frontend source read")?;
        let count = file.read(&mut buffer).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to read Go semantic frontend source `{}`: {error}",
                relative_path.display()
            ))
        })?;
        if count == 0 {
            break;
        }
        if count > remaining_bytes.saturating_sub(bytes.len()) {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend source exceeds {GO_FRONTEND_MAX_SOURCE_BYTES} bytes."
            )));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() > remaining_bytes {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend source exceeds {GO_FRONTEND_MAX_SOURCE_BYTES} bytes."
        )));
    }
    Ok(bytes)
}

fn skip_frontend_digest_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | ".hg" | "target" | "node_modules")
    )
}

fn is_frontend_digest_source(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("go.mod" | "go.sum")
    ) || path.extension().and_then(|extension| extension.to_str()) == Some("go")
}

fn materialize_source_snapshot(
    cache_root: &Path,
    snapshot: &FrontendSourceSnapshot,
) -> Result<PathBuf, GoSemanticProcessError> {
    materialize_source_snapshot_until(
        cache_root,
        snapshot,
        GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
    )
}

fn materialize_source_snapshot_until(
    cache_root: &Path,
    snapshot: &FrontendSourceSnapshot,
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    deadline.check("Go semantic frontend source materialization")?;
    let sources_root = ensure_private_subdirectory(cache_root, Path::new("sources"))?;
    let destination = sources_root.join(&snapshot.digest);
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "cached Go semantic frontend source `{}` is unsafe.",
                destination.display()
            )));
        }
        Ok(_) => {
            if source_snapshot_matches_until(&destination, snapshot, deadline)? {
                return Ok(destination);
            }
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "cached Go semantic frontend source `{}` failed content verification.",
                destination.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect cached Go semantic frontend source `{}`: {error}",
                destination.display()
            )));
        }
    }

    let mut staging = StagingDirectory::create_until(&sources_root, ".source-", deadline)?;
    for file in &snapshot.files {
        deadline.check("Go semantic frontend source materialization")?;
        let path = staging.path().join(&file.relative_path);
        if let Some(parent) = file.relative_path.parent()
            && !parent.as_os_str().is_empty()
        {
            ensure_private_subdirectory(staging.path(), parent)?;
        }
        write_new_private_file(&path, &file.bytes, false)?;
    }
    make_source_snapshot_read_only(staging.path(), snapshot)?;
    let prospective_bytes = snapshot
        .files
        .iter()
        .try_fold(0_u64, |total, file| {
            total.checked_add(u64::try_from(file.bytes.len()).unwrap_or(u64::MAX))
        })
        .ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "Go semantic frontend source byte count overflowed.".to_string(),
            )
        })?;
    let _capacity = published_cache_capacity_guard_until(
        cache_root,
        &sources_root,
        &destination,
        prospective_bytes,
        GO_FRONTEND_MAX_PUBLISHED_SOURCES,
        GO_FRONTEND_MAX_PUBLISHED_SOURCE_BYTES,
        deadline,
    )?;

    match fs::rename(staging.path(), &destination) {
        Ok(()) => {
            staging.mark_published();
            Ok(destination)
        }
        Err(_) if destination.is_dir() => {
            if source_snapshot_matches_until(&destination, snapshot, deadline)? {
                Ok(destination)
            } else {
                Err(GoSemanticProcessError::CommandFailed(format!(
                    "concurrently published Go semantic frontend source `{}` failed verification.",
                    destination.display()
                )))
            }
        }
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to publish Go semantic frontend source `{}`: {error}",
            destination.display()
        ))),
    }
}

fn source_snapshot_matches(
    directory: &Path,
    snapshot: &FrontendSourceSnapshot,
) -> Result<bool, GoSemanticProcessError> {
    source_snapshot_matches_until(
        directory,
        snapshot,
        GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
    )
}

fn source_snapshot_matches_until(
    directory: &Path,
    snapshot: &FrontendSourceSnapshot,
    deadline: GoOperationDeadline,
) -> Result<bool, GoSemanticProcessError> {
    deadline.check("Go semantic frontend source verification")?;
    let expected_files = snapshot
        .files
        .iter()
        .map(|file| (file.relative_path.clone(), file.bytes.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let expected_directories = snapshot
        .files
        .iter()
        .flat_map(|file| file.relative_path.ancestors().skip(1))
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>();
    let mut remaining = expected_files;
    if !verify_source_directory(
        directory,
        directory,
        &expected_directories,
        &mut remaining,
        deadline,
    )? {
        return Ok(false);
    }
    Ok(remaining.is_empty())
}

fn verify_source_directory(
    root: &Path,
    directory: &Path,
    expected_directories: &BTreeSet<PathBuf>,
    remaining: &mut BTreeMap<PathBuf, &[u8]>,
    deadline: GoOperationDeadline,
) -> Result<bool, GoSemanticProcessError> {
    deadline.check("Go semantic frontend source verification")?;
    let entries = fs::read_dir(directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to audit Go semantic frontend source directory `{}`: {error}",
            directory.display()
        ))
    })?;
    for entry in entries {
        deadline.check("Go semantic frontend source verification")?;
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to audit Go semantic frontend source entry: {error}"
            ))
        })?;
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|_| {
            GoSemanticProcessError::CommandFailed(
                "cached Go semantic frontend source escaped its root.".to_string(),
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to audit cached Go semantic frontend source `{}`: {error}",
                path.display()
            ))
        })?;
        if file_type.is_symlink() {
            return Ok(false);
        }
        if file_type.is_dir() {
            if !expected_directories.contains(relative)
                || !verify_source_directory(root, &path, expected_directories, remaining, deadline)?
            {
                return Ok(false);
            }
        } else if file_type.is_file() {
            let Some(expected) = remaining.remove(relative) else {
                return Ok(false);
            };
            if read_regular_file_no_follow_until(&path, deadline)? != expected {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}

fn security_digest_strings(parts: &[String]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        let bytes = part.as_bytes();
        hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(bytes);
    }
    format!("{:x}", hasher.finalize())
}

fn security_digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn published_cache_capacity_guard_until(
    cache_root: &Path,
    category_root: &Path,
    destination: &Path,
    prospective_bytes: u64,
    maximum_entries: usize,
    maximum_bytes: u64,
    deadline: GoOperationDeadline,
) -> Result<fs::File, GoSemanticProcessError> {
    // Published artifacts can be executing in another process, so deleting an
    // apparently old entry is not safe without per-artifact leases. A hard
    // owner-private bound prevents unbounded growth while preserving every
    // artifact that another process may still be using.
    let control = ensure_private_subdirectory(cache_root, Path::new("control"))?;
    let lifecycle = open_dependency_lock_file(&control.join("published-cache.lock"))?;
    lock_dependency_file_exclusive_until(&lifecycle, deadline)?;
    if fs::symlink_metadata(destination).is_ok() {
        return Ok(lifecycle);
    }

    let entries = fs::read_dir(category_root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to enumerate published Go cache category `{}`: {error}",
            category_root.display()
        ))
    })?;
    let mut published_entries = 0_usize;
    let mut published_bytes = 0_u64;
    for entry in entries {
        deadline.check("published Go cache capacity enforcement")?;
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect a published Go cache entry: {error}"
            ))
        })?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with('.'))
        {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect published Go cache entry `{}`: {error}",
                entry.path().display()
            ))
        })?;
        if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "published Go cache entry `{}` is unsafe.",
                entry.path().display()
            )));
        }
        published_entries = published_entries.saturating_add(1);
        published_bytes = published_bytes
            .saturating_add(published_cache_tree_size_until(&entry.path(), deadline)?);
    }
    if published_entries.saturating_add(1) > maximum_entries
        || published_bytes.saturating_add(prospective_bytes) > maximum_bytes
    {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "published Go cache capacity is exhausted under `{}`; remove obsolete owner-private cache entries before retrying.",
            category_root.display()
        )));
    }
    Ok(lifecycle)
}

fn published_cache_tree_size_until(
    root: &Path,
    deadline: GoOperationDeadline,
) -> Result<u64, GoSemanticProcessError> {
    let mut frontier = vec![(root.to_path_buf(), 0_usize)];
    let mut entries_seen = 0_usize;
    let mut bytes = 0_u64;
    while let Some((directory, depth)) = frontier.pop() {
        deadline.check("published Go cache size accounting")?;
        if depth > GO_LOCAL_DEPENDENCY_MAX_DEPTH {
            return Err(GoSemanticProcessError::CommandFailed(
                "published Go cache entry exceeds its directory-depth limit.".to_string(),
            ));
        }
        let entries = fs::read_dir(&directory).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to enumerate published Go cache entry `{}`: {error}",
                directory.display()
            ))
        })?;
        for entry in entries {
            deadline.check("published Go cache size accounting")?;
            entries_seen = entries_seen.saturating_add(1);
            if entries_seen > GO_DEPENDENCY_MAX_ENTRIES {
                return Err(GoSemanticProcessError::CommandFailed(
                    "published Go cache entry exceeds its entry-count limit.".to_string(),
                ));
            }
            let entry = entry.map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect published Go cache entry: {error}"
                ))
            })?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect published Go cache path `{}`: {error}",
                    path.display()
                ))
            })?;
            if metadata_is_link_or_reparse(&metadata) {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "published Go cache path `{}` must not be a link or reparse point.",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                frontier.push((path, depth.saturating_add(1)));
            } else if metadata.is_file() {
                bytes = bytes.checked_add(metadata.len()).ok_or_else(|| {
                    GoSemanticProcessError::CommandFailed(
                        "published Go cache byte count overflowed.".to_string(),
                    )
                })?;
            } else {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "published Go cache path `{}` has an unsupported type.",
                    path.display()
                )));
            }
        }
    }
    Ok(bytes)
}

fn ensure_frontend_binary(
    cache_root: &Path,
    source_dir: &Path,
    provenance: &FrontendBuildProvenance,
    toolchain: &PreparedGoToolchain,
) -> Result<PathBuf, GoSemanticProcessError> {
    ensure_frontend_binary_until(
        cache_root,
        source_dir,
        provenance,
        toolchain,
        false,
        GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
    )
}

fn ensure_frontend_binary_until(
    cache_root: &Path,
    source_dir: &Path,
    provenance: &FrontendBuildProvenance,
    toolchain: &PreparedGoToolchain,
    offline: bool,
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    deadline.check("Go semantic frontend build preparation")?;
    ensure_go_toolchain_supported(&provenance.toolchain_version)?;
    let builds_root = ensure_private_subdirectory(cache_root, Path::new("builds"))?;
    let destination = builds_root.join(provenance.cache_key());
    if destination.exists() {
        return verify_cached_build_until(&destination, provenance, deadline);
    }

    let proxy = dependency_population_proxy(toolchain, offline, deadline)?;
    let (proxy_value, local_trees) = proxy.into_frontend_build_inputs(source_dir);
    let mut staging = StagingDirectory::create_until(&builds_root, ".build-", deadline)?;
    let binary = staging.path().join(frontend_binary_name());
    let mut command = Command::new(&toolchain.executable);
    configure_go_environment(&mut command, toolchain);
    command
        .arg("build")
        .arg("-trimpath")
        .arg("-o")
        .arg(&binary)
        .arg(".")
        .current_dir(source_dir)
        .env("GOFLAGS", "-mod=readonly")
        .env("GONOPROXY", "none")
        .env("GOPROXY", &proxy_value)
        .env("GOWORK", "off");
    let output = run_prepared_go_command_with_local_trees_until(
        toolchain,
        command,
        &local_trees,
        BoundedCommandLimits::new(
            GO_BUILD_TIMEOUT,
            GO_BUILD_STDOUT_BYTES,
            GO_BUILD_STDERR_BYTES,
        ),
        deadline.cap(GO_BUILD_TIMEOUT),
        "reproducible Go semantic frontend build",
    )?;
    if !output.status.success() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend build failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let executable_digest =
        security_digest_bytes(&read_regular_file_no_follow_until(&binary, deadline)?);
    write_new_private_file(
        &staging.path().join("provenance"),
        provenance.stamp(&executable_digest).as_bytes(),
        false,
    )?;
    make_file_executable_read_only(&binary)?;
    let prospective_bytes = fs::symlink_metadata(&binary)
        .map(|metadata| metadata.len())
        .unwrap_or(u64::MAX)
        .saturating_add(
            u64::try_from(provenance.stamp(&executable_digest).len()).unwrap_or(u64::MAX),
        );
    let _capacity = published_cache_capacity_guard_until(
        cache_root,
        &builds_root,
        &destination,
        prospective_bytes,
        GO_FRONTEND_MAX_PUBLISHED_BUILDS,
        GO_FRONTEND_MAX_PUBLISHED_BUILD_BYTES,
        deadline,
    )?;

    match fs::rename(staging.path(), &destination) {
        Ok(()) => {
            staging.mark_published();
            Ok(destination.join(frontend_binary_name()))
        }
        Err(_) if destination.is_dir() => {
            verify_cached_build_until(&destination, provenance, deadline)
        }
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to publish verified Go semantic frontend build `{}`: {error}",
            destination.display()
        ))),
    }
}

fn verify_cached_build_until(
    directory: &Path,
    provenance: &FrontendBuildProvenance,
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    deadline.check("cached Go semantic frontend verification")?;
    let metadata = fs::symlink_metadata(directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect cached Go semantic frontend build `{}`: {error}",
            directory.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "cached Go semantic frontend build `{}` is unsafe.",
            directory.display()
        )));
    }
    let binary = directory.join(frontend_binary_name());
    let executable_digest =
        security_digest_bytes(&read_regular_file_no_follow_until(&binary, deadline)?);
    let stamp = read_regular_file_no_follow_until(&directory.join("provenance"), deadline)?;
    if stamp != provenance.stamp(&executable_digest).as_bytes() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "cached Go semantic frontend build `{}` failed provenance verification.",
            directory.display()
        )));
    }
    Ok(binary)
}

fn seal_executable(
    cache_root: &Path,
    bytes: &[u8],
    executable_digest: &str,
) -> Result<PathBuf, GoSemanticProcessError> {
    seal_executable_until(
        cache_root,
        bytes,
        executable_digest,
        GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
    )
}

fn seal_executable_until(
    cache_root: &Path,
    bytes: &[u8],
    executable_digest: &str,
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    deadline.check("Go semantic frontend sealing")?;
    let execution_root = ensure_private_subdirectory(cache_root, Path::new("executables"))?;
    let directory = execution_root.join(executable_digest);
    match fs::symlink_metadata(&directory) {
        Ok(_) => return verify_sealed_executable_until(&directory, bytes, deadline),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect sealed Go semantic frontend directory `{}`: {error}",
                directory.display()
            )));
        }
    }

    let mut staging = StagingDirectory::create_until(&execution_root, ".seal-", deadline)?;
    write_new_private_file(&staging.path().join(frontend_binary_name()), bytes, true)?;
    seal_execution_directory(staging.path())?;
    let _capacity = published_cache_capacity_guard_until(
        cache_root,
        &execution_root,
        &directory,
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        GO_FRONTEND_MAX_PUBLISHED_EXECUTABLES,
        GO_FRONTEND_MAX_PUBLISHED_EXECUTABLE_BYTES,
        deadline,
    )?;
    match fs::rename(staging.path(), &directory) {
        Ok(()) => {
            staging.mark_published();
            verify_sealed_executable_until(&directory, bytes, deadline)
        }
        Err(_) if fs::symlink_metadata(&directory).is_ok() => {
            verify_sealed_executable_until(&directory, bytes, deadline)
        }
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to publish sealed Go semantic frontend `{}`: {error}",
            directory.display()
        ))),
    }
}

fn verify_sealed_executable_until(
    directory: &Path,
    expected_bytes: &[u8],
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    deadline.check("sealed Go semantic frontend verification")?;
    let directory_metadata = fs::symlink_metadata(directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect sealed Go semantic frontend directory `{}`: {error}",
            directory.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&directory_metadata) || !directory_metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed Go semantic frontend directory `{}` is unsafe.",
            directory.display()
        )));
    }
    let executable = directory.join(frontend_binary_name());
    if read_regular_file_no_follow_until(&executable, deadline)? != expected_bytes {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed Go semantic frontend `{}` failed content verification.",
            executable.display()
        )));
    }
    verify_sealed_permissions(directory, &directory_metadata, &executable)?;
    Ok(executable)
}

fn prepare_go_toolchain(
    cache_root: &Path,
    toolchain: GoToolchain,
) -> Result<PreparedGoToolchain, GoSemanticProcessError> {
    prepare_go_toolchain_until(
        cache_root,
        toolchain,
        false,
        GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
    )
}

fn prepare_go_toolchain_until(
    cache_root: &Path,
    toolchain: GoToolchain,
    offline: bool,
    deadline: GoOperationDeadline,
) -> Result<PreparedGoToolchain, GoSemanticProcessError> {
    deadline.check("Go toolchain preparation")?;
    let bytes = read_regular_file_no_follow_until(&toolchain.executable, deadline)?;
    let digest = security_digest_bytes(&bytes);
    if digest != toolchain.executable_digest {
        return Err(GoSemanticProcessError::CommandFailed(
            "selected Go executable changed during preparation.".to_string(),
        ));
    }
    let executable = seal_go_toolchain_executable_until(cache_root, &bytes, &digest, deadline)?;
    let runtime_search_path = sealed_toolchain_path(&executable)?;
    let environment = CertifiedGoEnvironment::capture(cache_root, offline)?;
    let mut prepared = PreparedGoToolchain {
        executable,
        executable_digest: digest,
        canonical_selection: toolchain.canonical_selection,
        version: toolchain.version,
        host_target: toolchain.host_target,
        goroot: toolchain.goroot,
        runtime_search_path,
        closure: toolchain.closure,
        environment,
    };
    let (version, host_target) = probe_prepared_go_toolchain_until(&prepared, deadline)?;
    if host_target != GoHostTarget::current_process()? {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "selected Go executable targets {}, but polint is running on {}.",
            host_target.label(),
            GoHostTarget::current_process()?.label()
        )));
    }
    prepared.version = version;
    prepared.host_target = host_target;
    Ok(prepared)
}

fn probe_prepared_go_toolchain(
    toolchain: &PreparedGoToolchain,
) -> Result<(String, GoHostTarget), GoSemanticProcessError> {
    probe_prepared_go_toolchain_until(toolchain, GoOperationDeadline::after(GO_PROBE_TIMEOUT))
}

fn probe_prepared_go_toolchain_until(
    toolchain: &PreparedGoToolchain,
    deadline: GoOperationDeadline,
) -> Result<(String, GoHostTarget), GoSemanticProcessError> {
    let mut command = Command::new(&toolchain.executable);
    configure_go_probe_environment(
        &mut command,
        &toolchain.host_target,
        &toolchain.runtime_search_path,
    );
    command
        .env("GOROOT", &toolchain.goroot)
        .env("LANG", "C")
        .env("LC_ALL", "C");
    command.arg("version");
    let output = run_prepared_go_command_until(
        toolchain,
        command,
        BoundedCommandLimits::new(
            GO_PROBE_TIMEOUT,
            GO_PROBE_STDOUT_BYTES,
            GO_PROBE_STDERR_BYTES,
        ),
        deadline.cap(GO_PROBE_TIMEOUT),
        "sealed `go version` for Go semantic frontend",
    )?;
    if !output.status.success() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed `go version` exited with status {}",
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_go_toolchain(stdout.as_ref()).ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to parse sealed Go toolchain version and host target from `{}`",
            stdout.trim()
        ))
    })
}

fn run_prepared_go_command(
    toolchain: &PreparedGoToolchain,
    command: Command,
    limits: BoundedCommandLimits,
    label: &str,
) -> Result<BoundedCommandOutput, GoSemanticProcessError> {
    run_prepared_go_command_until(
        toolchain,
        command,
        limits,
        GoOperationDeadline::after(limits.timeout),
        label,
    )
}

fn run_prepared_go_command_until(
    toolchain: &PreparedGoToolchain,
    command: Command,
    limits: BoundedCommandLimits,
    deadline: GoOperationDeadline,
    label: &str,
) -> Result<BoundedCommandOutput, GoSemanticProcessError> {
    run_prepared_go_command_with_local_trees_until(toolchain, command, &[], limits, deadline, label)
}

fn run_prepared_go_command_with_local_trees_until(
    toolchain: &PreparedGoToolchain,
    command: Command,
    additional_roots: &[PathBuf],
    limits: BoundedCommandLimits,
    deadline: GoOperationDeadline,
    label: &str,
) -> Result<BoundedCommandOutput, GoSemanticProcessError> {
    run_prepared_go_command_with_local_scope_until(
        toolchain,
        command,
        &[],
        additional_roots,
        limits,
        deadline,
        label,
    )
}

fn run_prepared_go_command_with_local_scope_until(
    toolchain: &PreparedGoToolchain,
    command: Command,
    containing_paths: &[PathBuf],
    recursive_roots: &[PathBuf],
    limits: BoundedCommandLimits,
    deadline: GoOperationDeadline,
    label: &str,
) -> Result<BoundedCommandOutput, GoSemanticProcessError> {
    deadline.check(label)?;
    verify_go_toolchain_binding_until(toolchain, deadline)?;
    deadline.check(label)?;
    require_local_existing_paths_until(
        containing_paths,
        deadline,
        "Go command containing-path certification",
    )?;
    let output = run_bounded_command_with_local_trees_until(
        command,
        recursive_roots,
        limits,
        deadline,
        label,
    )?;
    deadline.check(label)?;
    verify_go_toolchain_binding_until(toolchain, deadline)?;
    Ok(output)
}

fn seal_go_toolchain_executable_until(
    cache_root: &Path,
    bytes: &[u8],
    executable_digest: &str,
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    deadline.check("Go toolchain sealing")?;
    let toolchains_root = ensure_private_subdirectory(cache_root, Path::new("toolchains"))?;
    let directory = toolchains_root.join(executable_digest);
    match fs::symlink_metadata(&directory) {
        Ok(_) => return verify_sealed_go_toolchain_until(&directory, bytes, deadline),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect sealed Go toolchain directory `{}`: {error}",
                directory.display()
            )));
        }
    }

    let mut staging = StagingDirectory::create_until(&toolchains_root, ".toolchain-", deadline)?;
    write_new_private_file(
        &staging.path().join(go_toolchain_binary_name()),
        bytes,
        true,
    )?;
    seal_execution_directory(staging.path())?;
    let _capacity = published_cache_capacity_guard_until(
        cache_root,
        &toolchains_root,
        &directory,
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        GO_FRONTEND_MAX_PUBLISHED_TOOLCHAINS,
        GO_FRONTEND_MAX_PUBLISHED_TOOLCHAIN_BYTES,
        deadline,
    )?;
    match fs::rename(staging.path(), &directory) {
        Ok(()) => {
            staging.mark_published();
            verify_sealed_go_toolchain_until(&directory, bytes, deadline)
        }
        Err(_) if fs::symlink_metadata(&directory).is_ok() => {
            verify_sealed_go_toolchain_until(&directory, bytes, deadline)
        }
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to publish sealed Go toolchain `{}`: {error}",
            directory.display()
        ))),
    }
}

fn verify_sealed_go_toolchain_until(
    directory: &Path,
    expected_bytes: &[u8],
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    deadline.check("sealed Go toolchain verification")?;
    let directory_metadata = fs::symlink_metadata(directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect sealed Go toolchain directory `{}`: {error}",
            directory.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&directory_metadata) || !directory_metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed Go toolchain directory `{}` is unsafe.",
            directory.display()
        )));
    }
    let executable = directory.join(go_toolchain_binary_name());
    if read_regular_file_no_follow_until(&executable, deadline)? != expected_bytes {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed Go toolchain `{}` failed content verification.",
            executable.display()
        )));
    }
    verify_sealed_permissions(directory, &directory_metadata, &executable)?;
    Ok(executable)
}

#[cfg(windows)]
fn go_toolchain_binary_name() -> &'static str {
    "go.exe"
}

#[cfg(not(windows))]
fn go_toolchain_binary_name() -> &'static str {
    "go"
}

#[cfg(unix)]
fn seal_execution_directory(directory: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(directory, fs::Permissions::from_mode(0o500)).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to make sealed Go semantic frontend directory immutable `{}`: {error}",
            directory.display()
        ))
    })
}

#[cfg(windows)]
fn seal_execution_directory(directory: &Path) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::windows::seal_private_path(directory, true).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to seal Go semantic frontend directory `{}`: {error}",
            directory.display()
        ))
    })
}

#[cfg(all(not(unix), not(windows)))]
fn seal_execution_directory(_directory: &Path) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn verify_sealed_permissions(
    _directory: &Path,
    directory_metadata: &fs::Metadata,
    executable: &Path,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let executable_metadata = fs::symlink_metadata(executable).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect sealed Go semantic frontend `{}`: {error}",
            executable.display()
        ))
    })?;
    let owned = directory_metadata.uid() == effective_user_id()
        && executable_metadata.uid() == effective_user_id();
    let modes_are_sealed = directory_metadata.permissions().mode() & 0o777 == 0o500
        && executable_metadata.permissions().mode() & 0o777 == 0o500;
    if !owned || !modes_are_sealed || !executable_metadata.is_file() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed Go semantic frontend `{}` failed ownership or permission verification.",
            executable.display()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn verify_sealed_permissions(
    directory: &Path,
    _directory_metadata: &fs::Metadata,
    executable: &Path,
) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::windows::verify_private_path(directory, true, true).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "sealed Go semantic frontend directory `{}` failed DACL verification: {error}",
            directory.display()
        ))
    })?;
    crate::go::semantic::windows::verify_private_path(executable, false, true).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "sealed Go semantic frontend `{}` failed DACL verification: {error}",
            executable.display()
        ))
    })
}

#[cfg(all(not(unix), not(windows)))]
fn verify_sealed_permissions(
    _directory: &Path,
    _directory_metadata: &fs::Metadata,
    _executable: &Path,
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

fn configure_go_environment(command: &mut Command, toolchain: &PreparedGoToolchain) {
    command.env_clear().envs(&toolchain.environment.variables);
    command
        .env("GOOS", &toolchain.host_target.os)
        .env("GOARCH", &toolchain.host_target.arch)
        .env("CGO_ENABLED", "0")
        .env("GOENV", "off")
        .env("GOFLAGS", "")
        .env("GO111MODULE", "on")
        .env("GOTOOLCHAIN", "local")
        .env("GOWORK", "off")
        .env("GOROOT", &toolchain.goroot)
        .env("PATH", &toolchain.runtime_search_path)
        .env("LANG", "C")
        .env("LC_ALL", "C");
}

fn sealed_toolchain_path(executable: &Path) -> Result<OsString, GoSemanticProcessError> {
    executable
        .parent()
        .map(|parent| parent.as_os_str().to_os_string())
        .ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "sealed Go toolchain executable has no parent directory.".to_string(),
            )
        })
}

#[cfg(unix)]
fn write_new_private_file(
    path: &Path,
    bytes: &[u8],
    executable: bool,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::OpenOptionsExt;

    let mode = if executable { 0o500 } else { 0o400 };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to create private Go semantic frontend file `{}`: {error}",
                path.display()
            ))
        })?;
    file.write_all(bytes).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to write private Go semantic frontend file `{}`: {error}",
            path.display()
        ))
    })?;
    file.sync_all().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to sync private Go semantic frontend file `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(unix)]
fn write_new_private_mutable_file(path: &Path, bytes: &[u8]) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to create private Go workspace file `{}`: {error}",
                path.display()
            ))
        })?;
    file.write_all(bytes).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to write private Go workspace file `{}`: {error}",
            path.display()
        ))
    })?;
    file.sync_all().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to sync private Go workspace file `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(unix)]
fn overwrite_private_mutable_file(path: &Path, bytes: &[u8]) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to open private Go workspace file `{}` for normalization: {error}",
                path.display()
            ))
        })?;
    if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "private Go workspace file `{}` is not regular.",
            path.display()
        )));
    }
    file.write_all(bytes).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to normalize private Go workspace file `{}`: {error}",
            path.display()
        ))
    })?;
    file.sync_all().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to sync normalized private Go workspace file `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(windows)]
fn write_new_private_file(
    path: &Path,
    bytes: &[u8],
    _executable: bool,
) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::windows::create_private_file(path, bytes, true).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to create sealed private Go semantic frontend file `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(windows)]
fn write_new_private_mutable_file(path: &Path, bytes: &[u8]) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::windows::create_private_file(path, bytes, false).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to create private Go workspace file `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(windows)]
fn overwrite_private_mutable_file(path: &Path, bytes: &[u8]) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::windows::overwrite_private_file(path, bytes).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to normalize private Go workspace file `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(all(not(unix), not(windows)))]
fn write_new_private_file(
    _path: &Path,
    _bytes: &[u8],
    _executable: bool,
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(all(not(unix), not(windows)))]
fn write_new_private_mutable_file(
    _path: &Path,
    _bytes: &[u8],
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go workspace preparation is unavailable on this platform.".to_string(),
    ))
}

#[cfg(all(not(unix), not(windows)))]
fn overwrite_private_mutable_file(
    _path: &Path,
    _bytes: &[u8],
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go workspace normalization is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn read_regular_file_no_follow(path: &Path) -> Result<Vec<u8>, GoSemanticProcessError> {
    read_regular_file_no_follow_until(path, GoOperationDeadline::after(GO_OPERATION_TIMEOUT))
}

#[cfg(unix)]
fn read_regular_file_no_follow_until(
    path: &Path,
    deadline: GoOperationDeadline,
) -> Result<Vec<u8>, GoSemanticProcessError> {
    use std::os::unix::fs::OpenOptionsExt;

    deadline.check("Go semantic frontend file reading")?;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to open Go semantic frontend file `{}` without following symlinks: {error}",
                path.display()
            ))
        })?;
    let metadata = file.metadata().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go semantic frontend file `{}`: {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file()
        || metadata.len() > u64::try_from(GO_FRONTEND_MAX_EXECUTABLE_BYTES).unwrap_or(u64::MAX)
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend file `{}` is not a bounded regular file.",
            path.display()
        )));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or_default());
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        deadline.check("Go semantic frontend file reading")?;
        let count = file.read(&mut buffer).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to read Go semantic frontend file `{}`: {error}",
                path.display()
            ))
        })?;
        if count == 0 {
            break;
        }
        if count > GO_FRONTEND_MAX_EXECUTABLE_BYTES.saturating_sub(bytes.len()) {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend file `{}` exceeds the size limit.",
                path.display()
            )));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let after = file.metadata().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to re-inspect Go semantic frontend file `{}`: {error}",
            path.display()
        ))
    })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != metadata.len()
        || !dependency_metadata_matches(&metadata, &after)
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend file `{}` changed while it was read.",
            path.display()
        )));
    }
    Ok(bytes)
}

#[cfg(windows)]
fn read_regular_file_no_follow(path: &Path) -> Result<Vec<u8>, GoSemanticProcessError> {
    read_regular_file_no_follow_until(path, GoOperationDeadline::after(GO_OPERATION_TIMEOUT))
}

#[cfg(windows)]
fn read_regular_file_no_follow_until(
    path: &Path,
    deadline: GoOperationDeadline,
) -> Result<Vec<u8>, GoSemanticProcessError> {
    validate_local_path_size_until(path, "Go semantic frontend file reading", deadline)?;
    let certified_path = path.to_path_buf();
    let contents = run_windows_file_io_certification(
        deadline,
        "Go semantic frontend file reading",
        move || {
            let file =
                crate::go::semantic::windows::SecureFile::open_regular_no_follow(&certified_path)
                    .map_err(|error| {
                    let context = format!(
                        "failed to open Go semantic frontend file `{}` securely: {error}",
                        certified_path.display()
                    );
                    windows_file_io_error(error, context)
                })?;
            file.read_bounded_until(GO_FRONTEND_MAX_EXECUTABLE_BYTES, deadline.end)
                .map_err(|error| {
                    let context = format!(
                        "failed to read Go semantic frontend file `{}` securely: {error}",
                        certified_path.display()
                    );
                    windows_file_io_error(error, context)
                })
        },
    )?;
    deadline.check("Go semantic frontend file reading")?;
    let digest: [u8; 32] = Sha256::digest(&contents.bytes).into();
    if digest != contents.sha256
        || u64::try_from(contents.bytes.len()).unwrap_or(u64::MAX) != contents.identity.size
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend file `{}` changed while it was read.",
            path.display()
        )));
    }
    Ok(contents.bytes)
}

#[cfg(windows)]
fn security_digest_toolchain_file_until(
    certified_scope: &crate::go::semantic::windows::CertifiedLocalTree,
    path: &Path,
    expected: &fs::Metadata,
    byte_limit: usize,
    deadline: GoOperationDeadline,
) -> Result<(u64, String), GoSemanticProcessError> {
    deadline.check("certified Go toolchain file hashing")?;
    let file = certified_scope
        .open_regular_no_follow(path)
        .map_err(|error| {
            let context = format!(
                "failed to open certified Go toolchain file `{}`: {error}",
                path.display()
            );
            windows_file_io_error(error, context)
        })?;
    if !windows_metadata_matches_identity(expected, file.identity()) {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "certified Go toolchain file `{}` changed before content hashing.",
            path.display()
        )));
    }
    let mut hasher = Sha256::new();
    let bytes_read = file
        .hash_into_until(
            &mut hasher,
            u64::try_from(byte_limit).unwrap_or(u64::MAX),
            deadline.end,
        )
        .map_err(|error| {
            let context = format!(
                "failed to hash certified Go toolchain file `{}`: {error}",
                path.display()
            );
            windows_file_io_error(error, context)
        })?;
    Ok((bytes_read, format!("{:x}", hasher.finalize())))
}

#[cfg(unix)]
fn security_digest_toolchain_file_until(
    path: &Path,
    expected: &fs::Metadata,
    byte_limit: usize,
    deadline: GoOperationDeadline,
) -> Result<(u64, String), GoSemanticProcessError> {
    use std::os::unix::fs::OpenOptionsExt;

    deadline.check("certified Go toolchain file hashing")?;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to open certified Go toolchain file `{}`: {error}",
                path.display()
            ))
        })?;
    let opened = file.metadata().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect certified Go toolchain file `{}`: {error}",
            path.display()
        ))
    })?;
    if !opened.is_file()
        || !dependency_metadata_matches(expected, &opened)
        || opened.len() > u64::try_from(byte_limit).unwrap_or(u64::MAX)
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "certified Go toolchain file `{}` changed before content hashing.",
            path.display()
        )));
    }
    let mut hasher = Sha256::new();
    let mut bytes_read = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        deadline.check("certified Go toolchain file hashing")?;
        let count = file.read(&mut buffer).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to hash certified Go toolchain file `{}`: {error}",
                path.display()
            ))
        })?;
        if count == 0 {
            break;
        }
        bytes_read = bytes_read.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
        if bytes_read > opened.len() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "certified Go toolchain file `{}` changed while it was hashed.",
                path.display()
            )));
        }
        hasher.update(&buffer[..count]);
    }
    let after = file.metadata().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to re-inspect certified Go toolchain file `{}`: {error}",
            path.display()
        ))
    })?;
    if bytes_read != opened.len() || !dependency_metadata_matches(&opened, &after) {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "certified Go toolchain file `{}` changed while it was hashed.",
            path.display()
        )));
    }
    Ok((bytes_read, format!("{:x}", hasher.finalize())))
}

#[cfg(all(not(unix), not(windows)))]
fn security_digest_toolchain_file_until(
    path: &Path,
    _expected: &fs::Metadata,
    byte_limit: usize,
    deadline: GoOperationDeadline,
) -> Result<(u64, String), GoSemanticProcessError> {
    security_digest_regular_file_until(path, byte_limit, deadline)
}

#[cfg(windows)]
fn run_windows_file_io_certification<T: Send + 'static>(
    deadline: GoOperationDeadline,
    operation: &'static str,
    work: impl FnOnce() -> Result<T, GoSemanticProcessError> + Send + 'static,
) -> Result<T, GoSemanticProcessError> {
    crate::go::semantic::windows::run_cancellable_file_io_pass(deadline.end, operation, work)
        .map_err(|error| {
            let context = format!("failed during {operation}: {error}");
            windows_file_io_error(error, context)
        })?
}

#[cfg(windows)]
fn windows_file_io_error(error: std::io::Error, context: String) -> GoSemanticProcessError {
    if error.kind() == std::io::ErrorKind::TimedOut {
        GoSemanticProcessError::Timeout(context)
    } else {
        GoSemanticProcessError::CommandFailed(context)
    }
}

#[cfg(all(not(unix), not(windows)))]
fn read_regular_file_no_follow(_path: &Path) -> Result<Vec<u8>, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(all(not(unix), not(windows)))]
fn read_regular_file_no_follow_until(
    _path: &Path,
    _deadline: GoOperationDeadline,
) -> Result<Vec<u8>, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(all(not(unix), not(windows)))]
fn security_digest_regular_file_until(
    _path: &Path,
    _byte_limit: usize,
    _deadline: GoOperationDeadline,
) -> Result<(u64, String), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "security-sensitive file hashing is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn make_file_executable_read_only(path: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o500)).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to seal Go semantic frontend executable `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(windows)]
fn make_file_executable_read_only(path: &Path) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::windows::seal_private_path(path, false).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to seal Go semantic frontend executable `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(all(not(unix), not(windows)))]
fn make_file_executable_read_only(_path: &Path) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn make_source_snapshot_read_only(
    root: &Path,
    snapshot: &FrontendSourceSnapshot,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    for file in &snapshot.files {
        fs::set_permissions(
            root.join(&file.relative_path),
            fs::Permissions::from_mode(0o400),
        )
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to seal Go semantic frontend source `{}`: {error}",
                file.relative_path.display()
            ))
        })?;
    }
    let mut directories = snapshot
        .files
        .iter()
        .flat_map(|file| file.relative_path.ancestors().skip(1))
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        fs::set_permissions(root.join(directory), fs::Permissions::from_mode(0o500)).map_err(
            |error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to seal Go semantic frontend source directory: {error}"
                ))
            },
        )?;
    }
    fs::set_permissions(root, fs::Permissions::from_mode(0o500)).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to seal Go semantic frontend source root `{}`: {error}",
            root.display()
        ))
    })
}

#[cfg(windows)]
fn make_source_snapshot_read_only(
    root: &Path,
    snapshot: &FrontendSourceSnapshot,
) -> Result<(), GoSemanticProcessError> {
    for file in &snapshot.files {
        let path = root.join(&file.relative_path);
        crate::go::semantic::windows::seal_private_path(&path, false).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to seal Go semantic frontend source `{}`: {error}",
                file.relative_path.display()
            ))
        })?;
    }
    let mut directories = snapshot
        .files
        .iter()
        .flat_map(|file| file.relative_path.ancestors().skip(1))
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        let path = root.join(directory);
        crate::go::semantic::windows::seal_private_path(&path, true).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to seal Go semantic frontend source directory `{}`: {error}",
                path.display()
            ))
        })?;
    }
    crate::go::semantic::windows::seal_private_path(root, true).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to seal Go semantic frontend source root `{}`: {error}",
            root.display()
        ))
    })
}

#[cfg(all(not(unix), not(windows)))]
fn make_source_snapshot_read_only(
    _root: &Path,
    _snapshot: &FrontendSourceSnapshot,
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn make_directory_tree_writable(root: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to reopen Go semantic frontend staging directory `{}`: {error}",
            root.display()
        ))
    })?;
    for entry in fs::read_dir(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go semantic frontend staging directory `{}`: {error}",
            root.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go semantic frontend staging entry: {error}"
            ))
        })?;
        if entry
            .file_type()
            .map_err(|_| {
                GoSemanticProcessError::CommandFailed(
                    "failed to inspect Go semantic frontend staging entry type.".to_string(),
                )
            })?
            .is_dir()
        {
            make_directory_tree_writable(&entry.path())?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn make_directory_tree_writable(root: &Path) -> Result<(), GoSemanticProcessError> {
    crate::go::semantic::windows::make_private_path_writable(root, true).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to reopen Go semantic frontend directory `{}`: {error}",
            root.display()
        ))
    })?;
    for entry in fs::read_dir(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go semantic frontend directory `{}`: {error}",
            root.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go semantic frontend entry: {error}"
            ))
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go semantic frontend entry `{}`: {error}",
                path.display()
            ))
        })?;
        if metadata_is_link_or_reparse(&metadata) {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "refusing to reopen Go semantic frontend reparse point `{}`.",
                path.display()
            )));
        }
        if metadata.is_dir() {
            make_directory_tree_writable(&path)?;
        } else if metadata.is_file() {
            crate::go::semantic::windows::make_private_path_writable(&path, false).map_err(
                |error| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to reopen Go semantic frontend file `{}`: {error}",
                        path.display()
                    ))
                },
            )?;
        } else {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend tree contains special entry `{}`.",
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn make_directory_tree_writable(_root: &Path) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

fn ensure_go_toolchain_supported(version: &str) -> Result<(), GoSemanticProcessError> {
    if !go_version_at_least(version, 1, 25) {
        return Err(GoSemanticProcessError::VersionUnsupported(format!(
            "polint-go-frontend source mode requires Go 1.25 or newer on PATH; found {version}"
        )));
    }
    Ok(())
}

pub(crate) fn local_go_toolchain_version() -> Result<String, GoSemanticProcessError> {
    local_go_toolchain().map(|toolchain| toolchain.version)
}

fn local_go_toolchain() -> Result<GoToolchain, GoSemanticProcessError> {
    local_go_toolchain_until(GoOperationDeadline::after(GO_OPERATION_TIMEOUT))
}

fn local_go_toolchain_until(
    deadline: GoOperationDeadline,
) -> Result<GoToolchain, GoSemanticProcessError> {
    deadline.check("Go toolchain selection")?;
    let search_path = std::env::var_os("PATH").ok_or_else(|| {
        GoSemanticProcessError::CommandUnavailable(
            "PATH is unavailable while resolving the Go semantic toolchain.".to_string(),
        )
    })?;
    local_go_toolchain_in_until(search_path, deadline)
}

fn local_go_toolchain_in(search_path: OsString) -> Result<GoToolchain, GoSemanticProcessError> {
    local_go_toolchain_in_until(
        search_path,
        GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
    )
}

fn local_go_toolchain_in_until(
    search_path: OsString,
    deadline: GoOperationDeadline,
) -> Result<GoToolchain, GoSemanticProcessError> {
    deadline.check("Go toolchain selection")?;
    let canonical_selection = resolve_go_executable(&search_path, deadline)?;
    require_local_existing_path_until(
        &canonical_selection,
        deadline,
        "Go toolchain executable certification",
    )?;
    let goroot_candidate = canonical_selection
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            GoSemanticProcessError::CommandUnavailable(format!(
                "selected Go executable `{}` is not located under GOROOT/bin.",
                canonical_selection.display()
            ))
        })?;
    require_local_existing_path_until(
        goroot_candidate,
        deadline,
        "Go toolchain root certification",
    )?;
    let goroot = infer_go_root(&canonical_selection)?;
    let executable_bytes = read_regular_file_no_follow_until(&canonical_selection, deadline)?;
    let executable_digest = security_digest_bytes(&executable_bytes);
    let target = GoHostTarget::current_process()?;

    let mut version_command = Command::new(&canonical_selection);
    let probe_path = canonical_selection
        .parent()
        .map(|path| path.as_os_str().to_os_string())
        .ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "selected Go executable has no parent directory.".to_string(),
            )
        })?;
    configure_go_probe_environment(&mut version_command, &target, &probe_path);
    version_command.arg("version");
    let output = run_bounded_command_with_local_trees_until(
        version_command,
        std::slice::from_ref(&goroot),
        BoundedCommandLimits::new(
            GO_PROBE_TIMEOUT,
            GO_PROBE_STDOUT_BYTES,
            GO_PROBE_STDERR_BYTES,
        ),
        deadline.cap(GO_PROBE_TIMEOUT),
        "`go version` for Go semantic frontend",
    )?;
    if !output.status.success() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "`go version` exited with status {}",
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let (version, host_target) = parse_go_toolchain(stdout.as_ref()).ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to parse Go toolchain version and host target from `{}`",
            stdout.trim()
        ))
    })?;

    let closure = capture_go_toolchain_closure_until(&goroot, &host_target, true, deadline)?;

    Ok(GoToolchain {
        executable: canonical_selection.clone(),
        executable_digest,
        canonical_selection,
        version,
        host_target,
        goroot,
        closure,
    })
}

fn capture_go_toolchain_closure(
    goroot: &Path,
    host_target: &GoHostTarget,
    hash_delegated_tools: bool,
) -> Result<GoToolchainClosure, GoSemanticProcessError> {
    capture_go_toolchain_closure_until(
        goroot,
        host_target,
        hash_delegated_tools,
        GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
    )
}

fn capture_go_toolchain_closure_until(
    goroot: &Path,
    host_target: &GoHostTarget,
    hash_delegated_tools: bool,
    deadline: GoOperationDeadline,
) -> Result<GoToolchainClosure, GoSemanticProcessError> {
    validate_local_path_size_until(goroot, "Go toolchain closure certification", deadline)?;
    #[cfg(windows)]
    {
        let goroot = goroot.to_path_buf();
        let host_target = host_target.clone();
        return run_windows_file_io_certification(
            deadline,
            "Go toolchain closure certification",
            move || {
                let certified_scope = crate::go::semantic::windows::certified_local_tree_until(
                    &goroot,
                    &[],
                    deadline.end,
                )
                .map_err(|error| {
                    windows_file_io_error(
                        error,
                        format!(
                            "failed to certify Go toolchain closure `{}`",
                            goroot.display()
                        ),
                    )
                })?;
                capture_go_toolchain_closure_inner(
                    &goroot,
                    &host_target,
                    hash_delegated_tools,
                    deadline,
                    None,
                    &certified_scope,
                )
            },
        );
    }
    #[cfg(not(windows))]
    capture_go_toolchain_closure_inner(goroot, host_target, hash_delegated_tools, deadline, None)
}

type ToolchainContentHashHook<'a> = dyn Fn(&Path, bool) + Sync + 'a;

fn capture_go_toolchain_closure_inner(
    goroot: &Path,
    host_target: &GoHostTarget,
    hash_delegated_tools: bool,
    deadline: GoOperationDeadline,
    content_hash_hook: Option<&ToolchainContentHashHook<'_>>,
    #[cfg(windows)] certified_scope: &crate::go::semantic::windows::CertifiedLocalTree,
) -> Result<GoToolchainClosure, GoSemanticProcessError> {
    #[cfg(not(windows))]
    require_local_scan_root(goroot, deadline)?;
    #[derive(Debug)]
    struct ToolchainClosureFile {
        relative_path: String,
        path: PathBuf,
        metadata: fs::Metadata,
        size: u64,
    }

    let delegated_tool_directory =
        PathBuf::from("pkg/tool").join(format!("{}_{}", host_target.os, host_target.arch));
    let root_metadata = fs::symlink_metadata(goroot).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go toolchain root `{}`: {error}",
            goroot.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&root_metadata) || !root_metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go toolchain root must be a direct regular directory.".to_string(),
        ));
    }
    let mut root_metadata_hasher = Sha256::new();
    root_metadata_hasher.update(b"polint-go-toolchain-root-metadata-v1");
    #[cfg(windows)]
    hash_toolchain_scoped_metadata(
        &mut root_metadata_hasher,
        certified_scope,
        goroot,
        &root_metadata,
    )?;
    #[cfg(not(windows))]
    hash_toolchain_metadata(&mut root_metadata_hasher, goroot, &root_metadata)?;
    let root_metadata_digest = format!("{:x}", root_metadata_hasher.finalize());
    let mut metadata_hasher = Sha256::new();
    metadata_hasher.update(b"polint-go-toolchain-metadata-v2");
    let mut frontier = vec![goroot.to_path_buf()];
    let mut entry_count = 0_usize;
    let mut byte_count = 0_u64;
    let mut closure_files = Vec::new();
    let mut content_entries = Vec::new();
    let mut delegated_tool_count = 0_usize;
    let mut launcher_seen = false;

    while let Some(directory) = frontier.pop() {
        deadline.check("Go toolchain closure certification")?;
        let directory_entries = fs::read_dir(&directory).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to enumerate Go toolchain closure `{}`: {error}",
                directory.display()
            ))
        })?;
        let mut entries = Vec::new();
        for entry in directory_entries {
            deadline.check("Go toolchain closure certification")?;
            let entry = entry.map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to enumerate Go toolchain closure `{}`: {error}",
                    directory.display()
                ))
            })?;
            push_bounded_directory_entry(
                &mut entries,
                entry,
                entry_count,
                GO_TOOLCHAIN_MAX_CLOSURE_ENTRIES,
                usize::from(!launcher_seen),
                "Go toolchain closure",
            )?;
        }
        deadline.check("Go toolchain closure certification")?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            deadline.check("Go toolchain closure certification")?;
            let path = entry.path();
            let relative = path.strip_prefix(goroot).map_err(|_| {
                GoSemanticProcessError::CommandFailed(
                    "Go toolchain closure escaped its selected root.".to_string(),
                )
            })?;
            let relative = relative.to_str().ok_or_else(|| {
                GoSemanticProcessError::CommandFailed(
                    "Go toolchain closure contains a non-Unicode path.".to_string(),
                )
            })?;
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go toolchain closure entry `{}`: {error}",
                    path.display()
                ))
            })?;
            if metadata_is_link_or_reparse(&metadata) {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go toolchain closure entry `{}` must not be a symlink.",
                    path.display()
                )));
            }
            if Path::new(relative) == Path::new("bin").join(go_toolchain_binary_name()) {
                if !metadata.is_file() {
                    return Err(GoSemanticProcessError::CommandFailed(
                        "selected Go launcher path is not a regular file.".to_string(),
                    ));
                }
                launcher_seen = true;
                // Runtime commands use the owner-private sealed launcher. Its
                // original selection path is therefore acquisition metadata,
                // and the executable bytes are bound separately.
                continue;
            }
            entry_count = entry_count.saturating_add(1);
            if entry_count > GO_TOOLCHAIN_MAX_CLOSURE_ENTRIES {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go toolchain closure contains more than {GO_TOOLCHAIN_MAX_CLOSURE_ENTRIES} entries."
                )));
            }
            hash_length_prefixed(&mut metadata_hasher, relative.as_bytes());
            if metadata.is_dir() {
                metadata_hasher.update(b"directory");
                #[cfg(windows)]
                let access = hash_toolchain_scoped_metadata(
                    &mut metadata_hasher,
                    certified_scope,
                    &path,
                    &metadata,
                )?;
                #[cfg(not(windows))]
                let access = {
                    hash_toolchain_metadata(&mut metadata_hasher, &path, &metadata)?;
                    toolchain_access_projection(&path, &metadata)
                };
                if hash_delegated_tools {
                    content_entries.push((relative.to_string(), false, 0_u64, access));
                }
                frontier.push(path);
            } else if metadata.is_file() {
                metadata_hasher.update(b"file");
                #[cfg(windows)]
                let access = hash_toolchain_scoped_metadata(
                    &mut metadata_hasher,
                    certified_scope,
                    &path,
                    &metadata,
                )?;
                #[cfg(not(windows))]
                let access = {
                    hash_toolchain_metadata(&mut metadata_hasher, &path, &metadata)?;
                    toolchain_access_projection(&path, &metadata)
                };
                if hash_delegated_tools {
                    content_entries.push((relative.to_string(), true, metadata.len(), access));
                }
                byte_count = byte_count.checked_add(metadata.len()).ok_or_else(|| {
                    GoSemanticProcessError::CommandFailed(
                        "Go toolchain closure byte count overflowed.".to_string(),
                    )
                })?;
                if byte_count > GO_TOOLCHAIN_MAX_CLOSURE_BYTES {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "Go toolchain closure exceeds the {GO_TOOLCHAIN_MAX_CLOSURE_BYTES}-byte limit."
                    )));
                }
                if Path::new(relative).starts_with(&delegated_tool_directory) {
                    delegated_tool_count = delegated_tool_count.saturating_add(1);
                }
                if hash_delegated_tools {
                    closure_files.push(ToolchainClosureFile {
                        relative_path: relative.to_string(),
                        path,
                        size: metadata.len(),
                        metadata,
                    });
                }
            } else {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go toolchain closure entry `{}` is not a regular file or directory.",
                    path.display()
                )));
            }
        }
    }

    if delegated_tool_count == 0 {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go toolchain closure contains no delegated tools under `{}`.",
            delegated_tool_directory.display()
        )));
    }
    let metadata_digest = format!("{:x}", metadata_hasher.finalize());
    let content_digest = if hash_delegated_tools {
        closure_files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let hash_file = |tool: &ToolchainClosureFile| {
            deadline.check("delegated Go tool certification")?;
            if let Some(hook) = content_hash_hook {
                hook(&tool.path, true);
            }
            #[cfg(windows)]
            let result = security_digest_toolchain_file_until(
                certified_scope,
                &tool.path,
                &tool.metadata,
                usize::try_from(GO_TOOLCHAIN_MAX_CLOSURE_BYTES).unwrap_or(usize::MAX),
                deadline,
            );
            #[cfg(not(windows))]
            let result = security_digest_toolchain_file_until(
                &tool.path,
                &tool.metadata,
                usize::try_from(GO_TOOLCHAIN_MAX_CLOSURE_BYTES).unwrap_or(usize::MAX),
                deadline,
            );
            if let Some(hook) = content_hash_hook {
                hook(&tool.path, false);
            }
            let (size, digest) = result?;
            if size != tool.size {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "delegated Go tool `{}` changed while its identity was captured.",
                    tool.path.display()
                )));
            }
            Ok((tool.relative_path.clone(), tool.size, digest))
        };
        #[cfg(windows)]
        let digests = closure_files
            .iter()
            .map(hash_file)
            .collect::<Result<Vec<_>, GoSemanticProcessError>>()?;
        #[cfg(not(windows))]
        let digests = closure_files
            .par_iter()
            .map(hash_file)
            .collect::<Result<Vec<_>, GoSemanticProcessError>>()?;
        let mut hasher = Sha256::new();
        hasher.update(b"polint-go-toolchain-content-v2");
        content_entries.sort_by(|left, right| left.0.cmp(&right.0));
        for (relative_path, file, size, access) in content_entries {
            hash_length_prefixed(&mut hasher, relative_path.as_bytes());
            hasher.update([u8::from(file)]);
            hasher.update(size.to_le_bytes());
            hasher.update(access.to_le_bytes());
        }
        for (relative_path, size, digest) in digests {
            hash_length_prefixed(&mut hasher, relative_path.as_bytes());
            hasher.update(size.to_le_bytes());
            hash_length_prefixed(&mut hasher, digest.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    } else {
        String::new()
    };
    let content_digest = security_digest_strings(&[
        format!("closure={content_digest}"),
        format!("host_target={}", host_target.label()),
    ]);
    let digest = security_digest_strings(&[
        format!("metadata={metadata_digest}"),
        format!("content={content_digest}"),
    ]);
    Ok(GoToolchainClosure {
        digest,
        content_digest,
        metadata_digest,
        root_metadata_digest,
        entry_count,
        byte_count,
        delegated_tool_count,
    })
}

fn verify_go_toolchain_binding_until(
    toolchain: &PreparedGoToolchain,
    deadline: GoOperationDeadline,
) -> Result<(), GoSemanticProcessError> {
    #[cfg(windows)]
    {
        let toolchain = toolchain.clone();
        run_windows_file_io_certification(
            deadline,
            "Go toolchain binding verification",
            move || {
                let certified_scope = crate::go::semantic::windows::certified_local_tree_until(
                    &toolchain.goroot,
                    &[],
                    deadline.end,
                )
                .map_err(|error| {
                    windows_file_io_error(
                        error,
                        format!(
                            "failed to recertify Go toolchain closure `{}`",
                            toolchain.goroot.display()
                        ),
                    )
                })?;
                verify_go_toolchain_binding_inner(&toolchain, deadline, &certified_scope)
            },
        )
    }
    #[cfg(not(windows))]
    verify_go_toolchain_binding_inner(toolchain, deadline)
}

fn verify_go_toolchain_binding_inner(
    toolchain: &PreparedGoToolchain,
    deadline: GoOperationDeadline,
    #[cfg(windows)] certified_scope: &crate::go::semantic::windows::CertifiedLocalTree,
) -> Result<(), GoSemanticProcessError> {
    deadline.check("Go toolchain binding verification")?;
    // The copied `go` launcher still delegates to tools and source data under
    // GOROOT. Re-certify the bounded closure metadata before and after every
    // command so a changed input can never authenticate output under the
    // content identity captured during preparation. File bytes are not
    // rehashed here; stable file identity, size, and change metadata make this
    // a bounded O(entries) check rather than another O(bytes) content pass.
    #[cfg(windows)]
    let current = capture_go_toolchain_closure_inner(
        &toolchain.goroot,
        &toolchain.host_target,
        false,
        deadline,
        None,
        certified_scope,
    )?;
    #[cfg(not(windows))]
    let current = capture_go_toolchain_closure_inner(
        &toolchain.goroot,
        &toolchain.host_target,
        false,
        deadline,
        None,
    )?;
    if current.metadata_digest != toolchain.closure.metadata_digest
        || current.root_metadata_digest != toolchain.closure.root_metadata_digest
        || current.entry_count != toolchain.closure.entry_count
        || current.byte_count != toolchain.closure.byte_count
        || current.delegated_tool_count != toolchain.closure.delegated_tool_count
    {
        return Err(GoSemanticProcessError::CommandFailed(
            "selected Go toolchain closure changed after preparation.".to_string(),
        ));
    }
    let executable_directory = toolchain.executable.parent().ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(
            "sealed Go toolchain executable has no parent directory.".to_string(),
        )
    })?;
    let directory_metadata = fs::symlink_metadata(executable_directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect sealed Go toolchain directory: {error}"
        ))
    })?;
    if metadata_is_link_or_reparse(&directory_metadata) || !directory_metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(
            "sealed Go toolchain directory is unsafe.".to_string(),
        ));
    }
    verify_sealed_permissions(
        executable_directory,
        &directory_metadata,
        &toolchain.executable,
    )
}

#[cfg(unix)]
fn hash_toolchain_metadata(
    hasher: &mut Sha256,
    _path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::MetadataExt;

    for value in [
        metadata.dev(),
        metadata.ino(),
        u64::from(metadata.mode()),
        metadata.len(),
        u64::try_from(metadata.mtime()).unwrap_or(u64::MAX),
        u64::try_from(metadata.mtime_nsec()).unwrap_or(u64::MAX),
        u64::try_from(metadata.ctime()).unwrap_or(u64::MAX),
        u64::try_from(metadata.ctime_nsec()).unwrap_or(u64::MAX),
    ] {
        hasher.update(value.to_le_bytes());
    }
    Ok(())
}

#[cfg(windows)]
fn hash_toolchain_scoped_metadata(
    hasher: &mut Sha256,
    certified_scope: &crate::go::semantic::windows::CertifiedLocalTree,
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<u32, GoSemanticProcessError> {
    let state = certified_scope
        .read_execute_state(path, metadata.is_dir())
        .map_err(|error| {
            windows_file_io_error(
                error,
                format!(
                    "failed to capture scoped Go toolchain identity for `{}`",
                    path.display()
                ),
            )
        })?;
    if !windows_metadata_matches_identity(metadata, state.identity) {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go toolchain entry `{}` changed during scoped identity capture.",
            path.display()
        )));
    }
    let access = state.effective_access.projection();
    hash_windows_file_identity(hasher, state.identity, access);
    Ok(access)
}

#[cfg(windows)]
fn hash_local_scoped_metadata(
    hasher: &mut Sha256,
    certified_scope: &crate::go::semantic::windows::CertifiedLocalTree,
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), GoSemanticProcessError> {
    let identity = certified_scope.identity_no_follow(path).map_err(|error| {
        windows_file_io_error(
            error,
            format!(
                "failed to capture scoped local Go input identity for `{}`",
                path.display()
            ),
        )
    })?;
    if !windows_metadata_matches_identity(metadata, identity) {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "local Go input `{}` changed during scoped identity capture.",
            path.display()
        )));
    }
    hash_windows_file_identity(hasher, identity, 0);
    Ok(())
}

#[cfg(windows)]
fn hash_windows_file_identity(
    hasher: &mut Sha256,
    identity: crate::go::semantic::windows::WindowsFileIdentity,
    access_projection: u32,
) {
    hasher.update(identity.volume_serial_number.to_le_bytes());
    hasher.update(identity.file_id);
    hasher.update(identity.size.to_le_bytes());
    hasher.update(identity.creation_time.to_le_bytes());
    hasher.update(identity.last_write_time.to_le_bytes());
    hasher.update(identity.change_time.to_le_bytes());
    hasher.update(identity.attributes.to_le_bytes());
    hasher.update([u8::from(identity.directory)]);
    hasher.update(access_projection.to_le_bytes());
}

#[cfg(unix)]
fn toolchain_access_projection(_path: &Path, metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111
}

#[cfg(all(not(unix), not(windows)))]
fn toolchain_access_projection(_path: &Path, metadata: &fs::Metadata) -> u32 {
    u32::from(metadata.permissions().readonly())
}

#[cfg(all(not(unix), not(windows)))]
fn hash_toolchain_metadata(
    hasher: &mut Sha256,
    _path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), GoSemanticProcessError> {
    hasher.update(metadata.len().to_le_bytes());
    hasher.update([u8::from(metadata.permissions().readonly())]);
    if let Ok(modified) = metadata.modified()
        && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
    {
        hasher.update(duration.as_secs().to_le_bytes());
        hasher.update(u64::from(duration.subsec_nanos()).to_le_bytes());
    }
    Ok(())
}

fn hash_length_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

fn infer_go_root(executable: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    let goroot = executable.parent().and_then(Path::parent).ok_or_else(|| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "selected Go executable `{}` is not located under GOROOT/bin.",
            executable.display()
        ))
    })?;
    let goroot = fs::canonicalize(goroot).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize selected Go toolchain root `{}`: {error}",
            goroot.display()
        ))
    })?;
    if !goroot.is_dir() {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "selected Go toolchain root `{}` is not a directory.",
            goroot.display()
        )));
    }
    Ok(goroot)
}

fn resolve_go_executable(
    search_path: &OsString,
    deadline: GoOperationDeadline,
) -> Result<PathBuf, GoSemanticProcessError> {
    for mut directory in std::env::split_paths(search_path) {
        deadline.check("Go toolchain selection")?;
        if directory.as_os_str().is_empty() {
            directory = PathBuf::from(".");
        }
        let Some(directory) = lexically_normalized_absolute_path(&directory) else {
            continue;
        };
        let candidate = directory.join(go_toolchain_binary_name());
        require_local_creation_root_until(&candidate, deadline, "Go toolchain selection")?;
        let Ok(metadata) = fs::symlink_metadata(&candidate) else {
            continue;
        };
        #[cfg(windows)]
        if metadata_is_link_or_reparse(&metadata) {
            return Err(GoSemanticProcessError::CommandUnavailable(format!(
                "selected Go executable `{}` must not be a symlink or reparse point.",
                candidate.display()
            )));
        }
        #[cfg(windows)]
        if !metadata.is_file() {
            continue;
        }
        #[cfg(not(windows))]
        if metadata.is_dir() {
            continue;
        }
        require_local_existing_path_until(&candidate, deadline, "Go toolchain selection")?;
        let canonical = fs::canonicalize(&candidate).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to canonicalize selected Go executable `{}`: {error}",
                candidate.display()
            ))
        })?;
        require_local_existing_path_until(&canonical, deadline, "Go toolchain selection")?;
        if canonical.is_file() {
            return Ok(canonical);
        }
    }
    Err(GoSemanticProcessError::CommandUnavailable(
        "go executable was not found for the Go semantic frontend.".to_string(),
    ))
}

fn configure_go_probe_environment(
    command: &mut Command,
    target: &GoHostTarget,
    search_path: &OsString,
) {
    command.env_clear();
    command
        .env("GOOS", &target.os)
        .env("GOARCH", &target.arch)
        .env("CGO_ENABLED", "0")
        .env("GOENV", "off")
        .env("GOFLAGS", "")
        .env("GO111MODULE", "on")
        .env("GOTOOLCHAIN", "local")
        .env("GOWORK", "off")
        .env("PATH", search_path);
}

fn parse_go_toolchain(output: &str) -> Option<(String, GoHostTarget)> {
    let mut parts = output.split_whitespace();
    let _go = parts.next()?;
    let _version_word = parts.next()?;
    let version = parts.next()?.to_string();
    let host_target = parts.next().and_then(GoHostTarget::parse_label)?;
    Some((version, host_target))
}

fn parse_go_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|part| part.starts_with("go") && part.len() > 2)
        .map(str::to_string)
}

fn go_version_at_least(version: &str, min_major: u32, min_minor: u32) -> bool {
    let Some((major, minor)) = parse_go_version_numbers(version) else {
        return false;
    };
    major > min_major || (major == min_major && minor >= min_minor)
}

fn parse_go_version_numbers(version: &str) -> Option<(u32, u32)> {
    let version = version.trim().trim_start_matches("go");
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

#[cfg(all(test, any(windows, target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    struct ChildCleanupGuard(Child);

    #[cfg(target_os = "linux")]
    impl ChildCleanupGuard {
        fn new(child: Child) -> Self {
            Self(child)
        }

        fn id(&self) -> u32 {
            self.0.id()
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for ChildCleanupGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn process_containment_support_matches_the_compile_time_platform_contract() {
        assert_eq!(
            go_semantic_process_containment_supported(),
            cfg!(any(windows, target_os = "linux", target_os = "macos"))
        );
    }

    #[test]
    fn test_go_semantic_concurrency_queue_is_fifo_and_cancellation_safe() {
        let mut state = TestGoSemanticConcurrencyState::default();
        let first = state.enqueue().expect("enqueue first waiter");
        let second = state.enqueue().expect("enqueue second waiter");
        let third = state.enqueue().expect("enqueue third waiter");

        assert!(state.can_admit(first));
        assert!(!state.can_admit(second));
        state.admit(first);
        assert!(!state.can_admit(second));

        state.release();
        assert!(state.can_admit(second));
        state.cancel(second);
        assert!(state.can_admit(third));
        state.admit(third);
        state.release();

        assert_eq!(state.active, 0);
        assert!(state.waiters.is_empty());
    }

    #[test]
    fn test_go_semantic_concurrency_scopes_allow_out_of_order_drop() {
        let outer = acquire_test_go_semantic_concurrency_scope().expect("acquire outer scope");
        let inner = acquire_test_go_semantic_concurrency_scope().expect("acquire inner scope");

        drop(outer);
        let reused =
            TestGoSemanticConcurrencyPermit::acquire(GoOperationDeadline::after(Duration::ZERO))
                .expect("the remaining inner scope keeps the thread-local permit reusable");
        drop(reused);
        drop(inner);

        TEST_GO_SEMANTIC_SCOPED_PERMIT.with(|slot| {
            assert!(slot.borrow().is_none());
        });
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn oversized_local_certification_path_is_rejected_before_ownership_clone() {
        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_vec(vec![b'x'; 1_048_577]))
        };
        #[cfg(windows)]
        let path = {
            use std::os::windows::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_wide(&vec![
                u16::from(b'x');
                32_768
            ]))
        };

        let error = require_local_existing_path_until(
            &path,
            GoOperationDeadline::after(Duration::from_secs(1)),
            "oversized path regression",
        )
        .expect_err("the raw path bound must run before any ownership clone or filesystem I/O");

        assert!(matches!(
            error,
            GoSemanticProcessError::CommandUnavailable(_)
        ));
        assert!(error.to_string().contains("oversized filesystem path"));
    }

    #[test]
    fn local_certification_path_batches_are_bounded_before_duplication() {
        let paths = vec![PathBuf::new(); MAX_LOCAL_CERTIFICATION_PATHS + 1];

        let error = validate_local_path_batch(&paths, "path batch regression", None)
            .expect_err("an oversized path batch must fail before duplication");

        assert!(matches!(
            error,
            GoSemanticProcessError::CommandUnavailable(_)
        ));
        assert!(error.to_string().contains("path certification limit"));
    }

    #[test]
    fn local_certification_path_batches_share_count_and_path_unit_limits() {
        let directories = vec![PathBuf::from("abc")];
        let files = vec![PathBuf::from("def")];

        let count_error = validate_local_path_batches_with_limits(
            &[&directories, &files],
            "combined path-count regression",
            None,
            1,
            6,
        )
        .expect_err("separate batches must share one path-count limit");
        assert!(
            count_error
                .to_string()
                .contains("1-path certification limit")
        );

        let unit_error = validate_local_path_batches_with_limits(
            &[&directories, &files],
            "combined path-unit regression",
            None,
            2,
            5,
        )
        .expect_err("separate batches must share one aggregate path-unit limit");
        assert!(unit_error.to_string().contains("aggregate path limit"));
    }

    #[cfg(unix)]
    #[test]
    fn local_certification_path_batches_bound_aggregate_path_bytes() {
        use std::os::unix::ffi::OsStringExt;

        let paths = (0..17)
            .map(|_| PathBuf::from(std::ffi::OsString::from_vec(vec![b'x'; 1024 * 1024])))
            .collect::<Vec<_>>();

        let error = validate_local_path_batch(&paths, "aggregate path regression", None)
            .expect_err("aggregate path storage must be bounded before duplication");

        assert!(matches!(
            error,
            GoSemanticProcessError::CommandUnavailable(_)
        ));
        assert!(error.to_string().contains("aggregate path limit"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_io_deadline_preserves_timeout_typing() {
        let error = windows_file_io_error(
            std::io::Error::new(std::io::ErrorKind::TimedOut, "deadline"),
            "bounded file read timed out".to_string(),
        );

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert_eq!(error.stable_reason(), "timeout");
    }

    #[cfg(windows)]
    #[test]
    fn windows_local_tree_deadline_preserves_process_timeout_typing() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let error = require_local_scan_root(temp.path(), GoOperationDeadline::at(Instant::now()))
            .expect_err("an expired local-tree deadline must remain a timeout");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert_eq!(error.stable_reason(), "timeout");
    }

    #[cfg(unix)]
    #[test]
    fn readdir_null_distinguishes_eof_from_an_enumeration_error() {
        assert!(
            classify_readdir_result(std::ptr::null_mut(), 0)
                .expect("zero errno is end-of-directory")
                .is_none()
        );

        let error = classify_readdir_result(std::ptr::null_mut(), libc::EIO)
            .expect_err("nonzero errno must fail closed");
        assert!(error.to_string().contains("failed to enumerate"));
    }

    #[cfg(any(unix, windows))]
    fn test_cache_root(temp: &tempfile::TempDir) -> PathBuf {
        temp.path().join("cache")
    }

    fn snapshot_repository_tree(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
        fn visit(root: &Path, directory: &Path, snapshot: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
            let mut entries = fs::read_dir(directory)
                .expect("read repository directory")
                .collect::<Result<Vec<_>, _>>()
                .expect("read repository entry");
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .expect("repository entry is below root")
                    .to_path_buf();
                let metadata = fs::symlink_metadata(&path).expect("inspect repository entry");
                if metadata.is_dir() {
                    snapshot.push((relative, None));
                    visit(root, &path, snapshot);
                } else {
                    assert!(
                        metadata.is_file(),
                        "unexpected repository entry: {relative:?}"
                    );
                    snapshot.push((
                        relative,
                        Some(fs::read(&path).expect("read repository file")),
                    ));
                }
            }
        }

        let mut snapshot = Vec::new();
        visit(root, root, &mut snapshot);
        snapshot
    }

    fn prepared_toolchain_for_test(version: &str) -> PreparedGoToolchain {
        PreparedGoToolchain {
            executable: PathBuf::from("/sealed/toolchain/go"),
            executable_digest: format!("digest-{version}"),
            canonical_selection: PathBuf::from("/selected/bin/go"),
            version: version.to_string(),
            host_target: GoHostTarget {
                os: "linux".to_string(),
                arch: "amd64".to_string(),
            },
            goroot: PathBuf::from("/selected/go"),
            runtime_search_path: OsString::from("/sealed/toolchain"),
            closure: GoToolchainClosure {
                digest: format!("closure-{version}"),
                content_digest: format!("content-{version}"),
                metadata_digest: format!("metadata-{version}"),
                root_metadata_digest: format!("root-metadata-{version}"),
                entry_count: 1,
                byte_count: 1,
                delegated_tool_count: 1,
            },
            environment: CertifiedGoEnvironment::for_test(),
        }
    }

    #[cfg(windows)]
    fn create_windows_descendant_reparse(root: &Path, target: &Path) {
        use std::os::windows::fs::symlink_dir;

        const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

        let link = root.join("descendant-link");
        match symlink_dir(target, &link) {
            Ok(()) => {}
            Err(error) if error.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD) => {
                let output = Command::new("cmd")
                    .args(["/D", "/C", "mklink", "/J"])
                    .arg(&link)
                    .arg(target)
                    .output()
                    .expect("run junction fallback");
                assert!(
                    output.status.success(),
                    "junction fallback failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(error) => panic!("create descendant directory symlink: {error}"),
        }
    }

    #[cfg(windows)]
    fn windows_local_tree_marker_command(marker: &Path) -> Command {
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper = "go::semantic::process::tests::windows_local_tree_spawn_marker_helper";
        let mut command = Command::new(test_binary);
        command
            .arg("--exact")
            .arg(helper)
            .arg("--nocapture")
            .env("POLINT_TEST_WINDOWS_LOCAL_TREE_MARKER", marker);
        command
    }

    #[test]
    fn configured_dependency_proxy_rejects_file_endpoints() {
        for value in [
            "https://proxy.example|file:///module-cache",
            " FILE:///module-cache,https://proxy.example",
        ] {
            let error = validate_goproxy_environment(std::ffi::OsStr::new(value))
                .expect_err("reachable file proxy endpoints must be rejected");
            assert!(matches!(
                error,
                GoSemanticProcessError::CommandUnavailable(_)
            ));
        }
        assert_eq!(
            normalize_goproxy_environment(std::ffi::OsStr::new(
                "direct,file:///ignored-after-terminal"
            ))
            .expect("Go ignores endpoints after direct"),
            OsString::from("direct")
        );
    }

    #[test]
    fn configured_dependency_proxy_normalizes_go_compatible_implicit_https() {
        assert_eq!(
            normalize_goproxy_environment(std::ffi::OsStr::new(
                ", proxy.corp.example | | [::1]:8080,"
            ))
            .expect("normalize valid implicit HTTPS endpoints"),
            OsString::from("https://proxy.corp.example|https://[::1]:8080")
        );
        assert!(validate_goproxy_environment(std::ffi::OsStr::new(",,|")).is_err());
    }

    #[test]
    fn configured_sumdb_rejects_file_alternates_with_unicode_whitespace() {
        let error =
            validate_gosumdb_environment(std::ffi::OsStr::new("verifier-key\u{a0}file:///sumdb"))
                .expect_err("file-backed sumdb alternate must be rejected");
        assert!(matches!(
            error,
            GoSemanticProcessError::CommandUnavailable(_)
        ));
        validate_gosumdb_environment(std::ffi::OsStr::new("verifier-key https://sumdb.example"))
            .expect("HTTPS sumdb alternate remains supported");
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn descendant_link_failure_prevents_command_spawn() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path().join("analysis-root");
        let outside = temp.path().join("outside");
        fs::create_dir(&root).expect("create analysis root");
        fs::create_dir(&outside).expect("create outside root");
        fs::write(outside.join("remote.go"), "package remote\n").expect("write outside source");
        symlink(outside.join("remote.go"), root.join("linked.go")).expect("create descendant link");
        let marker = temp.path().join("spawned");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("touch \"$1\"")
            .arg("polint-marker")
            .arg(&marker)
            .current_dir(&root);

        let error = run_bounded_command_with_local_trees_until(
            command,
            std::slice::from_ref(&root),
            BoundedCommandLimits::new(Duration::from_secs(5), 1024, 1024),
            GoOperationDeadline::after(Duration::from_secs(5)),
            "descendant link spawn probe",
        )
        .expect_err("descendant link certification must fail before spawn");

        assert!(error.to_string().contains("symbolic links"));
        assert!(!marker.exists(), "the rejected command must never execute");
    }

    #[cfg(windows)]
    #[test]
    fn ambiguous_windows_working_directory_prevents_command_spawn() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path().join("ambiguous.");
        crate::go::semantic::windows::create_private_directory(&root)
            .expect("create exact verbatim-only directory");
        let marker = temp.path().join("spawned");
        let mut command = windows_local_tree_marker_command(&marker);
        command.current_dir(&root);

        let error = run_bounded_command_with_local_trees_until(
            command,
            std::slice::from_ref(&root),
            BoundedCommandLimits::new(Duration::from_secs(5), 1024, 1024),
            GoOperationDeadline::after(Duration::from_secs(5)),
            "ambiguous Windows working-directory probe",
        )
        .expect_err("Win32-ambiguous working directory must fail before spawn");

        assert!(error.to_string().contains("working directory"));
        assert!(!marker.exists(), "the rejected command must never execute");
    }

    #[cfg(windows)]
    #[test]
    fn analysis_root_reparse_failure_prevents_command_spawn() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path().join("analysis-root");
        let target = temp.path().join("target");
        fs::create_dir(&root).expect("create analysis root");
        fs::create_dir(&target).expect("create reparse target");
        create_windows_descendant_reparse(&root, &target);
        let marker = temp.path().join("spawned");
        let mut command = windows_local_tree_marker_command(&marker);
        command.current_dir(&root);

        let error = run_bounded_command_with_local_trees_until(
            command,
            std::slice::from_ref(&root),
            BoundedCommandLimits::new(Duration::from_secs(5), 1024, 1024),
            GoOperationDeadline::after(Duration::from_secs(5)),
            "analysis-root reparse spawn probe",
        )
        .expect_err("analysis-root certification must fail before spawn");

        assert!(error.to_string().contains("reparse point"));
        assert!(!marker.exists(), "the rejected command must never execute");
    }

    #[cfg(windows)]
    #[test]
    fn ambient_proxy_reparse_failure_prevents_command_spawn() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let safe_current_dir = temp.path().join("safe-current-dir");
        let proxy = temp.path().join("ambient-proxy");
        let target = temp.path().join("target");
        fs::create_dir(&safe_current_dir).expect("create safe current directory");
        fs::create_dir(&proxy).expect("create ambient proxy root");
        fs::create_dir(&target).expect("create reparse target");
        create_windows_descendant_reparse(&proxy, &target);
        let marker = temp.path().join("spawned");
        let mut command = windows_local_tree_marker_command(&marker);
        command.current_dir(&safe_current_dir);

        let error = run_bounded_command_with_local_trees_until(
            command,
            std::slice::from_ref(&proxy),
            BoundedCommandLimits::new(Duration::from_secs(5), 1024, 1024),
            GoOperationDeadline::after(Duration::from_secs(5)),
            "ambient-proxy reparse spawn probe",
        )
        .expect_err("ambient-proxy certification must fail before spawn");

        assert!(error.to_string().contains("reparse point"));
        assert!(!marker.exists(), "the rejected command must never execute");
    }

    #[cfg(windows)]
    #[test]
    fn configured_file_proxy_failure_prevents_command_spawn() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let marker = temp.path().join("spawned");
        let mut toolchain = prepared_toolchain_for_test("go1.25.0");
        toolchain.environment.variables.insert(
            "GOPROXY".to_string(),
            OsString::from("https://proxy.example|file:///uncertified-cache"),
        );

        let result = dependency_population_proxy(
            &toolchain,
            false,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .and_then(|_| {
            windows_local_tree_marker_command(&marker)
                .status()
                .map_err(|error| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to run configured-file-proxy marker command: {error}"
                    ))
                })?
                .success()
                .then_some(())
                .ok_or_else(|| {
                    GoSemanticProcessError::CommandFailed(
                        "configured-file-proxy marker command failed".to_string(),
                    )
                })
        });
        let error = result.expect_err("configured file proxies must fail before command spawn");

        assert!(matches!(
            error,
            GoSemanticProcessError::CommandUnavailable(_)
        ));
        assert!(!marker.exists(), "the rejected command must never execute");
    }

    #[cfg(windows)]
    #[test]
    fn windows_local_tree_spawn_marker_helper() {
        let Some(marker) = std::env::var_os("POLINT_TEST_WINDOWS_LOCAL_TREE_MARKER") else {
            return;
        };
        fs::write(marker, b"spawned").expect("write Windows local-tree spawn marker");
    }

    fn build_provenance_for_test(version: &str) -> FrontendBuildProvenance {
        let toolchain = prepared_toolchain_for_test(version);
        FrontendBuildProvenance {
            source_digest: "source".to_string(),
            toolchain_version: toolchain.version,
            toolchain_executable_digest: toolchain.executable_digest,
            toolchain_content_digest: toolchain.closure.content_digest,
            host_target: toolchain.host_target,
            environment_policy: GO_ENVIRONMENT_POLICY,
        }
    }

    #[cfg(windows)]
    #[test]
    fn private_go_workspace_uses_go_compatible_windows_paths() {
        for path in [r"\\?\C:\repo\app", r"C:\repo\app"] {
            let encoded =
                quote_go_workspace_path(Path::new(path)).expect("encode safe workspace path");
            let decoded: String = serde_json::from_str(&encoded).expect("decode workspace path");
            assert_eq!(decoded, r"C:\repo\app");
            assert!(!decoded.contains('/'));
        }
    }

    #[cfg(windows)]
    #[test]
    fn embedded_frontend_runs_semantic_analysis_without_repository_writes() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path().join("repo");
        let app = root.join("app");
        let library = root.join("library");
        fs::create_dir_all(&app).expect("create app module");
        fs::create_dir_all(&library).expect("create library module");
        fs::write(
            app.join("go.mod"),
            "module example.test/app\n\ngo 1.25\n\nrequire example.test/library v0.0.0\n",
        )
        .expect("write app module");
        fs::write(
            app.join("app.go"),
            "package app\n\nimport \"example.test/library\"\n\nfunc Call() string { return library.Value() }\n",
        )
        .expect("write app source");
        fs::write(
            library.join("go.mod"),
            "module example.test/library\n\ngo 1.25\n",
        )
        .expect("write library module");
        fs::write(
            library.join("library.go"),
            "package library\n\nfunc Value() string { return \"ok\" }\n",
        )
        .expect("write library source");
        let config = GoAnalysisConfig {
            module_roots: vec!["app".to_string(), "library".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let before = snapshot_repository_tree(&root);
        let cache = test_cache_root(&temp);
        let _semantic_scope = acquire_test_go_semantic_concurrency_scope()
            .expect("acquire embedded Windows semantic frontend test scope");

        let prepared = PreparedGoSemanticFrontend::prepare_with_cache_root_for_analysis(
            &cache,
            Some((&root, &config)),
            &[],
            GoOperationDeadline::after(GO_OPERATION_TIMEOUT),
        )
        .expect("prepare the embedded Windows semantic frontend");
        let embedded_digest = embedded_frontend_hash();
        assert_eq!(
            prepared.source_digest.as_deref(),
            Some(embedded_digest.as_str())
        );

        let result = crate::go::semantic::client::GoSemanticClient::new(root.clone())
            .run_prepared(&config, &prepared)
            .expect("run real semantic analysis through the embedded frontend");

        assert!(
            result
                .output
                .rows
                .iter()
                .any(|row| row.kind == "package" && row.package_name == "library"),
            "semantic output must include the analyzed library package"
        );
        assert_eq!(snapshot_repository_tree(&root), before);
        assert!(!root.join("go.work").exists());
        assert!(!root.join("go.work.sum").exists());
        assert!(!app.join("go.sum").exists());
        assert!(!library.join("go.sum").exists());

        drop(prepared);
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(windows)]
    #[test]
    fn bounded_runner_windows_job_kills_descendant_retaining_output_pipes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pid_file = temp.path().join("windows-descendant.pid");
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper =
            "go::semantic::process::tests::bounded_runner_windows_parent_pipe_holder_helper";
        let mut command = Command::new(test_binary);
        command
            .env("POLINT_TEST_WINDOWS_DESCENDANT_PID_FILE", &pid_file)
            .arg("--exact")
            .arg(helper)
            .arg("--nocapture");
        let started = Instant::now();

        let error = run_bounded_command(
            command,
            BoundedCommandLimits::new(Duration::from_secs(2), 16 * 1024, 16 * 1024),
            "Windows Job Object pipe-retaining probe",
        )
        .expect_err("a descendant retaining output pipes must keep the deadline active");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert!(started.elapsed() < Duration::from_secs(5));
        let pid = fs::read_to_string(&pid_file)
            .expect("Windows descendant helper must report its pid")
            .trim()
            .parse::<u32>()
            .expect("Windows descendant helper pid");
        assert_windows_process_stops(pid);
    }

    #[cfg(windows)]
    #[test]
    #[allow(
        clippy::zombie_processes,
        reason = "the helper must exit without waiting so its descendant alone retains the captured pipes"
    )]
    fn bounded_runner_windows_parent_pipe_holder_helper() {
        let Some(pid_file) = std::env::var_os("POLINT_TEST_WINDOWS_DESCENDANT_PID_FILE") else {
            return;
        };
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper =
            "go::semantic::process::tests::bounded_runner_windows_descendant_pipe_holder_helper";
        let child = Command::new(test_binary)
            .env("POLINT_TEST_WINDOWS_PIPE_HOLDER", "1")
            .arg("--exact")
            .arg(helper)
            .arg("--nocapture")
            .spawn()
            .expect("spawn Windows descendant pipe holder");
        fs::write(pid_file, child.id().to_string()).expect("write Windows descendant pid");
    }

    #[cfg(windows)]
    #[test]
    fn bounded_runner_windows_descendant_pipe_holder_helper() {
        if std::env::var_os("POLINT_TEST_WINDOWS_PIPE_HOLDER").is_none() {
            return;
        }
        thread::sleep(Duration::from_secs(30));
    }

    #[cfg(windows)]
    #[allow(
        unsafe_code,
        reason = "the test polls one same-user process handle to verify Job Object cleanup"
    )]
    fn assert_windows_process_stops(pid: u32) {
        use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

        use windows_sys::Win32::Foundation::{
            ERROR_INVALID_PARAMETER, WAIT_OBJECT_0, WAIT_TIMEOUT,
        };
        use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;
        use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};

        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            // SAFETY: OpenProcess takes scalar arguments and returns a fresh
            // owned handle on success.
            let raw = unsafe { OpenProcess(SYNCHRONIZE, 0, pid) };
            if raw.is_null() {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
                    return;
                }
                panic!("failed to inspect Windows descendant {pid}: {error}");
            }
            // SAFETY: OpenProcess returned one fresh uniquely-owned handle.
            let handle = unsafe { OwnedHandle::from_raw_handle(raw.cast()) };
            // SAFETY: the handle remains live for the bounded wait.
            match unsafe { WaitForSingleObject(handle.as_raw_handle().cast(), 100) } {
                WAIT_OBJECT_0 => return,
                WAIT_TIMEOUT if Instant::now() < deadline => {}
                WAIT_TIMEOUT => panic!("Windows descendant process {pid} survived Job cleanup"),
                status => panic!("Windows process wait failed with status {status}"),
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn bounded_runner_keeps_deadline_active_until_descendant_pipe_eof() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pid_file = temp.path().join("descendant.pid");
        let mut command = Command::new("sh");
        command.arg("-c").arg(format!(
            "sleep 10 >&1 2>&2 & echo $! > '{}'; exit 0",
            pid_file.display()
        ));
        let started = Instant::now();

        let error = run_bounded_command(
            command,
            BoundedCommandLimits::new(Duration::from_millis(50), 1024, 1024),
            "pipe-retaining probe",
        )
        .expect_err("retained pipe must keep the deadline active");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert!(started.elapsed() < Duration::from_secs(2));
        if let Ok(pid) = fs::read_to_string(pid_file)
            && let Ok(pid) = pid.trim().parse::<i32>()
        {
            assert_process_stops(pid);
        }
    }

    #[cfg(unix)]
    #[test]
    fn bounded_runner_stops_spamming_build_output_without_unbounded_allocation() {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("while :; do printf 0123456789abcdef; done");
        let started = Instant::now();

        let error = run_bounded_command(
            command,
            BoundedCommandLimits::new(Duration::from_secs(2), 4096, 4096),
            "Go semantic frontend build",
        )
        .expect_err("spamming build output must fail closed");

        assert!(error.to_string().contains("4096-byte output limit"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_runner_enforces_one_budget_across_both_output_streams() {
        let mut command = Command::new("sh");
        command.arg("-c").arg(
            "i=0; while [ $i -lt 200 ]; do printf 0123456789abcdef; i=$((i + 1)); done; \
             i=0; while [ $i -lt 200 ]; do printf 0123456789abcdef >&2; i=$((i + 1)); done",
        );
        let started = Instant::now();

        let error = run_bounded_command(
            command,
            BoundedCommandLimits::new(Duration::from_secs(2), 4096, 4096),
            "dual-stream probe",
        )
        .expect_err("combined output must share one retention budget");

        assert!(
            error
                .to_string()
                .contains("combined stdout and stderr exceeded the 4096-byte output limit")
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_runner_kills_setsid_descendant_that_retains_output_pipes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pid_file = temp.path().join("setsid-descendant.pid");
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper_test =
            "go::semantic::process::tests::bounded_runner_setsid_parent_pipe_holder_helper";
        let mut command = Command::new(test_binary);
        command
            .env("POLINT_TEST_SETSID_PID_FILE", &pid_file)
            .arg("--exact")
            .arg(helper_test)
            .arg("--nocapture");
        let started = Instant::now();

        let error = run_bounded_command(
            command,
            BoundedCommandLimits::new(Duration::from_secs(2), 16 * 1024, 16 * 1024),
            "setsid pipe-retaining probe",
        )
        .expect_err("an escaped pipe holder must keep the deadline active");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert!(started.elapsed() < Duration::from_secs(5));
        let pid = fs::read_to_string(pid_file)
            .expect("setsid helper must report its pid")
            .trim()
            .parse::<i32>()
            .expect("setsid helper pid");
        assert_process_stops(pid);
    }

    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::zombie_processes,
        reason = "the bounded runner must observe and clean the escaped descendant after this helper exits"
    )]
    fn bounded_runner_setsid_parent_pipe_holder_helper() {
        let Some(pid_file) = std::env::var_os("POLINT_TEST_SETSID_PID_FILE") else {
            return;
        };
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper_test = "go::semantic::process::tests::bounded_runner_setsid_pipe_holder_helper";
        let _child = Command::new(test_binary)
            .env("POLINT_TEST_SETSID_PID_FILE", &pid_file)
            .arg("--exact")
            .arg(helper_test)
            .arg("--nocapture")
            .spawn()
            .expect("spawn setsid pipe holder");
        while !Path::new(&pid_file).exists() {
            thread::sleep(Duration::from_millis(1));
        }
    }

    #[cfg(unix)]
    #[test]
    #[allow(unsafe_code)]
    fn bounded_runner_setsid_pipe_holder_helper() {
        let Some(pid_file) = std::env::var_os("POLINT_TEST_SETSID_PID_FILE") else {
            return;
        };
        let session = unsafe { libc::setsid() };
        assert!(
            session >= 0,
            "setsid failed: {}",
            std::io::Error::last_os_error()
        );
        thread::sleep(Duration::from_millis(25));
        let mut pending_pid_file = PathBuf::from(&pid_file);
        pending_pid_file.set_extension("pending");
        fs::write(&pending_pid_file, std::process::id().to_string())
            .expect("write pending helper pid");
        fs::rename(pending_pid_file, pid_file).expect("publish helper pid");
        thread::sleep(Duration::from_secs(10));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_runner_kills_fast_double_fork_pipe_holder() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pid_file = temp.path().join("double-fork.pid");
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper_test =
            "go::semantic::process::tests::bounded_runner_double_fork_pipe_holder_helper";
        let mut command = Command::new(&test_binary);
        command
            .env("POLINT_TEST_DOUBLE_FORK_PID_FILE", &pid_file)
            .arg("--exact")
            .arg(helper_test)
            .arg("--nocapture");

        let error = run_bounded_command(
            command,
            BoundedCommandLimits::new(Duration::from_millis(150), 16 * 1024, 16 * 1024),
            "double-fork pipe-retaining probe",
        )
        .expect_err("a reparented pipe holder must keep the deadline active");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        let pid = fs::read_to_string(&pid_file)
            .expect("double-fork helper must report its pid")
            .trim()
            .parse::<i32>()
            .expect("double-fork helper pid");
        assert_process_stops(pid);
    }

    #[cfg(unix)]
    #[test]
    #[allow(unsafe_code)]
    fn timeout_cleanup_does_not_kill_unrelated_pre_exec_siblings() {
        use std::os::fd::AsRawFd;
        use std::os::unix::net::UnixStream;
        use std::os::unix::process::CommandExt;

        for iteration in 0..8 {
            let temp = tempfile::tempdir().expect("tempdir");
            let pid_file = temp.path().join(format!("double-fork-{iteration}.pid"));
            let test_binary = std::env::current_exe().expect("current test binary");
            let helper_test =
                "go::semantic::process::tests::bounded_runner_double_fork_pipe_holder_helper";
            let mut timed_command = Command::new(&test_binary);
            timed_command
                .env("POLINT_TEST_DOUBLE_FORK_PID_FILE", &pid_file)
                .arg("--exact")
                .arg(helper_test)
                .arg("--nocapture");
            let timed = thread::spawn(move || {
                run_bounded_command(
                    timed_command,
                    BoundedCommandLimits::new(Duration::from_millis(250), 16 * 1024, 16 * 1024),
                    "concurrent timeout probe",
                )
            });

            let wait_until = Instant::now() + Duration::from_secs(2);
            while !pid_file.exists() && Instant::now() < wait_until {
                thread::sleep(Duration::from_millis(1));
            }
            assert!(pid_file.exists(), "escaped helper did not report its pid");

            let (mut ready_parent, ready_child) = UnixStream::pair().expect("ready pipe");
            let (mut release_parent, release_child) = UnixStream::pair().expect("release pipe");
            let mut probe_command = Command::new("sh");
            probe_command.arg("-c").arg("printf sibling-ok");
            unsafe {
                probe_command.pre_exec(move || {
                    write_pre_exec_bytes(ready_child.as_raw_fd(), &[1])?;
                    let mut release = [0_u8; 1];
                    read_pre_exec_bytes(release_child.as_raw_fd(), &mut release)
                });
            }
            let probe = thread::spawn(move || {
                run_bounded_command(
                    probe_command,
                    BoundedCommandLimits::new(Duration::from_secs(2), 1024, 1024),
                    "unrelated pre-exec sibling",
                )
            });
            let mut ready = [0_u8; 1];
            ready_parent
                .read_exact(&mut ready)
                .expect("probe reached pre-exec barrier");

            let timeout_error = timed
                .join()
                .expect("timeout runner thread")
                .expect_err("escaped pipe holder must reach the timeout");
            assert!(matches!(timeout_error, GoSemanticProcessError::Timeout(_)));
            let _ = release_parent.write_all(&[1]);
            let probe_output = probe
                .join()
                .expect("probe runner thread")
                .expect("unrelated sibling must survive timeout cleanup");
            assert!(probe_output.status.success());
            assert_eq!(probe_output.stdout, b"sibling-ok");

            let pid = fs::read_to_string(&pid_file)
                .expect("double-fork helper must report its pid")
                .trim()
                .parse::<i32>()
                .expect("double-fork helper pid");
            assert_process_stops(pid);
        }
    }

    #[cfg(unix)]
    #[test]
    fn owner_sentinel_discovery_ends_when_the_descriptor_is_closed() {
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::MetadataExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("sentinel");
        let sentinel_file = fs::File::create(&path).expect("create sentinel");
        let metadata = sentinel_file.metadata().expect("sentinel metadata");
        let reported = OwnerSentinelIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        let pid = libc::pid_t::try_from(std::process::id()).expect("current pid");
        let sentinel = resolve_owner_sentinel_identity(pid, sentinel_file.as_raw_fd(), reported)
            .expect("resolve sentinel identity");

        assert!(
            process_holds_owner_sentinel(pid, sentinel).expect("inspect open sentinel"),
            "an open sentinel descriptor must prove runner ownership"
        );
        drop(sentinel_file);
        assert!(
            !process_holds_owner_sentinel(pid, sentinel).expect("inspect closed sentinel"),
            "closing the sentinel ends the ownership proof even while its vnode still exists"
        );
    }

    #[cfg(unix)]
    #[test]
    fn linux_fdinfo_identity_uses_mount_and_inode_without_following_the_descriptor() {
        let identity = parse_linux_fdinfo_sentinel_identity(
            b"pos:\t0\nflags:\t0100000\nmnt_id:\t42\nino:\t9001\n",
        )
        .expect("parse bounded fdinfo");

        assert_eq!(
            identity,
            OwnerSentinelIdentity {
                device: 42,
                inode: 9001,
            }
        );
        assert!(
            parse_linux_fdinfo_sentinel_identity(b"pos:\t0\nino:\t9001\n").is_err(),
            "missing mount identity must fail closed"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[allow(unsafe_code)]
    fn linux_global_sentinel_scan_skips_proven_unrelated_processes() {
        const HELPER_ENV: &str = "POLINT_TEST_NONDUMPABLE_SCAN_HELPER";
        const READY_ENV: &str = "POLINT_TEST_NONDUMPABLE_SCAN_READY";

        if std::env::var_os(HELPER_ENV).is_some() {
            let ready = PathBuf::from(std::env::var_os(READY_ENV).expect("helper ready path"));
            // SAFETY: PR_SET_DUMPABLE accepts scalar arguments and changes only
            // this disposable helper process.
            assert_eq!(unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) }, 0);
            fs::write(ready, b"ready").expect("publish non-dumpable helper readiness");
            thread::sleep(Duration::from_secs(10));
            return;
        }

        let _exclusive_containment_inspection = TEST_LINUX_CONTAINMENT_INSPECTION
            .get_or_init(|| RwLock::new(()))
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temp = tempfile::tempdir().expect("tempdir");
        let ready = temp.path().join("ready");
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper = "go::semantic::process::tests::linux_global_sentinel_scan_skips_proven_unrelated_processes";
        let child = ChildCleanupGuard::new(
            Command::new(test_binary)
                .arg("--exact")
                .arg(helper)
                .arg("--nocapture")
                .env(HELPER_ENV, "1")
                .env(READY_ENV, &ready)
                .spawn()
                .expect("spawn non-dumpable helper"),
        );
        let wait_until = Instant::now() + Duration::from_secs(2);
        while !ready.exists() && Instant::now() < wait_until {
            thread::sleep(Duration::from_millis(1));
        }
        let pid = libc::pid_t::try_from(child.id()).expect("helper pid");
        let identity = process_identity(pid)
            .expect("inspect helper identity")
            .expect("helper remains live");
        let sentinel = OwnerSentinelIdentity {
            device: u64::MAX,
            inode: u64::MAX,
        };
        let empty_baseline = BTreeMap::new();
        let mut same_tick_scan = OwnerSentinelScan::new();
        let same_tick = verified_owner_holder_with_scan(
            pid,
            sentinel,
            identity,
            &empty_baseline,
            &mut same_tick_scan,
        );
        let later_root = ProcessIdentity {
            start_primary: identity
                .start_primary
                .checked_add(1)
                .expect("helper start time can advance one tick"),
            start_secondary: identity.start_secondary,
        };
        assert_eq!(
            linux_owner_scan_identity(pid, identity, identity, &empty_baseline)
                .expect("equal-tick identity classification"),
            Some(identity),
            "equal-tick processes must remain in the descriptor scan"
        );
        assert_eq!(
            linux_owner_scan_identity(pid, identity, later_root, &empty_baseline)
                .expect("strictly older identity classification"),
            None,
            "strictly older processes must be excluded before descriptor inspection"
        );
        let baseline = capture_owner_scan_baseline(
            -1,
            Instant::now()
                .checked_add(COMMAND_OWNER_SCAN_TIMEOUT)
                .expect("baseline deadline"),
        );
        assert_eq!(
            baseline.get(&pid),
            Some(&identity),
            "the pre-release snapshot must retain the helper identity"
        );
        assert_eq!(
            linux_owner_scan_identity(pid, identity, identity, &baseline)
                .expect("unchanged baseline identity classification"),
            None,
            "an unchanged process observed before root release must be excluded"
        );
        let replacement = ProcessIdentity {
            start_primary: identity.start_primary.saturating_add(1),
            start_secondary: identity.start_secondary,
        };
        let stale_baseline = BTreeMap::from([(pid, replacement)]);
        assert_eq!(
            linux_owner_scan_identity(pid, identity, identity, &stale_baseline)
                .expect("replaced baseline identity classification"),
            Some(identity),
            "a PID whose identity differs from the baseline must remain in the scan"
        );
        let mut baseline_scan = OwnerSentinelScan::new();
        let baseline_holder =
            verified_owner_holder_with_scan(pid, sentinel, identity, &baseline, &mut baseline_scan);
        let mut older_scan = OwnerSentinelScan::new();
        let older = verified_owner_holder_with_scan(
            pid,
            sentinel,
            later_root,
            &empty_baseline,
            &mut older_scan,
        );
        assert!(ready.exists(), "non-dumpable helper did not become ready");
        match same_tick {
            Ok(None) => {}
            Ok(Some(_)) => panic!("the helper cannot hold the synthetic sentinel"),
            Err(error) => {
                assert!(error.contains("process"));
            }
        }
        assert!(
            baseline_holder
                .expect("an unchanged baseline process is provably unrelated")
                .is_none(),
            "baseline exclusion must happen before inaccessible descriptor inspection"
        );
        assert!(
            older
                .expect("an inaccessible process older than the root is unrelated")
                .is_none(),
            "an inaccessible process that predates the root cannot be its descendant"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_metadata_read_distinguishes_vanished_processes_from_denied_access() {
        struct FailingReader(Option<i32>);

        impl std::io::Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(self.0.map_or_else(
                    || std::io::Error::from(std::io::ErrorKind::NotFound),
                    std::io::Error::from_raw_os_error,
                ))
            }
        }

        for raw_error in [None, Some(libc::ESRCH)] {
            let observed = read_proc_reader_bounded(
                Path::new("/proc/vanished/stat"),
                FailingReader(raw_error),
                1024,
            )
            .expect("a process that vanishes during procfs read is absent");

            assert!(observed.is_none());
        }

        let denied = read_proc_reader_bounded(
            Path::new("/proc/restricted/stat"),
            FailingReader(Some(libc::EACCES)),
            1024,
        )
        .expect_err("denied procfs access must fail containment closed");
        assert!(denied.contains("failed to read process metadata"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_process_identity_treats_terminal_tasks_as_absent() {
        let stat = |state: &str, threads: u64| {
            format!(
                "123 (helper with ) marker) {state} 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 {threads} 18 42"
            )
        };

        for state in ["X", "x"] {
            assert_eq!(
                parse_linux_process_stat_identity(123, stat(state, 2).as_bytes())
                    .expect("parse terminal process state"),
                None
            );
        }
        assert_eq!(
            parse_linux_process_stat_identity(123, stat("Z", 1).as_bytes())
                .expect("parse single-thread zombie state"),
            None
        );
        let identity = Some(ProcessIdentity {
            start_primary: 42,
            start_secondary: 0,
        });
        assert_eq!(
            parse_linux_process_stat_identity(123, stat("Z", 2).as_bytes())
                .expect("parse multi-thread zombie leader"),
            identity,
            "live sibling threads must keep descriptor inspection fail-closed"
        );
        assert_eq!(
            parse_linux_process_stat_identity(123, stat("S", 1).as_bytes())
                .expect("parse live process state"),
            identity
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[allow(unsafe_code)]
    fn linux_child_enumeration_includes_nonleader_thread_children() {
        let temp = tempfile::tempdir().expect("tempdir");
        let child_pid_file = temp.path().join("thread-child.pid");
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper_test = "go::semantic::process::tests::linux_nonleader_thread_child_helper";
        let mut root = Command::new(test_binary)
            .env("POLINT_TEST_THREAD_CHILD_PID_FILE", &child_pid_file)
            .arg("--exact")
            .arg(helper_test)
            .arg("--nocapture")
            .spawn()
            .expect("spawn threaded helper");
        let wait_until = Instant::now() + Duration::from_secs(2);
        while !child_pid_file.exists() && Instant::now() < wait_until {
            thread::sleep(Duration::from_millis(1));
        }
        let child_pid = fs::read_to_string(&child_pid_file)
            .expect("thread child must report its pid")
            .trim()
            .parse::<libc::pid_t>()
            .expect("thread child pid");
        let root_pid = libc::pid_t::try_from(root.id()).expect("root pid");
        let mut budget = ProcessRefreshBudget::new(
            Instant::now()
                .checked_add(Duration::from_secs(2))
                .expect("test deadline"),
        );
        let children =
            child_process_ids(root_pid, &mut budget).expect("enumerate every root thread");
        unsafe {
            libc::kill(child_pid, libc::SIGKILL);
        }
        let _ = root.kill();
        let _ = root.wait();

        assert!(
            children.contains(&child_pid),
            "a child forked by a nonleader runtime thread must be tracked"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_nonleader_thread_child_helper() {
        let Some(child_pid_file) = std::env::var_os("POLINT_TEST_THREAD_CHILD_PID_FILE") else {
            return;
        };
        thread::spawn(move || {
            let mut child = Command::new("sleep")
                .arg("10")
                .spawn()
                .expect("spawn child from worker thread");
            fs::write(child_pid_file, child.id().to_string()).expect("write child pid");
            let _ = child.wait();
        })
        .join()
        .expect("worker thread");
    }

    #[cfg(unix)]
    #[test]
    #[allow(unsafe_code)]
    fn bounded_runner_double_fork_pipe_holder_helper() {
        let Some(pid_file) = std::env::var_os("POLINT_TEST_DOUBLE_FORK_PID_FILE") else {
            return;
        };
        let first = unsafe { libc::fork() };
        assert!(first >= 0, "first fork failed");
        if first > 0 {
            return;
        }
        let session = unsafe { libc::setsid() };
        if session < 0 {
            unsafe { libc::_exit(90) };
        }
        let second = unsafe { libc::fork() };
        if second < 0 {
            unsafe { libc::_exit(91) };
        }
        if second > 0 {
            unsafe { libc::_exit(0) };
        }
        fs::write(pid_file, std::process::id().to_string()).expect("write double-fork pid");
        thread::sleep(Duration::from_secs(10));
        unsafe { libc::_exit(0) };
    }

    #[cfg(unix)]
    #[test]
    fn bounded_runner_cleans_reparented_descendants_after_command_success() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pid_file = temp.path().join("detached-success.pid");
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper_test = "go::semantic::process::tests::bounded_runner_detached_success_helper";
        let mut command = Command::new(&test_binary);
        command
            .env("POLINT_TEST_DETACHED_SUCCESS_PID_FILE", &pid_file)
            .arg("--exact")
            .arg(helper_test)
            .arg("--nocapture");

        let output = run_bounded_command(
            command,
            BoundedCommandLimits::new(Duration::from_secs(2), 16 * 1024, 16 * 1024),
            "successful detached-descendant probe",
        )
        .expect("the root command should finish successfully");

        assert!(output.status.success());
        let pid = fs::read_to_string(&pid_file)
            .expect("detached helper must report its pid")
            .trim()
            .parse::<i32>()
            .expect("detached helper pid");
        assert_process_stops(pid);
    }

    #[cfg(unix)]
    #[test]
    #[allow(unsafe_code)]
    fn bounded_runner_detached_success_helper() {
        let Some(pid_file) = std::env::var_os("POLINT_TEST_DETACHED_SUCCESS_PID_FILE") else {
            return;
        };
        let first = unsafe { libc::fork() };
        assert!(first >= 0, "first fork failed");
        if first > 0 {
            return;
        }
        if unsafe { libc::setsid() } < 0 {
            unsafe { libc::_exit(90) };
        }
        let second = unsafe { libc::fork() };
        if second < 0 {
            unsafe { libc::_exit(91) };
        }
        if second > 0 {
            unsafe { libc::_exit(0) };
        }
        fs::write(pid_file, std::process::id().to_string()).expect("write detached pid");
        unsafe {
            libc::close(libc::STDOUT_FILENO);
            libc::close(libc::STDERR_FILENO);
        }
        thread::sleep(Duration::from_secs(10));
        unsafe { libc::_exit(0) };
    }

    #[cfg(unix)]
    #[test]
    fn absolute_deadline_prevents_spawn_instead_of_refreshing_the_timeout() {
        let temp = tempfile::tempdir().expect("tempdir");
        let marker = temp.path().join("spawned");
        let mut command = Command::new("sh");
        command.arg("-c").arg(format!(
            "printf spawned > {}",
            shell_single_quote(marker.to_string_lossy().as_ref())
        ));

        let error = run_bounded_command_until(
            command,
            BoundedCommandLimits::new(Duration::from_secs(5), 1024, 1024),
            GoOperationDeadline::at(Instant::now()),
            "expired probe",
        )
        .expect_err("an expired operation must not spawn a command");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert!(!marker.exists());
    }

    #[cfg(unix)]
    #[test]
    fn containment_verification_failure_does_not_deadlock_pre_exec() {
        let mut command = Command::new("sh");
        command
            .env("POLINT_TEST_INVALID_CONTAINMENT_SENTINEL", "1")
            .arg("-c")
            .arg("exit 0");
        let started = Instant::now();

        let error = run_bounded_command(
            command,
            BoundedCommandLimits::new(Duration::from_secs(2), 1024, 1024),
            "invalid containment sentinel probe",
        )
        .expect_err("invalid ownership proof must fail closed");

        assert!(error.to_string().contains("failed to start"));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "monitor failure must release a child blocked in pre-exec"
        );
    }

    #[cfg(unix)]
    #[test]
    fn prepared_frontend_does_not_reset_its_stored_deadline() {
        let mut frontend = PreparedGoSemanticFrontend::for_test("unused", None, None);
        frontend.operation_deadline = GoOperationDeadline::at(Instant::now());
        let temp = tempfile::tempdir().expect("tempdir");
        let marker = temp.path().join("executed");
        let mut command = Command::new("sh");
        command.arg("-c").arg(format!(
            "printf executed > {}",
            shell_single_quote(marker.to_string_lossy().as_ref())
        ));

        let error = frontend
            .run_command(
                command,
                BoundedCommandLimits::new(Duration::from_secs(5), 1024, 1024),
                "expired prepared frontend",
            )
            .expect_err("runtime must consume the preparation deadline");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert!(!marker.exists());
    }

    #[cfg(unix)]
    #[allow(unsafe_code)]
    fn assert_process_stops(pid: i32) {
        for _ in 0..40 {
            let result = unsafe { libc::kill(pid, 0) };
            if result != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        panic!("descendant process {pid} survived bounded command cleanup");
    }

    #[cfg(unix)]
    fn shell_single_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\\''"))
    }

    #[cfg(unix)]
    #[test]
    fn command_for_path_accepts_source_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join("go.mod"), "module test\n").expect("write go.mod");
        let command = command_for_path(temp.path().to_path_buf()).expect("source dir accepted");
        assert!(matches!(command, GoSemanticCommand::SourceDir(_)));
    }

    #[cfg(windows)]
    #[test]
    fn command_for_path_rejects_source_directory_on_windows() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join("go.mod"), "module test\n").expect("write go.mod");

        assert!(matches!(
            command_for_path(temp.path().to_path_buf()),
            Err(GoSemanticProcessError::CommandUnavailable(_))
        ));
    }

    #[test]
    fn embedded_frontend_hash_includes_schema() {
        assert_eq!(embedded_frontend_hash().len(), 64);
    }

    #[test]
    fn security_content_digest_is_full_sha256() {
        assert_eq!(
            security_digest_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn embedded_go_frontend_sources_match_workspace_sources() {
        let workspace_frontend =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("go-sidecar/polint-go-frontend");
        for (relative_path, embedded) in EMBEDDED_GO_FRONTEND_FILES {
            let workspace = fs::read_to_string(workspace_frontend.join(relative_path))
                .unwrap_or_else(|error| panic!("read workspace frontend {relative_path}: {error}"));
            assert_eq!(
                workspace, *embedded,
                "embedded Go semantic frontend drifted at {relative_path}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn source_dir_digest_changes_when_go_source_changes() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("go.mod"), "module example.test/frontend\n")
            .expect("write go.mod");
        fs::write(
            temp.path().join("main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("write main.go");
        let first = frontend_digest(&GoSemanticCommand::SourceDir(temp.path().to_path_buf()))
            .expect("digest source dir");

        fs::write(
            temp.path().join("main.go"),
            "package main\nfunc main() { println(\"changed\") }\n",
        )
        .expect("rewrite main.go");
        let second = frontend_digest(&GoSemanticCommand::SourceDir(temp.path().to_path_buf()))
            .expect("digest source dir again");

        assert_ne!(first, second);
    }

    #[cfg(windows)]
    #[test]
    fn source_dir_digest_is_unavailable_on_windows() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert!(matches!(
            frontend_digest(&GoSemanticCommand::SourceDir(temp.path().to_path_buf())),
            Err(GoSemanticProcessError::CommandUnavailable(_))
        ));
    }

    #[test]
    fn go_toolchain_version_parser_accepts_go_version_output() {
        assert_eq!(
            parse_go_version("go version go1.26.2 darwin/arm64"),
            Some("go1.26.2".to_string())
        );
        assert!(go_version_at_least("go1.25.0", 1, 25));
        assert!(!go_version_at_least("go1.24.9", 1, 25));
        assert_eq!(
            parse_go_toolchain("go version go1.26.2 darwin/arm64"),
            Some((
                "go1.26.2".to_string(),
                GoHostTarget {
                    os: "darwin".to_string(),
                    arch: "arm64".to_string(),
                }
            ))
        );
    }

    #[test]
    fn prepared_frontend_identity_tracks_executable_source_and_toolchain() {
        let baseline = PreparedGoSemanticFrontend::for_test(
            "executable-a",
            Some("source-a"),
            Some("go1.25.0"),
        );
        let executable_changed = PreparedGoSemanticFrontend::for_test(
            "executable-b",
            Some("source-a"),
            Some("go1.25.0"),
        );
        let source_changed = PreparedGoSemanticFrontend::for_test(
            "executable-a",
            Some("source-b"),
            Some("go1.25.0"),
        );
        let toolchain_changed = PreparedGoSemanticFrontend::for_test(
            "executable-a",
            Some("source-a"),
            Some("go1.26.0"),
        );

        assert_ne!(
            baseline.identity_digest(),
            executable_changed.identity_digest()
        );
        assert_ne!(baseline.identity_digest(), source_changed.identity_digest());
        assert_ne!(
            baseline.identity_digest(),
            toolchain_changed.identity_digest()
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_cache_root_rejects_symlink() {
        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        let cache = test_cache_root(&temp);
        std::os::unix::fs::symlink(outside.path(), &cache).expect("symlink cache root");

        let error = ensure_private_cache_root(&cache).expect_err("symlink root must fail");

        assert!(error.to_string().contains("must not be a symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn private_cache_root_rejects_symlink_ancestor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        let ancestor = temp.path().join("cache-link");
        std::os::unix::fs::symlink(outside.path(), &ancestor).expect("symlink cache ancestor");
        let cache = ancestor.join("cache");

        let error = ensure_private_cache_root(&cache).expect_err("symlink ancestor must fail");

        assert!(error.to_string().contains("ancestor"));
        assert!(error.to_string().contains("must not be a symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn private_cache_root_rejects_parent_components() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache = temp.path().join("missing").join("..").join("cache");

        let error = ensure_private_cache_root(&cache).expect_err("parent component must fail");

        assert!(error.to_string().contains("relative path components"));
    }

    #[cfg(unix)]
    #[test]
    fn expired_deadline_prevents_cache_root_creation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);

        let error = initialize_private_cache_root_until(
            &cache,
            GoOperationDeadline::after(Duration::from_nanos(1)),
        )
        .expect_err("expired operation must stop before cache creation");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert!(!cache.exists());
    }

    #[cfg(unix)]
    #[test]
    fn expired_deadline_prevents_installed_frontend_probe() {
        let temp = tempfile::tempdir().expect("tempdir");
        let candidate = temp.path().join(frontend_binary_name());
        std::os::unix::fs::symlink("missing-target", &candidate)
            .expect("seed hostile installed candidate");

        let error = installed_frontend_binary_in(
            temp.path(),
            GoOperationDeadline::after(Duration::from_nanos(1)),
        )
        .expect_err("expired operation must stop before candidate inspection");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert!(fs::symlink_metadata(candidate).is_ok());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn online_frontend_build_prefers_certified_ambient_proxy() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let download = temp.path().join("ambient/pkg/mod/cache/download");
        fs::create_dir(&source).expect("create source directory");
        fs::create_dir_all(&download).expect("create ambient download cache");
        let certified = certify_ambient_module_download_cache_candidate(
            download,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect("certify ambient download cache")
        .expect("retain ambient download cache");
        let local_url = file_proxy_url(&certified).expect("format local proxy URL");
        let proxy = compose_dependency_population_proxy(
            Some("https://proxy.example"),
            Some(certified.clone()),
        )
        .expect("compose frontend build proxy");

        let (value, local_trees) = proxy.into_frontend_build_inputs(&source);

        assert_eq!(
            value,
            OsString::from(format!("{local_url}|https://proxy.example"))
        );
        assert_eq!(local_trees, vec![source, certified]);
    }

    #[test]
    fn offline_frontend_build_excludes_ambient_proxy() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let toolchain = prepared_toolchain_for_test("go1.25.0");
        let proxy = dependency_population_proxy(
            &toolchain,
            true,
            GoOperationDeadline::after(Duration::ZERO),
        )
        .expect("select strict offline frontend build proxy");

        let (value, local_trees) = proxy.into_frontend_build_inputs(&source);

        assert_eq!(value, OsString::from("off"));
        assert_eq!(local_trees, vec![source]);
    }

    #[cfg(unix)]
    #[test]
    fn expired_deadline_prevents_ambient_module_cache_probe() {
        let temp = tempfile::tempdir().expect("tempdir");
        let candidate = temp.path().join("cache/download");

        let error = certify_ambient_module_download_cache_candidate(
            candidate.clone(),
            GoOperationDeadline::after(Duration::from_nanos(1)),
        )
        .expect_err("expired operation must stop before ambient cache inspection");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert!(!candidate.exists());
    }

    #[cfg(unix)]
    #[test]
    fn sealed_executable_rejects_hostile_preseed_with_wrong_bytes() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let expected = b"#!/bin/sh\nprintf 'expected\\n'\n";
        let digest = security_digest_bytes(expected);
        let directory =
            ensure_private_subdirectory(&cache, &Path::new("executables").join(&digest))
                .expect("execution directory");
        let executable = directory.join(frontend_binary_name());
        fs::write(&executable, b"#!/bin/sh\nprintf 'hostile\\n'\n").expect("preseed binary");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o500))
            .expect("executable mode");

        let error = seal_executable(&cache, expected, &digest).expect_err("preseed must fail");

        assert!(error.to_string().contains("failed content verification"));
    }

    #[cfg(unix)]
    #[test]
    fn prepared_binary_executes_sealed_bytes_after_source_path_is_replaced() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let source = temp.path().join("frontend");
        fs::write(&source, b"#!/bin/sh\nprintf 'A\\n'\n").expect("write frontend A");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).expect("frontend mode");
        let prepared =
            prepare_binary_frontend(&cache, &source, prepared_toolchain_for_test("go1.25.0"))
                .expect("prepare frontend A");
        fs::write(&source, b"#!/bin/sh\nprintf 'B\\n'\n").expect("replace source with B");

        let output = prepared
            .command(temp.path())
            .expect("construct prepared frontend command")
            .output()
            .expect("run sealed frontend");

        assert_eq!(String::from_utf8_lossy(&output.stdout), "A\n");
        fs::set_permissions(
            prepared.executable.parent().expect("sealed directory"),
            fs::Permissions::from_mode(0o700),
        )
        .expect("reopen sealed directory for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn sealed_execution_target_rejects_direct_mutation_and_replacement() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let source = temp.path().join("frontend");
        fs::write(&source, b"#!/bin/sh\nprintf 'sealed\\n'\n").expect("write frontend");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).expect("frontend mode");
        let prepared =
            prepare_binary_frontend(&cache, &source, prepared_toolchain_for_test("go1.25.0"))
                .expect("prepare frontend");
        let sealed_directory = prepared.executable.parent().expect("sealed directory");
        let source_bytes = read_regular_file_no_follow(&source).expect("read source bytes");
        let reused = seal_executable(&cache, &source_bytes, &security_digest_bytes(&source_bytes))
            .expect("reuse verified sealed frontend");
        let replacement = temp.path().join("replacement");
        fs::write(&replacement, b"#!/bin/sh\nprintf 'replacement\\n'\n")
            .expect("write replacement");

        let directory_mode = fs::metadata(sealed_directory)
            .expect("sealed directory metadata")
            .mode()
            & 0o777;
        assert_eq!(directory_mode, 0o500);
        assert_eq!(reused, prepared.executable);
        if effective_user_id() != 0 {
            assert!(
                fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&prepared.executable)
                    .is_err()
            );
            assert!(fs::rename(&replacement, &prepared.executable).is_err());
        }
        fs::set_permissions(sealed_directory, fs::Permissions::from_mode(0o700))
            .expect("reopen sealed directory for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn materialized_source_rejects_completion_marker_preseed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let snapshot = embedded_source_snapshot();
        let directory =
            ensure_private_subdirectory(&cache, &Path::new("sources").join(&snapshot.digest))
                .expect("preseed source directory");
        fs::write(directory.join(".complete"), "").expect("preseed marker");

        let error = materialize_source_snapshot(&cache, &snapshot)
            .expect_err("completion marker alone must not be trusted");

        assert!(error.to_string().contains("failed content verification"));
    }

    #[cfg(unix)]
    #[test]
    fn cached_source_build_is_rejected_before_lookup_for_old_toolchain() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let provenance = build_provenance_for_test("go1.24.9");
        let toolchain = prepared_toolchain_for_test("go1.24.9");
        ensure_private_subdirectory(&cache, &Path::new("builds").join(provenance.cache_key()))
            .expect("preseed old build cache");

        let error = ensure_frontend_binary(&cache, temp.path(), &provenance, &toolchain)
            .expect_err("old toolchain cache must fail");

        assert!(matches!(
            error,
            GoSemanticProcessError::VersionUnsupported(_)
        ));
    }

    #[test]
    fn source_build_cache_key_binds_only_stable_build_inputs() {
        let mut baseline = build_provenance_for_test("go1.25.0");
        baseline.source_digest = "source-a".to_string();
        let mut source_changed = baseline.clone();
        source_changed.source_digest = "source-b".to_string();
        let mut toolchain_changed = baseline.clone();
        toolchain_changed.toolchain_version = "go1.26.0".to_string();
        let mut executable_changed = baseline.clone();
        executable_changed.toolchain_executable_digest = "other-executable".to_string();
        let mut content_changed = baseline.clone();
        content_changed.toolchain_content_digest = "other-content".to_string();
        let mut target_changed = baseline.clone();
        target_changed.host_target.arch = "arm64".to_string();
        let mut environment_changed = baseline.clone();
        environment_changed.environment_policy = "different-policy";

        let keys = [
            baseline.cache_key(),
            source_changed.cache_key(),
            toolchain_changed.cache_key(),
            executable_changed.cache_key(),
            content_changed.cache_key(),
            target_changed.cache_key(),
            environment_changed.cache_key(),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();

        assert_eq!(keys.len(), 7);
        assert!(keys.iter().all(|key| key.len() == 64));
    }

    #[test]
    fn command_environment_overrides_semantic_go_modifiers() {
        let mut command = Command::new("unused");
        command
            .env("GOOS", "hostile-os")
            .env("GOARCH", "hostile-arch")
            .env("GOFLAGS", "-tags=hostile")
            .env("GOAMD64", "v4")
            .env("CGO_ENABLED", "1")
            .env("HOSTILE_SECRET", "must-not-survive");
        let toolchain = prepared_toolchain_for_test("go1.25.0");

        configure_go_environment(&mut command, &toolchain);

        let environment = command
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(environment.get("GOOS"), Some(&Some("linux".to_string())));
        assert_eq!(environment.get("GOARCH"), Some(&Some("amd64".to_string())));
        assert_eq!(environment.get("GOFLAGS"), Some(&Some(String::new())));
        assert_eq!(environment.get("CGO_ENABLED"), Some(&Some("0".to_string())));
        assert_eq!(environment.get("GOAMD64"), None);
        assert_eq!(environment.get("HOSTILE_SECRET"), None);
        assert_eq!(environment.get("GOPROXY"), Some(&Some("off".to_string())));
        assert_eq!(environment.get("GOVCS"), Some(&Some("off".to_string())));
        assert_eq!(
            environment.get("GOROOT"),
            Some(&Some("/selected/go".to_string()))
        );
        assert!(
            environment
                .get("PATH")
                .and_then(Option::as_ref)
                .is_some_and(|path| path.starts_with("/sealed/toolchain"))
        );
    }

    #[test]
    fn certified_environment_omits_ambient_custom_certificate_paths() {
        const HELPER_ENV: &str = "POLINT_TEST_CERTIFICATE_ENV_HELPER";
        const RESULT_ENV: &str = "POLINT_TEST_CERTIFICATE_ENV_RESULT";
        const CACHE_ENV: &str = "POLINT_TEST_CERTIFICATE_ENV_CACHE";

        if std::env::var_os(HELPER_ENV).is_some() {
            let cache = PathBuf::from(
                std::env::var_os(CACHE_ENV).expect("certificate environment helper cache"),
            );
            let result = PathBuf::from(
                std::env::var_os(RESULT_ENV).expect("certificate environment helper result"),
            );
            let environment = CertifiedGoEnvironment::capture(&cache, false)
                .expect("capture certified environment");
            assert!(!environment.variables.contains_key("SSL_CERT_FILE"));
            assert!(!environment.variables.contains_key("SSL_CERT_DIR"));
            fs::write(result, b"omitted").expect("write certificate environment result");
            return;
        }

        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = temp.path().join("cache");
        ensure_private_cache_root(&cache).expect("create private cache root");
        let result = temp.path().join("certificate-environment-result");
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper = "go::semantic::process::tests::certified_environment_omits_ambient_custom_certificate_paths";
        let output = Command::new(test_binary)
            .arg("--exact")
            .arg(helper)
            .arg("--nocapture")
            .env(HELPER_ENV, "1")
            .env(RESULT_ENV, &result)
            .env(CACHE_ENV, &cache)
            .env("SSL_CERT_FILE", temp.path().join("ambient-certificate.pem"))
            .env("SSL_CERT_DIR", temp.path().join("ambient-certificates"))
            .output()
            .expect("run certificate environment helper");

        assert!(
            output.status.success(),
            "certificate environment helper failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read(&result).expect("read certificate environment result"),
            b"omitted"
        );
    }

    #[cfg(unix)]
    #[test]
    fn strict_offline_environment_is_stable_under_hostile_ambient_network_state() {
        use std::os::unix::ffi::OsStringExt;

        const HELPER_ENV: &str = "POLINT_TEST_OFFLINE_ENV_HELPER";
        const RESULT_ENV: &str = "POLINT_TEST_OFFLINE_ENV_RESULT";
        const CACHE_ENV: &str = "POLINT_TEST_OFFLINE_ENV_CACHE";
        const MODE_ENV: &str = "POLINT_TEST_OFFLINE_ENV_MODE";

        if std::env::var_os(HELPER_ENV).is_some() {
            let cache = PathBuf::from(std::env::var_os(CACHE_ENV).expect("offline helper cache"));
            let result =
                PathBuf::from(std::env::var_os(RESULT_ENV).expect("offline helper result"));
            let offline = std::env::var(MODE_ENV).expect("offline helper mode") == "offline";
            let environment = CertifiedGoEnvironment::capture(&cache, offline)
                .expect("capture isolated certified environment");
            if offline {
                assert_eq!(
                    environment.variables.get("GOPROXY"),
                    Some(&OsString::from("off"))
                );
                assert_eq!(
                    environment.variables.get("GOSUMDB"),
                    Some(&OsString::from("off"))
                );
                for name in ["GOPRIVATE", "GONOPROXY", "GONOSUMDB", "GOINSECURE"] {
                    assert_eq!(environment.variables.get(name), Some(&OsString::new()));
                }
                for name in [
                    "HTTP_PROXY",
                    "HTTPS_PROXY",
                    "http_proxy",
                    "https_proxy",
                    "NO_PROXY",
                    "no_proxy",
                ] {
                    assert!(!environment.variables.contains_key(name));
                }
                assert!(
                    environment
                        .variables
                        .get("GOMODCACHE")
                        .is_some_and(|path| Path::new(path).starts_with(&cache))
                );
                assert!(
                    environment
                        .variables
                        .get("GOPATH")
                        .is_some_and(|path| Path::new(path).starts_with(&cache))
                );
            }
            fs::write(result, environment.digest).expect("write certified environment digest");
            return;
        }

        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = temp.path().join("cache");
        ensure_private_cache_root(&cache).expect("create private cache root");
        let test_binary = std::env::current_exe().expect("current test binary");
        let helper = "go::semantic::process::tests::strict_offline_environment_is_stable_under_hostile_ambient_network_state";
        let run = |name: &str, mode: &str, goproxy: OsString| {
            let result = temp.path().join(name);
            let mut command = Command::new(&test_binary);
            command
                .arg("--exact")
                .arg(helper)
                .arg("--nocapture")
                .env(HELPER_ENV, "1")
                .env(RESULT_ENV, &result)
                .env(CACHE_ENV, &cache)
                .env(MODE_ENV, mode)
                .env("GOPROXY", goproxy)
                .env("GOMODCACHE", temp.path().join("ambient-module-cache"))
                .env("GOPATH", temp.path().join("ambient-gopath"))
                .env("GOPRIVATE", "hostile.private")
                .env("GONOPROXY", "hostile.noproxy")
                .env("GONOSUMDB", "hostile.nosumdb")
                .env("GOINSECURE", "hostile.insecure")
                .env("HTTP_PROXY", "file:///hostile-http-proxy")
                .env("HTTPS_PROXY", "file:///hostile-https-proxy")
                .env("NO_PROXY", "hostile-no-proxy");
            if mode == "offline" {
                command.env("GOSUMDB", "verifier-key\u{a0}file:///hostile-sumdb");
            } else {
                command
                    .env("GOSUMDB", "sum.golang.org")
                    .env_remove("HTTP_PROXY")
                    .env_remove("HTTPS_PROXY")
                    .env_remove("http_proxy")
                    .env_remove("https_proxy")
                    .env_remove("NO_PROXY")
                    .env_remove("no_proxy");
            }
            let output = command
                .output()
                .expect("run isolated certified environment helper");
            assert!(
                output.status.success(),
                "certified environment helper failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            fs::read_to_string(result).expect("read certified environment digest")
        };
        let first = run(
            "offline-one",
            "offline",
            OsString::from_vec(b"file:///hostile-\xff".to_vec()),
        );
        let second = run(
            "offline-two",
            "offline",
            OsString::from("https://different.example"),
        );
        let online = run("online", "online", OsString::from("https://proxy.example"));
        assert_eq!(first, second);
        assert_ne!(first, online);
    }

    #[test]
    fn subprocess_stderr_redacts_goproxy_and_sumdb_userinfo() {
        let mut command = Command::new("unused");
        command
            .env("GOPROXY", "https://proxy-user:proxy-secret@proxy.example")
            .env(
                "GOSUMDB",
                "verifier-key https://sum-user:sum-secret@sumdb.example",
            )
            .env("HTTP_PROXY", "http-user:http-secret@http-proxy.example");
        let policy = command_stderr_redaction_policy(&command);
        let redacted = redact_subprocess_stderr(
            b"proxy-user:proxy-secret sum-user:sum-secret http-user:http-secret",
            &policy,
        );
        assert_eq!(redacted, REDACTED_SUBPROCESS_STDERR);
    }

    #[test]
    fn subprocess_stderr_redacts_proxy_url_paths_queries_and_fragments() {
        let goproxy = "https://proxy-path.example/private-goproxy-token,https://proxy-query.example?token=goproxy-query-token|https://proxy-fragment.example#goproxy-fragment-token";
        let gosumdb = "verifier-key https://sumdb.example/private-sumdb-token?key=sumdb-query-token#sumdb-fragment-token";
        let https_proxy = "https://http-proxy.example/private-http-token?auth=http-query-token#http-fragment-token";
        let mut command = Command::new("unused");
        command
            .env("GOPROXY", goproxy)
            .env("GOSUMDB", gosumdb)
            .env("HTTPS_PROXY", https_proxy);
        let policy = command_stderr_redaction_policy(&command);
        let stderr = format!("{goproxy}\n{gosumdb}\n{https_proxy}");
        let redacted = redact_subprocess_stderr(stderr.as_bytes(), &policy);

        assert_eq!(redacted, REDACTED_SUBPROCESS_STDERR);
    }

    #[test]
    fn subprocess_stderr_redaction_covers_partial_endpoint_echoes() {
        let mut command = Command::new("unused");
        command.env(
            "GOSUMDB",
            "verifier-key https://sumdb.example/private-token?key=query-secret#fragment-secret",
        );
        let policy = command_stderr_redaction_policy(&command);
        let redacted = redact_subprocess_stderr(
            b"GET /private-token?key=query-secret failed before fragment handling",
            &policy,
        );

        assert_eq!(redacted, REDACTED_SUBPROCESS_STDERR);
    }

    #[test]
    fn ambiguous_tiny_userinfo_redacts_the_message_without_global_replacement() {
        let mut command = Command::new("unused");
        command.env("GOPROXY", "https://:@proxy.example");
        let policy = command_stderr_redaction_policy(&command);
        let redacted = redact_subprocess_stderr(b"compile: unrelated: diagnostic", &policy);

        assert_eq!(redacted, REDACTED_SUBPROCESS_STDERR);
    }

    #[test]
    fn proxy_root_path_does_not_redact_unrelated_stderr_separators() {
        let mut command = Command::new("unused");
        command.env("GOPROXY", "https://proxy.example/");
        let policy = command_stderr_redaction_policy(&command);
        let stderr = b"https://proxy.example/ failed while reading workspace/pkg/main.go";
        let redacted = redact_subprocess_stderr(stderr, &policy);

        assert_eq!(
            redacted,
            "https://proxy.example/ failed while reading workspace/pkg/main.go"
        );
        assert!(redacted.contains("workspace/pkg/main.go"));
    }

    #[test]
    fn internal_file_proxy_does_not_suppress_ordinary_stderr() {
        let mut command = Command::new("unused");
        command.env(
            "GOPROXY",
            "file:///owner/cache/download|https://proxy.golang.org",
        );
        let policy = command_stderr_redaction_policy(&command);
        let redacted = redact_subprocess_stderr(b"ordinary Go dependency error", &policy);

        assert_eq!(redacted, "ordinary Go dependency error");
    }

    #[test]
    fn unexpected_url_userinfo_is_redacted_as_defense_in_depth() {
        let policy = CommandStderrRedactionPolicy::default();
        let redacted = redact_subprocess_stderr(
            b"GET https://unexpected:secret@example.test/path failed",
            &policy,
        );

        assert_eq!(redacted, "GET https://[REDACTED]@example.test/path failed");
    }

    #[test]
    fn certified_environment_identity_rotates_without_exposing_credentials() {
        let mut baseline = BTreeMap::from([
            (
                "GOPROXY".to_string(),
                OsString::from("https://user:secret@proxy.example"),
            ),
            ("GOMODCACHE".to_string(), OsString::from("/cache/a")),
            ("GOVCS".to_string(), OsString::from("off")),
        ]);
        let first = certified_environment_digest(&baseline);
        baseline.insert("GOMODCACHE".to_string(), OsString::from("/cache/b"));
        let cache_changed = certified_environment_digest(&baseline);
        baseline.insert(
            "GOPROXY".to_string(),
            OsString::from("https://user:other-secret@proxy.example"),
        );
        let proxy_changed = certified_environment_digest(&baseline);
        assert_ne!(first, cache_changed);
        assert_ne!(cache_changed, proxy_changed);

        let environment = CertifiedGoEnvironment {
            variables: baseline,
            digest: proxy_changed.clone(),
        };
        let debug = format!("{environment:?}");
        assert!(debug.contains(&proxy_changed));
        assert!(!debug.contains("user"));
        assert!(!debug.contains("secret"));
        assert!(!debug.contains("proxy.example"));
    }

    #[test]
    fn dependency_snapshot_digest_participates_in_frontend_identity() {
        let baseline = PreparedGoSemanticFrontend::for_test("frontend", None, Some("go1.25.0"));
        let mut changed = baseline.clone();
        Arc::make_mut(&mut changed.dependency_snapshot).content_digest =
            "different-dependencies".to_string();

        assert_ne!(baseline.identity_digest(), changed.identity_digest());
    }

    #[test]
    fn frontend_clones_share_heavy_verified_state() {
        let baseline = PreparedGoSemanticFrontend::for_test("frontend", None, Some("go1.25.0"));
        let cloned = baseline.clone();

        assert!(Arc::ptr_eq(&baseline.toolchain, &cloned.toolchain));
        assert!(Arc::ptr_eq(
            &baseline.dependency_snapshot,
            &cloned.dependency_snapshot
        ));
    }

    #[test]
    fn dependency_request_binds_toolchain_content_not_acquisition_metadata() {
        let baseline = prepared_toolchain_for_test("go1.25.0");
        let mut relocated = baseline.clone();
        relocated.canonical_selection = PathBuf::from("/relocated/bin/go");
        relocated.goroot = PathBuf::from("/relocated/go");
        relocated.closure.digest = "relocated-metadata".to_string();
        relocated.closure.metadata_digest = "relocated-entry-metadata".to_string();
        relocated.closure.root_metadata_digest = "relocated-root-metadata".to_string();
        assert_eq!(
            empty_dependency_snapshot_request_key(&baseline),
            empty_dependency_snapshot_request_key(&relocated)
        );

        relocated.closure.content_digest = "different-toolchain-content".to_string();
        assert_ne!(
            empty_dependency_snapshot_request_key(&baseline),
            empty_dependency_snapshot_request_key(&relocated)
        );
    }

    #[test]
    fn frontend_identity_ignores_acquisition_paths_metadata_and_environment() {
        let baseline =
            PreparedGoSemanticFrontend::for_test("frontend", Some("source"), Some("go1.25.0"));
        let mut relocated = baseline.clone();
        let toolchain = Arc::make_mut(&mut relocated.toolchain);
        toolchain.canonical_selection = PathBuf::from("/relocated/bin/go");
        toolchain.goroot = PathBuf::from("/relocated/go");
        toolchain.closure.digest = "relocated-metadata-closure".to_string();
        toolchain.closure.metadata_digest = "relocated-metadata".to_string();
        toolchain.closure.root_metadata_digest = "relocated-root".to_string();
        toolchain.environment.digest = "other-acquisition-environment".to_string();
        Arc::make_mut(&mut relocated.dependency_snapshot).workspace_digest =
            "relocated-workspace".to_string();

        assert_eq!(baseline.identity_digest(), relocated.identity_digest());
    }

    #[test]
    fn published_cache_capacity_has_exact_hard_boundaries() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let category = ensure_private_subdirectory(&cache, Path::new("capacity-test"))
            .expect("create cache category");
        let first = ensure_private_subdirectory(&category, Path::new("first"))
            .expect("create first published entry");
        write_new_private_file(&first.join("payload"), b"one", false)
            .expect("write first published payload");
        let ignored = ensure_private_subdirectory(&category, Path::new(".staging"))
            .expect("create ignored staging entry");
        write_new_private_file(&ignored.join("payload"), &[0_u8; 32], false)
            .expect("write ignored staging payload");
        let second = category.join("second");
        drop(
            published_cache_capacity_guard_until(
                &cache,
                &category,
                &second,
                3,
                2,
                6,
                GoOperationDeadline::after(Duration::from_secs(5)),
            )
            .expect("exact entry and byte boundary must be accepted"),
        );
        let second = ensure_private_subdirectory(&category, Path::new("second"))
            .expect("publish second entry");
        write_new_private_file(&second.join("payload"), b"two", false)
            .expect("write second published payload");

        let count_error = published_cache_capacity_guard_until(
            &cache,
            &category,
            &category.join("third"),
            0,
            2,
            6,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect_err("next published entry must exceed the count bound");
        assert!(matches!(
            count_error,
            GoSemanticProcessError::CommandUnavailable(_)
        ));
        let byte_error = published_cache_capacity_guard_until(
            &cache,
            &category,
            &category.join("third"),
            1,
            3,
            6,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect_err("next byte must exceed the byte bound");
        assert!(matches!(
            byte_error,
            GoSemanticProcessError::CommandUnavailable(_)
        ));
        drop(
            published_cache_capacity_guard_until(
                &cache,
                &category,
                &second,
                u64::MAX,
                0,
                0,
                GoOperationDeadline::after(Duration::from_secs(5)),
            )
            .expect("an existing verified destination remains usable at capacity"),
        );
    }

    #[cfg(unix)]
    #[test]
    fn published_cache_capacity_rejects_unsafe_entries() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let category = ensure_private_subdirectory(&cache, Path::new("capacity-test"))
            .expect("create cache category");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).expect("create symlink target");
        symlink(&outside, category.join("unsafe")).expect("create unsafe published entry");

        let error = published_cache_capacity_guard_until(
            &cache,
            &category,
            &category.join("new"),
            0,
            2,
            16,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect_err("unsafe published entries must fail closed");
        assert!(error.to_string().contains("unsafe"));
    }

    #[test]
    fn published_cache_capacity_serializes_concurrent_publishers() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        ensure_private_subdirectory(&cache, Path::new("control"))
            .expect("precreate lifecycle control directory");
        let category = ensure_private_subdirectory(&cache, Path::new("concurrent-capacity-test"))
            .expect("create cache category");
        let cache = Arc::new(cache);
        let category = Arc::new(category);
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let mut publishers = Vec::new();
        for index in 0..4 {
            let cache = Arc::clone(&cache);
            let category = Arc::clone(&category);
            let barrier = Arc::clone(&barrier);
            publishers.push(std::thread::spawn(move || {
                let staging = tempfile::Builder::new()
                    .prefix(".candidate-")
                    .tempdir_in(category.as_path())
                    .expect("create candidate staging")
                    .keep();
                write_new_private_file(&staging.join("payload"), b"x", false)
                    .expect("write candidate payload");
                let destination = category.join(format!("published-{index}"));
                barrier.wait();
                let result = published_cache_capacity_guard_until(
                    &cache,
                    &category,
                    &destination,
                    1,
                    1,
                    1,
                    GoOperationDeadline::after(Duration::from_secs(5)),
                );
                match result {
                    Ok(capacity) => {
                        fs::rename(&staging, &destination)
                            .expect("publish within held capacity lock");
                        drop(capacity);
                        true
                    }
                    Err(GoSemanticProcessError::CommandUnavailable(_)) => false,
                    Err(error) => panic!("unexpected capacity result: {error}"),
                }
            }));
        }
        let published = publishers
            .into_iter()
            .map(|publisher| publisher.join().expect("join concurrent publisher"))
            .filter(|published| *published)
            .count();
        assert_eq!(published, 1);
        let published_entries = fs::read_dir(category.as_path())
            .expect("enumerate published category")
            .collect::<Result<Vec<_>, _>>()
            .expect("inspect published category")
            .into_iter()
            .filter(|entry| {
                !entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with('.'))
            })
            .count();
        assert_eq!(published_entries, 1);
        assert!(
            remove_directory_tree_with_limits(
                cache.as_path(),
                32,
                4,
                GoOperationDeadline::after(Duration::from_secs(5)),
            )
            .expect("remove the completed private cache fixture")
        );
        assert!(!cache.exists());
    }

    #[test]
    fn offline_policy_is_absent_from_local_semantic_config_identity() {
        let online = GoAnalysisConfig {
            module_roots: vec![".".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: vec!["integration".to_string()],
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let mut offline = online.clone();
        offline.offline = true;
        let digest = |config: &GoAnalysisConfig| {
            let mut hasher = Sha256::new();
            hash_go_analysis_semantic_config(&mut hasher, config);
            format!("{:x}", hasher.finalize())
        };

        assert_eq!(digest(&online), digest(&offline));
    }

    #[test]
    fn package_selection_is_bound_to_local_semantic_config_identity() {
        let baseline = GoAnalysisConfig {
            module_roots: vec![".".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: vec!["integration".to_string()],
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let digest = |config: &GoAnalysisConfig| {
            let mut hasher = Sha256::new();
            hash_go_analysis_semantic_config(&mut hasher, config);
            format!("{:x}", hasher.finalize())
        };
        let baseline_digest = digest(&baseline);
        let mut packages = baseline.clone();
        packages.package_patterns = vec!["./cmd/...".to_string()];
        let mut tags = baseline.clone();
        tags.build_tags = vec!["production".to_string()];
        let mut tests = baseline;
        tests.include_tests = false;

        assert_ne!(baseline_digest, digest(&packages));
        assert_ne!(baseline_digest, digest(&tags));
        assert_ne!(baseline_digest, digest(&tests));
    }

    #[test]
    fn dependency_source_signature_binds_bom_build_constraints_not_function_bodies() {
        fn digest(source: &[u8]) -> String {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_go::LANGUAGE.into())
                .expect("initialize Go parser");
            let mut hasher = Sha256::new();
            hash_go_dependency_source_signature(
                &mut hasher,
                &mut parser,
                source,
                GoOperationDeadline::after(Duration::from_secs(1)),
            )
            .expect("hash Go dependency signature");
            format!("{:x}", hasher.finalize())
        }

        let linux = b"\xef\xbb\xbf//go:build linux\n\npackage selected\nimport _ \"example.test/dependency\"\nfunc Value() int { return 1 }\n";
        let linux_body_edit = b"\xef\xbb\xbf//go:build linux\n\npackage selected\nimport _ \"example.test/dependency\"\nfunc Value() int { return 2 }\n";
        let windows = b"\xef\xbb\xbf//go:build windows\n\npackage selected\nimport _ \"example.test/dependency\"\nfunc Value() int { return 1 }\n";

        assert_eq!(digest(linux), digest(linux_body_edit));
        assert_ne!(digest(linux), digest(windows));
    }

    #[test]
    fn recursive_local_seal_detects_a_new_nested_package() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path();
        let main = root.join("main.go");
        fs::write(&main, "package main\n").expect("write selected source");
        fs::create_dir(root.join("nested")).expect("create preexisting non-package directory");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let initial = capture_local_dependency_path_seal(
            root,
            &[],
            &[root.to_path_buf()],
            std::slice::from_ref(&main),
            None,
            deadline,
        )
        .expect("capture initial local seal");

        fs::create_dir_all(root.join("nested/newpkg")).expect("create nested package");
        fs::write(root.join("nested/newpkg/new.go"), "package newpkg\n")
            .expect("write nested package source");
        let current = capture_local_dependency_path_seal(
            root,
            &[],
            &[root.to_path_buf()],
            std::slice::from_ref(&main),
            Some(&initial.content_digest),
            deadline,
        )
        .expect("capture verification seal");

        assert_ne!(initial.metadata_digest, current.metadata_digest);
        assert_ne!(initial.entry_count, current.entry_count);
    }

    #[test]
    fn unrelated_files_and_cache_location_do_not_rotate_local_content_identity() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path();
        let main = root.join("main.go");
        fs::write(&main, "package main\n").expect("write selected source");
        fs::write(root.join("README.md"), "first\n").expect("write unrelated file");
        fs::create_dir_all(root.join(".polint/cache")).expect("create default cache");
        fs::create_dir_all(root.join("build/polint-cache")).expect("create custom cache");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let capture = |cache_relative: &Path| {
            capture_local_dependency_path_seal(
                root,
                &[cache_relative.to_path_buf()],
                &[root.to_path_buf()],
                std::slice::from_ref(&main),
                None,
                deadline,
            )
            .expect("capture local content identity")
        };
        let default_cache = capture(Path::new(".polint/cache"));
        fs::write(root.join("README.md"), "second\n").expect("change unrelated file");
        let custom_cache = capture(Path::new("build/polint-cache"));

        assert_eq!(default_cache.content_digest, custom_cache.content_digest);
        assert_ne!(default_cache.metadata_digest, custom_cache.metadata_digest);
    }

    #[cfg(unix)]
    #[test]
    fn touch_and_same_byte_atomic_replace_preserve_local_content_identity() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path();
        let source = root.join("main.go");
        let bytes = b"package main\n";
        fs::write(&source, bytes).expect("write source");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let capture = || {
            capture_local_dependency_path_seal(
                root,
                &[],
                &[root.to_path_buf()],
                std::slice::from_ref(&source),
                None,
                deadline,
            )
            .expect("capture local content identity")
        };
        let initial = capture();
        fs::File::options()
            .write(true)
            .open(&source)
            .expect("open source for touch")
            .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::now()))
            .expect("touch source");
        let touched = capture();
        let replacement = root.join("replacement.go");
        fs::write(&replacement, bytes).expect("write same-byte replacement");
        fs::rename(&replacement, &source).expect("atomically replace source");
        let replaced = capture();

        assert_eq!(initial.content_digest, touched.content_digest);
        assert_eq!(initial.content_digest, replaced.content_digest);
        assert_ne!(initial.metadata_digest, replaced.metadata_digest);
    }

    #[test]
    fn local_seal_filters_files_outside_the_go_package_universe() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path();
        let source = root.join("main.go");
        let unrelated = root.join("notes");
        let hidden = root.join(".git");
        fs::write(&source, "package main\n").expect("write selected source");
        fs::create_dir(&unrelated).expect("create unrelated directory");
        fs::create_dir(&hidden).expect("create ignored hidden directory");
        for index in 0..16 {
            fs::write(
                unrelated.join(format!("note-{index}.txt")),
                "not Go input\n",
            )
            .expect("write unrelated file");
            fs::write(hidden.join(format!("object-{index}")), "ignored\n")
                .expect("write ignored hidden file");
        }
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let seal = capture_local_dependency_path_seal_with_entry_limit(
            root,
            &[],
            &[root.to_path_buf()],
            std::slice::from_ref(&source),
            None,
            4,
            deadline,
        )
        .expect("irrelevant files must not exhaust the Go-universe bound");
        assert_eq!(seal.entry_count, 2, "source plus ordinary directory");

        for index in 0..3 {
            fs::write(
                unrelated.join(format!("discovered-{index}.go")),
                "package discovered\n",
            )
            .expect("write Go-universe marker");
        }
        let error = capture_local_dependency_path_seal_with_entry_limit(
            root,
            &[],
            &[root.to_path_buf()],
            std::slice::from_ref(&source),
            None,
            4,
            deadline,
        )
        .expect_err("Go-universe entries must remain bounded");
        assert!(error.to_string().contains("more than 4 entries"));
    }

    #[test]
    fn local_seal_rejects_a_flat_directory_beyond_its_entry_limit() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path();
        let selected = root.join("selected.go");
        fs::write(&selected, "package main\n").expect("write selected source");
        fs::write(root.join("second.go"), "package main\n").expect("write second source");
        fs::write(root.join("third.go"), "package main\n").expect("write third source");

        let error = capture_local_dependency_path_seal_with_entry_limit(
            root,
            &[],
            &[root.to_path_buf()],
            std::slice::from_ref(&selected),
            None,
            2,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect_err("the third entry must be rejected at the collection boundary");

        assert!(error.to_string().contains("more than 2 entries"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn local_seal_rejects_a_nonlocal_root_before_dependency_content_access() {
        let root = Path::new("/proc");
        let untouched = root.join("polint-local-dependency-must-not-be-touched");

        let error = capture_local_dependency_path_seal_with_entry_limit(
            root,
            &[],
            std::slice::from_ref(&untouched),
            &[],
            None,
            4,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect_err("a non-local root must fail before the dependency walk starts");

        assert!(matches!(
            error,
            GoSemanticProcessError::CommandUnavailable(_)
        ));
        assert!(error.to_string().contains("local filesystem boundary"));
        assert!(
            !error
                .to_string()
                .contains("failed to inspect local Go dependency directory")
        );
    }

    #[test]
    fn explicitly_selected_hidden_package_remains_in_the_local_seal() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path();
        let package = root.join(".generated");
        fs::create_dir(&package).expect("create explicitly selected hidden package");
        let source = package.join("main.go");
        fs::write(&source, "package generated\n").expect("write hidden package source");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let initial = capture_local_dependency_path_seal(
            root,
            &[],
            std::slice::from_ref(&package),
            std::slice::from_ref(&source),
            None,
            deadline,
        )
        .expect("capture selected hidden package");

        fs::write(package.join("added.go"), "package generated\n")
            .expect("add hidden package source");
        let current = capture_local_dependency_path_seal(
            root,
            &[],
            std::slice::from_ref(&package),
            std::slice::from_ref(&source),
            Some(&initial.content_digest),
            deadline,
        )
        .expect("recertify selected hidden package");

        assert_ne!(initial.metadata_digest, current.metadata_digest);
        assert_eq!(current.entry_count, initial.entry_count + 1);
    }

    #[test]
    fn local_scope_promotes_selected_files_to_directory_inclusions() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path();
        let package = root.join(".generated/nested");
        fs::create_dir_all(&package).expect("create selected hidden package");
        let source = package.join("main.go");
        fs::write(&source, "package generated\n").expect("write selected source");

        let scope = local_dependency_directory_scope_until(
            root,
            &[],
            std::slice::from_ref(&source),
            GO_DEPENDENCY_MAX_ENTRIES,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect("build bounded local dependency scope");
        let inclusions = scope.traversal;

        assert!(!inclusions.contains(&source));
        assert!(inclusions.contains(&package));
        assert!(inclusions.contains(&root.join(".generated")));
        assert!(inclusions.iter().all(|path| path.is_dir()));
    }

    #[test]
    fn sorted_local_path_index_uses_component_prefixes() {
        let root = PathBuf::from("/repo");
        let punctuation_sibling = root.join("a-b");
        let nested = root.join("a/deep");
        let mut paths = vec![punctuation_sibling.clone(), nested.clone()];
        paths.sort_unstable_by(|left, right| compare_local_path_components(left, right));

        assert!(sorted_local_paths_contain_descendant(
            &paths,
            &root.join("a")
        ));
        assert!(sorted_local_paths_contain_exact(
            &paths,
            &punctuation_sibling
        ));
        assert!(sorted_local_paths_contain_ancestor(
            &paths,
            &nested.join("child"),
            &root
        ));
        assert!(!sorted_local_paths_contain_ancestor(
            &paths,
            &root.join("unrelated"),
            &root
        ));
    }

    #[test]
    fn selected_local_inputs_must_not_overlap_repository_cache_roots() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = temp.path().join(".polint/cache");
        let empty_module = cache.join("generated-module");
        let package = cache.join("generated-package");
        fs::create_dir_all(&empty_module).expect("create selected empty module");
        fs::create_dir_all(&package).expect("create selected package");
        let source = package.join("main.go");
        fs::write(&source, "package generated\n").expect("write selected cached source");

        for (directories, files) in [
            (vec![empty_module], Vec::new()),
            (vec![package], vec![source]),
        ] {
            let error = reject_selected_local_inputs_in_repository_cache(
                &directories,
                &files,
                std::slice::from_ref(&cache),
            )
            .expect_err("selected cache-contained input must fail closed");
            assert!(error.to_string().contains("repository-cache exclusion"));
        }
    }

    #[test]
    fn workspace_auxiliary_parser_accepts_tabs_and_godebug_blocks() {
        let parsed = parse_workspace_auxiliary_directives(
            b"go 1.25\ntoolchain\tgo1.26.2\ngodebug\t(\n\tdefault=go1.21\n\tasynctimerchan=1\n)\n",
            GoOperationDeadline::after(Duration::from_secs(1)),
        )
        .expect("parse valid tab-separated directives");

        assert_eq!(parsed.toolchain.as_deref(), Some("go1.26.2"));
        assert_eq!(parsed.godebug, ["default=go1.21", "asynctimerchan=1"]);
    }

    #[test]
    fn prepared_command_uses_only_the_sealed_dependency_snapshot() {
        let frontend = PreparedGoSemanticFrontend::for_test("frontend", None, Some("go1.25.0"));
        let command = frontend
            .command(&std::env::current_dir().expect("current directory"))
            .expect("construct prepared frontend command");
        let environment = command
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(
            environment.get("GOMODCACHE"),
            Some(&Some(
                if cfg!(windows) {
                    r"C:\test\module-cache"
                } else {
                    "/test/module-cache"
                }
                .to_string()
            ))
        );
        assert_eq!(environment.get("GOPROXY"), Some(&Some("off".to_string())));
        assert_eq!(environment.get("GOSUMDB"), Some(&Some("off".to_string())));
        assert_eq!(environment.get("GOVCS"), Some(&Some("off".to_string())));
    }

    #[cfg(windows)]
    #[test]
    fn prepared_command_normalizes_every_windows_snapshot_path_for_go() {
        let mut frontend = PreparedGoSemanticFrontend::for_test("frontend", None, Some("go1.25.0"));
        {
            let snapshot = Arc::make_mut(&mut frontend.dependency_snapshot);
            snapshot.module_cache_root = PathBuf::from(r"\\?\C:\polint-cache\modules");
            snapshot.workspace_path = Some(PathBuf::from(r"\\?\C:\polint-cache\workspace\go.work"));
        }

        let command = frontend
            .command(Path::new(r"\\?\C:\repo"))
            .expect("construct Windows Go command");
        let environment = command
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(command.get_current_dir(), Some(Path::new(r"C:\repo")));
        assert_eq!(
            environment.get("GOMODCACHE"),
            Some(&Some(r"C:\polint-cache\modules".to_string()))
        );
        assert_eq!(
            environment.get("POLINT_GO_WORKSPACE"),
            Some(&Some(r"C:\polint-cache\workspace\go.work".to_string()))
        );
        assert!(
            frontend
                .dependency_snapshot
                .module_cache_root
                .starts_with(r"\\?\C:\")
        );
        assert!(
            frontend
                .dependency_snapshot
                .workspace_path
                .as_deref()
                .is_some_and(|path| path.starts_with(r"\\?\C:\"))
        );
    }

    #[test]
    fn dependency_snapshot_reservation_release_removes_its_lock_path() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let snapshots = ensure_private_subdirectory(&cache, Path::new("dependency-snapshots"))
            .expect("create dependency snapshots root");
        let key = "1".repeat(64);
        let reservation =
            DependencySnapshotReservation::create(&snapshots, &key).expect("reserve capacity");
        let reservation_path = reservation
            .path
            .as_ref()
            .expect("reservation has a path")
            .clone();
        assert!(reservation_path.is_file());
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let active = {
            let _lifecycle =
                dependency_lifecycle_lock_until(&snapshots, deadline).expect("lock lifecycle");
            cleanup_and_count_dependency_reservations_until(&snapshots, deadline)
                .expect("count active reservation")
        };
        assert_eq!(active, 1);

        reservation.release().expect("release reservation");

        assert!(!reservation_path.exists());
    }

    #[test]
    fn active_dependency_staging_is_not_collected() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let snapshots = ensure_private_subdirectory(&cache, Path::new("dependency-snapshots"))
            .expect("create dependency snapshots root");
        let staging_root = ensure_private_subdirectory(&snapshots, Path::new("staging"))
            .expect("create dependency staging root");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let staging = {
            let _lifecycle =
                dependency_lifecycle_lock_until(&snapshots, deadline).expect("lock lifecycle");
            StagingDirectory::create_dependency_until(&staging_root, ".dependency-", deadline)
                .expect("create live dependency stage")
        };
        let staging_path = staging.path().to_path_buf();
        let current_retained = {
            let _lifecycle =
                dependency_lifecycle_lock_until(&snapshots, deadline).expect("lock lifecycle");
            cleanup_and_count_orphaned_dependency_staging_until(&snapshots, &staging_path, deadline)
                .expect("skip the caller's current dependency stage")
        };
        let retained = {
            let _lifecycle =
                dependency_lifecycle_lock_until(&snapshots, deadline).expect("lock lifecycle");
            cleanup_and_count_orphaned_dependency_staging_until(
                &snapshots,
                &staging_root.join("not-the-live-stage"),
                deadline,
            )
            .expect("inspect live dependency stage")
        };

        assert_eq!(current_retained, 0);
        assert_eq!(retained, 1);
        assert!(staging_path.is_dir());
        staging
            .discard_dependency_until(&snapshots, deadline)
            .expect("discard stage under lifecycle lock");
        assert!(!staging_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn sealed_dependency_snapshot_can_be_retired_on_darwin() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let toolchain = prepared_toolchain_for_test("go1.25.0");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let snapshot = prepare_dependency_snapshot(&cache, &toolchain, None, &[], deadline)
            .expect("prepare sealed dependency snapshot");
        let snapshots_root = snapshot.snapshots_root.clone();
        let snapshot_path = snapshot.snapshot_root.clone();
        let snapshot_key = snapshot_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("snapshot key")
            .to_string();
        assert_eq!(
            fs::symlink_metadata(&snapshot_path)
                .expect("sealed snapshot metadata")
                .permissions()
                .mode()
                & 0o777,
            0o500
        );
        drop(snapshot);

        let _lifecycle =
            dependency_lifecycle_lock_until(&snapshots_root, deadline).expect("lock lifecycle");
        let retained = collect_retained_dependency_snapshots_until(&snapshots_root, deadline)
            .expect("collect retained snapshot")
            .into_iter()
            .find(|candidate| candidate.key == snapshot_key)
            .expect("retained snapshot candidate");
        assert!(
            retire_dependency_snapshot_until(&snapshots_root, &retained, deadline)
                .expect("retire sealed dependency snapshot")
        );
        assert!(!snapshot_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn sealed_corrupt_dependency_snapshot_can_be_quarantined_on_darwin() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let snapshots = ensure_private_subdirectory(&cache, Path::new("dependency-snapshots"))
            .expect("create dependency snapshots root");
        ensure_private_subdirectory(&snapshots, Path::new("staging")).expect("create staging root");
        let key = "e".repeat(64);
        let destination = ensure_private_subdirectory(&snapshots, Path::new(&key))
            .expect("create corrupt destination");
        write_new_private_file(&destination.join("corrupt"), b"corrupt", false)
            .expect("write corrupt payload");
        seal_dependency_envelope_path(&destination, true).expect("seal corrupt envelope");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let observed = dependency_snapshot_destination_identity_until(&destination, deadline)
            .expect("capture corrupt destination identity");

        quarantine_corrupt_dependency_snapshot_until(
            &snapshots,
            &key,
            observed.as_deref(),
            deadline,
        )
        .expect("quarantine sealed corrupt snapshot");

        assert!(!destination.exists());
    }

    #[cfg(unix)]
    #[test]
    fn failed_dependency_snapshot_move_reseals_anchored_directory() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let snapshots = ensure_private_subdirectory(&cache, Path::new("dependency-snapshots"))
            .expect("create dependency snapshots root");
        let source = ensure_private_subdirectory(&snapshots, Path::new(&"f".repeat(64)))
            .expect("create dependency snapshot");
        seal_dependency_envelope_path(&source, true).expect("seal dependency snapshot");
        let metadata = fs::symlink_metadata(&source).expect("sealed snapshot metadata");
        let missing_destination = snapshots.join("missing-parent").join("payload");

        move_dependency_snapshot_to_quarantine(
            &source,
            &missing_destination,
            &metadata,
            "move test Go dependency snapshot",
        )
        .expect_err("missing destination parent must fail the move");

        assert_eq!(
            fs::symlink_metadata(&source)
                .expect("resealed source metadata")
                .permissions()
                .mode()
                & 0o777,
            0o500
        );
    }

    #[test]
    fn colliding_dependency_lease_repair_releases_the_lifecycle_lock() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let snapshots = ensure_private_subdirectory(&cache, Path::new("dependency-snapshots"))
            .expect("create dependency snapshots root");
        ensure_private_subdirectory(&snapshots, Path::new("staging")).expect("create staging root");
        let repair_key = format!("aa{}", "0".repeat(62));
        let population_key = format!("aa{}", "1".repeat(62));
        let destination = ensure_private_subdirectory(&snapshots, Path::new(&repair_key))
            .expect("create corrupt destination");
        write_new_private_file(&destination.join("corrupt"), b"corrupt", false)
            .expect("write corrupt payload");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let observed = dependency_snapshot_destination_identity_until(&destination, deadline)
            .expect("capture corrupt destination identity");
        let population_lease =
            DependencySnapshotLease::acquire(&snapshots, &population_key, deadline)
                .expect("hold colliding population lease");
        let control = ensure_private_subdirectory(&snapshots, Path::new("control"))
            .expect("create lifecycle control");
        let leases =
            ensure_private_subdirectory(&control, Path::new("leases")).expect("create lease root");
        assert_eq!(
            dependency_lease_path(&leases, &repair_key).expect("repair lease path"),
            dependency_lease_path(&leases, &population_key).expect("population lease path")
        );

        assert!(
            !try_quarantine_corrupt_dependency_snapshot_until(
                &snapshots,
                &repair_key,
                &destination,
                observed.as_deref(),
                deadline,
            )
            .expect("colliding lease requests a retry")
        );
        drop(
            dependency_lifecycle_lock_until(&snapshots, deadline)
                .expect("retry path must release lifecycle before returning"),
        );
        drop(population_lease);
        quarantine_corrupt_dependency_snapshot_until(
            &snapshots,
            &repair_key,
            observed.as_deref(),
            deadline,
        )
        .expect("repair after population lease release");
        assert!(!destination.exists());
    }

    #[test]
    fn dependency_snapshot_capacity_waits_until_deadline_for_active_leases() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let snapshots = ensure_private_subdirectory(&cache, Path::new("dependency-snapshots"))
            .expect("create dependency snapshots root");
        let staging = ensure_private_subdirectory(&snapshots, Path::new("staging"))
            .expect("create staging root");
        let current_staging = staging.join("current-population");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let mut active_leases = Vec::new();
        for index in 0..GO_DEPENDENCY_MAX_PUBLISHED_SNAPSHOTS {
            let key = format!("{:02x}{}", index + 1, "0".repeat(62));
            ensure_private_subdirectory(&snapshots, Path::new(&key))
                .expect("create retained dependency snapshot");
            active_leases.push(
                DependencySnapshotLease::acquire(&snapshots, &key, deadline)
                    .expect("hold retained snapshot lease"),
            );
        }

        let request_key = "f".repeat(64);
        let error = acquire_or_reserve_dependency_snapshot_until(
            &snapshots,
            &request_key,
            &current_staging,
            GoOperationDeadline::after(Duration::from_millis(75)),
        )
        .expect_err("leased snapshots must keep capacity unavailable");
        assert!(
            matches!(error, GoSemanticProcessError::Timeout(_)),
            "capacity wait must remain bounded by the caller deadline: {error}"
        );

        let orphan = ensure_private_subdirectory(&staging, Path::new(".dependency-orphan"))
            .expect("create observable orphan stage");
        write_new_private_mutable_file(&orphan.join(".liveness"), b"")
            .expect("create unlocked orphan liveness marker");
        let waiter_snapshots = snapshots.clone();
        let waiter_key = request_key;
        let waiter_staging = current_staging;
        let (result_tx, result_rx) = mpsc::channel();
        let waiter = thread::spawn(move || {
            let result = acquire_or_reserve_dependency_snapshot_until(
                &waiter_snapshots,
                &waiter_key,
                &waiter_staging,
                GoOperationDeadline::after(Duration::from_secs(5)),
            );
            result_tx.send(result).expect("report capacity result");
        });
        let observation_deadline = Instant::now() + Duration::from_secs(2);
        while orphan.exists() && Instant::now() < observation_deadline {
            thread::sleep(COMMAND_MONITOR_INTERVAL);
        }
        assert!(
            !orphan.exists(),
            "capacity waiter must complete its initial lifecycle scan"
        );
        let lifecycle = dependency_lifecycle_lock_until(
            &snapshots,
            GoOperationDeadline::after(Duration::from_secs(2)),
        )
        .expect("observe waiter between capacity scans");
        assert!(
            matches!(result_rx.try_recv(), Err(mpsc::TryRecvError::Empty)),
            "capacity waiter must remain blocked while every snapshot is leased"
        );

        drop(active_leases.pop());
        drop(lifecycle);
        let (_lease, availability) = result_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("capacity waiter reports after lease release")
            .expect("reserve released dependency capacity");
        assert!(matches!(
            availability,
            DependencySnapshotAvailability::Reserved(_)
        ));
        waiter.join().expect("join capacity waiter");
    }

    #[test]
    fn pre_admission_dependency_stages_do_not_deadlock_capacity() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let snapshots = ensure_private_subdirectory(&cache, Path::new("dependency-snapshots"))
            .expect("create dependency snapshots root");
        let staging = ensure_private_subdirectory(&snapshots, Path::new("staging"))
            .expect("create staging root");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let stages = (0..=GO_DEPENDENCY_MAX_PUBLISHED_SNAPSHOTS)
            .map(|_| {
                StagingDirectory::create_dependency_until(&staging, ".dependency-", deadline)
                    .expect("create live pre-admission stage")
            })
            .collect::<Vec<_>>();

        let request_key = "e".repeat(64);
        let (_lease, availability) = acquire_or_reserve_dependency_snapshot_until(
            &snapshots,
            &request_key,
            stages[0].path(),
            deadline,
        )
        .expect("one waiting stage must win admission");

        assert!(matches!(
            availability,
            DependencySnapshotAvailability::Reserved(_)
        ));
    }

    #[test]
    fn concurrent_corrupt_dependency_repairers_converge() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let snapshots = ensure_private_subdirectory(&cache, Path::new("dependency-snapshots"))
            .expect("create dependency snapshots root");
        ensure_private_subdirectory(&snapshots, Path::new("staging")).expect("create staging root");
        let key = "b".repeat(64);
        let destination = ensure_private_subdirectory(&snapshots, Path::new(&key))
            .expect("create corrupt destination");
        write_new_private_file(&destination.join("corrupt"), b"corrupt", false)
            .expect("write corrupt payload");
        let observed = dependency_snapshot_destination_identity_until(
            &destination,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect("capture corrupt destination identity");
        let snapshots = Arc::new(snapshots);
        let key = Arc::new(key);
        let observed = Arc::new(observed);
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let mut repairers = Vec::new();
        for _ in 0..2 {
            let snapshots = Arc::clone(&snapshots);
            let key = Arc::clone(&key);
            let observed = Arc::clone(&observed);
            let barrier = Arc::clone(&barrier);
            repairers.push(std::thread::spawn(move || {
                barrier.wait();
                quarantine_corrupt_dependency_snapshot_until(
                    &snapshots,
                    &key,
                    observed.as_ref().as_deref(),
                    GoOperationDeadline::after(Duration::from_secs(5)),
                )
            }));
        }
        barrier.wait();
        for repairer in repairers {
            repairer
                .join()
                .expect("join corrupt snapshot repairer")
                .expect("repairer converges");
        }
        assert!(!snapshots.join(key.as_str()).exists());
    }

    #[test]
    fn unsafe_dependency_destination_is_quarantined_for_self_healing() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let snapshots = ensure_private_subdirectory(&cache, Path::new("dependency-snapshots"))
            .expect("create dependency snapshots root");
        ensure_private_subdirectory(&snapshots, Path::new("staging")).expect("create staging root");
        let key = "c".repeat(64);
        let destination = snapshots.join(&key);
        write_new_private_file(&destination, b"not a directory", false)
            .expect("write poisoned destination");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let observed = dependency_snapshot_destination_identity_until(&destination, deadline)
            .expect("capture poisoned destination identity");

        quarantine_corrupt_dependency_snapshot_until(
            &snapshots,
            &key,
            observed.as_deref(),
            deadline,
        )
        .expect("quarantine poisoned destination");

        assert!(!destination.exists());
    }

    #[cfg(unix)]
    #[test]
    fn dependency_destination_symlink_is_quarantined_without_following_it() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temporary directory");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("create private cache root");
        let snapshots = ensure_private_subdirectory(&cache, Path::new("dependency-snapshots"))
            .expect("create dependency snapshots root");
        ensure_private_subdirectory(&snapshots, Path::new("staging")).expect("create staging root");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).expect("create outside directory");
        let key = "d".repeat(64);
        let destination = snapshots.join(&key);
        symlink(&outside, &destination).expect("poison snapshot destination with symlink");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let observed = dependency_snapshot_destination_identity_until(&destination, deadline)
            .expect("capture poisoned symlink identity");

        quarantine_corrupt_dependency_snapshot_until(
            &snapshots,
            &key,
            observed.as_deref(),
            deadline,
        )
        .expect("quarantine destination symlink");

        assert!(!destination.exists());
        assert!(outside.is_dir(), "quarantine must not follow the symlink");
    }

    #[cfg(unix)]
    #[test]
    fn sealed_dependency_snapshot_rejects_same_path_content_mutation() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let toolchain = prepared_toolchain_for_test("go1.25.0");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let snapshot = prepare_dependency_snapshot(&cache, &toolchain, None, &[], deadline)
            .expect("prepare empty dependency snapshot");
        verify_dependency_snapshot(&snapshot, &toolchain, deadline)
            .expect("baseline snapshot verifies");

        fs::set_permissions(
            &snapshot.module_cache_root,
            fs::Permissions::from_mode(0o700),
        )
        .expect("reopen snapshot root");
        let dependency = snapshot.module_cache_root.join("same-path.go");
        fs::write(&dependency, "package changed\n").expect("mutate snapshot at same path");
        fs::set_permissions(&dependency, fs::Permissions::from_mode(0o400))
            .expect("reseal mutated dependency");
        fs::set_permissions(
            &snapshot.module_cache_root,
            fs::Permissions::from_mode(0o500),
        )
        .expect("reseal snapshot root");

        let error = verify_dependency_snapshot(&snapshot, &toolchain, deadline)
            .expect_err("same-path dependency mutation must be rejected");
        assert!(error.to_string().contains("snapshot changed"));
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn local_dependency_snapshot_ignores_polint_cache_lifecycle_only() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let config = GoAnalysisConfig {
            module_roots: vec![".".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let toolchain =
            prepare_go_toolchain(&cache, local_go_toolchain().expect("local Go toolchain"))
                .expect("prepare Go toolchain");
        for (repository, cache_relative) in [
            ("default-cache-repo", Path::new(".polint/cache")),
            ("custom-cache-repo", Path::new("build/polint-cache")),
            ("root-cache-repo", Path::new(".")),
        ] {
            let root = temp.path().join(repository);
            fs::create_dir(&root).expect("create repository");
            fs::write(root.join("go.mod"), "module example.test/app\n\ngo 1.25\n")
                .expect("write go.mod");
            fs::write(root.join("main.go"), "package main\nfunc main() {}\n")
                .expect("write Go source");
            let deadline = GoOperationDeadline::after(Duration::from_secs(60));
            let repository_cache = root.join(cache_relative);
            let repository_cache_roots = vec![
                repository_cache.join("analysis"),
                repository_cache.join("layers"),
            ];
            for cache_root in &repository_cache_roots {
                fs::create_dir_all(cache_root).expect("precreate active repository cache root");
            }
            let snapshot = prepare_dependency_snapshot(
                &cache,
                &toolchain,
                Some((&root, &config)),
                &repository_cache_roots,
                deadline,
            )
            .expect("prepare dependency snapshot with stabilized cache roots");

            fs::write(repository_cache.join("layers/entry"), b"mutable cache")
                .expect("populate repo-local cache");
            verify_dependency_snapshot(&snapshot, &toolchain, deadline)
                .expect("polint-owned cache lifecycle must not invalidate Go inputs");

            let cache_parent = if cache_relative == Path::new(".") {
                root.as_path()
            } else {
                repository_cache
                    .parent()
                    .expect("configured cache has a repository-contained parent")
            };
            fs::create_dir(cache_parent.join("unrelated-input"))
                .expect("create sibling beside configured cache");
            let error = match verify_dependency_snapshot(&snapshot, &toolchain, deadline) {
                Err(error) => error,
                Ok(()) => panic!("cache ancestor siblings must remain certified for {repository}"),
            };
            assert!(
                error
                    .to_string()
                    .contains("inputs changed after preparation")
            );

            fs::remove_dir(cache_parent.join("unrelated-input"))
                .expect("remove sibling before replacement regression");
            let replacement_snapshot = prepare_dependency_snapshot(
                &cache,
                &toolchain,
                Some((&root, &config)),
                &repository_cache_roots,
                deadline,
            )
            .expect("prepare snapshot before hostile cache-root replacement");
            fs::remove_file(repository_cache.join("layers/entry"))
                .expect("empty cache root before replacement");
            fs::remove_dir(repository_cache.join("layers")).expect("remove certified cache root");
            let outside = temp.path().join(format!("{repository}-outside"));
            fs::create_dir(&outside).expect("create replacement target");
            symlink(&outside, repository_cache.join("layers"))
                .expect("replace certified cache root with symlink");
            let error = verify_dependency_snapshot(&replacement_snapshot, &toolchain, deadline)
                .expect_err("cache-root symlink replacement must fail certification");
            assert!(
                error.to_string().contains("repository-cache exclusion"),
                "unexpected replacement error for {repository}: {error}"
            );
        }
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn published_dependency_snapshot_rejects_symlink_destination() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temp = tempfile::tempdir().expect("tempdir");
        let snapshots = temp.path().join("snapshots");
        let outside = temp.path().join("outside");
        fs::create_dir(&snapshots).expect("create snapshots root");
        fs::create_dir(&outside).expect("create outside directory");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o500))
            .expect("seal outside directory");
        let destination = snapshots.join("hostile");
        symlink(&outside, &destination).expect("create hostile destination symlink");

        let error = capture_published_dependency_closure(
            &snapshots,
            &destination,
            GoOperationDeadline::after(Duration::from_secs(1)),
        )
        .expect_err("symlink destination must be rejected");

        assert!(error.to_string().contains("direct regular directory"));
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o700))
            .expect("reopen outside directory");
    }

    #[cfg(unix)]
    #[test]
    fn published_dependency_snapshot_covers_workspace_and_root_metadata() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let snapshots = temp.path().join("snapshots");
        let destination = snapshots.join("snapshot");
        let modules = destination.join("modules");
        let workspace = destination.join("workspace");
        fs::create_dir_all(&modules).expect("create modules");
        fs::create_dir(&workspace).expect("create workspace");
        fs::write(workspace.join("go.work"), "go 1.25\n").expect("write workspace");
        seal_dependency_tree(
            &destination,
            GoOperationDeadline::after(Duration::from_secs(1)),
        )
        .expect("seal complete snapshot");
        let baseline = capture_published_dependency_closure(
            &snapshots,
            &destination,
            GoOperationDeadline::after(Duration::from_secs(1)),
        )
        .expect("capture sealed snapshot");

        assert_eq!(
            fs::symlink_metadata(&destination)
                .expect("snapshot metadata")
                .permissions()
                .mode()
                & 0o777,
            0o500
        );
        assert!(baseline.entry_count >= 3);
        fs::set_permissions(&workspace, fs::Permissions::from_mode(0o700))
            .expect("reopen workspace");
        fs::set_permissions(workspace.join("go.work"), fs::Permissions::from_mode(0o600))
            .expect("reopen workspace file");
        fs::write(workspace.join("go.work"), "go 1.26\n").expect("mutate workspace");
        fs::set_permissions(workspace.join("go.work"), fs::Permissions::from_mode(0o400))
            .expect("reseal workspace file");
        fs::set_permissions(&workspace, fs::Permissions::from_mode(0o500))
            .expect("reseal workspace");

        let changed = capture_published_dependency_closure(
            &snapshots,
            &destination,
            GoOperationDeadline::after(Duration::from_secs(1)),
        )
        .expect("capture mutated sealed snapshot");
        assert_ne!(baseline.content_digest, changed.content_digest);
        make_directory_tree_writable(&snapshots).expect("reopen snapshots for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn dependency_metadata_recertification_detects_a_restored_nested_child_swap() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        let nested = modules.join("example.test/module@v1.0.0");
        fs::create_dir_all(&nested).expect("create nested module cache");
        let source = nested.join("module.go");
        let bytes = b"package module\n";
        fs::write(&source, bytes).expect("write nested module source");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        seal_dependency_tree(&modules, deadline).expect("seal module cache");
        let baseline = capture_dependency_closure(&modules, deadline)
            .expect("capture baseline module closure");

        fs::set_permissions(&nested, fs::Permissions::from_mode(0o700))
            .expect("open nested module directory");
        fs::remove_file(&source).expect("remove original nested module source");
        fs::write(&source, bytes).expect("restore identical nested module bytes");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o400))
            .expect("seal replacement source");
        fs::set_permissions(&nested, fs::Permissions::from_mode(0o500))
            .expect("reseal nested module directory");

        let restored = capture_dependency_closure(&modules, deadline)
            .expect("capture restored module closure");
        assert_eq!(baseline.content_digest, restored.content_digest);
        assert_eq!(baseline.root_metadata_digest, restored.root_metadata_digest);
        assert_eq!(baseline.entry_count, restored.entry_count);
        assert_eq!(baseline.byte_count, restored.byte_count);
        assert_ne!(baseline.metadata_digest, restored.metadata_digest);
        make_directory_tree_writable(&modules).expect("reopen modules for cleanup");
    }

    #[cfg(windows)]
    #[test]
    fn dependency_closure_accepts_the_certified_scope_path_spelling() {
        let temp = tempfile::tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        let nested = modules.join("example.test/module@v1.0.0");
        fs::create_dir_all(&nested).expect("create nested module cache");
        fs::write(nested.join("module.go"), b"package module\n")
            .expect("write nested module source");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        seal_dependency_tree(&modules, deadline).expect("seal module cache");

        let closure = capture_dependency_closure(&modules, deadline)
            .expect("lexical input and certified scope spelling must remain equivalent");

        assert!(closure.entry_count >= 2);
        make_directory_tree_writable(&modules).expect("reopen modules for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn dependency_snapshot_rejects_external_local_replacements() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        let outside = temp.path().join("outside");
        fs::create_dir(&root).expect("create repository");
        fs::create_dir(&outside).expect("create external dependency");
        fs::write(
            root.join("go.mod"),
            "module example.test/app\n\ngo 1.25\n\nrequire example.test/dep v0.0.0\n\nreplace example.test/dep => ../outside\n",
        )
        .expect("write application go.mod");
        fs::write(
            root.join("main.go"),
            "package app\nimport _ \"example.test/dep\"\n",
        )
        .expect("write application source");
        fs::write(
            outside.join("go.mod"),
            "module example.test/dep\n\ngo 1.25\n",
        )
        .expect("write dependency go.mod");
        fs::write(outside.join("dep.go"), "package dep\n")
            .expect("write external dependency source");
        let config = GoAnalysisConfig {
            module_roots: vec![".".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);

        let error = PreparedGoSemanticFrontend::prepare_with_cache_root_for_analysis(
            &cache,
            Some((&root, &config)),
            &[],
            GoOperationDeadline::after(Duration::from_secs(60)),
        )
        .expect_err("external local replacement must not enter the dependency closure");

        assert!(
            error.to_string().contains("external local"),
            "unexpected rejection: {error:?}"
        );
        if cache.exists() {
            make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
        }
    }

    #[cfg(unix)]
    #[test]
    fn dependency_snapshot_binds_in_repo_replacement_content_to_identity() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        let dependency = root.join("dep");
        fs::create_dir_all(&dependency).expect("create local dependency");
        fs::write(
            root.join("go.mod"),
            "module example.test/app\n\ngo 1.25\n\nrequire example.test/dep v0.0.0\n\nreplace example.test/dep => ./dep\n",
        )
        .expect("write application go.mod");
        fs::write(
            root.join("main.go"),
            "package app\nimport _ \"example.test/dep\"\n",
        )
        .expect("write application source");
        fs::write(
            dependency.join("go.mod"),
            "module example.test/dep\n\ngo 1.25\n",
        )
        .expect("write dependency go.mod");
        fs::write(dependency.join("dep.go"), "package dep\n").expect("write dependency source");
        fs::write(dependency.join("go.sum"), "").expect("write dependency go.sum");
        let config = GoAnalysisConfig {
            module_roots: vec![".".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);

        let first = PreparedGoSemanticFrontend::prepare_with_cache_root_for_analysis(
            &cache,
            Some((&root, &config)),
            &[],
            GoOperationDeadline::after(Duration::from_secs(60)),
        )
        .expect("in-repository local replacement should be supported");
        assert!(first.dependency_snapshot.local_inputs.is_some());
        let first_identity = first.identity_digest();

        fs::write(dependency.join("go.sum"), "\n").expect("mutate local dependency go.sum");
        let manifest_error = verify_dependency_snapshot(
            &first.dependency_snapshot,
            &first.toolchain,
            GoOperationDeadline::after(Duration::from_secs(10)),
        )
        .expect_err("extra local module go.sum mutation must be detected");
        assert!(
            manifest_error
                .to_string()
                .contains("inputs changed after preparation")
        );

        fs::write(
            dependency.join("dep.go"),
            "package dep\n\nconst Changed = true\n",
        )
        .expect("mutate local dependency source");
        let error = verify_dependency_snapshot(
            &first.dependency_snapshot,
            &first.toolchain,
            GoOperationDeadline::after(Duration::from_secs(10)),
        )
        .expect_err("prepared local dependency mutation must be detected");
        assert!(error.to_string().contains("changed after preparation"));

        let second = PreparedGoSemanticFrontend::prepare_with_cache_root_for_analysis(
            &cache,
            Some((&root, &config)),
            &[],
            GoOperationDeadline::after(Duration::from_secs(60)),
        )
        .expect("mutated local replacement should prepare with a new identity");
        assert_ne!(first_identity, second.identity_digest());
        assert_ne!(
            first.dependency_snapshot.local_dependencies_digest,
            second.dependency_snapshot.local_dependencies_digest
        );
        if cache.exists() {
            make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
        }
    }

    #[cfg(unix)]
    #[test]
    fn dependency_snapshot_resolves_synthetic_multi_module_workspace_without_replaces() {
        let _semantic_scope =
            acquire_test_go_semantic_concurrency_scope().expect("acquire Go semantic test scope");
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        let app = root.join("services/app");
        let library = root.join("services/library");
        fs::create_dir_all(&app).expect("create app module");
        fs::create_dir_all(&library).expect("create library module");
        fs::write(
            app.join("go.mod"),
            "module example.test/app\n\ngo 1.25\n\nrequire example.test/library v0.0.0\n",
        )
        .expect("write app go.mod");
        fs::write(
            app.join("app.go"),
            "package app\nimport _ \"example.test/library\"\n",
        )
        .expect("write app source");
        fs::write(
            library.join("go.mod"),
            "module example.test/library\n\ngo 1.25\n",
        )
        .expect("write library go.mod");
        fs::write(library.join("library.go"), "package library\n").expect("write library source");
        let config = GoAnalysisConfig {
            module_roots: vec!["services/app".to_string(), "services/library".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);

        let prepared = PreparedGoSemanticFrontend::prepare_with_cache_root_for_analysis(
            &cache,
            Some((&root, &config)),
            &[],
            GoOperationDeadline::after(Duration::from_secs(60)),
        )
        .expect("synthetic multi-module workspace should remain valid when privatized");

        assert!(
            prepared
                .dependency_snapshot
                .local_inputs
                .as_ref()
                .is_some_and(|inputs| inputs.package_count >= 2)
        );
        assert!(!root.join("go.work").exists());
        assert!(!root.join("go.work.sum").exists());
        if cache.exists() {
            make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
        }
    }

    #[cfg(unix)]
    #[test]
    fn dependency_snapshot_uses_workspace_module_for_transitive_local_requirement() {
        let _semantic_scope =
            acquire_test_go_semantic_concurrency_scope().expect("acquire Go semantic test scope");
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        let app = root.join("services/app");
        let library = root.join("services/library");
        let dependency = root.join("services/dependency");
        fs::create_dir_all(&app).expect("create app module");
        fs::create_dir_all(&library).expect("create library module");
        fs::create_dir_all(&dependency).expect("create local dependency module");
        fs::write(
            app.join("go.mod"),
            "module example.test/app\n\ngo 1.25\n\nrequire example.test/dependency v0.0.0\n\nreplace example.test/dependency => ../dependency\n",
        )
        .expect("write app go.mod");
        fs::write(
            app.join("app.go"),
            "package app\nimport _ \"example.test/dependency\"\n",
        )
        .expect("write app source");
        fs::write(
            dependency.join("go.mod"),
            "module example.test/dependency\n\ngo 1.25\n\nrequire example.test/library v1.9.9\n",
        )
        .expect("write dependency go.mod");
        fs::write(
            dependency.join("dependency.go"),
            "package dependency\nimport _ \"example.test/library\"\n",
        )
        .expect("write dependency source");
        fs::write(
            library.join("go.mod"),
            "module example.test/library\n\ngo 1.25\n",
        )
        .expect("write library go.mod");
        fs::write(library.join("library.go"), "package library\n").expect("write library source");
        let config = GoAnalysisConfig {
            module_roots: vec!["services/app".to_string(), "services/library".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let before = snapshot_repository_tree(&root);
        let cache = test_cache_root(&temp);

        let prepared = PreparedGoSemanticFrontend::prepare_with_cache_root_for_analysis(
            &cache,
            Some((&root, &config)),
            &[],
            GoOperationDeadline::after(Duration::from_secs(60)),
        )
        .expect("transitive requirements must resolve workspace modules locally");

        assert!(
            prepared
                .dependency_snapshot
                .local_inputs
                .as_ref()
                .is_some_and(|inputs| inputs.package_count >= 3)
        );
        assert_eq!(snapshot_repository_tree(&root), before);
        if cache.exists() {
            make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
        }
    }

    #[cfg(unix)]
    #[test]
    fn dependency_snapshot_key_changes_when_source_imports_change() {
        let _semantic_scope =
            acquire_test_go_semantic_concurrency_scope().expect("acquire Go semantic test scope");
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        let app = root.join("services/app");
        let first_dependency = root.join("services/first");
        let second_dependency = root.join("services/second");
        for directory in [&app, &first_dependency, &second_dependency] {
            fs::create_dir_all(directory).expect("create module directory");
        }
        fs::write(
            app.join("go.mod"),
            "module example.test/app\n\ngo 1.25\n\nrequire (\n\texample.test/first v0.0.0\n\texample.test/second v0.0.0\n)\n\nreplace example.test/first => ../first\nreplace example.test/second => ../second\n",
        )
        .expect("write app go.mod");
        fs::write(
            app.join("app.go"),
            "package app\nimport _ \"example.test/first\"\nfunc Value() int { return 1 }\n",
        )
        .expect("write initial app source");
        fs::write(
            first_dependency.join("go.mod"),
            "module example.test/first\n\ngo 1.25\n",
        )
        .expect("write first dependency go.mod");
        fs::write(first_dependency.join("first.go"), "package first\n")
            .expect("write first dependency source");
        fs::write(
            second_dependency.join("go.mod"),
            "module example.test/second\n\ngo 1.25\n",
        )
        .expect("write second dependency go.mod");
        fs::write(second_dependency.join("second.go"), "package second\n")
            .expect("write second dependency source");
        let config = GoAnalysisConfig {
            module_roots: vec!["services/app".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: true,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let deadline = GoOperationDeadline::after(Duration::from_secs(60));
        let toolchain = prepare_go_toolchain_until(
            &cache,
            local_go_toolchain().expect("local Go toolchain"),
            true,
            deadline,
        )
        .expect("prepare offline Go toolchain");

        let first =
            prepare_dependency_snapshot(&cache, &toolchain, Some((&root, &config)), &[], deadline)
                .expect("prepare first package-selected snapshot");
        fs::write(
            app.join("app.go"),
            "package app\nimport _ \"example.test/first\"\nfunc Value() int { return 2 }\n",
        )
        .expect("change only a function body");
        let body_only =
            prepare_dependency_snapshot(&cache, &toolchain, Some((&root, &config)), &[], deadline)
                .expect("reuse snapshot after a dependency-neutral source edit");
        assert_eq!(first.snapshot_root, body_only.snapshot_root);
        assert_ne!(
            first.local_dependencies_digest,
            body_only.local_dependencies_digest
        );
        fs::write(
            app.join("app.go"),
            "package app\nimport _ \"example.test/second\"\nfunc Value() int { return 2 }\n",
        )
        .expect("change source-only import selection");
        let second =
            prepare_dependency_snapshot(&cache, &toolchain, Some((&root, &config)), &[], deadline)
                .expect("prepare source-updated package-selected snapshot");

        assert_ne!(first.snapshot_root, second.snapshot_root);
        assert_ne!(
            first.local_dependencies_digest,
            second.local_dependencies_digest
        );
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn dependency_snapshot_key_changes_when_local_replacement_manifest_changes() {
        let _semantic_scope =
            acquire_test_go_semantic_concurrency_scope().expect("acquire Go semantic test scope");
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        let app = root.join("services/app");
        let dependency = root.join("services/dependency");
        fs::create_dir_all(&app).expect("create app module");
        fs::create_dir_all(&dependency).expect("create dependency module");
        fs::write(
            app.join("go.mod"),
            "module example.test/app\n\ngo 1.25\n\nrequire example.test/dependency v0.0.0\n\nreplace example.test/dependency => ../dependency\n",
        )
        .expect("write app go.mod");
        fs::write(
            app.join("app.go"),
            "package app\nimport _ \"example.test/dependency\"\n",
        )
        .expect("write app source");
        fs::write(
            dependency.join("go.mod"),
            "module example.test/dependency\n\ngo 1.24\n",
        )
        .expect("write dependency go.mod");
        fs::write(dependency.join("dependency.go"), "package dependency\n")
            .expect("write dependency source");
        let config = GoAnalysisConfig {
            module_roots: vec!["services/app".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: true,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let deadline = GoOperationDeadline::after(Duration::from_secs(60));
        let toolchain = prepare_go_toolchain_until(
            &cache,
            local_go_toolchain().expect("local Go toolchain"),
            true,
            deadline,
        )
        .expect("prepare offline Go toolchain");

        let first =
            prepare_dependency_snapshot(&cache, &toolchain, Some((&root, &config)), &[], deadline)
                .expect("prepare first local-replacement snapshot");
        fs::write(
            dependency.join("go.mod"),
            "module example.test/dependency\n\ngo 1.25\n",
        )
        .expect("change only the replacement manifest");
        let second =
            prepare_dependency_snapshot(&cache, &toolchain, Some((&root, &config)), &[], deadline)
                .expect("prepare manifest-updated local-replacement snapshot");

        assert_ne!(first.snapshot_root, second.snapshot_root);
        assert_ne!(
            first.local_dependencies_digest,
            second.local_dependencies_digest
        );
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn dependency_source_binding_includes_explicitly_selectable_directories() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("create repository");
        let deadline = GoOperationDeadline::after(Duration::from_secs(5));
        let baseline =
            dependency_population_source_digest(&root, std::slice::from_ref(&root), &[], deadline)
                .expect("bind empty repository sources");

        for directory in ["testdata/package", "_hidden/package", ".hidden/package"] {
            let package = root.join(directory);
            fs::create_dir_all(&package).expect("create explicitly selectable package");
            fs::write(package.join("package.go"), "package selected\n")
                .expect("write explicitly selectable source");
            let changed = dependency_population_source_digest(
                &root,
                std::slice::from_ref(&root),
                &[],
                deadline,
            )
            .expect("bind explicitly selectable source");
            assert_ne!(baseline, changed, "source under {directory} must be bound");
            fs::remove_dir_all(root.join(directory.split('/').next().expect("top directory")))
                .expect("remove explicitly selectable package");
        }
    }

    #[cfg(unix)]
    #[test]
    fn unavailable_dependency_does_not_publish_an_incomplete_snapshot() {
        let _semantic_scope =
            acquire_test_go_semantic_concurrency_scope().expect("acquire Go semantic test scope");
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("create repository");
        fs::write(
            root.join("go.mod"),
            "module example.test/app\n\ngo 1.25\n\nrequire example.invalid/missing v1.0.0\n",
        )
        .expect("write app go.mod");
        fs::write(
            root.join("app.go"),
            "package app\nimport _ \"example.invalid/missing\"\n",
        )
        .expect("write app source");
        let config = GoAnalysisConfig {
            module_roots: vec![".".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: true,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let deadline = GoOperationDeadline::after(Duration::from_secs(60));
        let toolchain = prepare_go_toolchain_until(
            &cache,
            local_go_toolchain().expect("local Go toolchain"),
            true,
            deadline,
        )
        .expect("prepare offline Go toolchain");

        for attempt in 1..=2 {
            let error = prepare_dependency_snapshot(
                &cache,
                &toolchain,
                Some((&root, &config)),
                &[],
                deadline,
            )
            .expect_err("unavailable dependency must fail snapshot population");
            assert!(
                error
                    .to_string()
                    .contains("dependency snapshot package population failed"),
                "unexpected attempt {attempt} error: {error:?}"
            );
        }

        let snapshots = cache.join("dependency-snapshots");
        let published = fs::read_dir(&snapshots)
            .expect("read dependency snapshots root")
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.len() == 64
                    && name.bytes().all(|byte| byte.is_ascii_hexdigit())
                    && entry.file_type().is_ok_and(|kind| kind.is_dir())
            })
            .count();
        assert_eq!(published, 0, "failed population must remain unpublished");
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn checked_in_workspace_and_sum_are_copied_without_repository_writes() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        let app = root.join("app");
        let library = root.join("library");
        fs::create_dir_all(&app).expect("create app module");
        fs::create_dir_all(&library).expect("create library module");
        fs::write(app.join("go.mod"), "module example.test/app\n\ngo 1.25\n")
            .expect("write app go.mod");
        fs::write(
            library.join("go.mod"),
            "module example.test/library\n\ngo 1.25\n",
        )
        .expect("write library go.mod");
        fs::write(
            root.join("go.work"),
            "go 1.25\n\nuse (\n\t./app\n\t./library\n)\n",
        )
        .expect("write go.work");
        let checked_in_sum =
            b"golang.org/x/mod v0.36.0 h1:JJjpVx6myfUsUdAzZuOSTTmRE0PfZeNWzzvKrP7amb4=\n";
        fs::write(root.join("go.work.sum"), checked_in_sum).expect("write go.work.sum");
        let config = GoAnalysisConfig {
            module_roots: vec!["app".to_string(), "library".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let toolchain =
            prepare_go_toolchain(&cache, local_go_toolchain().expect("local Go toolchain"))
                .expect("prepare Go toolchain");
        let private_root = ensure_private_subdirectory(&cache, Path::new("workspace-test"))
            .expect("private workspace root");
        for path in [&root, &app, &library] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o500))
                .expect("make repository directory read-only");
        }
        for path in [
            app.join("go.mod"),
            library.join("go.mod"),
            root.join("go.work"),
            root.join("go.work.sum"),
        ] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o400))
                .expect("make repository manifest read-only");
        }

        let workspace = private_go_workspace(
            &private_root,
            &toolchain,
            &root,
            &config,
            GoOperationDeadline::after(Duration::from_secs(30)),
        )
        .expect("copy checked-in workspace into private cache");
        let private_path = workspace.path().expect("private workspace path");
        let mut private_sum = private_path.as_os_str().to_os_string();
        private_sum.push(".sum");

        assert!(private_path.starts_with(&cache));
        assert_eq!(
            fs::read(PathBuf::from(private_sum)).expect("private sum"),
            checked_in_sum
        );
        assert!(fs::read_dir(&root).expect("read repository").all(|entry| {
            !entry
                .expect("repository entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".polint-dependency-")
        }));

        for path in [&root, &app, &library] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .expect("restore repository directory");
        }
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn synthetic_workspace_is_created_with_an_unwritable_repository_parent() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let repository_parent = temp.path().join("read-only-parent");
        let root = repository_parent.join("repo");
        let app = root.join("app");
        let library = root.join("library");
        fs::create_dir_all(&app).expect("create app module");
        fs::create_dir_all(&library).expect("create library module");
        fs::write(app.join("go.mod"), "module example.test/app\n\ngo 1.25\n")
            .expect("write app go.mod");
        fs::write(
            library.join("go.mod"),
            "module example.test/library\n\ngo 1.25\n",
        )
        .expect("write library go.mod");
        let config = GoAnalysisConfig {
            module_roots: vec!["app".to_string(), "library".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let toolchain =
            prepare_go_toolchain(&cache, local_go_toolchain().expect("local Go toolchain"))
                .expect("prepare Go toolchain");
        let private_root = ensure_private_subdirectory(&cache, Path::new("workspace-test"))
            .expect("private workspace root");
        fs::set_permissions(&repository_parent, fs::Permissions::from_mode(0o500))
            .expect("make repository parent unwritable");

        let workspace = private_go_workspace(
            &private_root,
            &toolchain,
            &root,
            &config,
            GoOperationDeadline::after(Duration::from_secs(30)),
        )
        .expect("prepare synthetic workspace without adjacent writes");

        assert!(
            workspace
                .path()
                .is_some_and(|path| path.starts_with(&cache))
        );
        assert_eq!(
            fs::read_dir(&repository_parent)
                .expect("read repository parent")
                .count(),
            1
        );
        fs::set_permissions(&repository_parent, fs::Permissions::from_mode(0o700))
            .expect("restore repository parent");
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn private_workspace_preserves_tabbed_toolchain_and_godebug_block() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        let app = root.join("app");
        fs::create_dir_all(&app).expect("create app module");
        fs::write(app.join("go.mod"), "module example.test/app\n\ngo 1.25\n")
            .expect("write app go.mod");
        fs::write(
            root.join("go.work"),
            "go 1.25\ntoolchain\tgo1.25.0\ngodebug\t(\n\tdefault=go1.21\n\tasynctimerchan=1\n)\nuse\t./app\n",
        )
        .expect("write tabbed workspace");
        let config = GoAnalysisConfig {
            module_roots: vec!["app".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let toolchain =
            prepare_go_toolchain(&cache, local_go_toolchain().expect("local Go toolchain"))
                .expect("prepare Go toolchain");
        let private_root = ensure_private_subdirectory(&cache, Path::new("workspace-test"))
            .expect("private workspace root");

        let workspace = private_go_workspace(
            &private_root,
            &toolchain,
            &root,
            &config,
            GoOperationDeadline::after(Duration::from_secs(30)),
        )
        .expect("normalize tabbed workspace without losing directives");
        let bytes = fs::read(workspace.path().expect("private workspace path"))
            .expect("read normalized workspace");
        let auxiliary = parse_workspace_auxiliary_directives(
            &bytes,
            GoOperationDeadline::after(Duration::from_secs(1)),
        )
        .expect("parse normalized auxiliary directives");

        assert_eq!(auxiliary.toolchain.as_deref(), Some("go1.25.0"));
        assert_eq!(auxiliary.godebug, ["default=go1.21", "asynctimerchan=1"]);
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn synthetic_workspace_records_external_checksums_only_in_private_sum() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        let app = root.join("app");
        let library = root.join("library");
        fs::create_dir_all(&app).expect("create app module");
        fs::create_dir_all(&library).expect("create library module");
        fs::write(
            app.join("go.mod"),
            "module example.test/app\n\ngo 1.25\n\nrequire golang.org/x/mod v0.36.0\n",
        )
        .expect("write app go.mod");
        fs::write(
            app.join("app.go"),
            "package app\nimport \"golang.org/x/mod/module\"\nvar _ = module.Version{}\n",
        )
        .expect("write app source");
        fs::write(
            library.join("go.mod"),
            "module example.test/library\n\ngo 1.25\n",
        )
        .expect("write library go.mod");
        fs::write(library.join("library.go"), "package library\n").expect("write library source");
        let config = GoAnalysisConfig {
            module_roots: vec!["app".to_string(), "library".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);

        let prepared = PreparedGoSemanticFrontend::prepare_with_cache_root_for_analysis(
            &cache,
            Some((&root, &config)),
            &[],
            GoOperationDeadline::after(Duration::from_secs(90)),
        )
        .expect("populate synthetic workspace checksum state privately");
        let workspace = prepared
            .dependency_snapshot
            .workspace_path
            .as_ref()
            .expect("published private workspace");
        let mut sum = workspace.as_os_str().to_os_string();
        sum.push(".sum");
        let sum = fs::read_to_string(PathBuf::from(sum)).expect("private workspace sum");

        assert!(sum.contains("golang.org/x/mod v0.36.0"));
        assert!(!root.join("go.work").exists());
        assert!(!root.join("go.work.sum").exists());
        assert!(!app.join("go.sum").exists());
        assert!(
            prepared
                .dependency_snapshot
                .local_inputs
                .as_ref()
                .is_some_and(|inputs| inputs.package_count >= 2)
        );
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn root_module_checksums_are_written_only_to_the_private_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("create repository");
        fs::write(
            root.join("go.mod"),
            "module example.test/app\n\ngo 1.25\n\nrequire golang.org/x/mod v0.36.0\n",
        )
        .expect("write root go.mod");
        fs::write(
            root.join("app.go"),
            "package app\nimport \"golang.org/x/mod/module\"\nvar _ = module.Version{}\n",
        )
        .expect("write root source");
        let config = GoAnalysisConfig {
            module_roots: vec![".".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        };
        let cache = test_cache_root(&temp);

        let prepared = PreparedGoSemanticFrontend::prepare_with_cache_root_for_analysis(
            &cache,
            Some((&root, &config)),
            &[],
            GoOperationDeadline::after(Duration::from_secs(90)),
        )
        .expect("populate root-module checksum state privately");
        let workspace = prepared
            .dependency_snapshot
            .workspace_path
            .as_ref()
            .expect("root module uses a private workspace");
        let mut sum = workspace.as_os_str().to_os_string();
        sum.push(".sum");
        let sum = fs::read_to_string(PathBuf::from(sum)).expect("private workspace sum");

        assert!(sum.contains("golang.org/x/mod v0.36.0"));
        assert!(!root.join("go.sum").exists());
        assert!(!root.join("go.work").exists());
        assert!(!root.join("go.work.sum").exists());
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[test]
    fn directory_entry_collection_is_bounded_before_push() {
        let temp = tempfile::tempdir().expect("temporary directory");
        for index in 0..7 {
            fs::write(temp.path().join(format!("entry-{index}")), b"entry")
                .expect("write directory entry");
        }
        let mut discovered = fs::read_dir(temp.path())
            .expect("enumerate test directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect test entries")
            .into_iter();

        let mut bounded = Vec::new();
        for _ in 0..2 {
            push_bounded_directory_entry(
                &mut bounded,
                discovered.next().expect("bounded entry"),
                0,
                2,
                0,
                "test closure",
            )
            .expect("entry within limit");
        }
        let error = push_bounded_directory_entry(
            &mut bounded,
            discovered.next().expect("rejected entry"),
            0,
            2,
            0,
            "test closure",
        )
        .expect_err("entry beyond limit must be rejected before insertion");
        assert!(error.to_string().contains("more than 2 entries"));
        assert_eq!(bounded.len(), 2);

        let mut with_one_skip = Vec::new();
        for _ in 0..3 {
            push_bounded_directory_entry(
                &mut with_one_skip,
                discovered.next().expect("allowance entry"),
                0,
                2,
                1,
                "test closure",
            )
            .expect("explicit skip allowance remains bounded");
        }
        push_bounded_directory_entry(
            &mut with_one_skip,
            discovered.next().expect("entry beyond allowance"),
            0,
            2,
            1,
            "test closure",
        )
        .expect_err("only one deliberately skipped entry may exceed the closure limit");
        assert_eq!(with_one_skip.len(), 3);
    }

    #[cfg(unix)]
    #[test]
    fn toolchain_closure_walk_honors_a_near_expired_absolute_deadline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let target = GoHostTarget::current_process().expect("supported host");
        let tools = temp
            .path()
            .join("pkg/tool")
            .join(target.label().replace('/', "_"));
        fs::create_dir_all(&tools).expect("create delegated tools");
        fs::write(tools.join("compile"), b"compiler").expect("write delegated tool");
        let bulk = temp.path().join("bulk");
        fs::create_dir(&bulk).expect("create bulk directory");
        for index in 0..4_000 {
            fs::write(bulk.join(format!("entry-{index:04}")), b"x").expect("write closure entry");
        }
        let started = Instant::now();

        let error = capture_go_toolchain_closure_until(
            temp.path(),
            &target,
            true,
            GoOperationDeadline::after(Duration::from_nanos(1)),
        )
        .expect_err("near-expired closure walk must stop");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn toolchain_capture_rejects_a_same_size_swap_at_the_content_hash_boundary() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let goroot = temp.path().join("goroot");
        let target = GoHostTarget::current_process().expect("supported host");
        let tools = goroot
            .join("pkg/tool")
            .join(target.label().replace('/', "_"));
        fs::create_dir_all(goroot.join("bin")).expect("create fake toolchain bin");
        fs::create_dir_all(&tools).expect("create fake delegated tool directory");
        fs::write(goroot.join("bin").join(go_toolchain_binary_name()), b"go")
            .expect("write acquisition launcher");
        let delegated = tools.join("compile");
        let saved = temp.path().join("saved-compile");
        let replacement = temp.path().join("replacement-compile");
        fs::write(&delegated, b"compiler-a").expect("write original delegated tool");
        fs::write(&replacement, b"compiler-b").expect("write same-size replacement");
        let hook = |path: &Path, before: bool| {
            if path != delegated {
                return;
            }
            if before {
                fs::rename(&delegated, &saved).expect("save enumerated delegated tool");
                fs::rename(&replacement, &delegated).expect("install same-size replacement");
            } else {
                fs::rename(&delegated, &replacement).expect("remove same-size replacement");
                fs::rename(&saved, &delegated).expect("restore enumerated delegated tool");
            }
        };

        let error = capture_go_toolchain_closure_inner(
            &goroot,
            &target,
            true,
            GoOperationDeadline::after(Duration::from_secs(5)),
            Some(&hook),
        )
        .expect_err("content hashing must bind the enumerated file identity");

        assert!(error.to_string().contains("changed before content hashing"));
        assert_eq!(
            fs::read(&delegated).expect("read restored tool"),
            b"compiler-a"
        );
        assert_eq!(
            fs::read(&replacement).expect("read rejected replacement"),
            b"compiler-b"
        );
    }

    #[cfg(unix)]
    #[test]
    fn prepared_frontend_pins_selected_go_toolchain_after_launcher_replacement() {
        use std::os::unix::fs::PermissionsExt;

        fn write_launcher(path: &Path, label: &str, target: &str, goroot: &Path) {
            let script = format!(
                "#!/bin/sh\nif [ -n \"$GOPROXY\" ] && [ \"$GOPROXY\" != off ]; then\n  printf 'credential=%s\\n' \"$GOPROXY\"\nelif [ \"$1\" = version ]; then\n  printf 'go version go1.25.0 {target} {label}\\n'\nelif [ \"$1\" = env ] && [ \"$2\" = GOROOT ]; then\n  printf '%s\\n' '{}'\nelse\n  exit 9\nfi\n",
                goroot.display()
            );
            fs::write(path, script).expect("write fake go launcher");
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .expect("make fake go executable");
        }

        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let target = GoHostTarget::current_process()
            .expect("supported host")
            .label();
        let goroot_a = temp.path().join("goroot-a");
        let goroot_b = temp.path().join("goroot-b");
        fs::create_dir(&goroot_a).expect("create goroot A");
        fs::create_dir(&goroot_b).expect("create goroot B");
        let bin_a = goroot_a.join("bin");
        let bin_b = goroot_b.join("bin");
        fs::create_dir(&bin_a).expect("create bin A");
        fs::create_dir(&bin_b).expect("create bin B");
        let tools_a = goroot_a.join("pkg/tool").join(target.replace('/', "_"));
        let tools_b = goroot_b.join("pkg/tool").join(target.replace('/', "_"));
        fs::create_dir_all(&tools_a).expect("create delegated tools A");
        fs::create_dir_all(&tools_b).expect("create delegated tools B");
        fs::write(tools_a.join("compile"), b"compiler").expect("write delegated tool A");
        fs::write(tools_b.join("compile"), b"compiler").expect("write delegated tool B");
        let go_a = bin_a.join("go");
        let go_b = bin_b.join("go");
        write_launcher(&go_a, "A", &target, &goroot_a);
        write_launcher(&go_b, "B", &target, &goroot_b);
        let path_a = std::env::join_paths([&bin_a]).expect("PATH A");
        let path_b = std::env::join_paths([&bin_b]).expect("PATH B");
        let prepared_go_a =
            prepare_go_toolchain(&cache, local_go_toolchain_in(path_a).expect("toolchain A"))
                .expect("prepare toolchain A");
        let prepared_go_b =
            prepare_go_toolchain(&cache, local_go_toolchain_in(path_b).expect("toolchain B"))
                .expect("prepare toolchain B");
        let frontend = temp.path().join("frontend");
        fs::write(&frontend, "#!/bin/sh\nexec go version\n").expect("write frontend");
        fs::set_permissions(&frontend, fs::Permissions::from_mode(0o700))
            .expect("make frontend executable");
        let prepared_a =
            prepare_binary_frontend(&cache, &frontend, prepared_go_a).expect("prepare frontend A");
        let prepared_b =
            prepare_binary_frontend(&cache, &frontend, prepared_go_b).expect("prepare frontend B");
        let mut credential_probe = prepared_a.toolchain.as_ref().clone();
        credential_probe.environment.variables.insert(
            "GOPROXY".to_string(),
            OsString::from("https://probe-user:probe-secret@proxy.example"),
        );
        probe_prepared_go_toolchain_until(
            &credential_probe,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect("prepared version probe must not receive credential-bearing analysis env");
        write_launcher(&go_a, "REPLACED", &target, &goroot_a);

        let output = prepared_a
            .command(temp.path())
            .expect("construct prepared frontend command")
            .output()
            .expect("run prepared frontend");

        assert_ne!(prepared_a.identity_digest(), prepared_b.identity_digest());
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(" A"),
            "unexpected prepared frontend result: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        verify_go_toolchain_binding_until(
            &prepared_a.toolchain,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect("replacing the unused acquisition launcher remains safe");
        fs::write(tools_a.join("compile"), b"tampered")
            .expect("mutate delegated tool after preparation");
        let error = verify_go_toolchain_binding_until(
            &prepared_a.toolchain,
            GoOperationDeadline::after(Duration::from_secs(5)),
        )
        .expect_err("a changed delegated tool must invalidate runtime binding");
        assert!(error.to_string().contains("toolchain closure changed"));
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn frontend_identity_binds_full_toolchain_content_but_not_location_metadata() {
        use std::os::unix::fs::PermissionsExt;

        fn write_toolchain(
            root: &Path,
            launcher: &[u8],
            delegated: &[u8],
            standard_library: &[u8],
            target: &str,
        ) {
            let bin = root.join("bin");
            let tools = root.join("pkg/tool").join(target.replace('/', "_"));
            let runtime = root.join("src/runtime");
            fs::create_dir_all(&bin).expect("create fake toolchain bin");
            fs::create_dir_all(&tools).expect("create fake delegated tool directory");
            fs::create_dir_all(&runtime).expect("create fake standard-library directory");
            fs::write(bin.join("go"), launcher).expect("write fake launcher");
            fs::set_permissions(bin.join("go"), fs::Permissions::from_mode(0o700))
                .expect("make fake launcher executable");
            fs::write(tools.join("compile"), delegated).expect("write delegated tool");
            fs::write(runtime.join("runtime.go"), standard_library)
                .expect("write fake standard-library source");
        }

        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let target = GoHostTarget::current_process()
            .expect("supported host")
            .label();
        let launcher = format!(
            "#!/bin/sh\nif [ \"$1\" = version ]; then printf 'go version go1.25.0 {target}\\n'; else exit 9; fi\n"
        );
        let goroot_a = temp.path().join("goroot-a");
        let goroot_b = temp.path().join("goroot-b");
        let goroot_c = temp.path().join("goroot-c");
        let goroot_d = temp.path().join("goroot-d");
        let goroot_e = temp.path().join("goroot-e");
        write_toolchain(
            &goroot_a,
            launcher.as_bytes(),
            b"compiler-a",
            b"package runtime\nconst value = 1\n",
            &target,
        );
        write_toolchain(
            &goroot_b,
            launcher.as_bytes(),
            b"compiler-b",
            b"package runtime\nconst value = 1\n",
            &target,
        );
        write_toolchain(
            &goroot_c,
            launcher.as_bytes(),
            b"compiler-a",
            b"package runtime\nconst value = 1\n",
            &target,
        );
        write_toolchain(
            &goroot_d,
            launcher.as_bytes(),
            b"compiler-a",
            b"package runtime\nconst value = 2\n",
            &target,
        );
        write_toolchain(
            &goroot_e,
            launcher.as_bytes(),
            b"compiler-a",
            b"package runtime\nconst value = 1\n",
            &target,
        );
        fs::set_permissions(
            goroot_e
                .join("pkg/tool")
                .join(target.replace('/', "_"))
                .join("compile"),
            fs::Permissions::from_mode(0o700),
        )
        .expect("change delegated-tool execution contract");

        let selected_a = local_go_toolchain_in(
            std::env::join_paths([goroot_a.join("bin")]).expect("toolchain A PATH"),
        )
        .expect("select toolchain A");
        let selected_b = local_go_toolchain_in(
            std::env::join_paths([goroot_b.join("bin")]).expect("toolchain B PATH"),
        )
        .expect("select toolchain B");
        let selected_c = local_go_toolchain_in(
            std::env::join_paths([goroot_c.join("bin")]).expect("toolchain C PATH"),
        )
        .expect("select relocated byte-identical toolchain C");
        let selected_d = local_go_toolchain_in(
            std::env::join_paths([goroot_d.join("bin")]).expect("toolchain D PATH"),
        )
        .expect("select standard-library-mutated toolchain D");
        let selected_e = local_go_toolchain_in(
            std::env::join_paths([goroot_e.join("bin")]).expect("toolchain E PATH"),
        )
        .expect("select access-mutated toolchain E");
        assert_eq!(selected_a.executable_digest, selected_b.executable_digest);
        assert_ne!(
            selected_a.closure.content_digest,
            selected_b.closure.content_digest
        );
        assert_eq!(
            selected_a.closure.content_digest,
            selected_c.closure.content_digest
        );
        assert_ne!(selected_a.closure.digest, selected_c.closure.digest);
        assert_ne!(
            selected_a.closure.content_digest,
            selected_d.closure.content_digest
        );
        assert_ne!(
            selected_a.closure.content_digest,
            selected_e.closure.content_digest
        );

        let prepared_a =
            prepare_go_toolchain(&cache, selected_a).expect("prepare toolchain A closure");
        let prepared_b =
            prepare_go_toolchain(&cache, selected_b).expect("prepare toolchain B closure");
        let prepared_c =
            prepare_go_toolchain(&cache, selected_c).expect("prepare relocated toolchain C");
        let prepared_d =
            prepare_go_toolchain(&cache, selected_d).expect("prepare toolchain D closure");
        let frontend_source = temp.path().join("frontend");
        fs::write(&frontend_source, "#!/bin/sh\nprintf 'frontend\\n'\n")
            .expect("write fake frontend");
        fs::set_permissions(&frontend_source, fs::Permissions::from_mode(0o700))
            .expect("make fake frontend executable");
        let frontend_a = prepare_binary_frontend(&cache, &frontend_source, prepared_a)
            .expect("prepare frontend with toolchain A");
        let frontend_b = prepare_binary_frontend(&cache, &frontend_source, prepared_b)
            .expect("prepare frontend with toolchain B");
        let frontend_c = prepare_binary_frontend(&cache, &frontend_source, prepared_c)
            .expect("prepare frontend with relocated toolchain C");
        let frontend_d = prepare_binary_frontend(&cache, &frontend_source, prepared_d)
            .expect("prepare frontend with standard-library-mutated toolchain D");
        assert_ne!(frontend_a.identity_digest(), frontend_b.identity_digest());
        assert_eq!(frontend_a.identity_digest(), frontend_c.identity_digest());
        assert_ne!(frontend_a.identity_digest(), frontend_d.identity_digest());
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn source_snapshot_rejects_flat_file_overflow_during_walk() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("go.mod"), "module example.test/frontend\n")
            .expect("write go.mod");
        for index in 0..GO_FRONTEND_MAX_SOURCE_FILES {
            fs::write(
                temp.path().join(format!("source-{index:04}.go")),
                "package main\n",
            )
            .expect("write source");
        }

        let error = capture_source_snapshot(temp.path()).expect_err("file overflow must fail");

        assert!(error.to_string().contains("more than 512 files"));
    }

    #[cfg(unix)]
    #[test]
    fn source_snapshot_rejects_excessive_depth_during_walk() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("go.mod"), "module example.test/frontend\n")
            .expect("write go.mod");
        let mut directory = temp.path().to_path_buf();
        for index in 0..=GO_FRONTEND_MAX_SOURCE_DEPTH {
            directory.push(format!("d{index}"));
            fs::create_dir(&directory).expect("create nested directory");
        }

        let error = capture_source_snapshot(temp.path()).expect_err("depth overflow must fail");

        assert!(error.to_string().contains("traversal depth"));
    }

    #[cfg(unix)]
    #[test]
    fn source_snapshot_rejects_irrelevant_entry_overflow_during_walk() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("go.mod"), "module example.test/frontend\n")
            .expect("write go.mod");
        for index in 0..GO_FRONTEND_MAX_SOURCE_ENTRIES {
            fs::write(
                temp.path().join(format!("ignored-{index:04}.txt")),
                "ignored",
            )
            .expect("write irrelevant entry");
        }

        let error = capture_source_snapshot(temp.path()).expect_err("entry overflow must fail");

        assert!(error.to_string().contains("more than 4096 entries"));
    }

    #[cfg(unix)]
    #[test]
    fn staging_guard_quarantines_every_partial_permission_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        for state in 0..4 {
            let path;
            {
                let staging = StagingDirectory::create(temp.path(), ".failure-")
                    .expect("create staging directory");
                path = staging.path().to_path_buf();
                if state >= 1 {
                    write_new_private_file(&path.join("file"), b"contents", false)
                        .expect("write staging file");
                }
                if state >= 2 {
                    let nested = ensure_private_subdirectory(&path, Path::new("nested"))
                        .expect("create nested staging directory");
                    write_new_private_file(&nested.join("file"), b"contents", false)
                        .expect("write nested staging file");
                }
                if state >= 3 {
                    seal_execution_directory(&path).expect("seal staging directory");
                }
            }
            assert!(!path.exists(), "staging state {state} leaked");
            let mut quarantine = path.as_os_str().to_os_string();
            quarantine.push(".abandoned");
            assert!(PathBuf::from(quarantine).exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn staging_allocation_is_unique_under_concurrency() {
        let temp = tempfile::tempdir().expect("tempdir");
        let parent = std::sync::Arc::new(temp.path().to_path_buf());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(16));
        let mut handles = Vec::new();
        for _ in 0..16 {
            let parent = std::sync::Arc::clone(&parent);
            let barrier = std::sync::Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let staging = StagingDirectory::create(&parent, ".concurrent-")
                    .expect("allocate staging directory");
                let path = staging.path().to_path_buf();
                barrier.wait();
                path
            }));
        }
        let paths = handles
            .into_iter()
            .map(|handle| handle.join().expect("join staging allocator"))
            .collect::<BTreeSet<_>>();

        assert_eq!(paths.len(), 16);
        assert!(paths.iter().all(|path| !path.exists()));
    }

    #[cfg(unix)]
    #[test]
    fn stale_staging_cleanup_is_bounded_and_prefix_scoped() {
        let temp = tempfile::tempdir().expect("tempdir");
        let stale = temp.path().join(".source-stale");
        let unrelated = temp.path().join("published");
        fs::create_dir(&stale).expect("create stale staging directory");
        fs::create_dir(&unrelated).expect("create unrelated directory");

        cleanup_stale_staging_directories(temp.path(), ".source-", std::time::Duration::ZERO)
            .expect("bounded stale cleanup");

        assert!(!stale.exists());
        assert!(unrelated.exists());
    }

    #[cfg(unix)]
    #[test]
    fn expired_deadline_prevents_staging_allocation_and_cleanup() {
        let temp = tempfile::tempdir().expect("tempdir");
        let stale = temp.path().join(".source-stale");
        fs::create_dir(&stale).expect("create stale directory");
        let deadline = GoOperationDeadline::after(Duration::from_nanos(1));

        let error = StagingDirectory::create_until(temp.path(), ".source-", deadline)
            .expect_err("expired deadline must stop before staging mutation");

        assert!(matches!(error, GoSemanticProcessError::Timeout(_)));
        assert!(stale.exists());
    }

    #[cfg(unix)]
    #[test]
    fn stale_cleanup_leaves_oversized_tree_quarantined() {
        let temp = tempfile::tempdir().expect("tempdir");
        let stale = temp.path().join(".source-stale");
        fs::create_dir(&stale).expect("create stale directory");
        for index in 0..GO_FRONTEND_MAX_STALE_CLEANUP_ENTRIES {
            fs::write(stale.join(format!("entry-{index:04}")), b"x")
                .expect("seed oversized stale tree");
        }

        cleanup_stale_staging_directories(temp.path(), ".source-", Duration::ZERO)
            .expect("oversized cleanup remains controlled");

        assert!(stale.exists());
    }

    #[cfg(unix)]
    #[test]
    fn hostile_fifo_is_rejected_without_blocking() {
        let temp = tempfile::tempdir().expect("tempdir");
        let fifo = temp.path().join("frontend");
        let status = Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("run mkfifo");
        assert!(status.success());
        let (sender, receiver) = std::sync::mpsc::channel();
        let reader_path = fifo;
        let reader = std::thread::spawn(move || {
            let _ = sender.send(read_regular_file_no_follow(&reader_path));
        });

        let result = receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("FIFO rejection must not block");

        assert!(result.is_err());
        reader.join().expect("join FIFO reader");
    }
}

#[cfg(all(test, not(any(windows, target_os = "linux", target_os = "macos"))))]
mod unsupported_platform_tests {
    use super::*;

    #[test]
    fn bounded_commands_and_preparation_fail_before_process_or_cache_mutation() {
        assert!(!go_semantic_process_containment_supported());

        let temp = tempfile::tempdir().expect("temporary directory");
        let marker = temp.path().join("spawned");
        let helper = concat!(
            "go::semantic::process::unsupported_platform_tests::",
            "unsupported_platform_spawn_marker_helper"
        );
        let mut command = Command::new(std::env::current_exe().expect("current test binary"));
        command
            .env("POLINT_TEST_UNSUPPORTED_PLATFORM_MARKER", &marker)
            .arg("--exact")
            .arg(helper);

        let error = run_bounded_command(
            command,
            BoundedCommandLimits::new(Duration::from_secs(1), 1024, 1024),
            "unsupported-platform probe",
        )
        .expect_err("unsupported hosts must fail before spawn");
        assert!(matches!(
            error,
            GoSemanticProcessError::CommandUnavailable(_)
        ));
        assert!(!marker.exists(), "the rejected command must never execute");

        let cache = temp.path().join("cache");
        let error = PreparedGoSemanticFrontend::prepare_with_cache_root_for_analysis(
            &cache,
            None,
            &[],
            GoOperationDeadline::after(Duration::from_secs(1)),
        )
        .expect_err("unsupported hosts must fail before cache setup");
        assert!(matches!(
            error,
            GoSemanticProcessError::CommandUnavailable(_)
        ));
        assert!(
            !cache.exists(),
            "the rejected setup must not mutate its cache"
        );
    }

    #[test]
    fn unsupported_platform_spawn_marker_helper() {
        let Some(marker) = std::env::var_os("POLINT_TEST_UNSUPPORTED_PLATFORM_MARKER") else {
            return;
        };
        fs::write(marker, b"spawned").expect("write unsupported-platform marker");
    }
}
