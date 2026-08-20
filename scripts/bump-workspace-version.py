#!/usr/bin/env python3
"""Bump the version in the root workspace Cargo.toml.

Defaults to a patch bump. Pass `--minor` or `--major` when a release changes the
public SDK in a way that existing rule packs cannot absorb: pre-1.0, a caret
requirement like `polint = "0.1.17"` matches every later 0.1.z, so a breaking
change shipped as a patch reaches existing users on their next `cargo update`.

Updates [workspace.package] version, then rewrites every internal path dependency
that carries a version requirement — in the root manifest and in each member
manifest — to the new version. Those pins must move together: a member published
at 0.2.0 does not satisfy a sibling's `^0.1.x` requirement, so leaving one behind
breaks `cargo build` immediately after the bump. Prints the new version on stdout.

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

    cargo_path.write_text(text, encoding="utf-8")

    # Rewrite every internal path dependency that also carries a version requirement.
    # Matched on the path+version shape rather than on the current version, because a
    # pin is allowed to lag while it still resolves (`^0.1.7` accepted 0.1.17) and would
    # otherwise be skipped here and then fail to resolve after a minor bump.
    internal_dep = re.compile(
        r'(?m)^(?P<head>[A-Za-z0-9_-]+ = \{[^}\n]*?path = "[^"\n]+"[^}\n]*?version = ")'
        r'(?P<version>[^"\n]+)'
        r'(?P<tail>")'
    )

    manifests = [cargo_path] + sorted((root / "crates").glob("*/Cargo.toml"))
    updated: list[str] = []
    for manifest in manifests:
        body = manifest.read_text(encoding="utf-8")
        new_body, count = internal_dep.subn(
            lambda m: f"{m.group('head')}{new_ver}{m.group('tail')}", body
        )
        if count:
            manifest.write_text(new_body, encoding="utf-8")
            updated.append(f"{manifest.relative_to(root)}({count})")

    if not updated:
        print(
            "error: found no internal path+version dependency to update; the release "
            "would bump the workspace version and leave sibling pins behind",
            file=sys.stderr,
        )
        sys.exit(1)

    print(f"updated internal pins: {', '.join(updated)}", file=sys.stderr)
    print(new_ver, end="")


if __name__ == "__main__":
    main()
