# Locked Benchmark Suite Index (BENCH-01)

This document is the single index of the locked repo set used to establish the
v2.0 store-disabled performance/accuracy baselines and the downstream regression
gates. It records which suites run in CI, which are local-only, the pinned
commits, and how a developer materializes each checkout.

All `local_clone` checkout targets are git-ignored (`ignored_by_git = true`) and
resolved repo-relative under `research/evaluation-harness/repos/` unless a
manifest explicitly opts into `local_clone_policy = "allow_absolute"`.

## Scale repos (performance / `kind = "performance"`)

| Suite manifest | Repo | Languages | Pinned commit | CI? |
|----------------|------|-----------|---------------|-----|
| `grafana-grafana-scale.toml` | [grafana/grafana](https://github.com/grafana/grafana) (v11.4.0) | Go + TypeScript | `b58701869e1a11b696010a6f28bd96b68a2cf0d0` | CI (fast/nightly/release) |
| `gohugoio-hugo-scale.toml` | [gohugoio/hugo](https://github.com/gohugoio/hugo) (v0.140.0) | Go | `3f35721fb2c75a1f7cc5a7a14400b66e73d4b06e` | CI (fast/nightly/release) |
| `excalidraw-excalidraw-scale.toml` | [excalidraw/excalidraw](https://github.com/excalidraw/excalidraw) (v0.17.6) | TypeScript | `f1640710aae577cafb3c52ab2bf255a460c3ebf1` | CI (fast/nightly/release) |
| `devloupe-monorepo-local.toml` | Devloupe monorepo (private) | Go + TypeScript | local checkout, unpinned | **Local-only / NON-CI** |

### devloupe-monorepo-local — local-only, NON-CI

`devloupe-monorepo-local.toml` is a **private, local-only reference** and is
**excluded from CI**. It declares ONLY a `research` tier, so the CI
`fast`/`nightly`/`release` runs never resolve it, and it sets
`local_clone_policy = "allow_absolute"` so a developer can point `checkout.path`
at their own absolute checkout. Known baseline on the reference host: **~1GB peak
RSS, cold 7.4s / warm 4.6s**.

## Oracle / recall suites (part of the locked set)

These pre-existing suites are the accuracy oracles in the locked set and are
indexed here so the complete repo set lives in one place:

| Suite manifest | Repo | Kind | Role |
|----------------|------|------|------|
| `jelly-callgraph-micro.toml` | Jelly micro-fixtures | `call_graph_precision` | JS/TS call-graph recall oracle |
| `go-x-tools-rta-callgraph.toml` | [golang/tools](https://github.com/golang/tools) | `call_graph_precision` | Go RTA call-graph recall oracle |

The full locked set is therefore: the three CI scale repos
(grafana / hugo / excalidraw), the two micro/recall oracle suites
(Jelly + Go x/tools RTA), and the local-only `devloupe` reference.

## Materializing a checkout

Each CI scale suite clones its pinned commit into its repo-relative
`checkout.path`. For example:

```sh
git clone https://github.com/grafana/grafana \
  research/evaluation-harness/repos/grafana-grafana
git -C research/evaluation-harness/repos/grafana-grafana \
  checkout b58701869e1a11b696010a6f28bd96b68a2cf0d0
```

Repeat with the pinned commit from the table for `gohugoio-hugo` and
`excalidraw-excalidraw`. For the local-only `devloupe` reference, set
`checkout.path` to your own absolute checkout (permitted because the manifest
opts into `local_clone_policy = "allow_absolute"`); it is never cloned in CI.
