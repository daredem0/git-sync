#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

ROOT="$(mktemp -d -t git-sync-receive-matrix-XXXXXX)"
KEEP_TMP="${KEEP_TMP:-0}"

cleanup() {
  if [[ "${KEEP_TMP}" == "1" ]]; then
    echo "[INFO] KEEP_TMP=1 -> preserving temp directory: ${ROOT}"
    return
  fi
  rm -rf "${ROOT}"
}
trap cleanup EXIT

print_section() {
  echo
  echo "================================================================"
  echo "$1"
  echo "================================================================"
}

pass() {
  echo "[PASS] $1"
}

fail() {
  echo "[FAIL] $1"
  exit 1
}

run_expect_success() {
  local label="$1"
  shift
  local out_file="${ROOT}/cmd.out"
  local err_file="${ROOT}/cmd.err"
  if "$@" >"${out_file}" 2>"${err_file}"; then
    echo "[PASS] ${label}"
  else
    echo "[FAIL] ${label}"
    echo "--- stdout ---"
    cat "${out_file}" || true
    echo "--- stderr ---"
    cat "${err_file}" || true
    exit 1
  fi
}

run_expect_failure() {
  local label="$1"
  shift
  local out_file="${ROOT}/cmd.out"
  local err_file="${ROOT}/cmd.err"
  if "$@" >"${out_file}" 2>"${err_file}"; then
    echo "[FAIL] ${label} (unexpected success)"
    echo "--- stdout ---"
    cat "${out_file}" || true
    echo "--- stderr ---"
    cat "${err_file}" || true
    exit 1
  else
    echo "[PASS] ${label} (expected failure)"
  fi
}

rev_parse() {
  local repo="$1"
  local spec="$2"
  git -C "${repo}" rev-parse "${spec}"
}

incoming_ref_line_for_main() {
  local repo="$1"
  git -C "${repo}" for-each-ref refs/sync/incoming --format='%(refname) %(objectname)' \
    | grep '/heads/main ' || true
}

incoming_branch_line_for_main() {
  local repo="$1"
  git -C "${repo}" for-each-ref refs/heads/incoming --format='%(refname) %(objectname)' \
    | grep '/heads/main ' || true
}

print_repo_graph() {
  local case_label="$1"
  local stage="$2"
  local repo="$3"
  local first_ref
  first_ref="$(git -C "${repo}" for-each-ref --format='%(refname)' | sed -n '1p')"
  echo "[INFO] ${case_label} (${stage}) graph for ${repo}"
  if [[ -z "${first_ref}" ]]; then
    echo "  (no refs yet)"
    return
  fi
  git -C "${repo}" --no-pager log --oneline --graph --decorate --all | sed 's/^/  /'
}

print_repo_paths() {
  echo "[INFO] Repository paths for inspection:"
  echo "  origin:                     ${ROOT}/origin"
  echo "  work (bundle source):       ${ROOT}/work"
  echo "  recv_ff_ok:                 ${ROOT}/recv_ff_ok"
  echo "  recv_ff_fail:               ${ROOT}/recv_ff_fail"
  echo "  recv_ff_fail_w (worktree):  ${ROOT}/recv_ff_fail_w"
  echo "  recv_refonly_fail_prereq:   ${ROOT}/recv_refonly_fail_prereq"
  echo "  bundle archive:             ${BUNDLE}"
}

init_repo_main() {
  local path="$1"
  if git init -b main "${path}" >/dev/null 2>&1; then
    return
  fi
  git init "${path}" >/dev/null
  git -C "${path}" checkout -b main >/dev/null
}

print_section "Setup: origin and cloned work repo with merge history"
init_repo_main "${ROOT}/origin"
git -C "${ROOT}/origin" config user.name "Test User"
git -C "${ROOT}/origin" config user.email "test@example.com"

echo "line 1" > "${ROOT}/origin/app.txt"
run_expect_success "origin commit 1" git -C "${ROOT}/origin" add app.txt
run_expect_success "origin commit 1 write" git -C "${ROOT}/origin" commit -m "origin: initial"

echo "line 2" >> "${ROOT}/origin/app.txt"
run_expect_success "origin commit 2 add" git -C "${ROOT}/origin" add app.txt
run_expect_success "origin commit 2 write" git -C "${ROOT}/origin" commit -m "origin: second"

BASE_OID="$(rev_parse "${ROOT}/origin" HEAD)"
echo "[INFO] BASE_OID=${BASE_OID}"

run_expect_success "clone origin into work repo" git clone "${ROOT}/origin" "${ROOT}/work"
git -C "${ROOT}/work" config user.name "Test User"
git -C "${ROOT}/work" config user.email "test@example.com"

