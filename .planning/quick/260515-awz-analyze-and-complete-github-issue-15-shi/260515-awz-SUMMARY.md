# Summary

Completed GitHub issue #15 by adding an official composite action and the CLI
output mode the action uses by default.

## Changes

- Added root `action.yml` with release-asset install, fallback `cargo install`,
  `.polint/cache` restore/save, configurable args, working directory, cache key
  prefix, and `fail-on` convenience input.
- Added release automation to move the stable `v1` action tag to the reviewed
  release commit when `publish_action` is enabled.
- Added `--format github` to the CLI and repo-local rule runner, rendering
  GitHub Actions `error`, `warning`, and `notice` annotations.
- Documented action usage, cache keys, cold-run limits, same-repo publishing,
  and major tag versioning.
- Added integration coverage proving a repo-local rule host can emit GitHub
  annotations.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p polint diagnostics::tests::render_github_uses_workflow_annotations --locked`
- `cargo test -p polint check_with_local_rule_host_can_emit_github_annotations --locked`
- `cargo clippy -p polint --all-targets --locked -- -D warnings`
- Ruby YAML parse and metadata assertions for `action.yml` and `release.yml`
