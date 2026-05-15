# Quick Task 260515-awz: GitHub Action with caching

## Goal

Complete GitHub issue #15 by making polint usable from GitHub Actions through one
`uses:` step, with default cache restore/save behavior and clear documentation.

## Research Notes

- Issue #14 is closed and the cache layout now documents `.polint/cache` with
  `analysis`, `rules-target`, and `derived` categories.
- GitHub composite actions can package shell steps and other `uses:` steps,
  which is enough for install + cache restore/save + run behavior.
- GitHub annotation workflow commands use `::error`, `::warning`, and
  `::notice` with file/line/column metadata, so the issue's proposed
  `--format github` needs CLI support before the action can use that default.

## Tasks

1. Add a GitHub Actions diagnostic output format to the CLI and runner.
2. Add a composite `action.yml` that installs polint, restores `.polint/cache`,
   runs user-provided args, saves cache on completion, and preserves the polint
   exit code.
3. Add release automation that publishes the action by moving the stable `v1`
   action tag to the reviewed release commit.
4. Document the action, cache behavior, versioning, and cold-run caveats.
5. Run focused tests and lightweight metadata validation.
