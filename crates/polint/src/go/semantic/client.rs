use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::go::lifecycle::GoAnalysisConfig;
use crate::go::semantic::diagnostics::GO_SIDECAR_TIMEOUT;
use crate::go::semantic::process::{
    GoSemanticProcessError, command_for_frontend, frontend_digest, resolve_go_semantic_frontend,
};
use crate::go::semantic::protocol::{GoSemanticOutput, GoSemanticProtocolError, decode_ndjson};

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
            timeout: Duration::from_secs(30),
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
        let frontend = resolve_go_semantic_frontend()?;
        let digest = frontend_digest(&frontend)?;
        let mut command = command_for_frontend(&frontend, &self.root)?;
        append_request_args(&mut command, &self.root, config);
        let stdout = run_with_timeout(command, self.timeout, &self.root)?;
        let output = decode_ndjson(&stdout).map_err(GoSemanticClientError::from)?;
        Ok(GoSemanticClientRun {
            output,
            frontend_digest: digest,
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
    mut command: std::process::Command,
    timeout: Duration,
    root: &Path,
) -> Result<Vec<u8>, GoSemanticProcessError> {
    let _ = root;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_process_group(&mut command);
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            GoSemanticProcessError::CommandUnavailable(
                "go semantic frontend executable was not found.".to_string(),
            )
        } else {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to start go semantic frontend: {error}"
            ))
        }
    })?;
    let mut stdout_reader = child
        .stdout
        .take()
        .map(|stdout| thread::spawn(move || read_all(stdout)));
    let mut stderr_reader = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || read_all(stderr)));

    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to poll go semantic frontend: {error}"
            ))
        })? {
            let stdout = join_reader(stdout_reader.take())?;
            let stderr = join_reader(stderr_reader.take())?;
            if !status.success() {
                let stderr_text = String::from_utf8_lossy(&stderr).trim().to_string();
                let reason = if stderr_text.is_empty() {
                    format!("go semantic frontend exited with status {status}.")
                } else {
                    format!("go semantic frontend exited with status {status}: {stderr_text}")
                };
                return Err(GoSemanticProcessError::CommandFailed(reason));
            }
            return Ok(stdout);
        }
        if Instant::now() >= deadline {
            terminate_child_process_tree(&mut child);
            let _ = child.wait();
            let _ = join_reader(stdout_reader.take());
            let _ = join_reader(stderr_reader.take());
            return Err(GoSemanticProcessError::Timeout(format!(
                "{GO_SIDECAR_TIMEOUT}: go semantic frontend exceeded request timeout"
            )));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn read_all(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn join_reader(
    reader: Option<thread::JoinHandle<std::io::Result<Vec<u8>>>>,
) -> Result<Vec<u8>, GoSemanticProcessError> {
    let Some(reader) = reader else {
        return Ok(Vec::new());
    };
    reader
        .join()
        .map_err(|_| {
            GoSemanticProcessError::CommandFailed(
                "failed to join go semantic frontend output reader".to_string(),
            )
        })?
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to read go semantic frontend output: {error}"
            ))
        })
}

#[cfg(unix)]
fn configure_child_process_group(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_child_process_group(_command: &mut std::process::Command) {}

#[cfg(unix)]
fn terminate_child_process_tree(child: &mut Child) {
    let process_group = format!("-{}", child.id());
    let _ = std::process::Command::new("kill")
        .args(["-KILL", "--", &process_group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
}

#[cfg(not(unix))]
fn terminate_child_process_tree(child: &mut Child) {
    let _ = child.kill();
}

#[cfg(test)]
mod tests {
    use super::*;

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
            "{\"schema\":\"polint-go-semantic-1\",\"kind\":\"session_begin\"}\n",
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
        if cfg!(windows) {
            let mut command = std::process::Command::new("cmd");
            command.arg("/C").arg(format!("echo {stdout}"));
            command
        } else {
            let mut command = std::process::Command::new("sh");
            command
                .arg("-c")
                .arg(format!("printf '%s' '{}'", stdout.replace('\'', "'\\''")));
            command
        }
    }

    #[cfg(unix)]
    fn shell_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\\''"))
    }

    #[cfg(unix)]
    fn assert_process_exits(pid: i32) {
        for _ in 0..20 {
            if !process_exists(pid) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        panic!("descendant process {pid} was still running after Go semantic timeout");
    }

    #[cfg(unix)]
    fn process_exists(pid: i32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}
