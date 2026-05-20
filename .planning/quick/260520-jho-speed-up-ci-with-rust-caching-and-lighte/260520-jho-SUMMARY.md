# Quick Task 260520-jho Summary: CI speedup

**Completed:** 2026-05-20
**Status:** Complete

## Changes

- Added `Swatinem/rust-cache@v2` to Cargo-heavy CI jobs and disabled Cargo
  incremental compilation in CI to reduce cache churn.
- Split the old three-OS clippy/test/install matrix into focused jobs:
  Ubuntu clippy, Ubuntu full workspace tests, macOS/Windows platform library
  tests, and Ubuntu install smoke.
- Added explicit Go cache dependency paths where CI tests can invoke Go tooling.
- Kept SARIF, rustdoc, MSRV, rustfmt, and cargo-deny checks.

## Local Verification

- `ruby -e 'require "yaml"; YAML.load_file(".github/workflows/ci.yml")'`
- `git diff --check`
- `cargo fmt --all --check`
- `GOWORK=off go test ./...` in `tools/polint-go-symbols`
- `GOWORK=off go test ./...` in
  `crates/polint/go-sidecar/polint-go-symbols`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`

## Remote Measurement

The new GitHub Actions run is measured after pushing this branch update. The
measured runtime is reported in the session response.
