#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

if ! TAG="$(git describe --tags --exact-match HEAD 2>/dev/null)"; then
  echo "No exact tag on HEAD; skipping tag/version consistency check."
  exit 0
fi

TAG_VERSION="${TAG#v}"
CARGO_VERSION="$(grep -m1 '^version = ' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"

if [[ -z "${CARGO_VERSION}" ]]; then
  echo "Could not read package version from Cargo.toml." >&2
  exit 1
fi

if [[ "${TAG_VERSION}" != "${CARGO_VERSION}" ]]; then
  echo "Tag/version mismatch detected." >&2
  echo "  git tag:     ${TAG}" >&2
  echo "  tag version: ${TAG_VERSION}" >&2
  echo "  Cargo.toml:  ${CARGO_VERSION}" >&2
  echo "Use cargo-release (or equivalent) so tag and Cargo.toml version stay aligned." >&2
  exit 1
fi

echo "Tag/version check passed: ${TAG} == ${CARGO_VERSION}"
