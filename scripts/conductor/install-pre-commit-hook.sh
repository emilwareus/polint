#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${CONDUCTOR_WORKSPACE_PATH:-}" ]]; then
    echo "Conductor environment not detected; skipping pre-commit hook install."
    exit 0
fi

workspace_path="$(cd "${CONDUCTOR_WORKSPACE_PATH}" && pwd -P)"
current_path="$(pwd -P)"

if [[ "${current_path}" != "${workspace_path}" ]]; then
    echo "Expected to run from ${workspace_path}, got ${current_path}; skipping pre-commit hook install."
    exit 0
fi

hook_dir="${workspace_path}/scripts/conductor/git-hooks"

git config extensions.worktreeConfig true
git config --worktree core.hooksPath "${hook_dir}"

echo "Installed Conductor-only Git hooks from ${hook_dir}."
