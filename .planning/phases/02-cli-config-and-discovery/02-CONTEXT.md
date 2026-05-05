# Phase 2: CLI, Config, and Discovery - Context

**Gathered:** 2026-04-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 2 delivers the first usable `polint` loop: initialize a repo, scaffold a repo-local rule, discover supported source files, run `polint check`, load config or safe defaults, and render human/JSON output. This phase should close CLI-01, CLI-02, CLI-03, CFG-01, CFG-02, FS-01, DIAG-02, and the Phase 2 slice of TEST-02.

The existing `main` implementation already contains a working initial CLI/config/discovery flow. Planning should verify it, identify real Phase 2 gaps, and make narrow hardening fixes. Do not recreate the CLI or move work back into worktrees.

</domain>

<decisions>
## Implementation Decisions

### Existing implementation stance
- **D-01:** Treat the existing `main` implementation as the Phase 2 baseline, the same way Phase 1 treated the workspace baseline.
- **D-02:** Plan closure work around verification and targeted hardening, not a rewrite of `polint-cli`, `polint-config`, or `polint-fs`.
- **D-03:** Work directly in `/Users/emilwareus/Development/exlint` on `main`; do not create or use GSD worktrees.

### CLI command contracts
- **D-04:** Phase 2 focuses on `polint init`, `polint new-rule <language> <rule-name>`, and `polint check`.
- **D-05:** Preserve currently implemented later commands such as `explain`, `test-rules`, `profile-rules`, and `graph`, but do not claim CLI-04 complete in Phase 2. Full hardening for those commands belongs to Phase 8.
- **D-06:** `polint check` must accept `--profile`, `--format human|json|sarif`, `--no-cache`, and `--fail-on warn|error|none` for CLI-03. Phase 2 can verify the flags and basic behavior; full CLI-05 exit-code semantics remain Phase 8.
- **D-07:** JSON output should be parseable machine output on stdout. Human-only guidance such as "Run `polint init`" belongs in human output and must not corrupt JSON.
- **D-08:** The CLI should emphasize custom repo-local policy as code, not present built-in rules as a comprehensive packaged ruleset.

### Config behavior
- **D-09:** If `.polint.toml` is missing, `polint check` should run with a minimal default config and suggest `polint init` in human output.
- **D-10:** `polint init` should create `.polint.toml` and `.polint/rules` without overwriting an existing config.
- **D-11:** Phase 2 config support includes include/exclude globs, profiles, rule paths, rule config entries, severity overrides, and language sections.
- **D-12:** Repo-local custom Rust rule auto-compilation/loading is out of scope for Phase 2. `new-rule` scaffolding and `rules.paths` config support are enough here.

### Rule scaffolding
- **D-13:** `polint new-rule <language> <rule-name>` should create a repo-local Rust skeleton under `.polint/rules/<rule-name>/` with `Cargo.toml` and `src/lib.rs`.
- **D-14:** Language-focused skeletons should support Go and TS/JS families with useful capability defaults. Unknown/generic languages can fall back to a syntax-focused skeleton.
- **D-15:** Scaffolds should be compilable-looking and SDK-oriented, but this phase does not need to compile or dynamically load them.

### File discovery behavior
- **D-16:** Discovery should use the existing `ignore` + `globset` approach, respect `.gitignore`, include globs, exclude globs, and supported Go/TS/JS extensions.
- **D-17:** Discovery output must be deterministic by relative path.
- **D-18:** Supported extensions for Phase 2 are `.go`, `.ts`, `.tsx`, `.js`, and `.jsx`.
- **D-19:** Default excludes should continue to avoid common generated/dependency directories such as `.git`, `target`, `node_modules`, `vendor`, and generated protobuf Go files.

### Integration test focus
- **D-20:** Phase 2 should add or verify focused CLI integration tests for `init`, `new-rule`, missing-config default check, profiles, JSON output, and file discovery filtering.
- **D-21:** Do not expand into broad snapshot or property-test work unless a missing focused test directly blocks Phase 2 confidence. Snapshot/property coverage is mapped to later phases.
- **D-22:** Tests should use the existing `assert_cmd`, `predicates`, and `tempfile` pattern already present in `crates/polint-cli/tests/cli.rs`.

