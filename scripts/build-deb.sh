#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

if ! cargo deb --version >/dev/null 2>&1; then
  echo "cargo-deb is not installed." >&2
  echo "Install with: cargo install --locked cargo-deb" >&2
  exit 1
fi

./scripts/generate-manpages.sh
cargo build --locked --release
cargo deb --locked --no-build

echo "Debian package(s):"
find target/debian -maxdepth 1 -type f -name '*.deb' -print | sort
