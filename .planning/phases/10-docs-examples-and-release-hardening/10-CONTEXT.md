# Phase 10: Docs, Examples, and Release Hardening - Context

**Gathered:** 2026-05-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Make exlint/polint understandable, testable, and ready for a first release. This phase closes the public-facing v1 story: README completeness, example directories, fixture coverage, final verification, and honest documentation of what remains future work.

This phase does not add new analysis engines, new rule families, dynamic repo-local rule loading, crates.io publishing automation, or broad release infrastructure.

</domain>

<decisions>
## Implementation Decisions

### README and user path
- **D-01:** `[auto]` Treat `README.md` as the primary v1 user-facing document. It should cover what polint is, why it exists, non-goals, install/quickstart, config, SDK rule authoring, capabilities, rule testing, CI, examples, and roadmap.
- **D-02:** `[auto]` Keep README concise but complete. Prefer runnable commands and small snippets over long prose.
- **D-03:** `[auto]` Be explicit that built-in `examples/...` rules are SDK dogfood examples, not a comprehensive lint pack.
- **D-04:** `[auto]` Keep truthfulness constraints from prior phases: syntax/heuristic behavior must be labeled honestly; SARIF remains SARIF-like.

### Examples directory
- **D-05:** `[auto]` Expand the existing top-level `examples/` layout rather than moving examples into crate internals or creating a docs site.
- **D-06:** `[auto]` Ensure `examples/` contains the five roadmap-required examples: `basic`, `custom-rule-go`, `custom-rule-ts`, `go-branch-obligations`, and `ts-design-tokens`.
- **D-07:** `[auto]` Each example should explain its purpose, include a minimal `.polint.toml` or command path where useful, and show expected command usage with either installed `polint` or `cargo run -p polint-cli --`.
- **D-08:** `[auto]` Example docs should demonstrate the SDK and current CLI behavior only. Do not imply that generated repo-local Rust rules are automatically compiled or dynamically loaded by `polint check`.

### Fixtures and tests
- **D-09:** `[auto]` Keep test fixtures under `tests/fixtures/` and reuse the established `assert_cmd`, `tempfile`, `include_str!`, parsed JSON, and snapshot patterns.
- **D-10:** `[auto]` Add or harden integration proof for the mixed Go/TS fixture, because Phase 10 success criteria explicitly call out Go, TS, and mixed repositories.
- **D-11:** `[auto]` Add focused CLI/example smoke tests where they reduce risk, especially for example directories that are meant to be runnable or copyable.
- **D-12:** `[auto]` Avoid duplicating large fixture trees. Prefer small files and existing fixtures that exercise the requested public behavior.

### Release hardening
- **D-13:** `[auto]` Define release readiness as honest docs plus a clean verification matrix: `cargo fmt -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- **D-14:** `[auto]` Document remaining future work honestly in the README roadmap instead of adding placeholder code or TODO-only features.
- **D-15:** `[auto]` Close `FND-03` and finalize testing traceability only when the README/examples/fixtures are actually present and the workspace verification passes.
- **D-16:** `[auto]` Do not add crates.io publishing, release tagging, package metadata expansion, benchmark suites, or CI workflow files unless the existing code/docs make that necessary for the phase criteria.

### the agent's Discretion
- The agent may decide exact README section ordering and wording, provided the roadmap-required topics are present.
- The agent may choose whether examples get `.polint.toml`, source files, README updates, or a combination based on which makes them most useful and testable.
- The agent may add narrow integration tests for examples, mixed fixtures, README command assumptions, or final release checks where they protect the user-facing contract.
- The agent may update project planning docs at closeout so the final state points to milestone completion.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` — Phase 10 goal, requirements, and success criteria.
- `.planning/REQUIREMENTS.md` — `FND-03`, `TEST-01`, `TEST-02`, `TEST-03`, and `TEST-04` traceability.
- `.planning/PROJECT.md` — product value, constraints, non-goals, and truthfulness rules.
- `docs/INITIAL_PROMPT.md` §README requirements — source prompt for README content, example rule list, CI expectations, and final v1 shape.

### Prior decisions
- `.planning/phases/06-sdk-and-example-rules/06-CONTEXT.md` — SDK authoring, examples-as-dogfood, scaffolding, and rule testing decisions.
- `.planning/phases/08-ci-output-and-graph-commands/08-CONTEXT.md` — SARIF-like, CLI command, exit-code, and graph-output truthfulness decisions.

### Current user-facing artifacts
- `README.md` — primary v1 user-facing document to complete.
- `examples/` — current example directories to expand and harden.
- `tests/fixtures/` — Go, TS, and mixed repository fixture roots.
- `crates/polint-cli/tests/cli.rs` — established CLI integration test patterns.
- `crates/polint-rules/tests/snapshots.rs` — established snapshot test patterns for diagnostics.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `README.md`: already contains project overview, quickstart, config, built-in example rules, SDK snippet, CI, and roadmap; it needs final completeness, rule testing, examples, and release-hardening polish.
- `examples/basic/README.md`, `examples/custom-rule-go/README.md`, `examples/custom-rule-ts/README.md`: existing example docs are very short and can be expanded without changing core code.
- `examples/go-branch-obligations/authorize.go` and `examples/ts-design-tokens/Button.tsx`: source examples exist but need explanatory README/config coverage.
- `tests/fixtures/go`, `tests/fixtures/ts`, and `tests/fixtures/mixed`: fixture roots exist; mixed fixture is not currently covered by a CLI integration test.
- `crates/polint-cli/tests/cli.rs`: has helpers for temp repo setup, config writing, parsed JSON assertions, exit-code checks, cache tests, graph tests, and example rule tests.

### Established Patterns
- Documentation must not overclaim: built-ins are examples, heuristics are heuristic, and SARIF is SARIF-like.
- CLI integration tests use `assert_cmd`, `tempfile`, `include_str!`, and parsed JSON assertions rather than shelling out through external scripts.
- Output determinism remains a core project invariant. New docs/tests should preserve deterministic command examples and expected outputs.
- Example rules use `examples/...` IDs and the public SDK-facing APIs established in Phase 6.

### Integration Points
- README and example READMEs are the main documentation targets.
- `crates/polint-cli/tests/cli.rs` is the likely place for mixed fixture and example smoke tests.
- `tests/fixtures/mixed` should prove one repo can contain both Go and TS files.
- Requirement and roadmap closeout should update `FND-03` and any final testing traceability after verification passes.

</code_context>

<specifics>
## Specific Ideas

- The final README should preserve the prompt's core command path:
  ```bash
  polint init
  polint new-rule go my-project-policy
  polint check
  ```
- CI docs should recommend `polint check --profile full`.
- Rule testing docs should explain `polint test-rules`.
- Capabilities docs should explain that rules declare what facts they need so the engine computes only relevant analysis.
- Release hardening should prefer a smaller honest v1 over placeholder functionality.

</specifics>

<deferred>
## Deferred Ideas

- Actual crates.io publication, release tagging, binary packaging, and generated changelogs belong after v1 readiness is verified.
- Full automatic repo-local Rust rule compilation and caching remains future work.
- Exact Go semantic sidecar, dynamic branch coverage, and additional language adapters remain roadmap items.

</deferred>

---

*Phase: 10-docs-examples-and-release-hardening*
*Context gathered: 2026-05-01*
