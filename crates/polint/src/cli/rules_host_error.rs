//! User-facing errors when the repo-local rules host (`cargo run` on `.polint/rules`) fails.

const RULES_HOST_PREFIX: &str = "polint: rules host:";

/// `POLINT_RULES_TOOLCHAIN` is forwarded as `RUSTUP_TOOLCHAIN` when spawning `cargo` for the rules crate.
pub(crate) const POLINT_RULES_TOOLCHAIN: &str = "POLINT_RULES_TOOLCHAIN";

pub(crate) fn rules_host_error_message(
    manifest_display: &str,
    status: std::process::ExitStatus,
    stdout: &str,
    stderr: &str,
) -> String {
    let stderr = stderr.trim();
    let stdout = stdout.trim();
    let mut out = format!(
        "{RULES_HOST_PREFIX} failed: {manifest_display}\nstatus: {status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    if is_polint_msrv_error(stderr, stdout) {
        out.push_str(&format!("\n\n{}", msrv_followup()));
    } else if is_network_error(stderr, stdout) {
        out.push_str(&format!(
            "\n\n{RULES_HOST_PREFIX} cargo could not fetch dependencies (network or registry). Check network, VPN, and crates.io status; retry or use an offline vendored registry if applicable.\nSee docs/CONSUMER-SETUP.md"
        ));
    } else if is_manifest_error(stderr, stdout) {
        out.push_str(&format!(
            "\n\n{RULES_HOST_PREFIX} manifest or workspace error while building the rules crate. Inspect `.polint/rules/Cargo.toml` and the stderr above.\nSee docs/CONSUMER-SETUP.md"));
    } else if is_rustc_missing_error(stderr, stdout) {
        out.push_str(&format!(
            "\n\n{RULES_HOST_PREFIX} `rustc` or the toolchain may be missing. Install Rust or set {} / rust-toolchain.toml.\nSee docs/CONSUMER-SETUP.md",
            POLINT_RULES_TOOLCHAIN
        ));
    }

    out
}

fn is_polint_msrv_error(stderr: &str, stdout: &str) -> bool {
    let blob = format!("{stderr}\n{stdout}");
    if !blob.contains("polint") {
        return false;
    }
    blob.contains("requires rustc")
        || blob.contains("is not supported by the following package")
        || (blob.contains("cannot be built because it requires rustc") && blob.contains("newer"))
}

fn msrv_followup() -> String {
    let msrv = env!("CARGO_PKG_RUST_VERSION");
    let active = rustc_hint();
    format!(
        "{RULES_HOST_PREFIX} the `polint` library needs Rust {msrv}+ to compile repo-local rules.\n\
         Active compiler (hint): {active}\n\
         Fix: bump `rust-toolchain.toml` or run `{POLINT_RULES_TOOLCHAIN}={msrv} polint check`\n\
         See README (Minimum Rust) and docs/CONSUMER-SETUP.md"
    )
}

fn rustc_hint() -> String {
    if let Ok(t) = std::env::var("RUSTUP_TOOLCHAIN") {
        return format!("override `{t}` (RUSTUP_TOOLCHAIN)");
    }
    match std::process::Command::new("rustc").arg("-V").output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "unknown (could not run `rustc -V`)".to_string(),
    }
}

fn blob(stderr: &str, stdout: &str) -> String {
    format!("{stderr}\n{stdout}")
}

fn is_network_error(stderr: &str, stdout: &str) -> bool {
    let b = blob(stderr, stdout);
    b.contains("could not download")
        || b.contains("Failed to fetch")
        || b.contains("failed to download")
        || b.contains("network failure")
        || b.contains("Connection refused")
        || b.contains("Operation timed out")
}

fn is_manifest_error(stderr: &str, stdout: &str) -> bool {
    let b = blob(stderr, stdout);
    (b.contains("could not parse") && b.contains("Cargo.toml"))
        || b.contains("error: failed to parse manifest")
        || b.contains("no matching package found")
}

fn is_rustc_missing_error(stderr: &str, stdout: &str) -> bool {
    let b = blob(stderr, stdout);
    (b.contains("rustc: not found") || b.contains("'rustc' not found"))
        || b.contains("could not execute rustc")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_with_prefix() {
        let status = std::process::Command::new("true").status().unwrap();
        let s = rules_host_error_message(
            ".polint/rules/Cargo.toml",
            status,
            "",
            "error: rustc 1.94.0 is not supported by the following package:\n  polint@0.1.4 requires rustc 1.95",
        );
        assert!(s.starts_with("polint: rules host:"));
        assert!(s.contains("1.95"));
    }
}
