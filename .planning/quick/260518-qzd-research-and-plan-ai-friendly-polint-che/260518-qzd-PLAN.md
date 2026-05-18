---
quick_id: 260518-qzd
description: Research and plan ai-friendly polint check output format
status: planned
must_haves:
  truths:
    - `polint check --format ai-friendly` prints only summary, counts by rule, and max 10 examples.
    - Full diagnostic data is saved under `.polint/output/` in an ignored JSON file.
    - Help text and generated skills tell AI agents to use this mode to avoid context overload.
  artifacts:
    - `crates/polint/src/diagnostics/mod.rs`
    - `crates/polint/src/cli/mod.rs`
    - `crates/polint/src/runner/mod.rs`
    - `crates/polint/src/cli/skill.rs`
    - `docs/schemas/polint-ai-friendly-v1.json`
    - `README.md`, `docs/AGENT-PLAYBOOK.md`, `.claude/skills/polint/SKILL.md`
  key_links:
    - `.planning/quick/260518-qzd-research-and-plan-ai-friendly-polint-che/260518-qzd-RESEARCH.md`
---

# Plan: AI-Friendly polint Check Output

## Goal

Add `polint check --format ai-friendly` for AI coding agents. The command should
avoid flooding stdout, give enough summary to pick the next repair target, save
full structured diagnostics to `.polint/output/`, and teach agents how to query
that file without reading it wholesale.

## Public Contract

Command:

```bash
polint check --format ai-friendly --fail-on none
```

Behavior:

- Runs the same diagnostics as `polint check`.
- Applies existing profile, rule, ignore, baseline, and new-only filtering before
  summarizing.
- Writes `.polint/output/latest.json` and a timestamped sibling
  `.polint/output/check-YYYYMMDDTHHMMSSZ-<hash>.json`.
- Prints a compact terminal summary:
  - total diagnostics
  - triggered rule count
  - count per triggered rule
  - one example diagnostic per triggered rule, capped at 10 examples
  - JSON path and safe `jq` commands
- Preserves normal failure behavior from `--fail-on`.

Saved JSON:

- New schema: `docs/schemas/polint-ai-friendly-v1.json`.
- Includes `summary`, `examples`, and `diagnostics`.
- `diagnostics` uses the existing diagnostic shape from `polint-report-v1`.
- Includes `truncated` metadata if `--max-diagnostics` capped the saved
  diagnostics array.

## Task 1: Add Summary Model, Renderer, and JSON Persistence

Files:

- `crates/polint/src/diagnostics/mod.rs`
- `crates/polint/src/cli/mod.rs`
- `crates/polint/src/runner/mod.rs`
- `docs/schemas/polint-ai-friendly-v1.json`

Action:

1. Add `AiFriendly` to both CLI `FormatArg` enums and shared `OutputFormat`.
   Clap should expose the value as `ai-friendly`.
2. Add internal serializable structs:
   - `AiFriendlyReport`
   - `AiFriendlySummary`
   - `AiFriendlyRuleSummary`
   - `AiFriendlyExample`
   - `AiFriendlyTruncation`
3. Add a pure helper that accepts sorted diagnostics and returns:
   - counts by severity
   - counts by rule
   - deterministic examples, one per rule, max 10
4. Add a writer that creates `.polint/output/`, writes the timestamped JSON file,
   then writes or replaces `.polint/output/latest.json`.
5. Render stdout as compact text only. Do not include the full diagnostic JSON in
   stdout.
6. Keep parent and local-rule-host behavior aligned:
   - Parent `polint check --format ai-friendly` should still ask rule hosts for
     JSON and render AI-friendly after merging.
   - Direct `polint-local-rules check --format ai-friendly` should work for
     symmetry and help consistency.

Verify:

- Unit test summary counting and example selection with more than 10 rules.
- Unit test JSON serialization has stable fields and no absolute paths.
- Integration test with a temp repo and generated local rule pack verifies:
  - stdout has counts and at most 10 examples
  - stdout mentions `.polint/output/latest.json`
  - saved JSON parses and contains full diagnostics
  - exit status still follows `--fail-on`

Done:

- AI-friendly output exists and can be used without flooding command output.

## Task 2: Make `.polint/output/` Safely Ignored

Files:

- `crates/polint/src/cli/mod.rs`
- `README.md`
- tests covering `polint init`

Action:

1. Generalize `ensure_polint_nested_gitignore` to ensure multiple entries:
   `cache/` and `output/`.
2. `polint init` should create `.polint/output/` or at least ensure it is ignored.
   Prefer creating it so the path is discoverable after init.
3. `polint check --format ai-friendly` should call the same helper before writing
   the report, so older initialized repos do not accidentally track output.
4. Keep the root `.gitignore` unchanged unless an existing test requires it; the
   nested `.polint/.gitignore` is the right ownership boundary.

Verify:

- Existing init tests assert `.polint/.gitignore` includes both `cache/` and
  `output/`.
- Existing `.polint/.gitignore` content is preserved and missing entries are
  appended with a newline.
- Symlink/path-safety behavior remains consistent with existing init code.

Done:

- New and existing polint repos do not track `.polint/output/` by accident.

## Task 3: Teach Agents Through Help, Skill, and Docs

Files:

- `crates/polint/src/cli/mod.rs`
- `crates/polint/src/runner/mod.rs`
- `crates/polint/src/cli/skill.rs`
- `.claude/skills/polint/SKILL.md`
- `README.md`
- `docs/AGENT-PLAYBOOK.md`

Action:

1. Add top-level and `check` help guidance:
   - `polint -h`: mention AI agents should prefer
     `polint check --format ai-friendly`.
   - `polint check -h`: mention this format avoids context overload and writes
     queryable JSON under `.polint/output/`.
2. Update generated skill text:
   - Prefer `polint check --format ai-friendly --fail-on none` for AI agents.
   - State not to `cat` the whole JSON file.
   - Provide bounded `jq` examples:
     ```bash
     jq '.summary.by_rule' .polint/output/latest.json
     jq '[.diagnostics[] | select(.rule_id=="local/no-raw-colors")][0:20]' .polint/output/latest.json
     jq '.diagnostics[] | select(.file=="src/Button.tsx") | {rule_id, range, message}' .polint/output/latest.json | head -c 12000
     ```
3. Update the checked-in `.claude/skills/polint/SKILL.md` to match generated
   content.
4. Add README and agent playbook docs with the same command, the schema path, and
   the context-overload warning.

Verify:

- CLI help tests assert `ai-friendly` and the agent guidance are present in both
  `polint -h` and `polint check -h`.
- Existing `polint add-skill` tests assert the generated skill includes the new
  preferred AI-agent workflow.
- Docs mention JSON/SARIF remain available for CI and integrations.

Done:

- AI agents are steered to the safe output mode from help text and generated
  instructions before they run a noisy check.

## Acceptance Criteria

- `polint check --format ai-friendly --fail-on none` emits no full diagnostic
  stream to stdout.
- Terminal output includes counts for every triggered rule.
- Terminal output includes at most 10 example diagnostics.
- Full structured output is saved under `.polint/output/` and ignored by init.
- Saved JSON has a documented schema and can be filtered by `jq`.
- Help and skill text explicitly recommend this mode for AI agents.
- Existing `--format json`, `--format sarif`, GitHub output, human output,
  baseline behavior, ignore behavior, and exit-code behavior are unchanged.
