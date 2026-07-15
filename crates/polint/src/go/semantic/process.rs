use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::go::semantic::protocol::GO_SEMANTIC_SCHEMA;

pub(crate) const GO_SEMANTIC_FRONTEND_ENV: &str = "POLINT_GO_FRONTEND";
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

#[derive(Debug, Clone)]
pub(crate) enum GoSemanticCommand {
    Binary(PathBuf),
    SourceDir(PathBuf),
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedGoSemanticFrontend {
    executable: PathBuf,
    executable_digest: String,
    source_digest: Option<String>,
    toolchain_version: Option<String>,
}

impl PreparedGoSemanticFrontend {
    pub(crate) fn prepare() -> Result<Self, GoSemanticProcessError> {
        match resolve_go_semantic_frontend()? {
            GoSemanticCommand::Binary(executable) => {
                let executable_digest =
                    frontend_digest(&GoSemanticCommand::Binary(executable.clone()))?;
                Ok(Self {
                    executable,
                    executable_digest,
                    source_digest: None,
                    toolchain_version: None,
                })
            }
            GoSemanticCommand::SourceDir(source_dir) => {
                let source_digest =
                    frontend_digest(&GoSemanticCommand::SourceDir(source_dir.clone()))?;
                let toolchain_version = local_go_toolchain_version()?;
                let executable =
                    ensure_frontend_binary_with_version(&source_dir, &toolchain_version)?;
                let executable_digest =
                    frontend_digest(&GoSemanticCommand::Binary(executable.clone()))?;
                Ok(Self {
                    executable,
                    executable_digest,
                    source_digest: Some(source_digest),
                    toolchain_version: Some(toolchain_version),
                })
            }
        }
    }

    pub(crate) fn command(&self, root: &Path) -> Command {
        let mut command = Command::new(&self.executable);
        command.current_dir(root);
        command
    }

    pub(crate) fn identity_parts(&self) -> Vec<String> {
        vec![
            format!("executable_digest={}", self.executable_digest),
            format!(
                "source_digest={}",
                self.source_digest.as_deref().unwrap_or("prebuilt")
            ),
            format!(
                "toolchain_version={}",
                self.toolchain_version.as_deref().unwrap_or("embedded")
            ),
        ]
    }

    pub(crate) fn identity_digest(&self) -> String {
        let parts = self.identity_parts();
        let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        crate::cache::stable_hash(&refs)
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        executable_digest: &str,
        source_digest: Option<&str>,
        toolchain_version: Option<&str>,
    ) -> Self {
        Self {
            executable: PathBuf::from("polint-go-frontend-test"),
            executable_digest: executable_digest.to_string(),
            source_digest: source_digest.map(str::to_string),
            toolchain_version: toolchain_version.map(str::to_string),
        }
    }
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

    pub(crate) fn prepare() -> Self {
        match PreparedGoSemanticFrontend::prepare() {
            Ok(frontend) => Self::Ready(frontend),
            Err(error) => Self::SetupMissing {
                reason: error.stable_reason().to_string(),
                process_error: Some(error),
            },
        }
    }
}

pub(crate) fn resolve_go_semantic_frontend() -> Result<GoSemanticCommand, GoSemanticProcessError> {
    if let Ok(path) = std::env::var(GO_SEMANTIC_FRONTEND_ENV)
        && !path.trim().is_empty()
    {
        return command_for_path(PathBuf::from(path));
    }
    if let Some(path) = installed_frontend_binary()? {
        return Ok(GoSemanticCommand::Binary(path));
    }
    materialize_embedded_frontend().map(GoSemanticCommand::SourceDir)
}

pub(crate) fn command_for_path(path: PathBuf) -> Result<GoSemanticCommand, GoSemanticProcessError> {
    if path.is_file() {
        return Ok(GoSemanticCommand::Binary(path));
    }
    if path.join("go.mod").is_file() {
        return Ok(GoSemanticCommand::SourceDir(path));
    }
    Err(GoSemanticProcessError::CommandFailed(format!(
        "{GO_SEMANTIC_FRONTEND_ENV} must point to a polint-go-frontend binary or source directory."
    )))
}

fn installed_frontend_binary() -> Result<Option<PathBuf>, GoSemanticProcessError> {
    let executable = std::env::current_exe().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to resolve current executable: {error}"
        ))
    })?;
    let Some(directory) = executable.parent() else {
        return Ok(None);
    };
    let candidate = directory.join(frontend_binary_name());
    Ok(candidate.is_file().then_some(candidate))
}

