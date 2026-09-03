#!/usr/bin/env bash
# Resolve and materialize the pinned public benchmark corpora.
#
# One code path for both `eval-gate.yml` and `bench-run.yml` so the two workflows
# cannot drift apart on which commit they measure, and so they share one
# `actions/cache` entry: the cache key is the sha256 of the pin table this script
# prints, which changes only when a manifest pin changes.
#
#   fetch-corpus.sh pins  --suites callgraph [--only a,b] --out PATH
#   fetch-corpus.sh fetch --suites callgraph [--only a,b] [--npm-jelly]
#
# `pins` writes the pin table (id, commit, url, checkout, digest) and, under
# GitHub Actions, appends `key=<sha256>` to $GITHUB_OUTPUT.
#
# `--npm-jelly` installs the Jelly `tests/helloworld` npm tree from the
# `package-lock.json` committed upstream. That case carries 342 of the suite's
# 1,479 expected edges and cannot resolve without it, so a run that skips this
# must label its recall as a lower bound. The install writes
# `<out>/npm-tree.json` describing what happened, and never fails the caller:
# a benchmark reports what it could measure.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
mode=${1:?usage: fetch-corpus.sh <pins|fetch> --suites <set> [--only ids] [--npm-jelly] [--out PATH]}
shift

suites=callgraph
only=""
out=""
npm_jelly=0
while [ $# -gt 0 ]; do
  case $1 in
    --suites) suites=$2; shift 2 ;;
    --only) only=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    --npm-jelly) npm_jelly=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

only_args=()
if [ -n "$only" ]; then
  only_args=(--only "$only")
fi

case $mode in
  pins)
    : "${out:?--out is required for the pins mode}"
    mkdir -p "$(dirname "$out")"
    python3 "$repo_root/scripts/fetch-scale-repos.py" \
      --repo-root "$repo_root" --suites "$suites" "${only_args[@]}" --print-pins > "$out"
    cat "$out"
    if [ -n "${GITHUB_OUTPUT:-}" ]; then
      echo "key=$(sha256sum "$out" | cut -d' ' -f1)" >> "$GITHUB_OUTPUT"
    fi
    ;;

  fetch)
    python3 "$repo_root/scripts/fetch-scale-repos.py" \
      --repo-root "$repo_root" --suites "$suites" "${only_args[@]}"

    if [ "$npm_jelly" = 1 ]; then
      helloworld="$repo_root/research/evaluation-harness/repos/jelly/tests/helloworld"
      status=absent
      detail="the Jelly checkout is not present, so the case was not installed"
      if [ -d "$helloworld" ]; then
        if ! command -v npm >/dev/null 2>&1; then
          status=failed
          detail="npm is not installed on this machine"
        elif npm --prefix "$helloworld" ci --no-audit --no-fund >/dev/null 2>&1; then
          status=installed
          modules=$(find "$helloworld/node_modules" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | wc -l | tr -d ' ')
          lock_sha=$(sha256sum "$helloworld/package-lock.json" 2>/dev/null | cut -d' ' -f1)
          detail="npm ci from the upstream package-lock.json (sha256 ${lock_sha:-unknown}); $modules top-level packages"
        else
          status=failed
          detail="npm ci failed in tests/helloworld; the 342-edge case cannot resolve"
        fi
      fi
      echo "jelly helloworld npm tree: $status ($detail)" >&2
      if [ -n "$out" ]; then
        mkdir -p "$(dirname "$out")"
        STATUS="$status" DETAIL="$detail" python3 -c '
import json, os, sys
json.dump({"status": os.environ["STATUS"], "detail": os.environ["DETAIL"]},
          open(sys.argv[1], "w"), indent=2)
' "$out"
      fi
    fi
    ;;

  *)
    echo "unknown mode: $mode (expected pins or fetch)" >&2
    exit 2
    ;;
esac
