#![allow(
    dead_code,
    reason = "Phase 34 introduces the extension host before the later kernel orchestration plan consumes every path."
)]

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::discovery::DiscoveredExtension;
use super::manifest::{ExtensionActivationStatus, FactFamilyLabel};
use super::protocol::{
    ExtensionHandshakeRequest, ExtensionHandshakeResponse, ExtensionProtocolError,
    ExtensionProviderRunRequest, ExtensionProviderRunResponse, HANDSHAKE_SCHEMA,
    PROVIDER_RUN_SCHEMA,
};
use crate::diagnostics::{Diagnostic, extension_setup_diagnostic};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_STDOUT_LIMIT: usize = 1_048_576;
const DEFAULT_STDERR_LIMIT: usize = 16_384;

#[derive(Debug, Clone)]
pub(crate) struct ExtensionHost<R = StdCommandRunner> {
    repo_root: PathBuf,
    runner: R,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
}

impl ExtensionHost<StdCommandRunner> {
    pub(crate) fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self::with_runner(repo_root, StdCommandRunner)
    }
}

impl<R> ExtensionHost<R>
where
    R: ExtensionCommandRunner,
{
    pub(crate) fn with_runner(repo_root: impl Into<PathBuf>, runner: R) -> Self {
        Self {
            repo_root: repo_root.into(),
            runner,
            timeout: DEFAULT_TIMEOUT,
            stdout_limit: DEFAULT_STDOUT_LIMIT,
            stderr_limit: DEFAULT_STDERR_LIMIT,
        }
    }

    #[cfg(test)]
    fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub(crate) fn handshake(
        &self,
        extension: &DiscoveredExtension,
    ) -> ExtensionHostResult<ExtensionHandshakeResponse> {
        let request = ExtensionHandshakeRequest {
            schema_version: HANDSHAKE_SCHEMA.to_string(),
            extension_id: extension.extension_id.clone(),
            manifest_path: extension.manifest_path.clone(),
        };
        let spec = self.command_spec(
            &extension.manifest_path,
            vec!["handshake".to_string()],
            serde_json::to_vec(&request).expect("handshake request should serialize"),
        );
        let outcome = self.runner.run(spec);
        let response = self.parse_json_response::<ExtensionHandshakeResponse>(
            outcome,
            &extension.extension_id,
            None,
            ExtensionHostFailureKind::HandshakeFailed,
        )?;
        response.validate_schema().map_err(|error| {
            ExtensionHostError::from_protocol_error(&extension.extension_id, None, error)
        })?;
        Ok(response)
    }

    pub(crate) fn run_provider(
        &self,
        extension: &DiscoveredExtension,
        provider_id: &str,
        declared_inputs: Vec<FactFamilyLabel>,
        declared_outputs: Vec<FactFamilyLabel>,
        input_digest_labels: Vec<String>,
    ) -> ExtensionHostResult<ExtensionProviderRunResponse> {
        let request = ExtensionProviderRunRequest {
            schema_version: PROVIDER_RUN_SCHEMA.to_string(),
            extension_id: extension.extension_id.clone(),
            provider_id: provider_id.to_string(),
            declared_inputs,
            declared_outputs,
            input_digest_labels,
        };
        let spec = self.command_spec(
            &extension.manifest_path,
            vec!["run-provider".to_string(), provider_id.to_string()],
            serde_json::to_vec(&request).expect("provider request should serialize"),
        );
        let outcome = self.runner.run(spec);
        let response = self.parse_json_response::<ExtensionProviderRunResponse>(
            outcome,
            &extension.extension_id,
            Some(provider_id),
            ExtensionHostFailureKind::ProviderFailed,
        )?;
        response.validate_schema().map_err(|error| {
            ExtensionHostError::from_protocol_error(
                &extension.extension_id,
                Some(provider_id),
                error,
            )
        })?;
        Ok(response)
    }

    fn command_spec(
        &self,
        manifest_path: &str,
        extension_args: Vec<String>,
        stdin: Vec<u8>,
    ) -> ExtensionCommandSpec {
        let manifest_path = self.repo_root.join(manifest_path);
        let mut args = vec![
            "run".to_string(),
            "--manifest-path".to_string(),
            manifest_path.to_string_lossy().to_string(),
            "--".to_string(),
        ];
        args.extend(extension_args);
        let mut env = BTreeMap::new();
        env.insert(
            "CARGO_TARGET_DIR".to_string(),
            self.repo_root
                .join(".polint/cache/extensions-target")
                .to_string_lossy()
                .to_string(),
        );
        ExtensionCommandSpec {
            program: "cargo".to_string(),
            args,
            env,
            stdin,
            timeout: self.timeout,
            stdout_limit: self.stdout_limit,
            stderr_limit: self.stderr_limit,
        }
    }

    fn parse_json_response<T>(
        &self,
        outcome: ExtensionCommandOutcome,
        extension_id: &str,
        provider_id: Option<&str>,
        nonzero_kind: ExtensionHostFailureKind,
    ) -> ExtensionHostResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        if outcome.timed_out {
            return Err(ExtensionHostError::new(
                ExtensionHostFailureKind::Timeout,
                extension_id,
                provider_id,
                "extension command timed out",
            ));
        }
        if let Some(spawn_error) = outcome.spawn_error {
            return Err(ExtensionHostError::new(
                ExtensionHostFailureKind::BuildFailed,
                extension_id,
                provider_id,
                spawn_error,
            ));
        }
        if outcome.status != Some(0) {
            return Err(ExtensionHostError::new(
                classify_nonzero(nonzero_kind, &outcome.stderr),
                extension_id,
                provider_id,
                bounded_summary(&outcome.stderr, self.stderr_limit),
            ));
        }

        serde_json::from_slice(&outcome.stdout).map_err(|error| {
            ExtensionHostError::new(
                ExtensionHostFailureKind::MalformedResponse,
                extension_id,
                provider_id,
                error.to_string(),
            )
        })
    }
}

