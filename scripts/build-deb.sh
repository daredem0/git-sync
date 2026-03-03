#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

resolve_package_suffix() {
  if [[ -n "${GIT_SYNC_PACKAGE_SUFFIX:-}" ]]; then
    echo "${GIT_SYNC_PACKAGE_SUFFIX}"
    return
  fi

  "${REPO_ROOT}/scripts/detect-libc-suffix.sh"
}

sanitize_package_suffix() {
  local raw="$1"
  local sanitized
  sanitized="$(sed -E 's/[^A-Za-z0-9._+-]+/-/g; s/^-+//; s/-+$//' <<<"${raw}")"
  if [[ -z "${sanitized}" ]]; then
    echo "Derived package suffix is empty after sanitization." >&2
    exit 1
  fi
  echo "${sanitized}"
}

apply_package_suffix() {
  local suffix="$1"
  local files=()
  shopt -s nullglob
  files=(target/debian/*.deb)
  shopt -u nullglob

  if ((${#files[@]} == 0)); then
    echo "No Debian package was produced in target/debian." >&2
    exit 1
  fi

  for pkg in "${files[@]}"; do
    if [[ "${pkg}" == *"-${suffix}.deb" ]]; then
      continue
    fi
    mv "${pkg}" "${pkg%.deb}-${suffix}.deb"
  done
}

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

PACKAGE_SUFFIX="$(sanitize_package_suffix "$(resolve_package_suffix)")"
echo "Applying package suffix: ${PACKAGE_SUFFIX}"
apply_package_suffix "${PACKAGE_SUFFIX}"

echo "Debian package(s):"
find target/debian -maxdepth 1 -type f -name '*.deb' -print | sort
