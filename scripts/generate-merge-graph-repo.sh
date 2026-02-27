#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  generate-merge-graph-repo.sh <target-dir>

Creates a Git repository with a deterministic commit graph that includes merge commits.
Useful for testing bundle create/audit/import flows.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -ne 1 ]]; then
  usage >&2
  exit 1
fi

target_dir="$1"
mkdir -p "$target_dir"

if [[ -n "$(ls -A "$target_dir" 2>/dev/null)" ]]; then
  echo "error: target directory is not empty: $target_dir" >&2
  exit 1
fi

repo_dir="$(cd "$target_dir" && pwd)"
cd "$repo_dir"

if ! git init -b main >/dev/null 2>&1; then
  git init >/dev/null
  git checkout -b main >/dev/null
fi

git config user.name "Audit Bot"
git config user.email "audit@example.com"

author_name="Audit Bot"
author_email="audit@example.com"
clock_step=0
clock_start_epoch=1704067200

next_git_date() {
  local ts=$((clock_start_epoch + clock_step * 60))
  clock_step=$((clock_step + 1))
  printf "%s +0000" "$ts"
}

git_commit_all() {
  local message="$1"
  local git_date
  git_date="$(next_git_date)"
  git add -A
  GIT_AUTHOR_NAME="$author_name" \
  GIT_AUTHOR_EMAIL="$author_email" \
  GIT_AUTHOR_DATE="$git_date" \
  GIT_COMMITTER_NAME="$author_name" \
  GIT_COMMITTER_EMAIL="$author_email" \
  GIT_COMMITTER_DATE="$git_date" \
    git commit -m "$message" >/dev/null
}

git_merge_no_ff() {
  local message="$1"
  local branch="$2"
  local git_date
  git_date="$(next_git_date)"
  GIT_AUTHOR_NAME="$author_name" \
  GIT_AUTHOR_EMAIL="$author_email" \
  GIT_AUTHOR_DATE="$git_date" \
  GIT_COMMITTER_NAME="$author_name" \
  GIT_COMMITTER_EMAIL="$author_email" \
  GIT_COMMITTER_DATE="$git_date" \
    git merge --no-ff "$branch" -m "$message" >/dev/null
}

mkdir -p src docs
cat > app.txt <<'EOF'
app=git-sync
state=init
EOF
cat > config.ini <<'EOF'
mode=dev
EOF
git_commit_all "chore: initial commit"

echo "baseline=true" >> config.ini
echo "# Git Sync Fixture" > docs/README.md
git_commit_all "feat: baseline on main"
base_oid="$(git rev-parse HEAD)"
git tag -a sync/base -m "bundle base anchor" "$base_oid" >/dev/null

git checkout -b feature/login >/dev/null 2>&1
mkdir -p src
cat > src/login.txt <<'EOF'
login=enabled
strategy=password
EOF
git_commit_all "feat(login): add login module"
echo "password_policy=strong" >> src/login.txt
echo "auth_provider=internal" >> config.ini
git_commit_all "feat(login): wire login settings"

git checkout main >/dev/null 2>&1
git checkout -b feature/payments "$base_oid" >/dev/null 2>&1
mkdir -p src
cat > src/payments.txt <<'EOF'
payments=enabled
gateway=test
EOF
git_commit_all "feat(payments): add payments module"
echo "retry_count=3" >> src/payments.txt
git_commit_all "feat(payments): configure retries"

git checkout main >/dev/null 2>&1
cat > CHANGELOG.md <<'EOF'
## 0.1.0
- bootstrap fixture graph
EOF
git_commit_all "docs: add changelog"

git_merge_no_ff "merge: feature/login into main" feature/login
merge_login_oid="$(git rev-parse HEAD)"
git tag -a sync/merge-login -m "first merge anchor" "$merge_login_oid" >/dev/null

echo "request_timeout_seconds=30" >> config.ini
git_commit_all "chore: tune config after login merge"

git_merge_no_ff "merge: feature/payments into main" feature/payments
merge_payments_oid="$(git rev-parse HEAD)"
git tag -a sync/merge-payments -m "second merge anchor" "$merge_payments_oid" >/dev/null

echo "state=ready" >> app.txt
git_commit_all "chore: post-merge cleanup"
tip_oid="$(git rev-parse HEAD)"
git tag -a sync/tip -m "bundle tip anchor" "$tip_oid" >/dev/null

cat <<EOF
Fixture repository created: $repo_dir

Anchor refs:
  sync/base           $base_oid
  sync/merge-login    $merge_login_oid
  sync/merge-payments $merge_payments_oid
  sync/tip            $tip_oid

Suggested bundle command:
  cargo run -- create --repo "$repo_dir" --from sync/base --to sync/tip --output sync.bundle

Commit graph:
EOF

git --no-pager log --graph --oneline --decorate --all