### the agent's Discretion
- The agent may choose exact test fixture names and minimal refactors needed to make the Phase 2 surface clean.
- The agent may decide whether a Phase 2 gap is best fixed in `polint-cli`, `polint-config`, `polint-fs`, or `polint-diagnostics`, as long as the fix stays inside the Phase 2 boundary.
- The agent may keep existing implementations when verification proves they already satisfy a requirement.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Product and requirements
- `docs/INITIAL_PROMPT.md` - Original product prompt, command surface, crate responsibilities, and non-goals.
- `.planning/PROJECT.md` - Core value, constraints, current project decisions, and no-worktree repository layout.
- `.planning/REQUIREMENTS.md` - Phase 2 requirement IDs and traceability rows.
- `.planning/ROADMAP.md` - Phase 2 goal and success criteria.

### Prior phase decisions
- `.planning/phases/01-workspace-foundation/01-CONTEXT.md` - Locked decisions to use `main`, avoid worktrees, keep crate boundaries, and avoid pulling later-phase work into early phases.
- `.planning/phases/01-workspace-foundation/01-VERIFICATION.md` - Evidence that the workspace baseline is verified and Phase 2 can build on it.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint-cli/src/main.rs` - Existing clap command definitions and handlers for `init`, `new-rule`, `check`, output format selection, profile selection, `--no-cache`, and `--fail-on`.
- `crates/polint-config/src/lib.rs` - Existing `PolintConfig`, `LoadedConfig`, default config TOML, profile lookup, rule config lookup, and globset builders.
- `crates/polint-fs/src/lib.rs` - Existing file discovery and analysis file loading through `ignore`, `globset`, language detection, and deterministic relative-path sorting.
- `crates/polint-diagnostics/src/lib.rs` - Existing human, JSON, and SARIF-like rendering plus diagnostic sorting/deduplication helpers.
- `crates/polint-cli/tests/cli.rs` - Existing integration test style using `assert_cmd`, `predicates`, and `tempfile`.
- `tests/fixtures/` and `examples/` - Existing fixture/example material that later tests can reuse when useful.

### Established Patterns
- CLI commands return `Result<u8>` internally and map unexpected errors to process exit code 2 in `main`.
- Config missing is represented by `LoadedConfig { missing: true, config: PolintConfig::default(), ... }`.
- File discovery stores root-relative paths using `/` separators and sorts before returning.
- Diagnostics are rendered through a shared `render(OutputFormat, diagnostics)` entry point.
- Built-in example rules are exposed by `polint-rules`, but product positioning should still emphasize repo-local custom policies.

### Integration Points
- `polint-cli::check` calls config loading, discovery/loading, Go/TS analysis, built-in rule execution, diagnostic rendering, and exit-code selection.
- `polint-config` owns include/exclude/profile/rule config parsing; `polint-fs` consumes `LoadedConfig`.
- `polint-diagnostics` owns JSON output structure for DIAG-02; Phase 2 tests should validate parseability and representative fields.
- `polint-cache::Cache::default_for_repo` is currently touched by CLI, but deeper parse/fact cache persistence remains Phase 7.

</code_context>

<specifics>
## Specific Ideas

- The user wants GSD to stay in `/Users/emilwareus/Development/exlint` on `main`, with no worktrees.
- Phase 2 should produce a dependable first local workflow: `polint init`, create or inspect config, add/scaffold a rule, run `polint check`, and consume JSON output in tools.
- Keep stdout/stderr discipline practical for automation: machine formats should remain parseable.
- Keep wording honest where behavior is still scaffolded or later-phase hardening remains.

</specifics>

<deferred>
## Deferred Ideas

- CLI-04 full hardening for `explain`, `test-rules`, `profile-rules`, and `graph` belongs to Phase 8.
- CLI-05 complete exit-code semantics belongs to Phase 8, though Phase 2 may verify the basic `check` behavior that already exists.
- DIAG-03 full SARIF-like CI output belongs to Phase 8.
- Repo-local custom Rust rule auto-compilation/loading belongs to later runner and SDK work, not Phase 2.
- Cache read/write persistence and deterministic parallel execution belong to Phase 7.
- Snapshot and property test expansion belongs to Phases 3, 8, and 10 unless needed for a narrow Phase 2 confidence gap.

</deferred>

---

*Phase: 02-cli-config-and-discovery*
*Context gathered: 2026-04-28*
