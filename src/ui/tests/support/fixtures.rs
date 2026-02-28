//! Unit tests for fixtures.

use super::helpers::{commit_from_entries, commit_from_files, unique_temp_dir};
use crate::git::{self, CommitAuditEntry};
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct DiffFixture {
    pub(crate) source_dir: PathBuf,
    pub(crate) receiver_dir: PathBuf,
    pub(crate) bundle_archive_path: PathBuf,
    pub(crate) entries: Vec<CommitAuditEntry>,
}

impl Drop for DiffFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.source_dir);
        let _ = fs::remove_dir_all(&self.receiver_dir);
    }
}

/// Creates a representative text-diff fixture used by render/input tests.
pub(crate) fn create_diff_fixture() -> DiffFixture {
    let source_dir = unique_temp_dir("source");
    fs::create_dir_all(&source_dir).expect("must create source dir");
    let source_repo = git2::Repository::init(&source_dir).expect("must init source repo");

    let base_commit = commit_from_files(
        &source_repo,
        "base",
        &[("f.rs", "fn value() -> i32 { 1 }\n")],
        &[],
    );
    let tip_commit = commit_from_files(
        &source_repo,
        "tip",
        &[
            ("f.rs", "fn value() -> i32 { 2 }\n"),
            ("g.txt", "new file\n"),
        ],
        &[base_commit],
    );
    source_repo
        .reference("refs/heads/base", base_commit, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = source_dir.join("sync.bundle");
    let bundle_result = git::create_bundle(
        &source_dir,
        "refs/heads/base",
        "refs/heads/tip",
        &bundle_path,
    )
    .expect("must create bundle package");
    git::remove_unarchived_bundle_artifacts(&bundle_result)
        .expect("must remove unarchived artifacts");

    let receiver_dir = unique_temp_dir("receiver");
    fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo = git2::Repository::init_bare(&receiver_dir).expect("must init receiver");
    let mut source_remote = receiver_repo
        .remote_anonymous(source_dir.to_str().expect("source path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base ref");

    let head_entries = git::collect_head_audit_entries_for_bundle_input(
        &bundle_result.archive_path,
        &receiver_dir,
    )
    .expect("must collect head entries for fixture bundle");
    let entries = head_entries
        .first()
        .map(|entry| entry.commits.clone())
        .unwrap_or_default();
    assert_eq!(
        entries.len(),
        1,
        "fixture should contain one commit in base..tip"
    );

    DiffFixture {
        source_dir,
        receiver_dir,
        bundle_archive_path: bundle_result.archive_path,
        entries,
    }
}

/// Creates a fixture whose changed entry represents a non-text file change.
pub(crate) fn create_non_text_diff_fixture() -> DiffFixture {
    let source_dir = unique_temp_dir("source-non-text");
    fs::create_dir_all(&source_dir).expect("must create source dir");
    let source_repo = git2::Repository::init(&source_dir).expect("must init source repo");

    let base_commit =
        commit_from_entries(&source_repo, "base", &[("f.txt", b"base\n", 0o100644)], &[]);
    let tip_commit = commit_from_entries(
        &source_repo,
        "tip",
        &[
            ("f.txt", b"base\n", 0o100644),
            ("link-to-f", b"f.txt", 0o120000),
        ],
        &[base_commit],
    );
    source_repo
        .reference("refs/heads/base", base_commit, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = source_dir.join("sync.bundle");
    let bundle_result = git::create_bundle(
        &source_dir,
        "refs/heads/base",
        "refs/heads/tip",
        &bundle_path,
    )
    .expect("must create bundle package");
    git::remove_unarchived_bundle_artifacts(&bundle_result)
        .expect("must remove unarchived artifacts");

    let receiver_dir = unique_temp_dir("receiver-non-text");
    fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo = git2::Repository::init_bare(&receiver_dir).expect("must init receiver");
    let mut source_remote = receiver_repo
        .remote_anonymous(source_dir.to_str().expect("source path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base ref");

    let head_entries = git::collect_head_audit_entries_for_bundle_input(
        &bundle_result.archive_path,
        &receiver_dir,
    )
    .expect("must collect head entries for non-text fixture bundle");
    let entries = head_entries
        .first()
        .map(|entry| entry.commits.clone())
        .unwrap_or_default();
    assert_eq!(
        entries.len(),
        1,
        "non-text fixture should contain one commit in base..tip"
    );

    DiffFixture {
        source_dir,
        receiver_dir,
        bundle_archive_path: bundle_result.archive_path,
        entries,
    }
}
