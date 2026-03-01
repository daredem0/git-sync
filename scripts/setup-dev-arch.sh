#!/usr/bin/env bash
set -euo pipefail

tier="${1:-all}"

usage() {
  cat <<'EOF'
Usage: ./scripts/setup-dev-arch.sh [core|quality|release|all]

Installs local developer dependencies on Arch Linux for the selected tier.
EOF
}

if [[ "${tier}" != "core" && "${tier}" != "quality" && "${tier}" != "release" && "${tier}" != "all" ]]; then
  usage >&2
  exit 2
fi

if ! command -v pacman >/dev/null 2>&1; then
  echo "This script is for Arch Linux (pacman not found)." >&2
  exit 1
fi

if [[ "${EUID}" -eq 0 ]]; then
  sudo_cmd=()
else
  sudo_cmd=(sudo)
fi

is_tier_enabled() {
  local candidate="$1"
  [[ "${tier}" == "all" || "${tier}" == "${candidate}" ]]
}

packages=()

if is_tier_enabled core; then
  packages+=(git rustup)
fi

if is_tier_enabled quality; then
  packages+=(nodejs npm)
fi

if is_tier_enabled release; then
  packages+=(pandoc base-devel dpkg)
fi

if ((${#packages[@]} > 0)); then
  echo "Installing Arch packages: ${packages[*]}"
  "${sudo_cmd[@]}" pacman -S --needed --noconfirm "${packages[@]}"
fi

if command -v rustup >/dev/null 2>&1; then
  rustup default stable >/dev/null
fi

if is_tier_enabled quality; then
  echo "Installing cargo-llvm-cov"
  cargo install --locked cargo-llvm-cov
fi

if is_tier_enabled release; then
  echo "Installing cargo-deb"
  cargo install --locked cargo-deb
fi

echo "Done. Run ./scripts/check-tooling.sh ${tier} to verify."
