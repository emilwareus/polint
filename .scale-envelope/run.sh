#!/usr/bin/env bash
# Timed, exclusive excalidraw full-pipeline measurement.
# usage: run.sh <label> [capabilities-csv]
set -uo pipefail
cd /workspace/polint-scale-envelope
LABEL="${1:?label}"
CAPS="${2:-}"
REPO=research/evaluation-harness/repos/gohugoio-hugo
OUT=.scale-envelope/runs
mkdir -p "$OUT"

# The libtest binary, not the CLI's dep artifact: only the former answers --list.
BIN="${POLINT_TEST_BIN:-}"
if [ -z "$BIN" ]; then
  for candidate in $(ls -t target/release/deps/polint-* 2>/dev/null | grep -vE '\.(d|so|rlib)$'); do
    if "$candidate" --list >/dev/null 2>&1; then BIN="$candidate"; break; fi
  done
fi
[ -n "$BIN" ] || { echo "no libtest binary found" >&2; exit 2; }
echo "binary: $BIN ($(stat -c %y "$BIN"))"

# Never time on a busy box, and never time a run that inherits a poisoned cache.
LOAD=$(awk '{print $1}' /proc/loadavg)
echo "loadavg: $LOAD"
rm -rf "$REPO/.polint/cache"

ENVS=(POLINT_PERF_CHILD_REPO="$REPO" RUST_LOG="${RUST_LOG:-polint::kernel::stage=info}")
if [ -n "${EXTRA_ENV:-}" ]; then for kv in $EXTRA_ENV; do ENVS+=("$kv"); done; fi
if [ -n "$CAPS" ]; then ENVS+=(POLINT_PERF_CHILD_CAPABILITIES="$CAPS"); fi
if [ -n "${COLD_ONLY:-}" ]; then ENVS+=(POLINT_PERF_CHILD_COLD_ONLY=1); fi

env "${ENVS[@]}" \
  python3 .scale-envelope/rssrun.py --label "$LABEL" \
    --timeline "$OUT/$LABEL.timeline.json" \
    --as-limit-gb "${AS_LIMIT_GB:-11}" --timeout "${TIMEOUT:-2400}" \
    -- "$BIN" --exact eval::bench::runner::tests::perf_child_measure_entry \
       --nocapture --test-threads=1 \
  > "$OUT/$LABEL.stdout" 2> "$OUT/$LABEL.stderr"
RC=$?
echo "exit=$RC"
tail -2 "$OUT/$LABEL.stderr"
grep -o '<<<POLINT_PERF_POINT_BEGIN>>>.*<<<POLINT_PERF_POINT_END>>>' "$OUT/$LABEL.stdout" 2>/dev/null \
  | sed 's/<<<POLINT_PERF_POINT_BEGIN>>>//; s/<<<POLINT_PERF_POINT_END>>>//' \
  | python3 -m json.tool 2>/dev/null | head -30