run_expect_success "create feature/login branch" git -C "${ROOT}/work" checkout -b feature/login
echo "login=true" > "${ROOT}/work/login.cfg"
run_expect_success "feature/login add" git -C "${ROOT}/work" add login.cfg
run_expect_success "feature/login commit" git -C "${ROOT}/work" commit -m "feat: login"

run_expect_success "switch to main" git -C "${ROOT}/work" checkout main
run_expect_success "create feature/payments branch" git -C "${ROOT}/work" checkout -b feature/payments
echo "payments=true" > "${ROOT}/work/payments.cfg"
run_expect_success "feature/payments add" git -C "${ROOT}/work" add payments.cfg
run_expect_success "feature/payments commit" git -C "${ROOT}/work" commit -m "feat: payments"

run_expect_success "switch to main again" git -C "${ROOT}/work" checkout main
run_expect_success "merge feature/login" git -C "${ROOT}/work" merge --no-ff feature/login -m "merge login"
run_expect_success "merge feature/payments" git -C "${ROOT}/work" merge --no-ff feature/payments -m "merge payments"

TIP_OID="$(rev_parse "${ROOT}/work" refs/heads/main)"
echo "[INFO] TIP_OID=${TIP_OID}"

print_section "Bundle creation"
print_repo_graph "Bundle source (work/main)" "before bundle creation" "${ROOT}/work"
run_expect_success \
  "create bundle from clone history" \
  cargo run --quiet -- create --repo "${ROOT}/work" --from "${BASE_OID}" --to main --output "${ROOT}/sync.bundle"
print_repo_graph "Bundle source (work/main)" "after bundle creation" "${ROOT}/work"

BUNDLE="${ROOT}/sync.bundle.zip"
[[ -f "${BUNDLE}" ]] || fail "bundle archive missing: ${BUNDLE}"
pass "bundle archive exists: ${BUNDLE}"

print_section "Case 1: fast-forward-only passes"
run_expect_success "init FF-pass receiver (bare clone from origin)" git clone --bare "${ROOT}/origin" "${ROOT}/recv_ff_ok"
print_repo_graph "Case 1" "before receive" "${ROOT}/recv_ff_ok"
run_expect_success \
  "receive with --integrate fast-forward-only" \
  cargo run --quiet -- receive --repo "${ROOT}/recv_ff_ok" --bundle "${BUNDLE}" --integrate fast-forward-only
print_repo_graph "Case 1" "after receive" "${ROOT}/recv_ff_ok"

RECV_FF_OK_MAIN="$(rev_parse "${ROOT}/recv_ff_ok" refs/heads/main)"
if [[ "${RECV_FF_OK_MAIN}" == "${TIP_OID}" ]]; then
  pass "fast-forward-only advanced refs/heads/main to bundle tip"
else
  fail "fast-forward-only did not advance main (got ${RECV_FF_OK_MAIN}, want ${TIP_OID})"
fi

INCOMING_MAIN_LINE="$(incoming_ref_line_for_main "${ROOT}/recv_ff_ok")"
if [[ -n "${INCOMING_MAIN_LINE}" ]]; then
  pass "incoming namespace ref for main exists after FF pass"
  echo "[INFO] ${INCOMING_MAIN_LINE}"
else
  fail "incoming namespace ref for main missing after FF pass"
fi

print_section "Case 2: fast-forward-only fails on diverged target"
run_expect_success "init FF-fail receiver (bare clone from origin)" git clone --bare "${ROOT}/origin" "${ROOT}/recv_ff_fail"
run_expect_success "create work clone for divergence" git clone "${ROOT}/recv_ff_fail" "${ROOT}/recv_ff_fail_w"
git -C "${ROOT}/recv_ff_fail_w" config user.name "Test User"
git -C "${ROOT}/recv_ff_fail_w" config user.email "test@example.com"

echo "receiver-only change" > "${ROOT}/recv_ff_fail_w/receiver.txt"
run_expect_success "receiver divergence add" git -C "${ROOT}/recv_ff_fail_w" add receiver.txt
run_expect_success "receiver divergence commit" git -C "${ROOT}/recv_ff_fail_w" commit -m "receiver diverges"
run_expect_success "push diverged main back to bare receiver" git -C "${ROOT}/recv_ff_fail_w" push origin main

DIVERGED_OID="$(rev_parse "${ROOT}/recv_ff_fail" refs/heads/main)"
echo "[INFO] DIVERGED_OID=${DIVERGED_OID}"

print_repo_graph "Case 2" "before receive" "${ROOT}/recv_ff_fail"
run_expect_failure \
  "receive with --integrate fast-forward-only on diverged target" \
  cargo run --quiet -- receive --repo "${ROOT}/recv_ff_fail" --bundle "${BUNDLE}" --integrate fast-forward-only
