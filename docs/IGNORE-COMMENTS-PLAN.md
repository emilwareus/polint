# Comment Ignores And Ignore Statistics Plan

Implementation note: the feature described here has now been built. For the
current user-facing contract, see
[`docs/IGNORE-COMMENTS.md`](IGNORE-COMMENTS.md) and
[`docs/schemas/polint-ignores-v1.json`](schemas/polint-ignores-v1.json). This
file remains as the design record for the decisions behind the feature.

## Purpose

polint should support source-code ignore comments like other linters, but the
ignore system must also help teams pay down ignored policy violations. The
important user story is:

> An AI coding agent can find which rules are ignored, where they are ignored,
> and which ignores are worth fixing next.

This feature should be implemented as engine-level suppression, not as something
every rule author has to remember to check.

## Starting State

polint already has coarse file scoping:

- `[workspace].exclude` prevents files from being analyzed.
- `.gitignore` and git excludes are respected during discovery.
- `[[rules.config]].files` and `allow_files` can scope individual rules when
  the rule calls SDK helpers such as `file_in_scope`.

That was not enough for normal lint adoption. The implementation adds inline
`polint-ignore` comments, unused-ignore reporting, and `polint ignores` so
ignore debt is searchable.

## Settled Product Decisions

- Use familiar explicit directive names: `polint-ignore-next-line`,
  `polint-ignore-line`, `polint-ignore-start`, `polint-ignore-end`, and
  `polint-ignore-file`.
- Require at least one rule selector. Bare ignores are invalid because broad
  suppressions are hard for agents and humans to search safely.
- Add `[ignores].require_reason` from the first implementation. When it is
  enabled, directives without a `-- reason` produce a dedicated
  `polint/ignore-missing-reason` error.
- `polint ignores` always computes full truth. It scans directives, runs rules,
  matches suppressions, reports unused/malformed/missing-reason entries, and
  emits stats. Do not add a partial `--scan-only` path.
- Ignore comments suppress policy-rule diagnostics only. They do not suppress
  parser, internal, or capability/setup diagnostics.
- `polint check` hides suppressed diagnostics from normal output and from
  `--fail-on`; ignore debt is surfaced through ignore diagnostics and
  `polint ignores`.
- Whole-file ignores are valid only in the top-of-file comment/header region.
- Overlapping block ignores are allowed when their selector sets can be matched
  independently.
- `polint ignores --filter <rules>` includes broad selectors such as `local/*`
  when they affect the filtered rule.

## Comment Syntax

Support these directives:

```ts
// polint-ignore-next-line local/no-raw-colors -- legacy UI migration
const color = "#ff00aa";

const color = "#ff00aa"; // polint-ignore-line local/no-raw-colors -- generated snapshot

/* polint-ignore-start local/no-raw-colors -- old design system */
const a = "#fff";
const b = "#000";
/* polint-ignore-end local/no-raw-colors */

// polint-ignore-file local/go-import-boundaries -- generated adapter
```

Rule selectors should support:

- exact rule IDs, such as `local/no-raw-colors`
- comma-separated rule IDs, such as `local/a,local/b`
- prefix patterns, such as `local/*`
- `*`

Omitted selectors are malformed. Users must name the rule or rule pattern being
ignored so `polint ignores --filter ...` can find and explain the suppression.

The text after `--` is the human reason. Reasons are always preserved and shown
by `polint ignores`. Whether a reason is required is controlled by config:

```toml
[ignores]
require_reason = true
```

Default should be `false` for adoption. When `require_reason = false`, missing
reasons are allowed and produce no warning. When `require_reason = true`, missing
reasons produce `polint/ignore-missing-reason`.

## Suppression Semantics

Ignore comments should suppress diagnostics after all rules have run and before
output rendering and exit-code calculation.

This gives the right behavior:

- individual rules cannot forget to honor ignores
- JSON, SARIF, and human output all agree
- ignored diagnostics do not affect `--fail-on`
- ignore statistics are based on real suppressed diagnostics
- unused ignores can be detected accurately

Default suppressible diagnostics should be policy-rule diagnostics only. Do not
allow source comments to suppress tool/setup diagnostics such as:

- `parser/*`
- `internal/*`
- `polint/capability`

