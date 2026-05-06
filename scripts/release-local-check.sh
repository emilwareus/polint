#!/usr/bin/env bash
# Local dry-run of pieces of `.github/workflows/release.yml` (no git commit/push/tag).
#
# Usage:
#   ./scripts/release-local-check.sh              # build + package + cargo publish --dry-run
#   WITH_WINDOWS_GNU=1 ./scripts/release-local-check.sh   # also build x86_64-pc-windows-gnu (needs: brew install mingw-w64)
#   DRY_RUN=0 CRATES_IO_TOKEN=... ./scripts/release-local-check.sh   # real publish (careful)
#
# macOS (Apple Silicon): for `aarch64-unknown-linux-gnu` you need a linker:
#   brew tap messense/macos-cross-toolchains
#   brew install aarch64-unknown-linux-gnu
#   (expects `.cargo/config.toml` linker = "aarch64-linux-gnu-gcc")
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export RUSTDOCFLAGS='-D warnings'

echo "==> rustup targets (install if missing)"
rustup target list --installed | sed 's/^/    /'
need_targets=(aarch64-apple-darwin x86_64-apple-darwin aarch64-unknown-linux-gnu)
if [[ "${WITH_WINDOWS_GNU:-}" == "1" ]]; then
  need_targets+=(x86_64-pc-windows-gnu)
fi
for t in "${need_targets[@]}"; do
  rustup target add "$t" 2>/dev/null || true
done

echo "==> cargo fmt + clippy (-D warnings)"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

echo "==> cargo test workspace"
cargo test --workspace --all-features --locked

echo "==> cargo doc (workspace, -D warnings)"
cargo doc --workspace --all-features --no-deps --locked

if ! command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
  echo "!!! Skipping aarch64-unknown-linux-gnu build: no aarch64-linux-gnu-gcc (see script header)."
else
  echo "==> release build: aarch64-unknown-linux-gnu"
  cargo build --locked --release -p polint --target aarch64-unknown-linux-gnu
fi

echo "==> release build: aarch64-apple-darwin (host or common on Apple Silicon)"
cargo build --locked --release -p polint --target aarch64-apple-darwin

echo "==> release build: x86_64-apple-darwin"
cargo build --locked --release -p polint --target x86_64-apple-darwin

if [[ "${WITH_WINDOWS_GNU:-}" == "1" ]]; then
  echo "==> release build: x86_64-pc-windows-gnu (requires brew mingw-w64 + correct CC)"
  cargo build --locked --release -p polint --target x86_64-pc-windows-gnu
fi

package_one() {
  local target="$1" asset_os="$2" asset_arch="$3" bin_name="$4"
  local archive="polint-${asset_os}-${asset_arch}.tar.gz"
  local out="target/release-local-dist"
  rm -rf "$out/package" && mkdir -p "$out/package" "$out/dist"
  cp "target/${target}/release/${bin_name}" "$out/package/${bin_name}"
  if [[ "$bin_name" == polint ]]; then
    chmod 0755 "$out/package/${bin_name}"
  fi
  tar -C "$out/package" -czf "$out/dist/${archive}" "${bin_name}"
  python3 "${ROOT}/scripts/sha256_file.py" "$out/dist/${archive}" > "$out/dist/${archive}.sha256"
  echo "    packaged $out/dist/${archive}"
}

echo "==> package tar.gz + sha256 (same layout as CI; under target/release-local-dist/dist/)"
package_one aarch64-apple-darwin macos aarch64 polint
package_one x86_64-apple-darwin macos x86_64 polint
if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
  package_one aarch64-unknown-linux-gnu linux aarch64 polint
fi

if [[ "${WITH_WINDOWS_GNU:-}" == "1" ]] && [[ -f target/x86_64-pc-windows-gnu/release/polint.exe ]]; then
  package_one x86_64-pc-windows-gnu windows x86_64 polint.exe
fi

echo "==> cargo publish --dry-run (no token, no upload)"
DRY_RUN=1 ./scripts/publish-crates.sh

if [[ "${DRY_RUN:-1}" == "0" || "${DRY_RUN:-1}" == "false" ]]; then
  if [[ -z "${CRATES_IO_TOKEN:-}" ]]; then
    echo "error: set CRATES_IO_TOKEN for real publish" >&2
    exit 1
  fi
  echo "==> cargo publish (REAL — DRY_RUN=0)"
  ./scripts/publish-crates.sh
else
  echo "==> skip real publish (set DRY_RUN=0 CRATES_IO_TOKEN=... to publish)"
fi

echo "OK release-local-check complete."
