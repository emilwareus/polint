use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use crate::go::lifecycle::GoAnalysisConfig;
use crate::go::semantic::process::{
    GoSemanticProcessError, command_for_frontend, resolve_go_semantic_frontend,
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
    ) -> Result<GoSemanticOutput, GoSemanticClientError> {
        let frontend = resolve_go_semantic_frontend()?;
        let mut command = command_for_frontend(&frontend, &self.root)?;
        append_request_args(&mut command, &self.root, config);
        let stdout = run_with_timeout(command, self.timeout, &self.root)?;
        decode_ndjson(&stdout).map_err(GoSemanticClientError::from)
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

    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to poll go semantic frontend: {error}"
            ))
        })? {
            let output = child.wait_with_output().map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to collect go semantic frontend output: {error}"
                ))
            })?;
            if !status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let reason = if stderr.is_empty() {
                    format!("go semantic frontend exited with status {status}.")
                } else {
                    format!("go semantic frontend exited with status {status}: {stderr}")
                };
                return Err(GoSemanticProcessError::CommandFailed(reason));
            }
            return Ok(output.stdout);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(GoSemanticProcessError::Timeout(
                "GoSidecarTimeout: go semantic frontend exceeded request timeout".to_string(),
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
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
}
