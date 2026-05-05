#!/usr/bin/env bash
set -euo pipefail

repo="${POLINT_REPO:-emilwareus/exlint}"
# Default: latest stable GitHub Release (semver tags from the Release workflow).
# Override with e.g. POLINT_RELEASE_TAG=v0.2.0 for a specific tag.
tag="${POLINT_RELEASE_TAG:-latest}"
install_dir="${POLINT_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
  Darwin) os="macos" ;;
  Linux) os="linux" ;;
  *)
    echo "polint install: unsupported OS: $(uname -s)" >&2
    exit 1
    ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) arch="x86_64" ;;
  arm64 | aarch64) arch="aarch64" ;;
  *)
    echo "polint install: unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

asset="polint-${os}-${arch}.tar.gz"
if [[ "${tag}" == "latest" ]]; then
  base_url="https://github.com/${repo}/releases/latest/download"
else
  base_url="https://github.com/${repo}/releases/download/${tag}"
fi

if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -q "$1" -O "$2"; }
else
  echo "polint install: curl or wget is required to download release assets" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

echo "Downloading ${asset} from ${repo} release ${tag}..."
fetch "${base_url}/${asset}" "${tmp_dir}/${asset}"
fetch "${base_url}/${asset}.sha256" "${tmp_dir}/${asset}.sha256"

(
  cd "$tmp_dir"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "${asset}.sha256"
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "${asset}.sha256"
  else
    echo "polint install: shasum or sha256sum is required to verify ${asset}" >&2
    exit 1
  fi
  tar -xzf "$asset"
)

mkdir -p "$install_dir"
install -m 0755 "$tmp_dir/polint" "$install_dir/polint"

echo "Installed polint to ${install_dir}/polint"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *)
    echo "Note: ${install_dir} is not on PATH. Add it to your shell profile to run polint directly."
    ;;
esac

"$install_dir/polint" --version
