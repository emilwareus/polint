//! Cargo and `rustc` shims that count the compiler work a measured run does.
//!
//! `polint` resolves its Cargo program from `POLINT_CARGO`, so the harness points
//! that at the `polint-bench` binary. In shim mode the binary records one
//! invocation and then runs the real program with the argument vector it was
//! handed, so the measured command line is unchanged. The Cargo shim also
//! installs itself as `RUSTC_WRAPPER`: one non-probe `rustc` invocation is one
//! compiled unit, observed rather than inferred from the dependency graph.
//!
//! Records are one small JSON file per invocation rather than appended lines,
//! because Cargo runs `rustc` concurrently and create-new files need no locking.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

/// Which program the shim stands in for. Set by the harness on the `polint`
/// process, then re-pointed at `rustc` by the Cargo shim itself.
pub const SHIM_ROLE_ENV: &str = "POLINT_BENCH_SHIM_ROLE";
/// Directory the shim writes one record file into.
pub const SHIM_LOG_DIR_ENV: &str = "POLINT_BENCH_SHIM_LOG_DIR";
/// Absolute path of the real Cargo the Cargo shim runs.
pub const REAL_CARGO_ENV: &str = "POLINT_BENCH_REAL_CARGO";

/// Which program a shim invocation stands in for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShimRole {
    /// Started by `polint` through `POLINT_CARGO`.
    Cargo,
    /// Started by Cargo through `RUSTC_WRAPPER`; argument 0 is the real `rustc`.
    Rustc,
}

impl ShimRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Rustc => "rustc",
        }
    }
}

/// One observed child process.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationRecord {
    pub role: ShimRole,
    pub elapsed_ms: u64,
    /// Child exit code, or -1 when a signal terminated it.
    pub exit_code: i32,
    /// `rustc` only: Cargo asked the compiler a question instead of compiling.
    pub probe: bool,
}

/// Aggregate of the records one measured command produced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InvocationTotals {
    pub cargo_invocations: u64,
    pub cargo_failed_invocations: u64,
    pub cargo_wall_clock_ms: u64,
    pub rustc_invocations: u64,
    pub compiled_units: u64,
}

impl InvocationTotals {
    pub fn from_records(records: &[InvocationRecord]) -> Self {
        let mut totals = Self::default();
        for record in records {
            match record.role {
                ShimRole::Cargo => {
                    totals.cargo_invocations += 1;
                    totals.cargo_wall_clock_ms += record.elapsed_ms;
                    totals.cargo_failed_invocations += u64::from(record.exit_code != 0);
                }
                ShimRole::Rustc => {
                    totals.rustc_invocations += 1;
                    totals.compiled_units += u64::from(!record.probe);
                }
            }
        }
        totals
    }
}

/// Shim configuration, or `None` for a normal `polint-bench` run. Both variables
/// are set together by the harness, so running `polint-bench` by hand never
/// enters shim mode.
pub fn role_from_env() -> Option<(ShimRole, PathBuf)> {
    let role = match std::env::var(SHIM_ROLE_ENV).ok()?.as_str() {
        "cargo" => ShimRole::Cargo,
        "rustc" => ShimRole::Rustc,
        _ => return None,
    };
    Some((role, PathBuf::from(std::env::var_os(SHIM_LOG_DIR_ENV)?)))
}

/// Run the real program behind `role`, record the invocation, and return its
/// exit code so the caller can propagate it unchanged.
pub fn run(role: ShimRole, log_dir: &Path, args: &[OsString]) -> Result<i32> {
    let (program, child_args) = match role {
        ShimRole::Cargo => {
            let program = std::env::var_os(REAL_CARGO_ENV)
                .ok_or_else(|| anyhow!("{REAL_CARGO_ENV} must name the real cargo"))?;
            (PathBuf::from(program), args)
        }
        ShimRole::Rustc => {
            let (program, rest) = args
                .split_first()
                .ok_or_else(|| anyhow!("rustc shim expects the real rustc first"))?;
            (PathBuf::from(program), rest)
        }
    };

    let mut command = Command::new(&program);
    command.args(child_args);
    if role == ShimRole::Cargo {
        // Only the Cargo shim installs the wrapper, re-pointing the role so the
        // same binary behaves as the rustc shim one level down.
        command.env(
            "RUSTC_WRAPPER",
            std::env::current_exe().context("resolve polint-bench path")?,
        );
        command.env(SHIM_ROLE_ENV, ShimRole::Rustc.as_str());
    }

    let start = Instant::now();
    let status = command
        .status()
        .with_context(|| format!("failed to run {}", program.display()))?;
    let exit_code = status.code().unwrap_or(-1);
    write_record(
        log_dir,
        &InvocationRecord {
            role,
            elapsed_ms: start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            exit_code,
            probe: role == ShimRole::Rustc && is_probe(child_args),
        },
    )?;
    Ok(exit_code)
}

