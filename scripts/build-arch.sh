#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

if ! command -v makepkg >/dev/null 2>&1; then
  echo "makepkg is not installed." >&2
  echo "Install with your package manager (on Arch: sudo pacman -S base-devel)." >&2
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

PKGVER="$(grep -m1 '^version =' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
PACKAGER_EMAIL="${GIT_SYNC_PACKAGER_EMAIL:-f.leuze@outlook.de}"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT

cp packaging/arch/PKGBUILD "${WORK_DIR}/PKGBUILD"
cp target/release/git-sync "${WORK_DIR}/git-sync"
cp target/man/git-sync.1.gz "${WORK_DIR}/git-sync.1.gz"
cp target/man/git-sync-readme.7.gz "${WORK_DIR}/git-sync-readme.7.gz"
cp target/man/git-sync-architecture.7.gz "${WORK_DIR}/git-sync-architecture.7.gz"
cp LICENSE "${WORK_DIR}/LICENSE"

(
  cd "${WORK_DIR}"
  PKGVER="${PKGVER}" PACKAGER="${PACKAGER:-${PACKAGER_EMAIL}}" makepkg -f --clean
)

mkdir -p target/arch
find "${WORK_DIR}" -maxdepth 1 -type f -name '*.pkg.tar.*' -exec cp {} target/arch/ \;

echo "Arch package(s):"
find target/arch -maxdepth 1 -type f -name '*.pkg.tar.*' -print | sort
