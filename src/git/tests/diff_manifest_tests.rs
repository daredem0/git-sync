// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Tests for diff manifest behavior and invariants.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::support::*;
use super::*;

// Focus: tree-diff entry detection and deterministic diff-entry ordering.

fn commit_with_modes(
    repo: &git2::Repository,
    message: &str,
    files: &[(&str, &[u8], i32)],
    parent_oids: &[git2::Oid],
) -> git2::Oid {
    let mut builder = repo.treebuilder(None).expect("must create tree builder");
    for (path, content, mode) in files {
        let blob_id = repo.blob(content).expect("must create blob object");
        builder
            .insert(*path, blob_id, *mode)
            .expect("must insert file entry with mode");
    }

    let tree_id = builder.write().expect("must write tree");
    let tree = repo.find_tree(tree_id).expect("must find tree");
    let parent_commits: Vec<git2::Commit<'_>> = parent_oids
        .iter()
        .map(|oid| repo.find_commit(*oid).expect("must find parent commit"))
        .collect();
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();
    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create sig");
    repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
        .expect("must create commit")
}

// Verifies that collect_diff_entries returns an empty list when base and tip are the same commit.
#[test]
fn collect_diff_entries_returns_empty_when_base_equals_tip() {
    let repo_dir = temp_repo_dir("diff-entries-empty");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let commit_id = commit_from_files(&repo, "single commit", &[("file.txt", "content")], &[]);
    let entries = collect_diff_entries(&repo, commit_id, commit_id)
        .expect("collect_diff_entries should succeed for identical commits");
    assert!(
        entries.is_empty(),
        "no diff entries should be reported when base and tip are identical"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_diff_entries reports added, modified, and deleted file statuses.
#[test]
fn collect_diff_entries_detect_added_modified_deleted_files() {
    let repo_dir = temp_repo_dir("diff-entries-amd");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let base_commit_id = commit_from_files(
        &repo,
        "base commit",
        &[
            ("keep.txt", "same"),
            ("modify.txt", "before"),
            ("delete.txt", "to-delete"),
        ],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[
            ("keep.txt", "same"),
            ("modify.txt", "after"),
            ("added.txt", "new"),
        ],
        &[base_commit_id],
    );

    let entries = collect_diff_entries(&repo, base_commit_id, tip_commit_id)
        .expect("collect_diff_entries should produce diff entries");
    let mut by_path = std::collections::HashMap::new();
    for entry in entries {
        by_path.insert(entry.path.clone(), entry.status);
    }

    assert_eq!(
        by_path.get("added.txt"),
        Some(&ChangeStatus::Added),
        "added file must be reported as Added"
    );
    assert_eq!(
        by_path.get("modify.txt"),
        Some(&ChangeStatus::Modified),
        "modified file must be reported as Modified"
    );
    assert_eq!(
        by_path.get("delete.txt"),
        Some(&ChangeStatus::Deleted),
        "deleted file must be reported as Deleted"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_diff_entries returns path-sorted output for deterministic audits.
#[test]
fn collect_diff_entries_returns_stable_sorted_output() {
    let repo_dir = temp_repo_dir("diff-entries-sorted");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let base_commit_id =
        commit_from_files(&repo, "base commit", &[("z.txt", "1"), ("m.txt", "1")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("z.txt", "2"), ("m.txt", "2"), ("a.txt", "new")],
        &[base_commit_id],
    );

    let entries = collect_diff_entries(&repo, base_commit_id, tip_commit_id)
        .expect("collect_diff_entries should produce deterministic output");
    let paths: Vec<String> = entries.iter().map(|entry| entry.path.clone()).collect();

    assert_eq!(
        paths,
        vec![
            "a.txt".to_string(),
            "m.txt".to_string(),
            "z.txt".to_string()
        ],
        "diff entry list must be sorted by path"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_diff_entries reports renames with both old and new paths.
#[test]
fn collect_diff_entries_detect_renames() {
    let repo_dir = temp_repo_dir("diff-entries-rename");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let base_commit_id = commit_from_files(
        &repo,
        "base commit",
        &[("old_name.txt", "same-content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("new_name.txt", "same-content")],
        &[base_commit_id],
    );

    let entries = collect_diff_entries(&repo, base_commit_id, tip_commit_id)
        .expect("collect_diff_entries should detect rename changes");
    assert_eq!(
        entries.len(),
        1,
        "exactly one rename entry should be reported"
    );

    let rename = &entries[0];
    assert_eq!(
        rename.status,
        ChangeStatus::Renamed,
        "rename operation must be reported with Renamed status"
    );
    assert_eq!(
        rename.old_path.as_deref(),
        Some("old_name.txt"),
        "rename entry must include the old path"
    );
    assert_eq!(
        rename.path, "new_name.txt",
        "rename entry path must be the new path"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_diff_entries reports file-to-symlink transitions as type changes.
#[test]
fn collect_diff_entries_detect_type_changes() {
    let repo_dir = temp_repo_dir("diff-entries-typechange");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let base_commit_id = commit_with_modes(
        &repo,
        "base commit",
        &[("mode.txt", b"plain-file", 0o100644)],
        &[],
    );
    let tip_commit_id = commit_with_modes(
        &repo,
        "tip commit",
        &[("mode.txt", b"target/path", 0o120000)],
        &[base_commit_id],
    );

    let entries = collect_diff_entries(&repo, base_commit_id, tip_commit_id)
        .expect("collect_diff_entries should detect type changes");
    assert_eq!(
        entries.len(),
        1,
        "exactly one type-change entry should be reported"
    );

    let entry = &entries[0];
    assert_eq!(
        entry.status,
        ChangeStatus::TypeChanged,
        "mode transition must be reported as TypeChanged"
    );
    assert_eq!(entry.path, "mode.txt");
    assert_eq!(entry.old_mode, Some(0o100644));
    assert_eq!(entry.new_mode, Some(0o120000));
    assert!(
        entry.old_oid.is_some() && entry.new_oid.is_some(),
        "type-change entries should retain old/new object IDs"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}
