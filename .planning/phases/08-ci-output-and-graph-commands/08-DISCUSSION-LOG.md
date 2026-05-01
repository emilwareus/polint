# Phase 8: CI Output and Graph Commands - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-01
**Phase:** 08-ci-output-and-graph-commands
**Mode:** `--auto`
**Areas discussed:** Command Surface Contracts, SARIF-Like CI Output, Exit Code Semantics, Graph Output Contracts, CI and Test Proof

---

## Command Surface Contracts

| Option | Description | Selected |
|--------|-------------|----------|
| Harden existing commands | Keep existing command names and complete behavior/tests around them. | yes |
| Redesign CLI surface | Rename or restructure commands before hardening. | |
| Add dynamic custom-rule loading now | Expand `test-rules` into repo-local Rust compilation/loading. | |

**Auto-selected choice:** Harden existing commands.
**Notes:** Preserves Phase 2/6 decisions: dynamic repo-local Rust loading is not v1 CLI hardening work.

---

## SARIF-Like CI Output

| Option | Description | Selected |
|--------|-------------|----------|
| SARIF-like, required CI fields | Include rule IDs, locations, messages, severities, and fingerprints without overclaiming full SARIF certification. | yes |
| Full SARIF certification scope | Attempt broad SARIF completeness in this phase. | |
| Minimal parseability only | Only assert JSON parses and driver name exists. | |

**Auto-selected choice:** SARIF-like, required CI fields.
**Notes:** Aligns with `DIAG-03` wording and existing `render_sarif` implementation.

---

## Exit Code Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Exact v1 contract | `0` success, `1` fail-threshold diagnostics, `2` fatal tool/config/internal errors. | yes |
| Command-specific ad hoc exits | Allow different commands to define unrelated exit behavior. | |
| Warnings always fail | Ignore `--fail-on` and make warnings always return `1`. | |

**Auto-selected choice:** Exact v1 contract.
**Notes:** Applies consistently to diagnostic-running commands while graph/explain return success unless command execution fails.

---

## Graph Output Contracts

| Option | Description | Selected |
|--------|-------------|----------|
| Deterministic DOT only | Keep DOT as v1 graph output, prove deterministic import/function graph behavior. | yes |
| Add multiple graph formats | Add JSON, Mermaid, or image graph output. | |
| Add semantic module resolution | Add production Node/TS or Go semantic import resolution. | |

**Auto-selected choice:** Deterministic DOT only.
**Notes:** Carries forward Phase 4/5 decisions that graph facts are syntax-level for v1.

---

## CI and Test Proof

| Option | Description | Selected |
|--------|-------------|----------|
| Structured integration and snapshots | Use parsed JSON assertions, DOT integration checks, exit code tests, and representative SARIF snapshots. | yes |
| Substring-only smoke tests | Minimal checks without pinning structure. | |
| Broad docs/examples hardening | Move major README/GitHub Actions work into this phase. | |

**Auto-selected choice:** Structured integration and snapshots.
**Notes:** Matches existing `assert_cmd`, `tempfile`, parsed JSON, and `insta` patterns.

---

## the agent's Discretion

- Exact SARIF-like optional fields beyond roadmap-required fields.
- Exact wording for command output and unknown-rule/empty-graph messages.
- Exact split between renderer snapshots, graph unit tests, and CLI integration tests.

## Deferred Ideas

- Full SARIF certification.
- Dynamic repo-local Rust rule compilation/loading.
- Graph formats beyond DOT.
- Production Node/TypeScript import resolution.
- Broad documentation and GitHub Actions examples unless narrowly needed for Phase 8 CI proof.
