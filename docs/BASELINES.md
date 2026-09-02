# Baselines


Use a baseline when adopting polint in a repository that already has valid
findings. The baseline is always checked in at `.polint/baseline.yaml` as
compact YAML:

```yaml
version: 1

baseline:
  - "local/backend-context-propagation e337fbb73d44b2b7 backend/app/handler.go"
ignore:
  - "local/no-raw-colors 1b7c9a00e493aa21 frontend/Button.tsx"
```

Each entry is one string:

```text
<rule_id> <fingerprint> <file>
```

`baseline` entries are existing debt: they stay visible in human output but do
not fail the process. `ignore` entries are central accepted exceptions: they are
suppressed from output and failure. Baseline matching uses `rule_id +
fingerprint` and refreshes unambiguous moved paths; ignore matching is
file-specific to avoid suppressing unrelated findings with the same fingerprint.

```bash
polint baseline create
polint check --baseline --new-only
polint baseline update
```

`--new-only` emits and fails only on diagnostics not covered by the baseline or
central ignore list.

