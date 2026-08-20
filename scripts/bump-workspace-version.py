#!/usr/bin/env python3
"""Bump the version in the root workspace Cargo.toml.

Defaults to a patch bump. Pass `--minor` or `--major` when a release changes the
public SDK in a way that existing rule packs cannot absorb: pre-1.0, a caret
requirement like `polint = "0.1.17"` matches every later 0.1.z, so a breaking
change shipped as a patch reaches existing users on their next `cargo update`.

Updates [workspace.package] version and the `polint = { path = ..., version = ... }`
entry under [workspace.dependencies]. Prints the new version on stdout.

Used by `.github/workflows/release.yml`; also safe to run locally, then
`cargo build --workspace` and commit Cargo.toml + Cargo.lock.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


def bump(version: str, level: str = "patch") -> str:
    parts = version.strip().split(".")
    if len(parts) != 3 or not all(p.isdigit() for p in parts):
        raise ValueError(f"expected VERSION like 0.1.0, got {version!r}")
    major, minor, patch = (int(parts[0]), int(parts[1]), int(parts[2]))
    if level == "major":
        return f"{major + 1}.0.0"
    if level == "minor":
        return f"{major}.{minor + 1}.0"
    return f"{major}.{minor}.{patch + 1}"


def main() -> None:
    level = "patch"
    for arg in sys.argv[1:]:
        if arg in ("--minor", "--major", "--patch"):
            level = arg.lstrip("-")
        else:
            print(f"error: unknown argument {arg!r}", file=sys.stderr)
            sys.exit(2)

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
    new_ver = bump(current, level)
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
        rf'^(polint = {{ path = "crates/polint", version = "){old_re}("\s*\}})',
        rf"\g<1>{new_ver}\2",
        text,
        flags=re.MULTILINE,
    )
    if n != 1:
        print(
            "error: expected exactly one `polint` workspace dependency version to update",
            file=sys.stderr,
        )
        sys.exit(1)

    cargo_path.write_text(text, encoding="utf-8")
    print(new_ver, end="")


if __name__ == "__main__":
    main()
