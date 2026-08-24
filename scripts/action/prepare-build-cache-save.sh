#!/usr/bin/env bash
# Decide whether the rule-host build cache may be saved, and prune it first.
#
# Two things have to be true before compiler output is allowed into a cache
# entry that later runs will restore:
#
#   1. The run that produced it finished. A failed or cancelled run can leave a
#      torn target directory, and `actions/cache` never overwrites a key once it
#      exists, so one bad save would be permanent for that key.
#   2. Nothing built from repo-local rule sources is in it. Cargo's own
#      fingerprints already refuse to reuse a unit whose sources moved, but the
#      cache must not depend on that: removing the rule packages' own outputs
#      and fingerprints before saving means every restore starts without a rule
#      host and Cargo has to rebuild it from the sources in that checkout, while
#      the dependency units - the expensive part - stay reusable.
#
# If either cannot be established, this reports `save=false`. Skipping a save
# only costs the next run its speedup; it never blocks polint.
#
# Inputs (environment):
#   POLINT_ACTION_EXIT_CODE               polint's exit code, empty if it never ran
#   POLINT_ACTION_RULE_PACKAGES_FILE      newline-separated rule package directories
#   POLINT_ACTION_EXTENSION_PACKAGES_FILE newline-separated extension package directories
#   POLINT_ACTION_RULES_PROFILE           dev, release, or a named cargo profile
#   POLINT_ACTION_MAX_SIZE_MB             optional ceiling for the saved directories
#
# Outputs: save, save-skipped, size-mb

set -uo pipefail

output_buffer="$(mktemp 2>/dev/null)"
[[ -n "${output_buffer}" ]] || output_buffer="${GITHUB_OUTPUT:-/dev/null}"

flush() {
    if [[ "${output_buffer}" != "${GITHUB_OUTPUT:-/dev/null}" && -f "${output_buffer}" ]]; then
        [[ -n "${GITHUB_OUTPUT:-}" ]] && cat "${output_buffer}" >> "${GITHUB_OUTPUT}"
        rm -f "${output_buffer}"
    fi
    return 0
}
trap flush EXIT

emit() {
    printf '%s=%s\n' "$1" "$(printf '%s' "$2" | tr '\n\r' '  ')" >> "${output_buffer}"
}

size_mb=""

skip_save() {
    emit save false
    emit save-skipped "$1"
    emit size-mb "${size_mb}"
    echo "polint rule-host build cache: not saved ($1)"
    {
        echo "polint rule-host build cache: **not saved** — $1"
    } >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
    exit 0
}

cache_root="$(pwd -P)/.polint/cache"
rules_target="${cache_root}/rules-target"
extensions_target="${cache_root}/extensions-target"

# 1. Did this run finish?
#
# polint exits 0 when clean and 1 when it reports findings at or above the
# fail-on threshold; both mean the rule hosts built and ran. Any other status -
# an internal error, a rule-host build failure, a signal, a cancelled job that
# never produced the output at all - means the target directory describes an
# interrupted build.
case "${POLINT_ACTION_EXIT_CODE:-}" in
    0 | 1) ;;
    "") skip_save "polint did not run" ;;
    *) skip_save "polint exited ${POLINT_ACTION_EXIT_CODE}" ;;
esac

cargo_bin="${POLINT_CARGO:-${CARGO:-cargo}}"
if ! command -v "${cargo_bin}" >/dev/null 2>&1; then
    skip_save "cargo is not on PATH"
fi

profile_args=()
case "${POLINT_ACTION_RULES_PROFILE:-release}" in
    dev) ;;
    release) profile_args=(--release) ;;
    *) profile_args=(--profile "${POLINT_ACTION_RULES_PROFILE}") ;;
esac

# The package name cargo will use for artifact and fingerprint file names.
# `cargo pkgid` prints `<source>#<name>@<version>`, or `<source>#<version>` when
# the directory name already is the package name.
package_name_of() {
    local manifest="$1" spec suffix
    spec="$("${cargo_bin}" pkgid --manifest-path "${manifest}" 2>/dev/null)" || return 1
    [[ -n "${spec}" ]] || return 1
    suffix="${spec##*#}"
    case "${suffix}" in
        *@*) printf '%s\n' "${suffix%@*}" ;;
        [0-9]*)
            spec="${spec%#*}"
            printf '%s\n' "${spec##*/}"
            ;;
        *) printf '%s\n' "${suffix}" ;;
    esac
}

