#!/usr/bin/env bash
# Publish the public polint crates to crates.io.
# Prerequisites: clean git tree (CI) or pass --allow-dirty only locally.
#
# Usage:
#   CRATES_IO_TOKEN=... ./scripts/publish-crates.sh
#   DRY_RUN=1 ./scripts/publish-crates.sh   # cargo publish --dry-run/package smoke
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PACKAGES=(
  polint-macros
  polint
)

crate_version() {
  cargo pkgid -p "$1" | sed 's/.*#//'
}

crate_version_exists() {
  local name="$1"
  local version="$2"
  cargo search "$name" --limit 20 2>/dev/null | grep -q "^${name} = \"${version}\""
}

wait_for_crate_version() {
  local name="$1"
  local version="$2"

  for _ in {1..30}; do
    if crate_version_exists "$name" "$version"; then
      return 0
    fi
    sleep 10
  done

  echo "error: ${name} ${version} was not visible in the crates.io index after publishing" >&2
  return 1
}

if [[ "${DRY_RUN:-}" == "1" || "${DRY_RUN:-}" == "true" ]]; then
  echo "DRY_RUN: smoke-check packaging for ${PACKAGES[*]}."
  cargo publish -p polint-macros --dry-run --locked --allow-dirty

  macro_version="$(crate_version polint-macros)"
  if crate_version_exists polint-macros "$macro_version"; then
    cargo publish -p polint --dry-run --locked --allow-dirty
  else
    echo "DRY_RUN: polint depends on polint-macros ${macro_version}, which is not on crates.io yet."
    echo "DRY_RUN: checking polint package contents; full polint publish dry-run is possible after polint-macros is published."
    cargo package -p polint --list --locked --allow-dirty >/dev/null
  fi

  printf '\nPublish: %s\n' "${PACKAGES[*]}"
  exit 0
fi

if [[ -z "${CRATES_IO_TOKEN:-}" ]]; then
  echo "error: set CRATES_IO_TOKEN (create at https://crates.io/settings/tokens)" >&2
  exit 1
fi

for p in "${PACKAGES[@]}"; do
  echo "Publishing ${p}..."
  cargo publish -p "$p" --locked --token "${CRATES_IO_TOKEN}"
  if [[ "$p" == "polint-macros" ]]; then
    wait_for_crate_version "$p" "$(crate_version "$p")"
  fi
done

echo "Done. Verify: https://crates.io/crates/polint"
