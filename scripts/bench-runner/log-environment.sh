#!/usr/bin/env bash
# Capture the hardware and software power of the machine a benchmark ran on.
#
# Every number a benchmark produces is only comparable against another number
# taken on a comparable machine, so the runner records the machine beside the
# results. Writes three views of the same facts into the output directory:
#
#   environment.json  machine-readable, for comparing runs over time
#   environment.txt   the raw command output the JSON was distilled from
#   environment.md    a short markdown table for the job summary
#
# Usage: scripts/bench-runner/log-environment.sh <output-dir> [repo-root]
#
# Reusable by any workflow: on GitHub Actions it additionally records the
# runner OS/arch, the hosted image name, and the run URL.
set -euo pipefail

out_dir=${1:?usage: log-environment.sh <output-dir> [repo-root]}
repo_root=${2:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}
mkdir -p "$out_dir"

# Never let a missing optional tool abort the log: absent means "not installed".
version_of() {
  local binary=$1
  shift
  if command -v "$binary" >/dev/null 2>&1; then
    "$binary" "$@" 2>/dev/null | head -n 1
  else
    echo "absent"
  fi
}

export ENV_CAPTURED_AT_UTC
ENV_CAPTURED_AT_UTC=$(date -u +%Y-%m-%dT%H:%M:%SZ)

# ---- hardware -------------------------------------------------------------
export ENV_CPU_MODEL ENV_CPU_LOGICAL_CORES ENV_CPU_PHYSICAL_CORES ENV_CPU_MAX_MHZ
ENV_CPU_MODEL=$(awk -F': ' '/^model name/ {print $2; exit}' /proc/cpuinfo 2>/dev/null \
  || sysctl -n machdep.cpu.brand_string 2>/dev/null \
  || echo unknown)
ENV_CPU_LOGICAL_CORES=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo unknown)
ENV_CPU_PHYSICAL_CORES=$(awk -F': ' '/^core id/ {ids[$2]=1} END {n=0; for (id in ids) n++; print (n ? n : "unknown")}' \
  /proc/cpuinfo 2>/dev/null || echo unknown)
ENV_CPU_MAX_MHZ=$(awk -F': ' '/^cpu MHz/ {if ($2+0 > max) max=$2+0} END {printf "%.0f", max}' \
  /proc/cpuinfo 2>/dev/null || echo unknown)

export ENV_MEM_TOTAL_KB ENV_MEM_AVAILABLE_KB
ENV_MEM_TOTAL_KB=$(awk '/^MemTotal:/ {print $2; exit}' /proc/meminfo 2>/dev/null || echo unknown)
ENV_MEM_AVAILABLE_KB=$(awk '/^MemAvailable:/ {print $2; exit}' /proc/meminfo 2>/dev/null || echo unknown)

export ENV_DISK_FREE_REPO ENV_DISK_FREE_ROOT ENV_DISK_SIZE_REPO
ENV_DISK_FREE_REPO=$(df -Pk "$repo_root" 2>/dev/null | awk 'NR==2 {print $4}' || echo unknown)
ENV_DISK_SIZE_REPO=$(df -Pk "$repo_root" 2>/dev/null | awk 'NR==2 {print $2}' || echo unknown)
ENV_DISK_FREE_ROOT=$(df -Pk / 2>/dev/null | awk 'NR==2 {print $4}' || echo unknown)

# ---- operating system -----------------------------------------------------
export ENV_OS_KERNEL ENV_OS_RELEASE ENV_OS_ARCH ENV_CONTAINER
ENV_OS_KERNEL=$(uname -sr 2>/dev/null || echo unknown)
ENV_OS_ARCH=$(uname -m 2>/dev/null || echo unknown)
ENV_OS_RELEASE=$( { . /etc/os-release 2>/dev/null && echo "$PRETTY_NAME"; } || sw_vers -productVersion 2>/dev/null || echo unknown)
if [ -f /.dockerenv ] || grep -qE '(docker|containerd|kubepods)' /proc/1/cgroup 2>/dev/null; then
  ENV_CONTAINER=yes
else
  ENV_CONTAINER=no
fi

# ---- toolchains -----------------------------------------------------------
export ENV_RUSTC_VERSION ENV_CARGO_VERSION ENV_GO_VERSION ENV_NODE_VERSION
export ENV_NPM_VERSION ENV_PYTHON_VERSION ENV_GIT_VERSION
ENV_RUSTC_VERSION=$(version_of rustc --version)
ENV_CARGO_VERSION=$(version_of cargo --version)
ENV_GO_VERSION=$(version_of go version)
ENV_NODE_VERSION=$(version_of node --version)
ENV_NPM_VERSION=$(version_of npm --version)
ENV_PYTHON_VERSION=$(version_of python3 --version)
ENV_GIT_VERSION=$(version_of git --version)

