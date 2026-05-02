# Quick Task 260502-dql Summary

**Task:** Remove README note that the repository is named exlint now that the repo will be renamed to polint
**Date:** 2026-05-02
**Status:** Complete
**Commit:** this commit

## Changes

- Removed the README sentence that said the repository was named `exlint` while the CLI/crates were named `polint`.
- Left the rest of the README branding as `polint`.

## Verification

- `rg -n 'This repository is named|repository remains `exlint`|repo as `exlint`|repo.*exlint|exlint.*repo' README.md` returned no matches.
