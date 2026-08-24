#!/usr/bin/env bash
# Resolve the cache paths, keys, and covered packages for the polint action.
#
# Everything here is an optimization. A failure to resolve a key must degrade to
# "no cache", never to a failed job: `polint check` builds and runs the rule
# hosts itself and reports build failures itself. So this script writes its
# GitHub outputs to a buffer and flushes them once, on every exit path, and it
# never returns a nonzero status.
#
# Inputs (environment):
#   POLINT_ACTION_CACHE_RULE_BUILDS  "true" enables the rule-host build cache
#   POLINT_ACTION_RULE_PATHS         explicit rule package override, newline- or comma-separated
#   POLINT_ACTION_SCRIPT_DIR         directory holding this script and rule-paths.awk
#   POLINT_ACTION_STATE_DIR          writable directory for the covered-package lists
#   GITHUB_OUTPUT, GITHUB_STEP_SUMMARY, RUNNER_OS, RUNNER_ARCH
#
# Outputs:
#   analysis-cache, analysis-digest
#   build-cache, build-cache-skipped, env-digest, deps-digest
#   rule-packages-file, extension-packages-file, rules-profile

set -uo pipefail

script_dir="${POLINT_ACTION_SCRIPT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)}"
# Outputs are buffered and flushed once, so a half-finished resolution can never
# leave a partially described cache decision behind.
output_buffer="$(mktemp 2>/dev/null)"
summary_buffer="$(mktemp 2>/dev/null)"
[[ -n "${output_buffer}" ]] || output_buffer="${GITHUB_OUTPUT:-/dev/null}"
[[ -n "${summary_buffer}" ]] || summary_buffer="${GITHUB_STEP_SUMMARY:-/dev/null}"

flush() {
    if [[ "${output_buffer}" != "${GITHUB_OUTPUT:-/dev/null}" && -f "${output_buffer}" ]]; then
        [[ -n "${GITHUB_OUTPUT:-}" ]] && cat "${output_buffer}" >> "${GITHUB_OUTPUT}"
        rm -f "${output_buffer}"
    fi
    if [[ "${summary_buffer}" != "${GITHUB_STEP_SUMMARY:-/dev/null}" && -f "${summary_buffer}" ]]; then
        [[ -n "${GITHUB_STEP_SUMMARY:-}" ]] && cat "${summary_buffer}" >> "${GITHUB_STEP_SUMMARY}"
        rm -f "${summary_buffer}"
    fi
    return 0
}
trap flush EXIT

emit() {
    # Values are single-line by construction; fold anything unexpected so a
    # stray newline can never inject a second output.
    printf '%s=%s\n' "$1" "$(printf '%s' "$2" | tr '\n\r' '  ')" >> "${output_buffer}"
}

summary() {
    printf '%s\n' "$1" >> "${summary_buffer}"
}

skip_build_cache() {
    emit build-cache false
    emit build-cache-skipped "$1"
    echo "polint rule-host build cache: skipped ($1)"
    finish
}

finish() {
    if [[ "${analysis_cache}" == "true" ]]; then
        emit analysis-cache true
        emit analysis-digest "${analysis_digest}"
    else
        emit analysis-cache false
    fi
    exit 0
}

analysis_cache=false
analysis_digest=""

# --- hashing ------------------------------------------------------------

if command -v sha256sum >/dev/null 2>&1; then
    hash_stdin() { sha256sum | cut -d ' ' -f 1; }
elif command -v shasum >/dev/null 2>&1; then
    hash_stdin() { shasum -a 256 | cut -d ' ' -f 1; }
else
    echo "::warning::neither sha256sum nor shasum is on PATH; polint caching is disabled for this run."
    emit build-cache false
    emit build-cache-skipped "no sha256 tool on PATH"
    emit analysis-cache false
    exit 0
fi

file_digest() {
    if [[ -f "$1" ]]; then hash_stdin < "$1"; else printf 'absent\n'; fi
}

# --- cache root ---------------------------------------------------------

workdir="$(pwd -P)"
default_cache_root="${workdir}/.polint/cache"