# ---- polint under test ----------------------------------------------------
export ENV_POLINT_COMMIT ENV_POLINT_COMMIT_SHORT ENV_POLINT_BRANCH ENV_POLINT_DIRTY
export ENV_POLINT_CRATE_VERSION ENV_POLINT_BIN_VERSION
ENV_POLINT_COMMIT=$(git -C "$repo_root" rev-parse HEAD 2>/dev/null || echo unknown)
ENV_POLINT_COMMIT_SHORT=$(git -C "$repo_root" rev-parse --short HEAD 2>/dev/null || echo unknown)
ENV_POLINT_BRANCH=$(git -C "$repo_root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)
if [ -n "$(git -C "$repo_root" status --porcelain 2>/dev/null)" ]; then
  ENV_POLINT_DIRTY=yes
else
  ENV_POLINT_DIRTY=no
fi
# Version fields straight from cargo metadata, not from a hand-copied constant.
ENV_POLINT_CRATE_VERSION=$(cd "$repo_root" && cargo metadata --no-deps --format-version 1 2>/dev/null \
  | python3 -c 'import json,sys
try:
    packages = json.load(sys.stdin)["packages"]
except Exception:
    print("unknown"); raise SystemExit(0)
print(next((p["version"] for p in packages if p["name"] == "polint"), "unknown"))' || echo unknown)
polint_bin=${POLINT_BIN:-$repo_root/target/release/polint}
if [ -x "$polint_bin" ]; then
  ENV_POLINT_BIN_VERSION=$("$polint_bin" --version 2>/dev/null | head -n 1)
else
  ENV_POLINT_BIN_VERSION="not built"
fi

# ---- CI runner ------------------------------------------------------------
export ENV_CI ENV_RUNNER_OS ENV_RUNNER_ARCH ENV_RUNNER_NAME ENV_RUNNER_IMAGE
export ENV_GITHUB_WORKFLOW ENV_GITHUB_RUN_URL
if [ -n "${GITHUB_ACTIONS:-}" ]; then
  ENV_CI="github-actions"
  ENV_RUNNER_OS=${RUNNER_OS:-unknown}
  ENV_RUNNER_ARCH=${RUNNER_ARCH:-unknown}
  ENV_RUNNER_NAME=${RUNNER_NAME:-unknown}
  ENV_RUNNER_IMAGE="${ImageOS:-unknown}/${ImageVersion:-unknown}"
  ENV_GITHUB_WORKFLOW=${GITHUB_WORKFLOW:-unknown}
  ENV_GITHUB_RUN_URL="${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-}/actions/runs/${GITHUB_RUN_ID:-}"
else
  ENV_CI="local"
  ENV_RUNNER_OS=""
  ENV_RUNNER_ARCH=""
  ENV_RUNNER_NAME=""
  ENV_RUNNER_IMAGE=""
  ENV_GITHUB_WORKFLOW=""
  ENV_GITHUB_RUN_URL=""
fi

# ---- raw view -------------------------------------------------------------
{
  echo "captured_at_utc: $ENV_CAPTURED_AT_UTC"
  echo
  echo "== uname -a =="; uname -a 2>&1 || true
  echo
  echo "== /etc/os-release =="; cat /etc/os-release 2>/dev/null || echo "(none)"
  echo
  echo "== cpu =="; lscpu 2>/dev/null || grep -E 'model name|^processor' /proc/cpuinfo 2>/dev/null || echo "(none)"
  echo
  echo "== memory =="; free -h 2>/dev/null || cat /proc/meminfo 2>/dev/null | head -5 || echo "(none)"
  echo
  echo "== disk =="; df -h 2>/dev/null || echo "(none)"
  echo
  echo "== toolchains =="
  echo "rustc:  $ENV_RUSTC_VERSION"
  echo "cargo:  $ENV_CARGO_VERSION"
  echo "go:     $ENV_GO_VERSION"
  echo "node:   $ENV_NODE_VERSION"
  echo "npm:    $ENV_NPM_VERSION"
  echo "python: $ENV_PYTHON_VERSION"
  echo "git:    $ENV_GIT_VERSION"
  echo
  echo "== polint under test =="
  echo "commit:        $ENV_POLINT_COMMIT"
  echo "branch:        $ENV_POLINT_BRANCH"
  echo "worktree dirty: $ENV_POLINT_DIRTY"
  echo "crate version: $ENV_POLINT_CRATE_VERSION"
  echo "binary:        $ENV_POLINT_BIN_VERSION"
  echo
  echo "== runner =="
  echo "context: $ENV_CI"
  echo "runner:  ${ENV_RUNNER_OS:-n/a}/${ENV_RUNNER_ARCH:-n/a} image ${ENV_RUNNER_IMAGE:-n/a}"
} > "$out_dir/environment.txt" 2>&1

# ---- json + markdown ------------------------------------------------------
OUT_DIR="$out_dir" python3 <<'PY'
import json
import os

env = os.environ


def number(name):
    raw = env.get(name, "unknown")
    try:
        return int(raw)
    except (TypeError, ValueError):
        return None


def gib(kb):
    return None if kb is None else round(kb / 1024 / 1024, 2)


record = {
    "schema_version": "polint-bench-environment-1",
    "captured_at_utc": env["ENV_CAPTURED_AT_UTC"],
    "context": env["ENV_CI"],
    "hardware": {
        "cpu_model": env["ENV_CPU_MODEL"],
        "cpu_logical_cores": number("ENV_CPU_LOGICAL_CORES"),
        "cpu_physical_cores": number("ENV_CPU_PHYSICAL_CORES"),
        "cpu_max_mhz": number("ENV_CPU_MAX_MHZ"),
        "mem_total_gib": gib(number("ENV_MEM_TOTAL_KB")),
        "mem_available_gib": gib(number("ENV_MEM_AVAILABLE_KB")),
        "disk_size_repo_gib": gib(number("ENV_DISK_SIZE_REPO")),
        "disk_free_repo_gib": gib(number("ENV_DISK_FREE_REPO")),
        "disk_free_root_gib": gib(number("ENV_DISK_FREE_ROOT")),
        "arch": env["ENV_OS_ARCH"],
    },
    "os": {
        "kernel": env["ENV_OS_KERNEL"],
        "release": env["ENV_OS_RELEASE"],
        "containerized": env["ENV_CONTAINER"] == "yes",
    },
    "toolchains": {
        "rustc": env["ENV_RUSTC_VERSION"],
        "cargo": env["ENV_CARGO_VERSION"],
        "go": env["ENV_GO_VERSION"],
        "node": env["ENV_NODE_VERSION"],
        "npm": env["ENV_NPM_VERSION"],
        "python": env["ENV_PYTHON_VERSION"],
        "git": env["ENV_GIT_VERSION"],
    },
    "polint_under_test": {
        "commit": env["ENV_POLINT_COMMIT"],
        "commit_short": env["ENV_POLINT_COMMIT_SHORT"],
        "branch": env["ENV_POLINT_BRANCH"],
        "worktree_dirty": env["ENV_POLINT_DIRTY"] == "yes",
        "crate_version": env["ENV_POLINT_CRATE_VERSION"],
        "binary_version": env["ENV_POLINT_BIN_VERSION"],
    },
    "runner": {
        "os": env["ENV_RUNNER_OS"] or None,
        "arch": env["ENV_RUNNER_ARCH"] or None,
        "name": env["ENV_RUNNER_NAME"] or None,
        "image": env["ENV_RUNNER_IMAGE"] or None,
        "workflow": env["ENV_GITHUB_WORKFLOW"] or None,
        "run_url": env["ENV_GITHUB_RUN_URL"] or None,
    },
}

out_dir = env["OUT_DIR"]
with open(os.path.join(out_dir, "environment.json"), "w", encoding="utf-8") as handle:
    json.dump(record, handle, indent=2, sort_keys=True)
    handle.write("\n")

hardware = record["hardware"]
polint = record["polint_under_test"]
rows = [
    ("CPU", f"{hardware['cpu_model']} ({hardware['cpu_logical_cores']} logical cores)"),
    ("RAM", f"{hardware['mem_total_gib']} GiB total, {hardware['mem_available_gib']} GiB available"),
    ("Disk free", f"{hardware['disk_free_repo_gib']} GiB (repo filesystem)"),
    ("Kernel", record["os"]["kernel"]),
    ("OS", record["os"]["release"]),
    ("Container", "yes" if record["os"]["containerized"] else "no"),
    ("rustc / cargo", f"{record['toolchains']['rustc']} / {record['toolchains']['cargo']}"),
    ("Go", record["toolchains"]["go"]),
    ("Node / npm", f"{record['toolchains']['node']} / {record['toolchains']['npm']}"),
    ("polint commit", f"`{polint['commit_short']}` on `{polint['branch']}`"
                      + (" (dirty worktree)" if polint["worktree_dirty"] else "")),
    ("polint version", f"{polint['crate_version']} (binary: {polint['binary_version']})"),
    ("Context", record["context"]),
]
if record["runner"]["os"]:
    runner = record["runner"]
    rows.append(("Runner", f"{runner['os']}/{runner['arch']}, image {runner['image']}"))

lines = ["| field | value |", "| --- | --- |"]
lines += [f"| {name} | {value} |" for name, value in rows]
with open(os.path.join(out_dir, "environment.md"), "w", encoding="utf-8") as handle:
    handle.write("\n".join(lines) + "\n")
PY

echo "environment logged to $out_dir/environment.{json,txt,md}" >&2
