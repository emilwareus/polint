# Comment Ignores

polint supports source comments that suppress policy diagnostics after rules have
run. Rules still report findings normally; the engine applies the ignore layer,
records what was suppressed, and reports stale or malformed ignores.

## Syntax

Every ignore must name at least one rule selector. Bare ignores are malformed.

```ts
// polint-ignore-next-line local/no-raw-colors -- legacy fixture
const color = "#ff00aa";

const token = "TODO"; // polint-ignore-line local/no-placeholder-literals -- generated file

// polint-ignore-start local/* -- generated compatibility block
const a = "TODO";
const b = "#ff00aa";
// polint-ignore-end local/*
```

Supported directives:

| Directive | Effect |
|-----------|--------|
| `polint-ignore-next-line RULES -- reason` | Suppresses matching diagnostics on the next line. |
| `polint-ignore-line RULES -- reason` | Suppresses matching diagnostics on the current line. |
| `polint-ignore-start RULES -- reason` / `polint-ignore-end RULES` | Suppresses matching diagnostics between the two comments. The `end` selector must match the `start` selector list. |
| `polint-ignore-file RULES -- reason` | Suppresses matching diagnostics in the file. It must appear before the first code line. |

`RULES` is a comma-separated list using the same matching style as profiles and
`--only-rule`: exact IDs, `prefix/*`, or `*`.

The `-- reason` text is optional by default. Repositories can require it:

```toml
[ignores]
require_reason = true
```

When reasons are required, an ignore without a reason still suppresses matching
policy diagnostics, but `polint check` emits `polint/ignore-missing-reason` so
the debt is visible.

## Suppression Scope

Comment ignores suppress policy diagnostics only. They never suppress parser
errors, internal rule failures, capability/setup diagnostics, or other
`polint/*` diagnostics.

polint extracts comments with a small language-aware scanner for supported source
files. It skips quoted strings, single-quoted strings, and backtick strings so a
literal containing `polint-ignore-next-line` is not treated as an ignore.

## Health Diagnostics

`polint check` can emit these engine diagnostics:

| Rule ID | Severity | Meaning |
|---------|----------|---------|
| `polint/unused-ignore` | warning | A valid suppressing directive did not suppress any diagnostics. |
| `polint/malformed-ignore` | error | A directive is syntactically invalid, misplaced, unmatched, or unclosed. |
| `polint/ignore-missing-reason` | error | `[ignores].require_reason = true` and the directive has no `-- reason`. |

## Inspecting Ignores

Use `polint ignores` when an agent or maintainer needs to find suppressions to
remove or repair.

```bash
polint ignores
polint ignores --shortstat
polint ignores --stat
polint ignores --filter local/no-raw-colors,local/*
polint ignores --format json
```

The command runs the same analysis as `polint check`, applies ignores internally,
and reports the ignore directives instead of diagnostics. `--filter` accepts a
comma-separated list of rule selectors. A directive is shown when its selector
overlaps the filter or when it suppressed a diagnostic matching the filter.

JSON output follows
[`docs/schemas/polint-ignores-v1.json`](schemas/polint-ignores-v1.json). The
report includes:

- `summary`: total directives, active/unused/malformed/missing-reason counts,
  suppressed diagnostic count, and affected file/rule counts.
- `directives`: each public directive with file, line, selector list, status,
  optional reason/problem, and suppressed diagnostic fingerprints.
- `by_rule` and `by_file`: grouped statistics for quick dashboards.

For AI-agent cleanup, start with:

```bash
polint ignores --stat --filter local/my-rule
```

Then fix the underlying code, remove the ignore, and re-run:

```bash
polint check --format json --only-rule local/my-rule
```

See [`examples/comment-ignores`](../examples/comment-ignores/README.md) for a
complete mini-repo with one suppressed finding and one visible finding from the
same rule.
