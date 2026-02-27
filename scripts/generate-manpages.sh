#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

if ! command -v pandoc >/dev/null 2>&1; then
  echo "pandoc is not installed." >&2
  echo "Install with your package manager (for example: sudo pacman -S pandoc)." >&2
  exit 1
fi

mkdir -p target/man

pandoc -s -f gfm -t man \
  -V title=git-sync-audit \
  -V section=1 \
  -V source=git-sync-audit \
  docs/git-sync-audit.1.md \
  -o target/man/git-sync-audit.1

pandoc -s -f gfm -t man \
  -V title=git-sync-audit-readme \
  -V section=7 \
  -V source=git-sync-audit \
  README.md \
  -o target/man/git-sync-audit-readme.7

pandoc -s -f gfm -t man \
  -V title=git-sync-audit-architecture \
  -V section=7 \
  -V source=git-sync-audit \
  SDD_SAD.md \
  -o target/man/git-sync-audit-architecture.7

gzip -9 -f \
  target/man/git-sync-audit.1 \
  target/man/git-sync-audit-readme.7 \
  target/man/git-sync-audit-architecture.7

echo "Wrote:"
echo "  target/man/git-sync-audit.1.gz"
echo "  target/man/git-sync-audit-readme.7.gz"
echo "  target/man/git-sync-audit-architecture.7.gz"