# `POLINT_CACHE_DIR` moves the cache root, and this action only knows the
# default layout. Resolve it the way polint does - absolute as given, relative
# to the repository - so pointing it at the layout the action already caches is
# not treated as a move.
if [[ -n "${POLINT_CACHE_DIR:-}" ]]; then
    configured_root="${POLINT_CACHE_DIR}"
    case "${configured_root}" in
        /*) ;;
        *) configured_root="${workdir}/${configured_root}" ;;
    esac
    mkdir -p "${configured_root}" "${default_cache_root}" 2>/dev/null
    configured_physical="$(cd "${configured_root}" 2>/dev/null && pwd -P)"
    default_physical="$(cd "${default_cache_root}" 2>/dev/null && pwd -P)"
    if [[ -z "${configured_physical}" || "${configured_physical}" != "${default_physical}" ]]; then
        echo "::warning::POLINT_CACHE_DIR points outside ${default_cache_root}; this action only caches the default .polint/cache layout, so it is caching nothing this run."
        skip_build_cache "POLINT_CACHE_DIR moves the cache root"
    fi
fi

# Analysis, layer, derived, and semantic-store artifacts stay under the
# analysis key. polint validates each artifact against current sources (path +
# content + config + rule/options + capability plan + cache format + polint
# version), so these are never reused as build output. The directories only
# need to exist for the cache steps to resolve them.
mkdir -p "${default_cache_root}/analysis" "${default_cache_root}/layers" \
    "${default_cache_root}/derived" "${default_cache_root}/semantic-store" 2>/dev/null

# --- rule packages ------------------------------------------------------

reject_reason=""
normalized_path=""

normalize_package_path() {
    local raw="$1"
    normalized_path=""
    reject_reason=""
    case "${raw}" in
        "")
            reject_reason="empty rule path"
            return 1
            ;;
        *[[:cntrl:]]*)
            reject_reason="rule path contains a control character"
            return 1
            ;;
        /* | \~* | [A-Za-z]:/*)
            reject_reason="rule path ${raw} is not relative to the working directory"
            return 1
            ;;
        *\\*)
            reject_reason="rule path ${raw} is not a relative POSIX path"
            return 1
            ;;
    esac

    local path
    path="$(printf '%s' "${raw}" | sed -e 's|//*|/|g' -e 's|^\(\./\)*||' -e 's|/*$||')"
    if [[ -z "${path}" ]]; then
        reject_reason="rule path ${raw} is empty after normalization"
        return 1
    fi
    case "/${path}/" in
        */../*)
            reject_reason="rule path ${raw} contains a .. component"
            return 1
            ;;
    esac
    if [[ ! -d "${path}" ]]; then
        reject_reason="rule path ${path} is not a directory"
        return 1
    fi
    if [[ ! -f "${path}/Cargo.toml" ]]; then
        reject_reason="rule path ${path} has no Cargo.toml"
        return 1
    fi
    local physical
    physical="$(cd "${path}" 2>/dev/null && pwd -P)"
    case "${physical}" in
        "${workdir}" | "${workdir}"/*) ;;
        *)
            reject_reason="rule path ${path} resolves outside the working directory"
            return 1
            ;;
    esac
    normalized_path="${path}"
    return 0
}

# An explicit `rule-paths` input overrides the config, for repositories whose
# layout this action cannot read (or should not read) for itself.
override_paths="$(
    printf '%s\n' "${POLINT_ACTION_RULE_PATHS:-}" \
        | tr ',' '\n' \
        | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' \
        | grep -v '^$'
)"

config_status="default"
config_reason=""
raw_rule_paths=".polint/rules"
if [[ -n "${override_paths}" ]]; then
    config_status="input"
    raw_rule_paths="${override_paths}"
elif [[ -f .polint.toml ]]; then
    resolved="$(awk -f "${script_dir}/rule-paths.awk" < .polint.toml 2>/dev/null)"
    case "$(printf '%s\n' "${resolved}" | head -n 1)" in
        "status=default")
            config_status="default"
            ;;
        "status=paths")
            config_status="paths"
            raw_rule_paths="$(printf '%s\n' "${resolved}" | tail -n +2)"
            ;;
        *)
            config_status="unsupported"
            config_reason="$(printf '%s\n' "${resolved}" | sed -n 's/^reason=//p' | head -n 1)"
            [[ -n "${config_reason}" ]] || config_reason="could not read [rules].paths"
            ;;
    esac
fi

# --- analysis cache key -------------------------------------------------

rule_paths=""
rule_paths_reason=""
if [[ "${config_status}" == "unsupported" ]]; then
    rule_paths_reason="${config_reason}"
else
    while IFS= read -r candidate; do
        [[ -n "${candidate}" ]] || continue
        if ! normalize_package_path "${candidate}"; then
            rule_paths=""
            rule_paths_reason="${reject_reason}"
            break
        fi
        rule_paths="${rule_paths}${normalized_path}"$'\n'
    done <<< "${raw_rule_paths}"
fi
rule_paths="$(printf '%s' "${rule_paths}" | grep -v '^$' | LC_ALL=C sort -u)"

hash_rust_sources() {
    # Deterministic digest lines for one package's manifests and sources.
    local package="$1"
    printf '%s/Cargo.toml=%s\n' "${package}" "$(file_digest "${package}/Cargo.toml")"
    printf '%s/Cargo.lock=%s\n' "${package}" "$(file_digest "${package}/Cargo.lock")"
    if [[ -d "${package}/src" ]]; then
        find "${package}/src" -type f -name '*.rs' -print 2>/dev/null | LC_ALL=C sort | while IFS= read -r source; do
            printf '%s=%s\n' "${source}" "$(file_digest "${source}")"
        done
    fi
}

analysis_digest="$(
    {
        printf 'polint-analysis-inputs-v1\n'
        for candidate in .polint.toml Cargo.lock rust-toolchain.toml; do
            printf '%s=%s\n' "${candidate}" "$(file_digest "${candidate}")"
        done
        if [[ -n "${rule_paths_reason}" ]]; then
            printf 'rule-paths=unresolved:%s\n' "${rule_paths_reason}"
            # Keep the default layout in the key even when the configured one
            # cannot be resolved, so the common repository still partitions on
            # its rule sources.
            [[ -d .polint/rules ]] && hash_rust_sources ".polint/rules"
        else
            while IFS= read -r package; do
                [[ -n "${package}" ]] || continue
                printf 'rule-path=%s\n' "${package}"
                hash_rust_sources "${package}"
            done <<< "${rule_paths}"
        fi
    } | hash_stdin
)"
analysis_cache=true

# --- rule-host build cache ----------------------------------------------

if [[ "${POLINT_ACTION_CACHE_RULE_BUILDS:-}" != "true" ]]; then
    skip_build_cache "cache-rule-builds is not true"
fi

# `POLINT_RULES_TARGET_DIR` moves the rule-host target directory. Resolve it the
# way polint does - absolute as given, relative to the cache root - so pointing
# it at the directory the action already caches is not treated as a move.
if [[ -n "${POLINT_RULES_TARGET_DIR:-}" ]]; then
    configured_target="${POLINT_RULES_TARGET_DIR}"
    case "${configured_target}" in
        /*) ;;
        *) configured_target="${default_cache_root}/${configured_target}" ;;
    esac
    mkdir -p "${configured_target}" "${default_cache_root}/rules-target" 2>/dev/null
    configured_physical="$(cd "${configured_target}" 2>/dev/null && pwd -P)"
    default_physical="$(cd "${default_cache_root}/rules-target" 2>/dev/null && pwd -P)"
    if [[ -z "${configured_physical}" || "${configured_physical}" != "${default_physical}" ]]; then
        echo "::warning::POLINT_RULES_TARGET_DIR points outside ${default_cache_root}/rules-target; this action only caches the default layout."
        skip_build_cache "POLINT_RULES_TARGET_DIR moves the rule-host target directory"
    fi
fi

if [[ -n "${rule_paths_reason}" ]]; then
    echo "::warning::${rule_paths_reason}; set the rule-paths input to cache rule-host builds for this repository."
    skip_build_cache "${rule_paths_reason}"
fi

# Extension packages are discovered from a fixed directory, so they need no
# configuration to cover. They compile with the same toolchain into the same
# cache root, and dropping them from the entry would silently stop caching a
# build the old whole-directory cache did cover.
extension_paths=""
if [[ -d .polint/extensions ]]; then
    for candidate in .polint/extensions/*/; do
        [[ -f "${candidate}Cargo.toml" ]] || continue
        if normalize_package_path "${candidate}"; then
            extension_paths="${extension_paths}${normalized_path}"$'\n'
        fi
    done
