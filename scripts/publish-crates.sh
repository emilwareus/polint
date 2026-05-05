#!/usr/bin/env bash
# Publish polint crates to crates.io in dependency order.
# Prerequisites: clean git tree (CI) or pass --allow-dirty only locally.
#
# Usage:
#   CRATES_IO_TOKEN=... ./scripts/publish-crates.sh
#   DRY_RUN=1 ./scripts/publish-crates.sh   # polint-diagnostics dry-run + verify ordered list
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PACKAGES=(
  polint-diagnostics
  polint-config
  polint-core
  polint-cache
  polint-graph
  polint-fs
  polint-go
  polint-ts
  polint-sdk
  polint-runner
  polint-cli
)

if [[ "${DRY_RUN:-}" == "1" || "${DRY_RUN:-}" == "true" ]]; then
  echo "DRY_RUN: smoke-check packaging for polint-diagnostics + list publish order."
  cargo publish -p polint-diagnostics --dry-run
  printf '\nPublish order (%d crates):\n' "${#PACKAGES[@]}"
  printf '  - %s\n' "${PACKAGES[@]}"
  exit 0
fi

if [[ -z "${CRATES_IO_TOKEN:-}" ]]; then
  echo "error: set CRATES_IO_TOKEN (create at https://crates.io/settings/tokens)" >&2
  exit 1
fi

last="${PACKAGES[$(( ${#PACKAGES[@]} - 1 ))]}"
for p in "${PACKAGES[@]}"; do
  echo "Publishing ${p}..."
  cargo publish -p "$p" --token "${CRATES_IO_TOKEN}"
  if [[ "$p" != "$last" ]]; then
    echo "Waiting 90s for crates.io index before the next crate..."
    sleep 90
  fi
done

echo "Done. Verify: https://crates.io/crates/polint-cli"
