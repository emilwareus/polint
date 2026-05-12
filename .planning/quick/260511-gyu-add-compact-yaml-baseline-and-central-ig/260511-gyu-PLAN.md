# Quick Task 260511-gyu: Compact YAML Baseline And Central Ignore Ratchet

**Date:** 2026-05-11
**Status:** In Progress

## Goal

Implement a compact human-editable YAML baseline file so mature repos can ratchet
polint adoption: existing diagnostics are tracked as baseline debt, central
ignore entries intentionally suppress selected diagnostics, and CI can fail only
on new diagnostics.

## Tasks

### 1. Baseline Engine
- **Files:** `crates/polint/src/baseline.rs`, `crates/polint/src/lib.rs`, workspace manifests
- **Action:** Add a YAML parser/renderer for `.polint-baseline.yaml` with `version`,
  `baseline`, and `ignore` string arrays. Parse entries as
  `<rule_id> <fingerprint> <file>`, dedupe by kind/rule/fingerprint/file, classify
  diagnostics as new/existing/ignored/fixed/stale-path.
- **Verify:** Unit tests for parse, malformed entries, dedupe, classification, and
  rendered compact YAML.

### 2. CLI Workflow
- **Files:** `crates/polint/src/cli/mod.rs`, `crates/polint/src/runner/mod.rs`
- **Action:** Add `polint baseline create|update`, `polint check --baseline`, and
  `polint check --baseline ... --new-only`. Apply comment ignores before baseline
  classification. Central ignore suppresses diagnostics; baseline suppresses
  failure while still counting debt.
- **Verify:** Integration tests for create, check, new-only, central ignore,
  update, malformed file, and local rule-host behavior.

### 3. Docs And Agent Guidance
- **Files:** `README.md`, `docs/AGENT-PLAYBOOK.md`, generated skill text
- **Action:** Document the compact YAML protocol and recommended CI command.
- **Verify:** Grep/docs review plus full Rust test suite.