fi
extension_paths="$(printf '%s' "${extension_paths}" | grep -v '^$' | LC_ALL=C sort -u)"

if [[ -z "${rule_paths}" && -z "${extension_paths}" ]]; then
    skip_build_cache "no repo-local rule package found"
fi

state_dir="${POLINT_ACTION_STATE_DIR:-${RUNNER_TEMP:-}}"
if [[ -z "${state_dir}" ]] || ! mkdir -p "${state_dir}/polint-action" 2>/dev/null; then
    skip_build_cache "no writable state directory for the covered package list"
fi
rule_packages_file="${state_dir}/polint-action/rule-packages"
extension_packages_file="${state_dir}/polint-action/extension-packages"
if ! : > "${rule_packages_file}" 2>/dev/null || ! : > "${extension_packages_file}" 2>/dev/null; then
    skip_build_cache "could not record the covered package list"
fi
printf '%s\n' "${rule_paths}" | grep -v '^$' >> "${rule_packages_file}"
printf '%s\n' "${extension_paths}" | grep -v '^$' >> "${extension_packages_file}"

# `polint check` spawns cargo with the working directory as its cwd, so rustup
# and cargo resolve the toolchain and config from here. Reading the resolved
# compiler beats hashing rust-toolchain.toml alone: it also covers a pinned
# action toolchain and a floating `stable` that moved.
cargo_bin="${POLINT_CARGO:-${CARGO:-cargo}}"
if [[ -n "${POLINT_RULES_TOOLCHAIN:-}" ]]; then
    export RUSTUP_TOOLCHAIN="${POLINT_RULES_TOOLCHAIN}"
