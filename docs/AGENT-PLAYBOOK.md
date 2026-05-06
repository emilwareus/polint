# Agent playbook (polint)

Use this when wiring coding agents or CI to polint.

## Setup

1. Install polint (`cargo install polint --locked` or build from source).
2. Run `polint init` once per repo; add rules with `polint new-rule …`.
3. Optional: `polint add-skill` so the agent finds local usage docs.

## Machine-readable output

- **JSON report**: `polint check --format json`
- **Schema**: [`docs/schemas/polint-report-v1.json`](schemas/polint-report-v1.json)
- **Stable ordering**: diagnostics are sorted by file, range, rule id, message, fingerprint; use `--max-diagnostics N` to truncate emitted reports **after** that sort. The cap does not change `--fail-on`.

Validate against expectations by deserializing with your agent’s JSON stack; do not rely on stderr prose for pass/fail when JSON is available.

## Focused runs

- **One rule pattern**: `--only-rule PATTERN` (same matching as profiles: exact id, `prefix/*`, or `*`).
- **Cap noise**: `--max-diagnostics N`
- **Severity gate**: `--fail-on warn|error|none`

Example:

```bash
polint check --format json --fail-on error --only-rule 'local/*' path/to/dir
```

## Explaining harvester facts

```bash
polint explain go-test --file internal/foo/service/bar_test.go --test TestAuthorize
```

Emits JSON for one [`TestFact`](../crates/polint/src/core/mod.rs). See [facts/go-tests.md](facts/go-tests.md).

## CI snippet

```yaml
- uses: actions/checkout@v6
- uses: dtolnay/rust-toolchain@stable
- run: cargo install polint --locked
- run: polint check --format json --fail-on error
```

SARIF for GitHub Code Scanning: `polint check --format sarif` then
`github/codeql-action/upload-sarif` (see root `.github/workflows/ci.yml`).

## Prompt starter (copy-paste)

> Use the polint JSON report (`polint check --format json`). The schema is in
> `docs/schemas/polint-report-v1.json`. Parse `diagnostics[]`; each item has
> `rule_id`, `severity`, `file`, `range`, `message`, optional `fix`. Apply fixes
> and re-run until the report is empty or only allowed severities remain.

## Troubleshooting

See [CONSUMER-SETUP.md](CONSUMER-SETUP.md) for rules-host errors, env vars, and SARIF help URIs.

## Cookbook patterns

Small, composable rule ideas (implement in `.polint/rules`):

1. **Require `t.Run`** for table-style tests — check `subtest_count` / `subtest_names` on [`TestFact`](../crates/polint/src/core/mod.rs).
2. **Substring in test name** — `TestFact.name.contains("Integration")`.
3. **Forbid import path** — scan [`ImportFact`](../crates/polint/src/core/mod.rs) for a prefix.

Golden JSON: use `examples/ts-design-tokens` or similar; integration tests in
`crates/polint/tests/cli.rs` assert filters and report shape.