Those are parse, engine, or setup correctness problems rather than repo policy
violations.

Whole-file directives apply only when they appear in the top-of-file
comment/header region before the first non-comment code. This prevents a buried
comment from silently disabling an entire file.

Block directives may overlap. The matcher should treat each active directive as
an independent selector/range rather than rejecting useful overlapping ranges.

## Engine Data Model

Add an internal suppression module with data shaped roughly like this:

```text
IgnoreDirective
- file
- line
- target range
- kind: line | next-line | file | block-start | block-end
- rule selectors
- reason
- raw text
- parse status

SuppressedDiagnostic
- diagnostic fingerprint
- rule_id
- file
- diagnostic range
- directive file and line

IgnoreReport
- directives
- suppressions
- unused directives
- malformed directives
- missing-reason directives
- summary by rule
- summary by file
```

This should stay an engine/reporting concern. Normal rule authors should not
receive broad ignore state through `RuleCtx`.

## Comment Scanning

Use language-aware comment extraction, not raw regex over whole source text.
Strings like `"// polint-ignore-next-line local/rule"` must not count as
directives.

Initial language support:

- Go line and block comments from tree-sitter nodes.
- TS/JS/TSX/JSX line and block comments through Oxc trivia/comment support or a
  parser-backed scanner.

Future language adapters should expose comment directives through the same
adapter contract.

## `polint ignores` Command

Add a top-level command:

```bash
polint ignores
polint ignores --stat
polint ignores --shortstat
polint ignores --stat --shortstat
polint ignores --filter local/no-raw-colors,local/go-import-boundaries
polint ignores --filter 'local/*'
polint ignores --format json
```

The command should always run the full ignore analysis path. It should not have
a scan-only mode because agents need to know whether ignores are active, unused,
malformed, or missing required reasons.

The default output should list ignore locations:

```text
src/Button.tsx:12 next-line local/no-raw-colors active suppressed=1 reason="legacy UI migration"
src/legacy.go:1 file local/go-import-boundaries active suppressed=8 reason="generated adapter"
src/Card.tsx:44 line local/no-raw-colors unused reason="old ignore"
src/Panel.tsx:8 next-line local/no-raw-colors missing-reason suppressed=1
```

`--shortstat` should print one compact line:

```text
7 ignore directives, 4 active, 1 unused, 1 malformed, 1 missing reason, 14 suppressed diagnostics across 3 rules in 4 files
```

`--stat` should print grouped detail:

```text
Rule                         Directives  Suppressed  Unused  Files
local/no-raw-colors          4           6           1       3
local/go-import-boundaries   2           8           0       1
local/*                      1           0           0       1
```

`--filter` should take a comma-separated list of rule selectors and should use
the same matching semantics as profiles and `--only-rule`: exact ID,
`prefix/*`, or `*`.

When `--filter` is present, the command should show only directives and
suppressed diagnostics relevant to matching rules. This is the AI-agent use
case: find ignored locations for one rule, then fix the underlying code.

Filtering must include direct and indirect matches. For example,
`polint ignores --filter local/no-raw-colors` should include directives for
`local/no-raw-colors`, `local/*`, and `*` when those directives suppress or could
suppress that rule.

## JSON Output

`polint ignores --format json` should be stable enough for agents to parse.
Suggested shape:

```json
{
  "schema": "polint-ignores-v1",
  "summary": {
    "directives": 7,
    "active": 4,
    "unused": 1,
    "malformed": 1,
    "missing_reasons": 1,
    "suppressed_diagnostics": 14,
    "rules": 3,
    "files": 4
  },
  "directives": [
    {
      "file": "src/Button.tsx",
      "line": 12,
      "kind": "next-line",
      "selectors": ["local/no-raw-colors"],
      "reason": "legacy UI migration",
      "status": "active",
      "suppressed": [
        {
          "rule_id": "local/no-raw-colors",
          "fingerprint": "..."
        }
      ]
    }
  ],
  "by_rule": [],
  "by_file": []
}
```

The JSON schema lives in
[`docs/schemas/polint-ignores-v1.json`](schemas/polint-ignores-v1.json).

Valid directive statuses should include:

- `active`
- `unused`
- `malformed`
- `missing_reason`