fi
rustc_version="$(rustc -vV 2>/dev/null)"
cargo_version="$("${cargo_bin}" -V 2>/dev/null)"
if [[ -z "${rustc_version}" || -z "${cargo_version}" ]]; then
    echo "::warning::no Rust toolchain resolved in ${workdir}; skipping the rule-host build cache. polint reports rule-host build failures itself."
    skip_build_cache "no Rust toolchain resolved"
fi

# Mirrors polint's own rule-host profile resolution: unset means release, an
# empty or dev/debug value means the dev profile, anything else is a named
# profile. The prune step has to clean the same profile polint built.
if [[ -z "${POLINT_RULES_PROFILE+set}" ]]; then
    rules_profile="release"
else
    rules_profile="$(printf '%s' "${POLINT_RULES_PROFILE}" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
    case "$(printf '%s' "${rules_profile}" | tr '[:upper:]' '[:lower:]')" in
        "" | dev | debug) rules_profile="dev" ;;
        release) rules_profile="release" ;;
    esac
fi

# Everything that changes what a compiled artifact means. Cargo also
# fingerprints profile, features, flags, and dependency hashes per unit, so this
# digest partitions entries; it is not the freshness check.
env_digest="$(
    {
        printf 'polint-rule-build-env-v2\n'
        printf 'runner-os=%s\n' "${RUNNER_OS:-unknown}"
        printf 'runner-arch=%s\n' "${RUNNER_ARCH:-unknown}"
        printf 'polint-rules-profile=%s\n' "${rules_profile}"
        printf 'rustflags=%s\n' "${RUSTFLAGS-<unset>}"
        printf 'cargo-encoded-rustflags=%s\n' "${CARGO_ENCODED_RUSTFLAGS-<unset>}"
        printf 'cargo-build-rustflags=%s\n' "${CARGO_BUILD_RUSTFLAGS-<unset>}"
        printf 'cargo-build-target=%s\n' "${CARGO_BUILD_TARGET-<unset>}"
        printf 'cargo-incremental=%s\n' "${CARGO_INCREMENTAL-<unset>}"
        printf 'rustc-wrapper=%s\n' "${RUSTC_WRAPPER-<unset>}"
        printf 'rustc-workspace-wrapper=%s\n' "${RUSTC_WORKSPACE_WRAPPER-<unset>}"
        printf 'rustup-toolchain=%s\n' "${RUSTUP_TOOLCHAIN-<unset>}"
        printf 'rustc=%s\n' "${rustc_version}"
        printf 'cargo=%s\n' "${cargo_version}"
    } | hash_stdin
)"

# Everything that decides which dependency units Cargo resolves and builds.
# Root files are cwd-discovered by cargo and rustup; per-package files cover
# both standalone packages and workspace members. `.polint.toml` contributes
# only the resolved package list: nothing else in it reaches the compiler.
deps_digest="$(
    {
        printf 'polint-rule-build-deps-v2\n'
        for candidate in Cargo.toml Cargo.lock rust-toolchain.toml rust-toolchain .cargo/config.toml .cargo/config; do
            printf '%s=%s\n' "${candidate}" "$(file_digest "${candidate}")"
        done
        while IFS= read -r package; do
            [[ -n "${package}" ]] || continue
            printf 'package=%s\n' "${package}"
            printf '%s/Cargo.toml=%s\n' "${package}" "$(file_digest "${package}/Cargo.toml")"
            printf '%s/Cargo.lock=%s\n' "${package}" "$(file_digest "${package}/Cargo.lock")"
        done <<< "$(printf '%s\n%s\n' "${rule_paths}" "${extension_paths}" | grep -v '^$')"
    } | hash_stdin
)"

mkdir -p "${default_cache_root}/rules-target" "${default_cache_root}/extensions-target" 2>/dev/null

emit build-cache true
emit build-cache-skipped ""
emit env-digest "${env_digest}"
emit deps-digest "${deps_digest}"
emit rule-packages-file "${rule_packages_file}"
emit extension-packages-file "${extension_packages_file}"
emit rules-profile "${rules_profile}"

summary "### polint rule-host build cache"
summary ""
summary "| input | value |"
summary "| --- | --- |"
summary "| runner | ${RUNNER_OS:-unknown}/${RUNNER_ARCH:-unknown} |"
summary "| rule packages | $(printf '%s' "${rule_paths}" | tr '\n' ' ') |"
summary "| extension packages | $(printf '%s' "${extension_paths}" | tr '\n' ' ') |"
summary "| cargo profile | ${rules_profile} |"
summary "| build env digest | \`${env_digest}\` |"
summary "| build deps digest | \`${deps_digest}\` |"

finish
