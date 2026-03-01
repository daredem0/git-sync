#!/usr/bin/env bash
set -euo pipefail

tier="${1:-all}"

usage() {
  cat <<'EOF'
Usage: ./scripts/check-tooling.sh [core|quality|release|all]

Checks whether local developer tooling is installed for the selected tier.
EOF
}

if [[ "${tier}" != "core" && "${tier}" != "quality" && "${tier}" != "release" && "${tier}" != "all" ]]; then
  usage >&2
  exit 2
fi

missing=()
warnings=()

has() {
  command -v "$1" >/dev/null 2>&1
}

check_required() {
  local cmd="$1"
  if has "${cmd}"; then
    printf 'ok      %s\n' "${cmd}"
  else
    printf 'missing %s\n' "${cmd}"
    missing+=("${cmd}")
  fi
}

is_tier_enabled() {
  local candidate="$1"
  [[ "${tier}" == "all" || "${tier}" == "${candidate}" ]]
}

printf 'Checking tooling tier: %s\n' "${tier}"

if is_tier_enabled core; then
  echo ""
  echo "[core]"
  check_required cargo
  check_required git
fi

if is_tier_enabled quality; then
  echo ""
  echo "[quality]"
  check_required cargo-llvm-cov
  check_required node
  check_required npm
  check_required npx
fi

if is_tier_enabled release; then
  echo ""
  echo "[release]"
  check_required pandoc

  has_deb=false
  has_arch=false
  if has cargo-deb; then
    printf 'ok      cargo-deb\n'
    has_deb=true
  else
    warnings+=("cargo-deb (needed for Debian packaging)")
    printf 'warn    cargo-deb (needed for Debian packaging)\n'
  fi

  if has makepkg; then
    printf 'ok      makepkg\n'
    has_arch=true
  else
    warnings+=("makepkg (needed for Arch packaging)")
    printf 'warn    makepkg (needed for Arch packaging)\n'
  fi

  if [[ "${has_deb}" == "false" && "${has_arch}" == "false" ]]; then
    missing+=("cargo-deb or makepkg")
  fi
fi

if ((${#warnings[@]} > 0)); then
  echo ""
  echo "Warnings:"
  for item in "${warnings[@]}"; do
    printf '  - %s\n' "${item}"
  done
fi

if ((${#missing[@]} > 0)); then
  echo ""
  echo "Missing required tooling:"
  for item in "${missing[@]}"; do
    printf '  - %s\n' "${item}"
  done
  echo ""
  echo "Install helpers:"
  echo "  - Arch Linux:   ./scripts/setup-dev-arch.sh ${tier}"
  echo "  - Debian/Ubuntu: ./scripts/setup-dev-ubuntu.sh ${tier}"
  exit 1
fi

echo ""
echo "Tooling check passed for tier: ${tier}"
