#!/usr/bin/env python3
"""Bump the patch version in the root workspace Cargo.toml.

Updates:
  - [workspace.package] version
  - each `polint-* = { path = ..., version = "..." }` entry to match

Prints the new version on stdout (for CI). Exits with error if parsing fails.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


def bump_patch(version: str) -> str:
    parts = version.strip().split(".")
    if len(parts) != 3 or not all(p.isdigit() for p in parts):
        raise ValueError(f"expected VERSION like 0.1.0, got {version!r}")
    major, minor, patch = (int(parts[0]), int(parts[1]), int(parts[2]))
    return f"{major}.{minor}.{patch + 1}"


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    cargo_path = root / "Cargo.toml"
    text = cargo_path.read_text(encoding="utf-8")

    m = re.search(
        r"(?ms)^\[workspace\.package\].*?^version = \"([^\"]+)\"",
        text,
    )
    if not m:
        print("error: could not find [workspace.package] version", file=sys.stderr)
        sys.exit(1)
    current = m.group(1)
    new_ver = bump_patch(current)
    old_re = re.escape(current)

    def replace_workspace_package_block(match: re.Match[str]) -> str:
        block = match.group(0)
        return re.sub(
            r"^version = \"[^\"]+\"",
            f'version = "{new_ver}"',
            block,
            count=1,
            flags=re.MULTILINE,
        )

    text, n = re.subn(
        r"(?ms)^\[workspace\.package\].*?(?=^\[|\Z)",
        replace_workspace_package_block,
        text,
        count=1,
    )
    if n != 1:
        print("error: failed to replace [workspace.package] block", file=sys.stderr)
        sys.exit(1)

    text, n = re.subn(
        rf'^(polint-\w+ = {{ path = "[^"]+", version = "){old_re}("\s*\}})',
        rf"\g<1>{new_ver}\2",
        text,
        flags=re.MULTILINE,
    )
    if n == 0:
        print("error: no polint-* workspace dependency versions updated", file=sys.stderr)
        sys.exit(1)

    cargo_path.write_text(text, encoding="utf-8")
    print(new_ver, end="")


if __name__ == "__main__":
    main()