pub(crate) type ExtensionHostResult<T> = Result<T, ExtensionHostError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtensionHostError {
    pub(crate) kind: ExtensionHostFailureKind,
    pub(crate) extension_id: String,
    pub(crate) provider_id: Option<String>,
    pub(crate) summary: String,
}

impl ExtensionHostError {
    fn new(
        kind: ExtensionHostFailureKind,
        extension_id: impl Into<String>,
        provider_id: Option<&str>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            extension_id: extension_id.into(),
            provider_id: provider_id.map(str::to_string),
            summary: sanitize_summary(&summary.into()),
        }
    }

    fn from_protocol_error(
        extension_id: &str,
        provider_id: Option<&str>,
        error: ExtensionProtocolError,
    ) -> Self {
        match error {
            ExtensionProtocolError::UnsupportedProtocol { expected, actual } => Self::new(
                ExtensionHostFailureKind::UnsupportedProtocol,
                extension_id,
                provider_id,
                format!("unsupported protocol schema {actual}; expected {expected}"),
            ),
        }
    }

    pub(crate) fn activation_status(&self) -> ExtensionActivationStatus {
        match self.kind {
            ExtensionHostFailureKind::MalformedResponse
            | ExtensionHostFailureKind::UnsupportedProtocol => {
                ExtensionActivationStatus::ValidationFailed
            }
            ExtensionHostFailureKind::BuildFailed
            | ExtensionHostFailureKind::HandshakeFailed
            | ExtensionHostFailureKind::ProviderFailed
            | ExtensionHostFailureKind::Timeout => ExtensionActivationStatus::Failed,
        }
    }

    pub(crate) fn diagnostic(&self) -> Diagnostic {
        extension_setup_diagnostic(
            self.kind.as_str(),
            &self.extension_id,
            self.provider_id.as_deref(),
            &self.summary,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExtensionHostFailureKind {
    BuildFailed,
    HandshakeFailed,
    ProviderFailed,
    Timeout,
    MalformedResponse,
    UnsupportedProtocol,
}

impl ExtensionHostFailureKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::BuildFailed => "build_failed",
            Self::HandshakeFailed => "handshake_failed",
            Self::ProviderFailed => "provider_failed",
            Self::Timeout => "timeout",
            Self::MalformedResponse => "malformed_response",
            Self::UnsupportedProtocol => "unsupported_protocol",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtensionCommandSpec {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) env: BTreeMap<String, String>,
    pub(crate) stdin: Vec<u8>,
    pub(crate) timeout: Duration,
    pub(crate) stdout_limit: usize,
    pub(crate) stderr_limit: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtensionCommandOutcome {
    pub(crate) status: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) timed_out: bool,
    pub(crate) spawn_error: Option<String>,
}

pub(crate) trait ExtensionCommandRunner {
    fn run(&self, spec: ExtensionCommandSpec) -> ExtensionCommandOutcome;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StdCommandRunner;

impl ExtensionCommandRunner for StdCommandRunner {
    fn run(&self, spec: ExtensionCommandSpec) -> ExtensionCommandOutcome {
        run_std_command(&spec)
    }
}

fn run_std_command(spec: &ExtensionCommandSpec) -> ExtensionCommandOutcome {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .envs(&spec.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return ExtensionCommandOutcome {
                status: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                timed_out: false,
                spawn_error: Some(error.to_string()),
            };
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let input = spec.stdin.clone();
        thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
    }

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() >= spec.timeout => {
                let _ = child.kill();
                let output = child.wait_with_output().ok();
                return ExtensionCommandOutcome {
                    status: output.as_ref().and_then(|output| output.status.code()),
                    stdout: output
                        .as_ref()
                        .map(|output| truncate_bytes(&output.stdout, spec.stdout_limit))
                        .unwrap_or_default(),
                    stderr: output
                        .as_ref()
                        .map(|output| truncate_bytes(&output.stderr, spec.stderr_limit))
                        .unwrap_or_default(),
                    timed_out: true,
                    spawn_error: None,
                };
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                return ExtensionCommandOutcome {
                    status: None,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    timed_out: false,
                    spawn_error: Some(error.to_string()),
                };
            }
        }
    }

    match child.wait_with_output() {
        Ok(output) => ExtensionCommandOutcome {
            status: output.status.code(),
            stdout: truncate_bytes(&output.stdout, spec.stdout_limit),
            stderr: truncate_bytes(&output.stderr, spec.stderr_limit),
            timed_out: false,
            spawn_error: None,
        },
        Err(error) => ExtensionCommandOutcome {
            status: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            timed_out: false,
            spawn_error: Some(error.to_string()),
        },
    }
}

