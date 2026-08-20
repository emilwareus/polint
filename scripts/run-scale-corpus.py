#!/usr/bin/env python3
"""Fetch pinned scale repos and regenerate the committed scale-corpus artifact.

Runs the env-gated Rust regenerator that measures LOC, peak RSS, and cold/warm
wall-clock for each suite listed in `tests/golden-corpus/inputs.toml`. Writes
`research/evaluation-harness/baselines/scale-corpus-run.json`.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path


ARTIFACT_REL = "research/evaluation-harness/baselines/scale-corpus-run.json"
TEST_FILTER = "eval::bench::scale_corpus::tests::regenerate_scale_corpus_run"


def repo_root_from_script() -> Path:
    return Path(__file__).resolve().parent.parent


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=None,
        help="Repository root (defaults to parent of scripts/)",
    )
    parser.add_argument(
        "--skip-fetch",
        action="store_true",
        help="Do not run fetch-scale-repos.py first",
    )
    args = parser.parse_args()
    root = (args.repo_root or repo_root_from_script()).resolve()

    if not args.skip_fetch:
        fetch = subprocess.run(
            [sys.executable, str(root / "scripts/fetch-scale-repos.py"), "--repo-root", str(root)],
            cwd=root,
            check=False,
        )
        if fetch.returncode != 0:
            return fetch.returncode

    env = os.environ.copy()
    env["POLINT_WRITE_SCALE_CORPUS"] = "1"
    # Fail closed when a pin is absent: this target exists to publish numbers.
    env["POLINT_REQUIRE_BENCH_CORPUS"] = "1"
    # Isolated child re-execs this same test binary; release keeps numbers realistic.
    # Filter + --exact belong after `--` so cargo does not treat them as its own flags.
    cmd = [
        "cargo",
        "test",
        "--release",
        "-p",
        "polint",
        "--lib",
        "--locked",
        "--",
        TEST_FILTER,
        "--exact",
        "--nocapture",
    ]
    print("running:", " ".join(cmd), file=sys.stderr)
    measured = subprocess.run(cmd, cwd=root, env=env, check=False)
    if measured.returncode != 0:
        return measured.returncode

    artifact = root / ARTIFACT_REL
    if not artifact.is_file():
        print(f"missing artifact after measurement: {artifact}", file=sys.stderr)
        return 1
    print(f"ok wrote {artifact.relative_to(root)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
