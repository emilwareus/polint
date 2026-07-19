use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::go::lifecycle::GoAnalysisConfig;
use crate::go::semantic::diagnostics::GO_SIDECAR_TIMEOUT;
use crate::go::semantic::process::{
    BoundedCommandLimits, GO_OPERATION_TIMEOUT, GoOperationDeadline, GoSemanticProcessError,
    PreparedGoSemanticFrontend, go_command_working_directory, run_bounded_command,
};
#[cfg(all(test, any(windows, target_os = "linux", target_os = "macos")))]
use crate::go::semantic::protocol::decode_ndjson;
use crate::go::semantic::protocol::{
    GO_SEMANTIC_MAX_OUTPUT_BYTES, GoSemanticOutput, GoSemanticProtocolError, decode_ndjson_until,
};

const GO_SEMANTIC_MAX_STDERR_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub(crate) enum GoSemanticClientError {
    Process(GoSemanticProcessError),
    Protocol(GoSemanticProtocolError),
}

impl std::fmt::Display for GoSemanticClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Process(error) => write!(f, "{error}"),
            Self::Protocol(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for GoSemanticClientError {}

impl From<GoSemanticProcessError> for GoSemanticClientError {
    fn from(error: GoSemanticProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<GoSemanticProtocolError> for GoSemanticClientError {
    fn from(error: GoSemanticProtocolError) -> Self {
        Self::Protocol(error)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GoSemanticClient {
    root: PathBuf,
    timeout: Duration,
}

#[derive(Debug)]
pub(crate) struct GoSemanticClientRun {
    pub(crate) output: GoSemanticOutput,
    pub(crate) frontend_digest: String,
}

impl GoSemanticClient {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            timeout: GO_OPERATION_TIMEOUT,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_timeout(root: PathBuf, timeout: Duration) -> Self {
        Self { root, timeout }
    }

    pub(crate) fn run(
        &self,
        config: &GoAnalysisConfig,
    ) -> Result<GoSemanticClientRun, GoSemanticClientError> {
        let frontend = PreparedGoSemanticFrontend::prepare_for_analysis_until(
            &self.root,
            config,
            &[],
            GoOperationDeadline::after(self.timeout),
        )?;
        self.run_prepared(config, &frontend)
    }

    pub(crate) fn run_prepared(
        &self,
        config: &GoAnalysisConfig,
        frontend: &PreparedGoSemanticFrontend,
    ) -> Result<GoSemanticClientRun, GoSemanticClientError> {
        let certified_root = frontend
            .certified_analysis_root()
            .unwrap_or(self.root.as_path());
        let command_root = go_command_working_directory(certified_root)?;
        let root = command_root.as_path();
        let mut command = frontend.command(root)?;
        append_request_args(&mut command, root, config);
        let (stdout, deadline) = run_prepared_with_timeout(command, self.timeout, frontend)?;
        let output = match decode_ndjson_until(&stdout, deadline) {
            Ok(output) => output,
            Err(GoSemanticProtocolError::DeadlineExceeded) => {
                return Err(GoSemanticClientError::Process(
                    GoSemanticProcessError::Timeout(format!(
                        "{GO_SIDECAR_TIMEOUT}: Go semantic protocol decoding exceeded its operation deadline"
                    )),
                ));
            }
            Err(error) => return Err(GoSemanticClientError::Protocol(error)),
        };
        Ok(GoSemanticClientRun {
            output,
            frontend_digest: frontend.identity_digest(),
        })
    }
}

fn append_request_args(
    command: &mut std::process::Command,
    root: &Path,
    config: &GoAnalysisConfig,
) {
    command
        .arg("semantic")
        .arg("--root")
        .arg(root.as_os_str())
        .arg("--module-roots")
        .arg(config.module_roots.join(","))
        .arg("--patterns")
        .arg(config.package_patterns.join(","))
        .arg("--tests")
        .arg(config.include_tests.to_string())
        .arg("--build-tags")
        .arg(config.build_tags.join(","))
        .arg("--ndjson");
    if config.offline {
        command.env("GONOSUMDB", "*").env("GOPROXY", "off");
    }
}

fn run_with_timeout(
    command: std::process::Command,
    timeout: Duration,
    root: &Path,
) -> Result<Vec<u8>, GoSemanticProcessError> {
    let _ = root;
    let output = run_bounded_command(
        command,
        semantic_command_limits(timeout),
        &format!("{GO_SIDECAR_TIMEOUT}: go semantic frontend"),
    )?;
    successful_stdout(output)
}

fn run_prepared_with_timeout(
    command: std::process::Command,
    timeout: Duration,
    frontend: &PreparedGoSemanticFrontend,
) -> Result<(Vec<u8>, Instant), GoSemanticProcessError> {
    let (output, deadline) = frontend.run_command_with_deadline(
        command,
        semantic_command_limits(timeout),
        &format!("{GO_SIDECAR_TIMEOUT}: go semantic frontend"),
    )?;
    successful_stdout(output).map(|stdout| (stdout, deadline))
}

const fn semantic_command_limits(timeout: Duration) -> BoundedCommandLimits {
    BoundedCommandLimits::new(
        timeout,
        GO_SEMANTIC_MAX_OUTPUT_BYTES,
        GO_SEMANTIC_MAX_STDERR_BYTES,
    )
}

fn successful_stdout(
    output: crate::go::semantic::process::BoundedCommandOutput,
) -> Result<Vec<u8>, GoSemanticProcessError> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let reason = if stderr.is_empty() {
            format!("go semantic frontend exited with status {}.", output.status)
        } else {
            format!(
                "go semantic frontend exited with status {}: {stderr}",
                output.status
            )
        };
        return Err(GoSemanticProcessError::CommandFailed(reason));
    }
    Ok(output.stdout)
}

#[cfg(all(test, any(windows, target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    static FAKE_STDOUT_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn default_client_uses_the_bounded_operation_timeout() {
        let client = GoSemanticClient::new(PathBuf::from("fixture"));

        assert_eq!(client.timeout, GO_OPERATION_TIMEOUT);
    }

    #[test]
    fn explicit_client_timeout_is_preserved() {
        let timeout = Duration::from_millis(25);
        let client = GoSemanticClient::with_timeout(PathBuf::from("fixture"), timeout);

        assert_eq!(client.timeout, timeout);
    }

    #[test]
    fn timeout_error_uses_go_sidecar_timeout_category() {
        let command = if cfg!(windows) {
            let mut command = std::process::Command::new("cmd");
            command.args(["/C", "ping -n 3 127.0.0.1 > nul"]);
            command
        } else {
            let mut command = std::process::Command::new("sh");
            command.args(["-c", "sleep 2"]);
            command
        };
        let err = run_with_timeout(command, Duration::from_millis(1), Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("GoSidecarTimeout"));
    }

    #[test]
    fn command_failure_captures_stderr() {
        let command = if cfg!(windows) {
            let mut command = std::process::Command::new("cmd");
            command.args(["/C", "echo semantic boom 1>&2 && exit /B 7"]);
            command
        } else {
            let mut command = std::process::Command::new("sh");
            command.args(["-c", "echo semantic boom >&2; exit 7"]);
            command
        };
        let err = run_with_timeout(command, Duration::from_secs(5), Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("semantic boom"));
    }

    #[cfg(unix)]
    #[test]
    fn command_failure_redacts_proxy_credentials_from_stderr() {
        let mut command = std::process::Command::new("sh");
        command
            .env(
                "HTTPS_PROXY",
                "https://proxy-user:super-secret@proxy.example.test:8443",
            )
            .arg("-c")
            .arg("printf '%s' \"$HTTPS_PROXY\" >&2; exit 7");

        let error = run_with_timeout(command, Duration::from_secs(5), Path::new("."))
            .expect_err("fake frontend must fail");
        let message = error.to_string();

        assert!(!message.contains("proxy-user"));
        assert!(!message.contains("super-secret"));
        assert!(message.contains("subprocess stderr may contain configured credentials"));
    }

    #[test]
    fn schema_mismatch_from_fake_sidecar_is_typed_protocol_error() {
        let command = fake_stdout_command("{\"schema\":\"wrong\",\"kind\":\"session_begin\"}\n");
        let stdout = run_with_timeout(command, Duration::from_secs(5), Path::new("."))
            .expect("fake sidecar exits");
        let err = decode_ndjson(&stdout).unwrap_err();
        assert!(matches!(err, GoSemanticProtocolError::UnsupportedSchema(_)));
    }

    #[test]
    fn missing_terminator_from_fake_sidecar_is_typed_protocol_error() {
        let command = fake_stdout_command(
            "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_begin\"}\n",
        );
        let stdout = run_with_timeout(command, Duration::from_secs(5), Path::new("."))
            .expect("fake sidecar exits");
        let err = decode_ndjson(&stdout).unwrap_err();
        assert_eq!(err, GoSemanticProtocolError::MissingEnd);
    }

    #[test]
    fn no_surviving_go_frontend_process_after_timeout() {
        let command = if cfg!(windows) {
            let mut command = std::process::Command::new("cmd");
            command.args(["/C", "ping -n 10 127.0.0.1 > nul"]);
            command
        } else {
            let mut command = std::process::Command::new("sleep");
            command.arg("10");
            command
        };
        let started = Instant::now();
        let err = run_with_timeout(command, Duration::from_millis(1), Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("GoSidecarTimeout"));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "timeout cleanup should not wait for the fake frontend to finish naturally"
        );
    }

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_descendant_processes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pid_file = temp.path().join("sleep.pid");
        let mut command = std::process::Command::new("sh");
        command.arg("-c").arg(format!(
            "sleep 10 & echo $! > {}; wait",
            shell_quote(pid_file.to_string_lossy().as_ref())
        ));

        let err = run_with_timeout(command, Duration::from_millis(25), Path::new(".")).unwrap_err();

        assert!(err.to_string().contains("GoSidecarTimeout"));
        if let Ok(pid) = std::fs::read_to_string(pid_file)
            && let Ok(pid) = pid.trim().parse::<i32>()
        {
            assert_process_exits(pid);
        }
    }

    #[cfg(unix)]
    #[test]
    fn large_stdout_is_drained_while_child_runs() {
        let mut command = std::process::Command::new("sh");
        command.arg("-c").arg(
            "i=0; while [ $i -lt 5000 ]; do printf 0123456789abcdef0123456789abcdef; i=$((i + 1)); done",
        );

        let stdout = run_with_timeout(command, Duration::from_secs(2), Path::new("."))
            .expect("large stdout exits");

        assert_eq!(stdout.len(), 160_000);
    }

    fn fake_stdout_command(stdout: &str) -> std::process::Command {
        let path = std::env::temp_dir().join(format!(
            "polint-fake-sidecar-{}-{}.ndjson",
            std::process::id(),
            FAKE_STDOUT_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, stdout).expect("write fake sidecar stdout fixture");
        if cfg!(windows) {
            let mut command = std::process::Command::new("cmd");
            command.arg("/C").arg("type").arg(path);
            command
        } else {
            let mut command = std::process::Command::new("cat");
            command.arg(path);
            command
        }
    }

    #[cfg(unix)]
    fn shell_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\\''"))
    }

    #[cfg(unix)]
    #[allow(unsafe_code)]
    fn assert_process_exits(pid: i32) {
        for _ in 0..20 {
            if !process_exists(pid) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        panic!("descendant process {pid} was still running after Go semantic timeout");
    }

    #[cfg(unix)]
    #[allow(unsafe_code)]
    fn process_exists(pid: i32) -> bool {
        let result = unsafe { libc::kill(pid, 0) };
        result == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
}