fn classify_nonzero(
    default_kind: ExtensionHostFailureKind,
    stderr: &[u8],
) -> ExtensionHostFailureKind {
    let stderr = String::from_utf8_lossy(stderr);
    if stderr.contains("could not compile") || stderr.contains("failed to compile") {
        ExtensionHostFailureKind::BuildFailed
    } else {
        default_kind
    }
}

fn bounded_summary(bytes: &[u8], limit: usize) -> String {
    sanitize_summary(&String::from_utf8_lossy(&truncate_bytes(bytes, limit)))
}

fn sanitize_summary(summary: &str) -> String {
    summary
        .replace('\\', "/")
        .lines()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_bytes(bytes: &[u8], limit: usize) -> Vec<u8> {
    bytes.iter().copied().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::extensions::manifest::ExtensionManifest;
    use std::cell::RefCell;
    use tempfile::TempDir;

    #[derive(Debug)]
    struct FakeRunner {
        outcomes: RefCell<Vec<ExtensionCommandOutcome>>,
        specs: RefCell<Vec<ExtensionCommandSpec>>,
    }

    impl FakeRunner {
        fn new(outcomes: Vec<ExtensionCommandOutcome>) -> Self {
            Self {
                outcomes: RefCell::new(outcomes),
                specs: RefCell::new(Vec::new()),
            }
        }
    }

    impl ExtensionCommandRunner for FakeRunner {
        fn run(&self, spec: ExtensionCommandSpec) -> ExtensionCommandOutcome {
            self.specs.borrow_mut().push(spec);
            self.outcomes.borrow_mut().remove(0)
        }
    }

    fn extension() -> DiscoveredExtension {
        DiscoveredExtension {
            extension_id: "demo".to_string(),
            manifest_path: ".polint/extensions/demo/Cargo.toml".to_string(),
            activation_status: ExtensionActivationStatus::Discovered,
            manifest: ExtensionManifest::repo_local("demo").unwrap(),
            source_digest: crate::analysis_kernel::incremental::Digest::absent(
                crate::analysis_kernel::incremental::DigestKind::ExtensionCode,
                "source",
            ),
            dependency_digest: crate::analysis_kernel::incremental::Digest::absent(
                crate::analysis_kernel::incremental::DigestKind::ExtensionCode,
                "dependency",
            ),
            digest_input_paths: Vec::new(),
        }
    }

    fn outcome(status: i32, stdout: &str, stderr: &str) -> ExtensionCommandOutcome {
        ExtensionCommandOutcome {
            status: Some(status),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
            timed_out: false,
            spawn_error: None,
        }
    }

    #[test]
    fn successful_handshake_uses_cargo_run_without_shell_string() {
        let temp = TempDir::new().expect("create temp repo");
        let runner = FakeRunner::new(vec![outcome(
            0,
            r#"{"schema_version":"polint-extension-handshake-v1","extension_id":"demo","activation_status":"handshake_ok","providers":[],"diagnostics":[]}"#,
            "",
        )]);
        let host = ExtensionHost::with_runner(temp.path(), runner);

        let response = host.handshake(&extension()).unwrap();

        assert_eq!(
            response.activation_status,
            ExtensionActivationStatus::HandshakeOk
        );
        let spec = host.runner.specs.borrow();
        assert_eq!(spec[0].program, "cargo");
        assert!(spec[0].args.contains(&"run".to_string()));
        assert!(spec[0].args.contains(&"--manifest-path".to_string()));
        assert!(spec[0].args.contains(&"handshake".to_string()));
        assert!(spec[0].env.contains_key("CARGO_TARGET_DIR"));
    }

    #[test]
    fn nonzero_exit_is_classified_without_emitted_facts() {
        let temp = TempDir::new().expect("create temp repo");
        let runner = FakeRunner::new(vec![outcome(1, "", "thread panicked at absolute/path")]);
        let host = ExtensionHost::with_runner(temp.path(), runner);

        let error = host.handshake(&extension()).unwrap_err();

        assert_eq!(error.kind, ExtensionHostFailureKind::HandshakeFailed);
        assert_eq!(error.activation_status(), ExtensionActivationStatus::Failed);
        assert_eq!(error.diagnostic().rule_id, "polint/extension");
    }

    #[test]
    fn invalid_json_is_malformed_response() {
        let temp = TempDir::new().expect("create temp repo");
        let runner = FakeRunner::new(vec![outcome(0, "not-json", "")]);
        let host = ExtensionHost::with_runner(temp.path(), runner);

        let error = host.handshake(&extension()).unwrap_err();

        assert_eq!(error.kind, ExtensionHostFailureKind::MalformedResponse);
        assert_eq!(
            error.activation_status(),
            ExtensionActivationStatus::ValidationFailed
        );
    }

    #[test]
    fn timeout_is_classified() {
        let temp = TempDir::new().expect("create temp repo");
        let runner = FakeRunner::new(vec![ExtensionCommandOutcome {
            status: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            timed_out: true,
            spawn_error: None,
        }]);
        let host =
            ExtensionHost::with_runner(temp.path(), runner).with_timeout(Duration::from_millis(1));

        let error = host.handshake(&extension()).unwrap_err();

        assert_eq!(error.kind, ExtensionHostFailureKind::Timeout);
    }
}
