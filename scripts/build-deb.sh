#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

if ! cargo deb --version >/dev/null 2>&1; then
  echo "cargo-deb is not installed." >&2
  echo "Install with: cargo install --locked cargo-deb" >&2
  exit 1
fi

ensure_prebuilt_inputs() {
  local required=(
    "target/release/git-sync"
    "target/man/git-sync.1.gz"
    "target/man/git-sync-readme.7.gz"
    "target/man/git-sync-architecture.7.gz"
  )
  for path in "${required[@]}"; do
    if [[ ! -f "${path}" ]]; then
      echo "Missing required prebuilt artifact: ${path}" >&2
      exit 1
    fi
  done
}

if [[ "${GIT_SYNC_USE_PREBUILT:-0}" == "1" ]]; then
  echo "Using prebuilt release binary and manpages from target/."
  ensure_prebuilt_inputs
else
  ./scripts/generate-manpages.sh
  cargo build --locked --release
fi

cargo deb --locked --no-build

echo "Debian package(s):"
find target/debian -maxdepth 1 -type f -name '*.deb' -print | sort
