#!/usr/bin/env bash
set -euo pipefail

repo="${POLINT_REPO:-emilwareus/exlint}"
tag="${POLINT_RELEASE_TAG:-polint-main}"
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

if ! command -v gh >/dev/null 2>&1; then
  cat >&2 <<'EOF'
polint install: GitHub CLI (`gh`) is required for private release downloads.
Install it, then run `gh auth login` before retrying.
EOF
  exit 1
fi

if ! gh auth status -h github.com >/dev/null 2>&1; then
  cat >&2 <<'EOF'
polint install: `gh` is not authenticated for github.com.
Run `gh auth login` before retrying.
EOF
  exit 1
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

echo "Downloading ${asset} from ${repo} release ${tag}..."
gh release download "$tag" \
  --repo "$repo" \
  --pattern "$asset" \
  --pattern "${asset}.sha256" \
  --dir "$tmp_dir" \
  --clobber

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
