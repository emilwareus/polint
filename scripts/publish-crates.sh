#!/usr/bin/env bash
# Publish the unified `polint` crate to crates.io.
# Prerequisites: clean git tree (CI) or pass --allow-dirty only locally.
#
# Usage:
#   CRATES_IO_TOKEN=... ./scripts/publish-crates.sh
#   DRY_RUN=1 ./scripts/publish-crates.sh   # cargo publish --dry-run for polint
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PACKAGES=(
  polint
)

if [[ "${DRY_RUN:-}" == "1" || "${DRY_RUN:-}" == "true" ]]; then
  echo "DRY_RUN: smoke-check packaging for polint."
  cargo publish -p polint --dry-run --locked --allow-dirty
  printf '\nPublish: %s\n' "${PACKAGES[*]}"
  exit 0
fi

if [[ -z "${CRATES_IO_TOKEN:-}" ]]; then
  echo "error: set CRATES_IO_TOKEN (create at https://crates.io/settings/tokens)" >&2
  exit 1
fi

for p in "${PACKAGES[@]}"; do
  echo "Publishing ${p}..."
  cargo publish -p "$p" --token "${CRATES_IO_TOKEN}"
done

echo "Done. Verify: https://crates.io/crates/polint"
