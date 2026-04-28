# Phase 2: CLI, Config, and Discovery - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-04-28
**Phase:** 02-cli-config-and-discovery
**Mode:** auto
**Areas discussed:** Existing implementation stance, CLI command contracts, Config behavior, File discovery behavior, Integration test focus

---

## Existing implementation stance

| Option | Description | Selected |
|--------|-------------|----------|
| Verify and harden existing implementation | Treat current `main` implementation as baseline and close real Phase 2 gaps only. | x |
| Rewrite the CLI/config/discovery flow | Recreate Phase 2 implementation from scratch. | |
| Only document current behavior | Avoid implementation hardening and only update GSD docs. | |

**User's choice:** Auto-selected recommended default: verify and harden existing implementation.
**Notes:** This carries forward the Phase 1 stance that existing `main` work is real but must be verified before requirements are marked complete.

---

## CLI command contracts

| Option | Description | Selected |
|--------|-------------|----------|
| Focus on init, new-rule, and check | Close Phase 2 around CLI-01, CLI-02, CLI-03 and avoid overclaiming later commands. | x |
| Treat every current command as Phase 2 scope | Pull `explain`, `test-rules`, `profile-rules`, and graph hardening into Phase 2. | |
| Reduce the CLI to only check | Remove or ignore `init` and `new-rule` despite Phase 2 requirements. | |

**User's choice:** Auto-selected recommended default: focus on `init`, `new-rule`, and `check`.
**Notes:** Existing extra commands can remain, but full CLI-04 hardening stays Phase 8.

| Option | Description | Selected |
|--------|-------------|----------|
| Keep stdout parseable for JSON | Human hints stay in human output so machine output can be parsed. | x |
| Always print helpful hints | Print guidance regardless of output format. | |
| Suppress all hints | Never suggest `polint init`. | |

**User's choice:** Auto-selected recommended default: keep JSON stdout parseable.
**Notes:** This is important for DIAG-02 and CI/agent consumption.

---

## Config behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Run with defaults when config is missing | Use default config and suggest `polint init` in human output. | x |
| Fail until config exists | Require `.polint.toml` before any check can run. | |
| Auto-create config during check | Mutate the repo when running `polint check`. | |

**User's choice:** Auto-selected recommended default: run with defaults when config is missing.
**Notes:** This matches CFG-02 and keeps `check` useful immediately.

| Option | Description | Selected |
|--------|-------------|----------|
| Support Phase 2 config surface | Include/exclude globs, profiles, rule paths, rule config, severity overrides, and language sections. | x |
| Minimal config only | Only include/exclude and profiles. | |
| Full custom rule loading now | Build automatic custom Rust rule compilation/loading in this phase. | |

**User's choice:** Auto-selected recommended default: support the Phase 2 config surface and defer full custom rule loading.
**Notes:** Rule auto-loading belongs to later SDK/plugin work.

---

## File discovery behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Use ignore plus globset | Honor `.gitignore`, include/exclude rules, supported extensions, and deterministic relative-path ordering. | x |
| Simple recursive walk | Ignore the existing `ignore` crate approach. | |
| Defer discovery hardening | Leave FS-01 weak until later. | |

**User's choice:** Auto-selected recommended default: use `ignore` plus `globset`.
**Notes:** This aligns with the stack research and current `polint-fs` implementation.

---

## Integration test focus

| Option | Description | Selected |
|--------|-------------|----------|
| Focused CLI integration tests | Cover init, new-rule, missing config default check, profiles, JSON output, and discovery filtering. | x |
| Only rely on existing tests | Do not add test coverage even if Phase 2 gaps appear. | |
| Broad snapshots and property tests now | Pull later testing phases into Phase 2. | |

**User's choice:** Auto-selected recommended default: focused CLI integration tests.
**Notes:** Broader snapshot and property coverage remains mapped to later phases.

---

## the agent's Discretion

- Exact test fixture names and helper structure.
- Minimal refactors needed to keep Phase 2 behavior coherent.
- Whether an already implemented behavior only needs verification or a narrow fix.

## Deferred Ideas

- Full CLI-04 command hardening.
- Complete CLI-05 exit-code semantics.
- Full DIAG-03 SARIF-like CI output.
- Repo-local custom Rust rule auto-compilation/loading.
- Cache persistence and deterministic parallel execution.
- Broad snapshot/property test expansion.