## Check Output Integration

`polint check` should:

1. load source files and rule diagnostics
2. scan ignore directives
3. suppress matching diagnostics
4. add `polint/unused-ignore` diagnostics for directives that matched nothing
5. add `polint/malformed-ignore` diagnostics for invalid directives
6. add `polint/ignore-missing-reason` diagnostics when
   `[ignores].require_reason = true`
7. render the remaining diagnostics
8. calculate `--fail-on` from the remaining diagnostics

Unused-ignore diagnostics should be warnings by default. Malformed-ignore
diagnostics should be errors because the author probably intended to suppress
something and failed.

Missing-reason diagnostics should be errors when the config requires reasons and
should not be emitted when the config does not require reasons.

## Local Rule Host Integration

Repo-local rules run through child `.polint/rules` hosts. Suppression should be
applied once by the parent CLI after all child diagnostics are collected.

Add a hidden or internal child-host mode if needed so `polint-local-rules` can
return raw diagnostics without applying ignore comments itself. This prevents
double suppression and prevents false unused-ignore reports when multiple rule
hosts exist.

## Implementation Steps

1. Add `[ignores]` config with `require_reason = false` by default.
2. Add directive data types and parser.
3. Add language-aware comment extraction for Go and TS/JS.
4. Add suppression matching against diagnostic file, start line/range, and rule
   ID.
5. Integrate suppression into `polint check` and `polint-local-rules` parent
   orchestration.
6. Add `polint ignores` with `--stat`, `--shortstat`, `--filter`, and
   `--format json`.
7. Add JSON schema under `docs/schemas/`.
8. Add docs and examples for humans and agents.
9. Add generated skill knowledge so `polint add-skill` teaches agents how to
   discover and pay down ignores.

## Skill And Agent Documentation

The generated skill text in
[`crates/polint/src/cli/skill.rs`](../crates/polint/src/cli/skill.rs) and the
agent-facing playbook in [`docs/AGENT-PLAYBOOK.md`](AGENT-PLAYBOOK.md) now
describe this command.

The skill should teach agents to use:

```bash
polint ignores --shortstat
polint ignores --stat
polint ignores --filter local/no-raw-colors --format json
```

Recommended agent workflow:

1. Run `polint check --format json`.
2. If diagnostics are absent but policy debt may be hidden, run
   `polint ignores --shortstat`.
3. For a target rule, run `polint ignores --filter <rule-id> --format json`.
4. Fix the ignored code, remove the ignore comment, and rerun `polint check`.
5. Keep going until the ignore report for that rule is empty or only accepted
   long-term suppressions remain.

Live usage docs can describe this command because it is implemented.

## Tests

Add realistic temp-repo tests for:

- `polint-ignore-line`
- `polint-ignore-next-line`
- `polint-ignore-file`
- `polint-ignore-start` / `polint-ignore-end`
- multiple comma-separated rule selectors
- wildcard selectors
- selector-required malformed diagnostics for bare ignores
- `[ignores].require_reason = false` allowing missing reasons
- `[ignores].require_reason = true` producing `polint/ignore-missing-reason`
- unused ignore diagnostics
- malformed ignore diagnostics
- top-of-file-only `polint-ignore-file`
- overlapping block ignores
- ignores not suppressing `parser/*`, `internal/*`, or `polint/capability`
- ignored diagnostics not affecting `--fail-on`
- JSON/SARIF/human output agreement
- `polint ignores --stat`
- `polint ignores --shortstat`
- `polint ignores --stat --shortstat`
- `polint ignores --filter rule-a,rule-b`
- `polint ignores --filter rule-a` including broad `local/*` or `*` selectors
- generated local-rule host diagnostics suppressed only once by the parent
- Go comments and TS/JS/TSX comments
- comments inside strings not treated as directives

## Non-Goals For First Version

- Centralized TOML suppressions. Keep this for a later feature after comment
  ignores are proven.
- Expiring suppressions. Useful later, but not needed for the first version.
- User-defined directive names. Keep one canonical syntax.
- Suppressing parser/internal/setup diagnostics by default.
- Bare all-rule ignores. A selector is required from day one.
- Scan-only ignore listing. `polint ignores` should compute active/unused status
  from real diagnostics.
