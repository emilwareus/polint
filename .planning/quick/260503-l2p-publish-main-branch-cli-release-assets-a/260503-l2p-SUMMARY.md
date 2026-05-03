# Quick Task 260503-l2p Summary

**Date:** 2026-05-03
**Status:** Complete

## Completed

- Added `.github/workflows/publish-cli.yml`, a `main`-only release workflow
  that builds Linux and macOS `polint` archives and publishes them to the
  `polint-main` GitHub release.
- Added `scripts/install.sh`, an authenticated private-repo installer that uses
  `gh release download`, verifies SHA-256 checksums, and installs `polint` into
  `~/.local/bin` by default.
- Updated the README installation section with the private one-liner, install
  directory override, and release-channel behavior.

## Verification

- `bash -n scripts/install.sh`
- `ruby -e 'require "yaml"; YAML.load_file(".github/workflows/publish-cli.yml"); puts "workflow yaml ok"'`
- `gh api --method GET -H "Accept: application/vnd.github.v3.raw+json" repos/emilwareus/exlint/contents/README.md -f ref=main`
- `git diff --check`