/// Whether a `rustc` argument vector answers a question Cargo asked rather than
/// compiling a unit. Cargo probes target and feature support with `--print`/`-vV`
/// and with no `--crate-name`; every real unit carries a `--crate-name`.
pub fn is_probe(args: &[OsString]) -> bool {
    let mut named = false;
    let mut queried = false;
    for arg in args {
        named |= arg == OsStr::new("--crate-name");
        queried |= arg == OsStr::new("-vV") || arg == OsStr::new("--version");
        queried |= arg
            .to_str()
            .is_some_and(|text| text == "--print" || text.starts_with("--print="));
    }
    queried || !named
}

/// Publish one record atomically: fill a `.part` file, then rename it into the
/// `.json` name the reader looks for.
///
/// A record has to be complete or absent. Writing in place leaves a truncated
/// file behind when the disk fills mid-write, and the reader cannot tell that
/// from a record it should count, so a full disk would silently corrupt a
/// measurement instead of failing it.
fn write_record(dir: &Path, record: &InvocationRecord) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let json = serde_json::to_string(record)?;
    // `create_new` plus a suffix keeps concurrent rustc shims from clobbering
    // each other's record.
    for attempt in 0..64u32 {
        let name = format!(
            "{}-{}-{stamp}-{attempt}",
            record.role.as_str(),
            std::process::id()
        );
        let staged = dir.join(format!("{name}.part"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
        {
            Ok(mut file) => {
                use std::io::Write as _;
                file.write_all(json.as_bytes())
                    .with_context(|| format!("failed to write {}", staged.display()))?;
                let path = dir.join(format!("{name}.json"));
                return fs::rename(&staged, &path)
                    .with_context(|| format!("failed to publish {}", path.display()));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to create {}", staged.display()));
            }
        }
    }
    Err(anyhow!(
        "could not allocate a record file in {}",
        dir.display()
    ))
}

/// Read every record a measured command wrote. A missing directory means the
/// command never reached Cargo, which is a real result of zero invocations.
pub fn read_records(dir: &Path) -> Result<Vec<InvocationRecord>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut records = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().and_then(OsStr::to_str) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        records.push(
            serde_json::from_str(&raw)
                .with_context(|| format!("failed to parse {}", path.display()))?,
        );
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_crate_compilation_is_not_a_probe() {
        assert!(!is_probe(&args(&["--crate-name", "polint", "src/lib.rs"])));
    }

    #[test]
    fn compiler_queries_are_probes() {
        assert!(is_probe(&args(&["-", "--print=file-names"])));
        assert!(is_probe(&args(&["-vV"])));
        // Cargo probes target info with a placeholder crate name and `--print`.
        assert!(is_probe(&args(&["--crate-name", "___", "--print=sysroot"])));
    }

    #[test]
    fn totals_separate_cargo_starts_compiled_units_and_probes() {
        let record = |role, exit_code, probe| InvocationRecord {
            role,
            elapsed_ms: 10,
            exit_code,
            probe,
        };
        let totals = InvocationTotals::from_records(&[
            record(ShimRole::Cargo, 0, false),
            record(ShimRole::Cargo, 101, false),
            record(ShimRole::Rustc, 0, true),
            record(ShimRole::Rustc, 0, false),
        ]);
        assert_eq!(totals.cargo_invocations, 2);
        assert_eq!(totals.cargo_failed_invocations, 1);
        assert_eq!(totals.cargo_wall_clock_ms, 20);
        assert_eq!(totals.rustc_invocations, 2);
        assert_eq!(totals.compiled_units, 1);
    }

    #[test]
    fn records_round_trip_through_the_log_directory() {
        let dir = tempfile::tempdir().unwrap();
        let record = InvocationRecord {
            role: ShimRole::Rustc,
            elapsed_ms: 7,
            exit_code: 0,
            probe: false,
        };
        write_record(dir.path(), &record).unwrap();
        write_record(dir.path(), &record).unwrap();
        assert_eq!(
            read_records(dir.path()).unwrap(),
            vec![record.clone(), record]
        );
    }

    #[test]
    fn a_missing_log_directory_reports_no_invocations() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_records(&dir.path().join("absent")).unwrap().is_empty());
    }

    #[test]
    fn a_half_written_record_is_never_visible_to_the_reader() {
        // A disk that fills mid-write leaves the staged file behind. Counting it
        // as a record, or failing to parse it, would corrupt or abort a
        // measurement that the rest of the directory describes correctly.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("rustc-1-2-0.part"), "").unwrap();
        assert!(read_records(dir.path()).unwrap().is_empty());
    }
}
