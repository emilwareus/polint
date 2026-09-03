#!/usr/bin/env python3
"""Clone the golden-corpus benchmark repositories at their pinned commits.

Pins are read from the suite manifests listed in
`tests/golden-corpus/inputs.toml`: `scale_suite_manifests` by default, and
`callgraph_suite_manifests` (the external graph-accuracy corpus) under
`--suites callgraph`. Checkouts land under the gitignored
`research/evaluation-harness/repos/` paths declared in those manifests.
Floating branch or tag tips are rejected: only an exact `source_commit` SHA
is checked out.

`--only` narrows a suite set to named suite ids without changing which list is
read, so a hosted runner can take the two scale repositories it fits and leave
the one it does not.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
import tomllib
from pathlib import Path


FULL_SHA_LEN = 40
INPUTS_REL = "tests/golden-corpus/inputs.toml"

# Suite set name -> the inputs.toml key that lists its suite manifests.
MANIFEST_LISTS = {
    "scale": "scale_suite_manifests",
    "callgraph": "callgraph_suite_manifests",
}


def repo_root_from_script() -> Path:
    return Path(__file__).resolve().parent.parent


def load_inputs(root: Path) -> dict:
    path = root / INPUTS_REL
    with path.open("rb") as handle:
        data = tomllib.load(handle)
    if data.get("schema_version") != "polint-golden-corpus-inputs-1":
        raise SystemExit(f"unsupported golden-corpus schema in {path}")
    return data


def select_manifests(root: Path, inputs: dict, suites: str) -> list[str]:
    path = root / INPUTS_REL
    keys = list(MANIFEST_LISTS.values()) if suites == "all" else [MANIFEST_LISTS[suites]]
    manifests: list[str] = []
    for key in keys:
        listed = inputs.get(key)
        if not isinstance(listed, list) or not listed:
            raise SystemExit(f"{path} must declare a non-empty {key} list")
        manifests.extend(listed)
    return manifests


def load_suite_pin(root: Path, relative_manifest: str) -> dict[str, str]:
    path = root / relative_manifest
    with path.open("rb") as handle:
        suite = tomllib.load(handle)
    source_url = suite.get("source_url")
    source_commit = suite.get("source_commit")
    checkout = suite.get("checkout") or {}
    checkout_path = checkout.get("path")
    suite_id = suite.get("id")
    if not isinstance(source_url, str) or not source_url:
        raise SystemExit(f"{path}: missing source_url")
    if not isinstance(source_commit, str) or len(source_commit) != FULL_SHA_LEN:
        raise SystemExit(
            f"{path}: source_commit must be a full {FULL_SHA_LEN}-char SHA, got {source_commit!r}"
        )
    if not all(ch in "0123456789abcdef" for ch in source_commit.lower()):
        raise SystemExit(f"{path}: source_commit is not a hex SHA: {source_commit!r}")
    if not isinstance(checkout_path, str) or not checkout_path:
        raise SystemExit(f"{path}: missing checkout.path")
    if Path(checkout_path).is_absolute():
        raise SystemExit(f"{path}: checkout.path must be repo-relative")
    if not isinstance(suite_id, str) or not suite_id:
        raise SystemExit(f"{path}: missing id")
    return {
        "id": suite_id,
        "manifest": relative_manifest,
        "source_url": source_url,
        "source_commit": source_commit.lower(),
        "checkout_path": checkout_path,
    }


def run_git(args: list[str], *, cwd: Path | None = None) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=cwd,
        check=True,
        text=True,
        capture_output=True,
    )
    return completed.stdout.strip()


def ensure_checkout(root: Path, pin: dict[str, str], *, dry_run: bool) -> None:
    dest = root / pin["checkout_path"]
    commit = pin["source_commit"]
    url = pin["source_url"]

    if dry_run:
        print(f"{pin['id']}\t{commit}\t{url}\t{pin['checkout_path']}")
        return

    dest.parent.mkdir(parents=True, exist_ok=True)

    if dest.exists() and not (dest / ".git").exists():
        raise SystemExit(f"{dest} exists but is not a git checkout; remove it and retry")

    if not dest.exists():
        print(f"cloning {url} -> {dest}", file=sys.stderr)
        run_git(["clone", "--no-checkout", url, str(dest)])
    else:
        print(f"fetching {dest}", file=sys.stderr)
        run_git(["remote", "set-url", "origin", url], cwd=dest)
        run_git(["fetch", "--tags", "origin"], cwd=dest)

    # Pins may be commit SHAs or annotated-tag object SHAs (excalidraw v0.17.6).
    # Always check out the peeled commit so HEAD is a commit object while still
    # verifying the manifest pin resolves to that same commit.
    try:
        peeled = run_git(["rev-parse", f"{commit}^{{commit}}"], cwd=dest).lower()
    except subprocess.CalledProcessError as err:
        raise SystemExit(
            f"{dest}: cannot resolve pin {commit} to a commit: {err.stderr}"
        ) from err
    run_git(["checkout", "--force", peeled], cwd=dest)
    head = run_git(["rev-parse", "HEAD"], cwd=dest).lower()
    if head != peeled:
        raise SystemExit(f"{dest}: HEAD {head} != peeled pin {peeled} (from {commit})")
    if peeled == commit:
        print(f"ok {pin['id']} @ {head}", file=sys.stderr)
    else:
        print(
            f"ok {pin['id']} @ {head} (peeled from annotated tag {commit})",
            file=sys.stderr,
        )


def print_pins_stable(pins: list[dict[str, str]]) -> None:
    """Machine-readable pin list for the inventory gate test."""
    for pin in pins:
        digest = hashlib.sha256(
            f"{pin['id']}|{pin['source_commit']}|{pin['source_url']}|{pin['checkout_path']}".encode()
        ).hexdigest()[:16]
        print(
            f"{pin['id']}\t{pin['source_commit']}\t{pin['source_url']}\t{pin['checkout_path']}\t{digest}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=None,
        help="Repository root (defaults to parent of scripts/)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print pins without cloning",
    )
    parser.add_argument(
        "--print-pins",
        action="store_true",
        help="Print stable pin rows (implies --dry-run)",
    )
    parser.add_argument(
        "--suites",
        choices=[*MANIFEST_LISTS, "all"],
        default="scale",
        help="Which suite list to fetch (default: scale)",
    )
    parser.add_argument(
        "--only",
        default="",
        help=(
            "Comma-separated suite ids to keep from the selected list; "
            "an id that is not in the list is an error, never a silent no-op"
        ),
    )
    args = parser.parse_args()
    root = (args.repo_root or repo_root_from_script()).resolve()
    inputs = load_inputs(root)
    pins = [
        load_suite_pin(root, rel) for rel in select_manifests(root, inputs, args.suites)
    ]
    pins.sort(key=lambda pin: pin["id"])

    wanted = [name for name in args.only.split(",") if name]
    if wanted:
        known = {pin["id"] for pin in pins}
        unknown = [name for name in wanted if name not in known]
        if unknown:
            raise SystemExit(
                f"--only names suite id(s) not in --suites {args.suites}: "
                f"{', '.join(unknown)} (known: {', '.join(sorted(known))})"
            )
        pins = [pin for pin in pins if pin["id"] in wanted]

    if args.print_pins:
        print_pins_stable(pins)
        return 0

    for pin in pins:
        ensure_checkout(root, pin, dry_run=args.dry_run)
    return 0


if __name__ == "__main__":
    sys.exit(main())