fn frontend_binary_name() -> &'static str {
    if cfg!(windows) {
        "polint-go-frontend.exe"
    } else {
        "polint-go-frontend"
    }
}

fn materialize_embedded_frontend() -> Result<PathBuf, GoSemanticProcessError> {
    let hash = embedded_frontend_hash();
    let parent = std::env::temp_dir()
        .join("polint-go-frontend")
        .join(env!("CARGO_PKG_VERSION"));
    let directory = parent.join(&hash);
    let marker = directory.join(".complete");
    if marker.is_file() {
        return Ok(directory);
    }
    if directory.exists() && embedded_frontend_files_match(&directory) {
        fs::write(&marker, "").map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to mark embedded Go semantic frontend `{}` complete: {error}",
                directory.display()
            ))
        })?;
        return Ok(directory);
    }
    if directory.exists() {
        fs::remove_dir_all(&directory).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to replace incomplete embedded Go semantic frontend `{}`: {error}",
                directory.display()
            ))
        })?;
    }
    fs::create_dir_all(&parent).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to create embedded Go semantic frontend cache `{}`: {error}",
            parent.display()
        ))
    })?;
    let staging = parent.join(format!(
        ".{hash}-{}-{}",
        std::process::id(),
        unique_materialization_suffix()
    ));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to clear stale embedded Go semantic frontend staging directory `{}`: {error}",
                staging.display()
            ))
        })?;
    }
    write_embedded_frontend_files(&staging)?;
    fs::write(staging.join(".complete"), "").map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to write embedded Go semantic frontend completion marker `{}`: {error}",
            staging.display()
        ))
    })?;
    match fs::rename(&staging, &directory) {
        Ok(()) => Ok(directory),
        Err(_) if marker.is_file() => {
            let _ = fs::remove_dir_all(&staging);
            Ok(directory)
        }
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to publish embedded Go semantic frontend directory `{}`: {error}",
            directory.display()
        ))),
    }
}

fn embedded_frontend_files_match(directory: &Path) -> bool {
    EMBEDDED_GO_FRONTEND_FILES
        .iter()
        .all(|(relative_path, contents)| {
            fs::read_to_string(directory.join(relative_path))
                .as_deref()
                .is_ok_and(|existing| existing == *contents)
        })
}

fn write_embedded_frontend_files(directory: &Path) -> Result<(), GoSemanticProcessError> {
    for (relative_path, contents) in EMBEDDED_GO_FRONTEND_FILES {
        let path = directory.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to create embedded Go semantic frontend directory `{}`: {error}",
                    parent.display()
                ))
            })?;
        }
        fs::write(&path, contents).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to write embedded Go semantic frontend file `{}`: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn unique_materialization_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

pub(crate) fn embedded_frontend_hash() -> String {
    let mut parts = Vec::new();
    parts.push(GO_SEMANTIC_SCHEMA.to_string());
    for (relative_path, contents) in EMBEDDED_GO_FRONTEND_FILES {
        parts.push(format!(
            "{relative_path}:{}",
            crate::cache::stable_hash(&[*contents])
        ));
    }
    let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::cache::stable_hash(&part_refs)
}

pub(crate) fn frontend_digest(
    frontend: &GoSemanticCommand,
) -> Result<String, GoSemanticProcessError> {
    match frontend {
        GoSemanticCommand::Binary(path) => {
            let bytes = fs::read(path).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to read Go semantic frontend binary `{}` for digest: {error}",
                    path.display()
                ))
            })?;
            Ok(stable_bytes_hash(&bytes))
        }
        GoSemanticCommand::SourceDir(path) => source_dir_digest(path),
    }
}

fn source_dir_digest(path: &Path) -> Result<String, GoSemanticProcessError> {
    let mut files = Vec::new();
    collect_frontend_source_files(path, path, &mut files)?;
    files.sort();
    let mut parts = vec![GO_SEMANTIC_SCHEMA.to_string()];
    for relative_path in files {
        let full_path = path.join(&relative_path);
        let bytes = fs::read(&full_path).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to read Go semantic frontend source `{}` for digest: {error}",
                full_path.display()
            ))
        })?;
        parts.push(format!(
            "{}:{}",
            relative_path.to_string_lossy().replace('\\', "/"),
            stable_bytes_hash(&bytes)
        ));
    }
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Ok(crate::cache::stable_hash(&refs))
}

