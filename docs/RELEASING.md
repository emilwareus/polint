# Releasing

polint ships from a single workflow on `main`.

## Workflows (`.github/workflows/`)

| Workflow | Trigger | Purpose |
|---|---|---|
| `ci.yml` | Push/PR to `main` | `rustfmt`, then `clippy -D warnings` + `cargo test --workspace` on Ubuntu, Windows, and macOS. Includes an ignored `cargo install` smoke test that mirrors the crates.io install path. |
| `release.yml` | Manual (`workflow_dispatch` on `main`) | Patch-bump via `scripts/bump-workspace-version.py`, push the bump commit to `main`, create the annotated tag `vX.Y.Z`, then optionally publish all crates to crates.io and attach CLI archives to the matching GitHub Release. |

## Secrets

| Secret | Required for | Notes |
|---|---|---|
| _(none)_ | `ci.yml` | Uses the default `GITHUB_TOKEN`. |
| _(none)_ | `release.yml` (typical) | `GITHUB_TOKEN` can push tags and manage releases when branch protection allows it. |
| `WORKFLOW_PUSH_TOKEN` | `release.yml` when `main` is protected | PAT with `contents: write` and the right to push to protected `main`. |
| `CRATES_IO_TOKEN` | `release.yml` with **Publish crates** on | Publish-scoped token from <https://crates.io/settings/tokens>. |

## Ship a version

1. Open **Actions → Release → Run workflow** on `main`.
2. Leave **Publish crates** and **Build CLI** on (defaults) for a full release; turn either off for a partial release.
3. The workflow:
   - bumps the workspace patch version,
   - commits and pushes to `main`,
   - creates and pushes the annotated tag `vX.Y.Z`,
   - publishes the `polint` crate to crates.io (when enabled),
   - builds the cross-platform CLI matrix and uploads archives to the GitHub Release for that tag (when enabled).

## Smoke-test crates publish locally

```bash
DRY_RUN=1 ./scripts/publish-crates.sh
```

This walks the ordered publish without uploading anything.

## Manual bump (only outside the workflow)

```bash
python3 scripts/bump-workspace-version.py
cargo build --workspace
git commit -am "chore(release): bump crate version to <new>"
```

Then either run **Release** to take it from there, or push a tag yourself.
