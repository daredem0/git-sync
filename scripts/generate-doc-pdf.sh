#!/usr/bin/env bash
set -euo pipefail

DOCS_DIR="${1:-target/doc}"
OUTPUT_PATH="${2:-}"
ENTRY_FILE="${3:-index.html}"

if [[ ! -d "${DOCS_DIR}" ]]; then
  echo "Docs directory not found: ${DOCS_DIR}" >&2
  echo "Run cargo doc first." >&2
  exit 1
fi

CRATE_DOC_DIR="$(grep -m1 '^name = ' Cargo.toml | sed -E 's/name = "([^"]+)"/\1/' | tr '-' '_')"
ENTRY_PATH="${CRATE_DOC_DIR}/${ENTRY_FILE}"

if [[ ! -f "${DOCS_DIR}/${ENTRY_PATH}" ]]; then
  echo "Expected crate docs entry not found: ${DOCS_DIR}/${ENTRY_PATH}" >&2
  exit 1
fi

if [[ -z "${OUTPUT_PATH}" ]]; then
  OUTPUT_PATH="target/docs-pdf/${CRATE_DOC_DIR}-rustdoc.pdf"
fi

mkdir -p "$(dirname "${OUTPUT_PATH}")"

node scripts/generate-doc-pdf.mjs \
  --docs-dir "${DOCS_DIR}" \
  --entry "${ENTRY_PATH}" \
  --output "${OUTPUT_PATH}"

echo "Docs PDF written to: ${OUTPUT_PATH}"
