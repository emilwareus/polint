//! Verifies `cargo install --path crates/polint-cli` from the workspace root produces a
//! runnable binary on Linux, macOS, and Windows.
//!
//! This is `#[ignore]` by default (slow: full release install). CI runs:
//! `cargo test -p polint-cli --test cargo_install_smoke --locked -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("polint-cli crate should be at <root>/crates/polint-cli")
        .to_path_buf()
}

fn installed_polint(root: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "polint.exe"
    } else {
        "polint"
    };
    root.join("bin").join(name)
}

#[test]
#[ignore = "slow: run in CI with: cargo test -p polint-cli --test cargo_install_smoke --locked -- --ignored"]
fn cargo_install_to_custom_prefix_runs_version() {
    let temp = tempfile::tempdir().unwrap();
    let install_root = temp.path();
    let ws = workspace_root();

    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(&ws)
        .args([
            "install",
            "--locked",
            "--path",
            "crates/polint-cli",
            "--root",
            install_root
                .to_str()
                .expect("install root path should be valid UTF-8 for cargo"),
            "--force",
        ])
        .status()
        .expect("spawn cargo install");
    assert!(
        status.success(),
        "cargo install failed (run from workspace root with same layout as CI)",
    );

    let bin = installed_polint(install_root);
    assert!(
        bin.exists(),
        "expected installed binary at {}",
        bin.display()
    );

    let output = Command::new(&bin)
        .arg("--version")
        .output()
        .expect("spawn installed polint");
    assert!(
        output.status.success(),
        "polint --version failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("polint"),
        "unexpected --version stdout: {stdout:?}",
    );
}