fn collect_frontend_source_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), GoSemanticProcessError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to read Go semantic frontend source directory `{}` for digest: {error}",
            directory.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to read Go semantic frontend source directory entry `{}` for digest: {error}",
                directory.display()
            ))
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to read Go semantic frontend source file type `{}` for digest: {error}",
                path.display()
            ))
        })?;
        if file_type.is_dir() && !skip_frontend_digest_dir(&path) {
            collect_frontend_source_files(root, &path, files)?;
        } else if file_type.is_file() && is_frontend_digest_source(&path) {
            let relative_path = path.strip_prefix(root).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to relativize Go semantic frontend source `{}` for digest: {error}",
                    path.display()
                ))
            })?;
            files.push(relative_path.to_path_buf());
        }
    }
    Ok(())
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

fn stable_bytes_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub(crate) fn command_for_frontend(
    frontend: &GoSemanticCommand,
    root: &Path,
) -> Result<Command, GoSemanticProcessError> {
    match frontend {
        GoSemanticCommand::Binary(path) => {
            let mut command = Command::new(path);
            command.current_dir(root);
            Ok(command)
        }
        GoSemanticCommand::SourceDir(path) => {
            // Build the frontend binary once (cached next to the materialized
            // source) and run that, rather than `go run .` on every request. The
            // cold compile of x/tools can approach the per-request timeout, so
            // building here — outside the timed analysis request — keeps the
            // timeout covering only analysis and avoids flaky CI timeouts.
            let binary = ensure_frontend_binary(path)?;
            let mut command = Command::new(binary);
            command.current_dir(root);
            Ok(command)
        }
    }
}

/// Build the embedded frontend into a cached `polint-go-frontend` binary next to
/// its materialized source, returning the binary path. Concurrent builds are
/// made safe by compiling to a unique staging path and atomically renaming.
fn ensure_frontend_binary(source_dir: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    let version = local_go_toolchain_version()?;
    ensure_frontend_binary_with_version(source_dir, &version)
}

fn ensure_frontend_binary_with_version(
    source_dir: &Path,
    toolchain_version: &str,
) -> Result<PathBuf, GoSemanticProcessError> {
    let binary = source_dir.join(frontend_binary_name());
    if binary.is_file() {
        return Ok(binary);
    }
    ensure_go_toolchain_supported(toolchain_version)?;
    let staging = source_dir.join(format!(
        ".build-{}-{}",
        std::process::id(),
        unique_materialization_suffix()
    ));
    let output = Command::new("go")
        .arg("build")
        .arg("-o")
        .arg(&staging)
        .arg(".")
        .current_dir(source_dir)
        .env("GOWORK", "off")
        .env("GOTOOLCHAIN", "local")
        .output()
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to start go build of embedded frontend: {error}"
            ))
        })?;
    if !output.status.success() {
        let _ = fs::remove_file(&staging);
        // If another process already published the binary, use it.
        if binary.is_file() {
            return Ok(binary);
        }
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "go build of embedded frontend failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    match fs::rename(&staging, &binary) {
        Ok(()) => Ok(binary),
        Err(_) if binary.is_file() => {
            let _ = fs::remove_file(&staging);
            Ok(binary)
        }
        Err(error) => {
            let _ = fs::remove_file(&staging);
            Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to publish embedded frontend binary `{}`: {error}",
                binary.display()
            )))
        }
    }
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
    let output = Command::new("go")
        .arg("version")
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                GoSemanticProcessError::CommandUnavailable(
                    "go executable was not found for the Go semantic frontend.".to_string(),
                )
            } else {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to run `go version` for Go semantic frontend: {error}"
                ))
            }
        })?;
    if !output.status.success() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "`go version` exited with status {}",
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_go_version(stdout.as_ref()).ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to parse Go toolchain version from `{}`",
            stdout.trim()
        ))
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_for_path_accepts_source_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join("go.mod"), "module test\n").expect("write go.mod");
        let command = command_for_path(temp.path().to_path_buf()).expect("source dir accepted");
        assert!(matches!(command, GoSemanticCommand::SourceDir(_)));
    }

    #[test]
    fn embedded_frontend_hash_includes_schema() {
        assert!(!embedded_frontend_hash().is_empty());
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

    #[test]
    fn go_toolchain_version_parser_accepts_go_version_output() {
        assert_eq!(
            parse_go_version("go version go1.26.2 darwin/arm64"),
            Some("go1.26.2".to_string())
        );
        assert!(go_version_at_least("go1.25.0", 1, 25));
        assert!(!go_version_at_least("go1.24.9", 1, 25));
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
}
