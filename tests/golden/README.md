# Golden characterization outputs

Committed, normalized CLI reports for behavioural lock-in. These are
**no-unintended-change** baselines, not correctness oracles.

## Layout

| Path | Role |
|------|------|
| [`outputs/`](outputs/) | Normalized golden files, one per case |
| `outputs/<case>.cost.json` | Wall-clock + peak RSS sidecar beside each golden |
| `crates/polint/tests/golden.rs` | Harness: run real `polint` CLI, normalize, compare |

Cases are derived from [`../golden-corpus/inputs.toml`](../golden-corpus/inputs.toml):

- **Example self-pairs** — each `examples/<name>/.polint/rules` pack against its
  parent example directory, `--format json`
- **Scale checkouts** — optional; missing clones print a loud
  `GOLDEN SKIP` and continue (run `make fetch-scale-repos` to materialize)

Eval-fixture trees are inventory-only here: they have no rule packs, so empty
`check` reports would not lock useful diagnostic sets. Capability coverage for
those trees belongs elsewhere. Other output formats (`sarif`, `ai-friendly`) are
omitted from the seed set to stay within PR budget.

## Normalization

Before compare or commit, the harness:

1. Parses stdout as JSON
2. Sorts `diagnostics` by `stable_fingerprint`
3. Rewrites absolute paths that fall under the case/repo root to relative form
4. Replaces `tool.version` with a fixed placeholder
5. Drops volatile fields (`generated_at`, durations, timings, hostname, threads)
   if present
6. Re-serializes as compact JSON with sorted object keys

## Cost sidecars

Each ran case also records wall-clock ms and peak RSS (bytes) from
`eval/bench/measure.rs` inside the rules-host process (env
`POLINT_GOLDEN_COST_PATH`). Committed `*.cost.json` values are the baseline;
CI fails when measured wall-clock or peak RSS exceeds baseline × 1.20 (with
absolute noise floors of +50 ms / +16 MiB).

## Failure output

On mismatch, the harness prints **lost** and **new** diagnostics as a set
difference on `stable_fingerprint` (with rule/file/message labels), not a raw
text dump alone.

## Regenerating

Rewrite diagnostic goldens:

```bash
POLINT_UPDATE_GOLDENS=1 cargo test -p polint --test golden --locked
```

Rewrite cost sidecars only (does not touch diagnostic goldens):

```bash
POLINT_UPDATE_GOLDEN_COSTS=1 cargo test -p polint --test golden --locked
```

**CI never sets those variables** (enforced by `ci_workflows_never_set_golden_update_env`).
Treat any golden-file diff as intentional behaviour change requiring a separate,
justified PR. Without the env flag the harness only compares.
