# Quick Task 260518-qzd: AI-Friendly polint Check Output - Research

**Date:** 2026-05-18
**Status:** Complete

## Question

How should `polint check` expose large diagnostic sets to AI coding agents without
filling the agent context with thousands of findings?

## External Findings

- Anthropic's Claude Code docs frame context as the scarce resource: the
  conversation includes command output, file reads, and messages, and performance
  can degrade as the window fills. This directly supports making terminal output
  small by default for agent-oriented lint runs.
  Source: https://code.claude.com/docs/en/best-practices
- The same docs recommend separating exploration, planning, and implementation,
  and using verification commands. The `ai-friendly` mode should therefore make
  the first command a safe triage command, then point the agent to selective
  follow-up commands.
  Source: https://code.claude.com/docs/en/best-practices
- Claude structured-output docs emphasize schema-conformant JSON for downstream
  processing, and note that malformed or inconsistent JSON creates retry/error
  handling. The saved file should have a documented schema and stable field order
  rather than prose-only output.
  Source: https://platform.claude.com/docs/en/build-with-claude/structured-outputs
- OpenAI Codex base instructions describe AGENTS.md files as repo instructions
  loaded into the agent context and emphasize concise, actionable communication.
  The polint generated skill should give one preferred command and a few bounded
  `jq` follow-ups, not a long tutorial.
  Source: https://github.com/openai/codex/blob/main/codex-rs/protocol/src/prompts/base_instructions/default.md
- Recent agent-systems research points in the same direction: externalize large
  data into files/stores and pass structured, validated summaries across the
  context boundary. This maps cleanly to "small stdout, full JSON on disk".
  Sources:
  - https://arxiv.org/abs/2602.07398
  - https://arxiv.org/abs/2604.08224

## Codebase Findings

- `polint check` already supports `--format human|github|json|sarif`,
  `--max-diagnostics`, `--only-rule`, `--profile`, `--baseline`, `--new-only`,
  `--stat`, and `--shortstat` in `crates/polint/src/cli/mod.rs`.
- Repo-local rule hosts are child binaries. The parent CLI invokes them with
  `--format json` and merges diagnostics, so the AI-friendly renderer should live
  in the parent after filtering, ignores, and baseline classification.
- `crates/polint/src/runner/mod.rs` has its own `FormatArg` and renderer path for
  direct `polint-local-rules check` use. It should also accept `ai-friendly` for
  help consistency, even though the parent normally asks children for JSON.
- `crates/polint/src/diagnostics/mod.rs` owns `OutputFormat`, JSON rendering,
  deterministic diagnostic sorting, and report filtering. This is the right place
  for shared summary/report helpers.
- `polint init` currently creates `.polint/cache` and `.polint/.gitignore`
  containing `cache/`. The new output path should be initialized and ignored
  alongside cache.
- Generated skills come from `crates/polint/src/cli/skill.rs`. The checked-in
  `.claude/skills/polint/SKILL.md` mirrors that generated content and should be
  kept aligned.

## Recommended Shape

Add `polint check --format ai-friendly`.

Terminal output should be deliberately small:

```text
polint: 38 diagnostics across 4 rules. Full JSON: .polint/output/latest.json

By rule
  error local/no-raw-colors: 19
  warn  local/require-tests: 11
  warn  local/import-boundary: 6
  info  polint/capability: 2

Examples, max 10
  local/no-raw-colors src/Button.tsx:12:7 Use a design token instead of a raw color literal.
  local/require-tests backend/payments.go:88:1 Branch should have related test evidence.

JSON format: versioned object with `summary`, `examples`, and `diagnostics`.
Do not read the whole file into an AI prompt. Query it:
  jq '.summary.by_rule' .polint/output/latest.json
  jq '.diagnostics[] | select(.rule_id=="local/no-raw-colors") | {file, range, message}' .polint/output/latest.json | head -c 12000
  jq '[.diagnostics[] | select(.rule_id=="local/no-raw-colors")][0:20]' .polint/output/latest.json
```

Persist the full machine data under `.polint/output/`, preferably writing both:

- `.polint/output/check-YYYYMMDDTHHMMSSZ-<short-hash>.json` for durable run
  history.
- `.polint/output/latest.json` as a stable path agents can reuse without globbing.

The file should be a new `polint-ai-friendly-v1` schema, not just the existing
`polint-report-v1`, because it needs a first-class summary and examples section:

```json
{
  "version": 1,
  "schema": "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-ai-friendly-v1.json",
  "tool": {"name": "polint", "version": "x.y.z"},
  "generated_at": "2026-05-18T17:25:39Z",
  "summary": {
    "total_diagnostics": 38,
    "rules_triggered": 4,
    "by_severity": {"error": 19, "warn": 17, "info": 2},
    "by_rule": [
      {
        "rule_id": "local/no-raw-colors",
        "total": 19,
        "by_severity": {"error": 19, "warn": 0, "info": 0}
      }
    ],
    "examples_limit": 10
  },
  "examples": [
    {
      "rule_id": "local/no-raw-colors",
      "severity": "error",
      "file": "src/Button.tsx",
      "range": {"start_line": 12, "start_col": 7, "end_line": 12, "end_col": 14},
      "message": "Use a design token instead of a raw color literal.",
      "stable_fingerprint": "..."
    }
  ],
  "diagnostics": []
}
```

## Ordering Rules

- Counts include all diagnostics after normal report filters: ignores, baseline
  mode, `--new-only`, `--profile`, and `--only-rule`.
- Examples are one diagnostic per triggered rule, capped at 10 examples total.
- Example choice should be deterministic and severity-aware: error before warn
  before info, then higher count, then `rule_id`, then the existing diagnostic
  sort order.
- If `--max-diagnostics` is supplied, preserve the existing meaning by capping
  persisted `diagnostics`, and include a truncation field so agents know whether
  the saved file is complete. Rule counts should still reflect the uncapped set.

## Pitfalls

- Do not print the full JSON to stdout in AI-friendly mode.
- Do not fork rule execution. Build from the already-filtered diagnostics.
- Do not make examples call internal modules. Tests for repo-local rules must keep
  using public `polint::sdk::prelude::*` and `polint::runner::run_cli`.
- Do not silently leave `.polint/output/` trackable. `polint init` must ignore it,
  and `check --format ai-friendly` should ensure the nested ignore entry exists
  before writing output.
- Keep the saved JSON free of absolute workspace paths.
- Keep SARIF and existing JSON stable. AI-friendly is a new contract, not a
  modification to `polint-report-v1`.