print_repo_graph "Case 2" "after receive attempt" "${ROOT}/recv_ff_fail"

AFTER_FF_FAIL_MAIN="$(rev_parse "${ROOT}/recv_ff_fail" refs/heads/main)"
if [[ "${AFTER_FF_FAIL_MAIN}" == "${DIVERGED_OID}" ]]; then
  pass "diverged target main remained unchanged after FF failure"
else
  fail "diverged target main changed unexpectedly (${AFTER_FF_FAIL_MAIN} != ${DIVERGED_OID})"
fi

INCOMING_MAIN_LINE="$(incoming_ref_line_for_main "${ROOT}/recv_ff_fail")"
if [[ -n "${INCOMING_MAIN_LINE}" ]]; then
  pass "incoming namespace ref for main exists after FF failure"
  echo "[INFO] ${INCOMING_MAIN_LINE}"
else
  fail "incoming namespace ref for main missing after FF failure"
fi

print_section "Case 3: create-refs-only passes and keeps target untouched"
print_repo_graph "Case 3" "before receive" "${ROOT}/recv_ff_fail"
run_expect_success \
  "receive with --integrate create-refs-only --incoming-as-branches on diverged receiver" \
  cargo run --quiet -- receive --repo "${ROOT}/recv_ff_fail" --bundle "${BUNDLE}" --integrate create-refs-only --incoming-as-branches
print_repo_graph "Case 3" "after receive" "${ROOT}/recv_ff_fail"

AFTER_REFONLY_PASS_MAIN="$(rev_parse "${ROOT}/recv_ff_fail" refs/heads/main)"
if [[ "${AFTER_REFONLY_PASS_MAIN}" == "${DIVERGED_OID}" ]]; then
  pass "create-refs-only kept refs/heads/main unchanged"
else
  fail "create-refs-only changed refs/heads/main unexpectedly (${AFTER_REFONLY_PASS_MAIN} != ${DIVERGED_OID})"
fi

INCOMING_MAIN_LINE="$(incoming_ref_line_for_main "${ROOT}/recv_ff_fail")"
if [[ -n "${INCOMING_MAIN_LINE}" ]]; then
  pass "incoming namespace ref for main exists after create-refs-only pass"
  echo "[INFO] ${INCOMING_MAIN_LINE}"
else
  fail "incoming namespace ref for main missing after create-refs-only pass"
fi

INCOMING_BRANCH_MAIN_LINE="$(incoming_branch_line_for_main "${ROOT}/recv_ff_fail")"
if [[ -n "${INCOMING_BRANCH_MAIN_LINE}" ]]; then
  pass "incoming branch mirror for main exists after --incoming-as-branches"
  echo "[INFO] ${INCOMING_BRANCH_MAIN_LINE}"
else
  fail "incoming branch mirror for main missing after --incoming-as-branches"
fi

print_section "Case 4: create-refs-only failure variants"
run_expect_success "init empty receiver without prerequisites" git init --bare "${ROOT}/recv_refonly_fail_prereq"
print_repo_graph "Case 4A (missing prerequisites)" "before receive" "${ROOT}/recv_refonly_fail_prereq"
run_expect_failure \
  "create-refs-only fails when prerequisites are missing" \
  cargo run --quiet -- receive --repo "${ROOT}/recv_refonly_fail_prereq" --bundle "${BUNDLE}" --integrate create-refs-only
print_repo_graph "Case 4A (missing prerequisites)" "after receive attempt" "${ROOT}/recv_refonly_fail_prereq"

INCOMING_MAIN_LINE="$(incoming_ref_line_for_main "${ROOT}/recv_refonly_fail_prereq")"
if [[ -z "${INCOMING_MAIN_LINE}" ]]; then
  pass "no incoming refs were created when import failed before object availability"
else
  fail "unexpected incoming refs present on prerequisite failure: ${INCOMING_MAIN_LINE}"
fi

run_expect_failure \
  "create-refs-only fails with missing bundle path" \
  cargo run --quiet -- receive --repo "${ROOT}/recv_refonly_fail_prereq" --bundle "${ROOT}/does-not-exist.bundle.zip" --integrate create-refs-only
print_repo_graph "Case 4B (missing bundle path)" "after receive attempt" "${ROOT}/recv_refonly_fail_prereq"

print_section "Summary"
pass "All receive integration matrix cases completed"
print_repo_paths
if [[ "${KEEP_TMP}" == "1" ]]; then
  echo "[INFO] KEEP_TMP=1 -> repos are preserved for inspection"
else
  echo "[INFO] KEEP_TMP=0 -> temp repos will be deleted at exit"
  echo "[INFO] Re-run with KEEP_TMP=1 to keep repos for manual inspection"
fi
