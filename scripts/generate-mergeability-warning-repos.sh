#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  generate-mergeability-warning-repos.sh [target-dir]

Creates a deterministic sender/receiver fixture with diverged history,
then runs git-sync receive in mergeability and fast-forward-only modes.

When no target directory is provided, a temporary directory is created.
USAGE
}

script_log() {
  printf '[SCRIPT] %s\n' "$*"
}

script_fail() {
  printf '[SCRIPT] ERROR: %s\n' "$*" >&2
  exit 1
}

run_quiet() {
  "$@" >/dev/null 2>&1
}

run_git_sync() {
  local log_file="$1"
  shift

  local cmd_display
  cmd_display="$(printf '%q ' "$@")"

  script_log "Run git-sync command:"
  script_log "  ${cmd_display% }"
  "$@" 2>&1 | tee "${log_file}" | sed 's/^/[GIT-SYNC] /'
}

init_repo_main() {
  local path="$1"
  if git init -b main "${path}" >/dev/null 2>&1; then
    return
  fi
  run_quiet git init "${path}"
  run_quiet git -C "${path}" checkout -b main
}

configure_user() {
  local repo="$1"
  run_quiet git -C "${repo}" config user.name "Mergeability Bot"
  run_quiet git -C "${repo}" config user.email "mergeability@example.com"
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -gt 1 ]]; then
  usage >&2
  exit 1
fi

if [[ $# -eq 1 ]]; then
  TARGET_DIR="$1"
  mkdir -p "${TARGET_DIR}"
  if [[ -n "$(ls -A "${TARGET_DIR}" 2>/dev/null)" ]]; then
    script_fail "target directory is not empty: ${TARGET_DIR}"
  fi
else
  TARGET_DIR="$(mktemp -d -t git-sync-mergeability-warning-XXXXXX)"
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="$(cd "${TARGET_DIR}" && pwd)"
GIT_SYNC_BIN="${REPO_ROOT}/target/debug/git-sync"

script_log "Prepare diverged sender/receiver fixture and run receive diagnostics."
script_log "Workspace: ${TARGET_DIR}"
script_log "Build git-sync debug binary."
run_quiet cargo build --quiet --manifest-path "${REPO_ROOT}/Cargo.toml"

ORIGIN="${TARGET_DIR}/origin"
SENDER="${TARGET_DIR}/sender"
RECEIVER_BARE="${TARGET_DIR}/receiver-bare"
RECEIVER_WORK="${TARGET_DIR}/receiver-work"
BUNDLE_PATH="${TARGET_DIR}/sync.bundle"
BUNDLE_ARCHIVE="${BUNDLE_PATH}.zip"

CREATE_LOG="${TARGET_DIR}/create.log"
MERGEABILITY_LOG="${TARGET_DIR}/receive-check-mergeability.log"
RECEIVE_FAIL_LOG="${TARGET_DIR}/receive-fast-forward-only.log"

init_repo_main "${ORIGIN}"
configure_user "${ORIGIN}"

cat > "${ORIGIN}/conflict.txt" <<'TEXT'
line=base
TEXT
run_quiet git -C "${ORIGIN}" add conflict.txt
run_quiet git -C "${ORIGIN}" commit -m "origin: base"
BASE_OID="$(git -C "${ORIGIN}" rev-parse HEAD)"

run_quiet git clone "${ORIGIN}" "${SENDER}"
configure_user "${SENDER}"
cat > "${SENDER}/conflict.txt" <<'TEXT'
line=sender-change
TEXT
run_quiet git -C "${SENDER}" add conflict.txt
run_quiet git -C "${SENDER}" commit -m "sender: change same line"

run_git_sync "${CREATE_LOG}" \
  "${GIT_SYNC_BIN}" create \
  --repo "${SENDER}" \
  --from "${BASE_OID}" \
  --to main \
  --output "${BUNDLE_PATH}"

[[ -f "${BUNDLE_ARCHIVE}" ]] || script_fail "bundle archive missing: ${BUNDLE_ARCHIVE}"

run_quiet git clone --bare "${ORIGIN}" "${RECEIVER_BARE}"
run_quiet git clone "${RECEIVER_BARE}" "${RECEIVER_WORK}"
configure_user "${RECEIVER_WORK}"
cat > "${RECEIVER_WORK}/conflict.txt" <<'TEXT'
line=receiver-change
TEXT
run_quiet git -C "${RECEIVER_WORK}" add conflict.txt
run_quiet git -C "${RECEIVER_WORK}" commit -m "receiver: change same line"
run_quiet git -C "${RECEIVER_WORK}" push origin main

run_git_sync "${MERGEABILITY_LOG}" \
  "${GIT_SYNC_BIN}" receive \
  --repo "${RECEIVER_BARE}" \
  --bundle "${BUNDLE_ARCHIVE}" \
  --integrate fast-forward-only \
  --check-mergeability

if ! grep -Eq "\(conflicted\)|\(clean\)|\(unknown\)" "${MERGEABILITY_LOG}"; then
  script_fail "mergeability status missing in git-sync output"
fi

set +e
run_git_sync "${RECEIVE_FAIL_LOG}" \
  "${GIT_SYNC_BIN}" receive \
  --repo "${RECEIVER_BARE}" \
  --bundle "${BUNDLE_ARCHIVE}" \
  --integrate fast-forward-only
RECEIVE_EXIT=$?
set -e

if [[ ${RECEIVE_EXIT} -eq 0 ]]; then
  script_fail "expected fast-forward-only receive to fail on diverged target"
fi

if ! grep -q "diverged (non-fast-forward)" "${RECEIVE_FAIL_LOG}"; then
  script_fail "expected diverged failure reason was not printed"
fi

INCOMING_REF="$(git -C "${RECEIVER_BARE}" for-each-ref refs/sync/incoming --format='%(refname)' | grep '/heads/main$' | head -n1 || true)"
if [[ -z "${INCOMING_REF}" ]]; then
  script_fail "expected incoming namespace ref after failed receive"
fi

script_log "Expected non-fast-forward failure confirmed."
script_log "Incoming ref preserved at: ${INCOMING_REF}"
script_log "Done."
script_log "Fixture root: ${TARGET_DIR}"
script_log "create log: ${CREATE_LOG}"
script_log "mergeability log: ${MERGEABILITY_LOG}"
script_log "ff-only failure log: ${RECEIVE_FAIL_LOG}"
