---
quick_id: 260719-rld
status: complete
description: Optimize polint integration test suite speed by reducing cli.rs subprocess invocations without building or running tests
---

# Quick Task 260719-rld Plan

Reduce integration-test process overhead while preserving the existing CLI assertions and leaving CI configuration untouched.

## Task 1: Reuse immutable CLI help output

**Files:** `crates/polint/tests/common/mod.rs`, `crates/polint/tests/cli.rs`

**Action:** Add process-wide cached helpers for the fixed public help commands and replace repeated direct invocations in `cli.rs`. Keep marker checks and help-content assertions unchanged.

**Verify:** Read the diff and recount textual `polint_cmd()` call sites. Do not build or run tests locally; push for CI validation.

**Done:** Each distinct help command runs at most once per integration-test binary.

## Task 2: Build example rule binaries once

**Files:** `crates/polint/tests/common/mod.rs`

**Action:** Replace repeated `cargo run` wrappers with lazy, concurrency-safe one-time `cargo build` setup per example and direct execution of the resulting binary.

**Verify:** Inspect manifests, binary names, target paths, platform executable suffix handling, and the diff. Do not build or run tests locally; push for CI validation.

**Done:** The 16 example-rule invocations execute prebuilt binaries after at most four setup builds.