# Cargo names library artifacts after the crate name, which replaces dashes, and
# suffixes unit outputs and fingerprint directories with a hex hash. Anything
# still carrying either name is output built from these sources.
hex8='[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]'
leftovers_for() {
    local target="$1" name="$2" crate="${2//-/_}"
    [[ -d "${target}" ]] || return 0
    find "${target}" \( \
        -name "${name}" -o -name "${name}.d" -o -name "${name}.exe" -o -name "${name}.pdb" \
        -o -name "${name}-${hex8}*" -o -name "${crate}-${hex8}*" -o -name "lib${crate}-${hex8}*" \
        \) -print 2>/dev/null
}

prune_package() {
    local package="$1" target="$2"
    shift 2
    local name leftovers
    if ! name="$(package_name_of "${package}/Cargo.toml")"; then
        skip_save "could not resolve the package name for ${package}"
    fi
    if ! CARGO_TARGET_DIR="${target}" "${cargo_bin}" clean \
        --manifest-path "${package}/Cargo.toml" --package "${name}" "$@" >/dev/null 2>&1; then
        skip_save "could not prune ${package} from the build cache"
    fi
    # Named profiles and future layout changes are covered by proving the
    # removal rather than trusting the flags: nothing named after the package
    # may remain anywhere under the target directory.
    leftovers="$(leftovers_for "${target}" "${name}")"
    if [[ -n "${leftovers}" ]]; then
        skip_save "build output for ${package} survived pruning"
    fi
    echo "pruned ${package} (${name}) from ${target}"
}

while IFS= read -r package; do
    [[ -n "${package}" ]] || continue
    prune_package "${package}" "${rules_target}" ${profile_args[@]+"${profile_args[@]}"}
done < <(cat "${POLINT_ACTION_RULE_PACKAGES_FILE:-/dev/null}" 2>/dev/null)

# The extension host runs `cargo run` without a profile flag, so its output is
# always the dev profile.
while IFS= read -r package; do
    [[ -n "${package}" ]] || continue
    prune_package "${package}" "${extensions_target}"
done < <(cat "${POLINT_ACTION_EXTENSION_PACKAGES_FILE:-/dev/null}" 2>/dev/null)

# Incremental state only speeds up a later recompile of the same unit. The rule
# packages are recompiled from scratch on every restore anyway, so carrying it
# through the cache buys nothing and can dominate the entry on the dev profile.
for target in "${rules_target}" "${extensions_target}"; do
    [[ -d "${target}" ]] || continue
    find "${target}" -maxdepth 3 -type d -name incremental -prune -exec rm -rf {} + 2>/dev/null
done

total_kb=0
for target in "${rules_target}" "${extensions_target}"; do
    [[ -d "${target}" ]] || continue
    measured="$(du -sk "${target}" 2>/dev/null | awk 'NR == 1 { print $1 }')"
    case "${measured}" in
        '' | *[!0-9]*) continue ;;
    esac
    total_kb=$((total_kb + measured))
done
size_mb=$(((total_kb + 1023) / 1024))

# GitHub evicts caches least-recently-used within a 10 GB per-repository budget,
# so a large entry is not an error - it is a trade against every other cache in
# the repository. Report it always; refuse the save only against a ceiling the
# caller chose.
max_mb="${POLINT_ACTION_MAX_SIZE_MB:-}"
{
    echo "### polint rule-host build cache"
    echo
    echo "| measure | value |"
    echo "| --- | --- |"
    echo "| pruned size | ${size_mb} MB |"
    echo "| size ceiling | ${max_mb:-none} |"
} >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
echo "polint rule-host build cache: ${size_mb} MB after pruning"

case "${max_mb}" in
    '') ;;
    *[!0-9]*)
        echo "::warning::build-cache-max-size-mb is not a number: ${max_mb}"
        ;;
    *)
        if [[ "${size_mb}" -gt "${max_mb}" ]]; then
            skip_save "pruned build cache is ${size_mb} MB, over the ${max_mb} MB ceiling"
        fi
        ;;
esac

emit save true
emit save-skipped ""
emit size-mb "${size_mb}"
exit 0
