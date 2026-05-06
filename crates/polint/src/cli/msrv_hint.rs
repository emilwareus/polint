//! When `cargo build` of `.polint/rules` fails because the active `rustc` is below
//! `polint`'s `rust-version`, Cargo's stderr is easy to misread. We add a short, actionable note.

use std::process::Command;

/// `rust-version` from the `polint` package manifest (compile-time).
pub(crate) fn polint_msrv() -> &'static str {
    env!("CARGO_PKG_RUST_VERSION")
}

/// Heuristic: Cargo failed the rules package because `polint`'s MSRV is higher than `rustc`.
pub(crate) fn is_polint_rules_cargo_msrv_error(stderr: &str, stdout: &str) -> bool {
    let blob = format!("{stderr}\n{stdout}");
    if !blob.contains("polint") {
        return false;
    }
    blob.contains("requires rustc")
        || blob.contains("is not supported by the following package")
        || (blob.contains("cannot be built because it requires rustc") && blob.contains("newer"))
}

pub(crate) fn rules_host_msrv_followup_note() -> String {
    let msrv = polint_msrv();
    let active = active_rustc_hint();
    format!(
        "polint: repo-local rules compile the `polint` library, which requires Rust {msrv} or newer.\n\
         Active compiler (hint): {active}\n\
         Fix: add or update `rust-toolchain.toml` at the repository root (run `polint init` in a fresh repo), or run:\n\
         \n\
             RUSTUP_TOOLCHAIN={msrv} polint check\n\
         \n\
         See README: https://github.com/emilwareus/polint#minimum-rust-version"
    )
}

fn active_rustc_hint() -> String {
    if let Ok(t) = std::env::var("RUSTUP_TOOLCHAIN") {
        return format!("override `{t}` (RUSTUP_TOOLCHAIN)");
    }
    match Command::new("rustc").arg("-V").output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "unknown (could not run `rustc -V`)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_standard_msrv_stderr() {
        let stderr = r#"error: rustc 1.94.0 is not supported by the following package:
  polint@0.1.4 requires rustc 1.95"#;
        assert!(is_polint_rules_cargo_msrv_error(stderr, ""));
    }

    #[test]
    fn ignores_unrelated_errors() {
        assert!(!is_polint_rules_cargo_msrv_error(
            "error: could not compile `foo`",
            ""
        ));
    }

    #[test]
    fn requires_polint_mention() {
        assert!(!is_polint_rules_cargo_msrv_error(
            "error: rustc 1.50.0 is not supported by the following package:\n  bar@1.0 requires rustc 1.99",
            ""
        ));
    }
}
