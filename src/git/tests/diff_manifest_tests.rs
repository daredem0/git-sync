use super::support::*;
use super::*;

// Focus: tree-diff change detection and stable manifest rendering (TSV/JSON).
// Verifies that collect_changed_files returns an empty list when base and tip are the same commit.
#[test]
fn collect_changed_files_returns_empty_when_base_equals_tip() {
    let repo_dir = temp_repo_dir("changes-empty");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let commit_id = commit_from_files(&repo, "single commit", &[("file.txt", "content")], &[]);

    let changes = collect_changed_files(&repo_dir, commit_id, commit_id)
        .expect("collect_changed_files should succeed for identical commits");
    assert!(
        changes.is_empty(),
        "no changed files should be reported when base and tip are identical"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_changed_files reports added, modified, and deleted file statuses.
#[test]
fn collect_changed_files_detects_added_modified_deleted_files() {
    let repo_dir = temp_repo_dir("changes-amd");
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

    let changes = collect_changed_files(&repo_dir, base_commit_id, tip_commit_id)
        .expect("collect_changed_files should produce a changed-file manifest");

    let mut by_path = std::collections::HashMap::new();
    for change in changes {
        by_path.insert(change.path.clone(), change.status);
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

// Verifies that collect_changed_files returns results sorted by path for deterministic audit output.
#[test]
fn collect_changed_files_returns_stable_sorted_output() {
    let repo_dir = temp_repo_dir("changes-sorted");
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

    let changes = collect_changed_files(&repo_dir, base_commit_id, tip_commit_id)
        .expect("collect_changed_files should produce deterministic output");
    let paths: Vec<String> = changes.iter().map(|c| c.path.clone()).collect();

    assert_eq!(
        paths,
        vec![
            "a.txt".to_string(),
            "m.txt".to_string(),
            "z.txt".to_string()
        ],
        "changed file list must be sorted by path"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_changed_files reports renames with both old and new paths.
#[test]
fn collect_changed_files_detects_renames() {
    let repo_dir = temp_repo_dir("changes-rename");
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

    let changes = collect_changed_files(&repo_dir, base_commit_id, tip_commit_id)
        .expect("collect_changed_files should detect rename changes");
    assert_eq!(
        changes.len(),
        1,
        "exactly one rename change should be reported"
    );

    let rename = &changes[0];
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

// Verifies that render_manifest returns a stable header and no entries for an empty change list.
#[test]
fn render_manifest_returns_only_header_for_empty_list() {
    let output = render_manifest(&[]);
    assert_eq!(
        output, "STATUS\tPATH\tOLD_PATH\tOLD_OID\tNEW_OID\n",
        "empty manifest should contain only the header line"
    );
}

// Verifies that render_manifest formats A/M/D/R entries in a deterministic tab-separated format.
#[test]
fn render_manifest_formats_added_modified_deleted_and_renamed_entries() {
    let added_oid =
        git2::Oid::from_str("1111111111111111111111111111111111111111").expect("must parse oid");
    let modified_old_oid =
        git2::Oid::from_str("2222222222222222222222222222222222222222").expect("must parse oid");
    let modified_new_oid =
        git2::Oid::from_str("3333333333333333333333333333333333333333").expect("must parse oid");
    let deleted_oid =
        git2::Oid::from_str("4444444444444444444444444444444444444444").expect("must parse oid");
    let renamed_old_oid =
        git2::Oid::from_str("5555555555555555555555555555555555555555").expect("must parse oid");
    let renamed_new_oid =
        git2::Oid::from_str("6666666666666666666666666666666666666666").expect("must parse oid");

    let changes = vec![
        ChangedFile {
            status: ChangeStatus::Added,
            path: "a.txt".to_string(),
            old_path: None,
            old_oid: None,
            new_oid: Some(added_oid),
        },
        ChangedFile {
            status: ChangeStatus::Modified,
            path: "m.txt".to_string(),
            old_path: None,
            old_oid: Some(modified_old_oid),
            new_oid: Some(modified_new_oid),
        },
        ChangedFile {
            status: ChangeStatus::Deleted,
            path: "d.txt".to_string(),
            old_path: None,
            old_oid: Some(deleted_oid),
            new_oid: None,
        },
        ChangedFile {
            status: ChangeStatus::Renamed,
            path: "new_name.txt".to_string(),
            old_path: Some("old_name.txt".to_string()),
            old_oid: Some(renamed_old_oid),
            new_oid: Some(renamed_new_oid),
        },
    ];

    let output = render_manifest(&changes);
    let expected = concat!(
        "STATUS\tPATH\tOLD_PATH\tOLD_OID\tNEW_OID\n",
        "A\ta.txt\t-\t-\t1111111111111111111111111111111111111111\n",
        "M\tm.txt\t-\t2222222222222222222222222222222222222222\t3333333333333333333333333333333333333333\n",
        "D\td.txt\t-\t4444444444444444444444444444444444444444\t-\n",
        "R\tnew_name.txt\told_name.txt\t5555555555555555555555555555555555555555\t6666666666666666666666666666666666666666\n"
    );
    assert_eq!(
        output, expected,
        "manifest output must use deterministic tab-separated line rendering"
    );
}

// Verifies that render_manifest_json returns an empty JSON array when no changes are provided.
#[test]
fn render_manifest_json_returns_empty_array_for_empty_list() {
    let output = render_manifest_json(&[]).expect("json rendering should succeed");
    let value: serde_json::Value =
        serde_json::from_str(&output).expect("json output should be valid");
    assert_eq!(
        value,
        serde_json::json!([]),
        "empty change list must render as an empty JSON array"
    );
}

// Verifies that render_manifest_json includes expected fields and preserves entry ordering.
#[test]
fn render_manifest_json_formats_entries_and_preserves_order() {
    let first_new_oid =
        git2::Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").expect("must parse oid");
    let second_old_oid =
        git2::Oid::from_str("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").expect("must parse oid");
    let second_new_oid =
        git2::Oid::from_str("cccccccccccccccccccccccccccccccccccccccc").expect("must parse oid");

    let changes = vec![
        ChangedFile {
            status: ChangeStatus::Added,
            path: "a.txt".to_string(),
            old_path: None,
            old_oid: None,
            new_oid: Some(first_new_oid),
        },
        ChangedFile {
            status: ChangeStatus::Renamed,
            path: "new.txt".to_string(),
            old_path: Some("old.txt".to_string()),
            old_oid: Some(second_old_oid),
            new_oid: Some(second_new_oid),
        },
    ];

    let output = render_manifest_json(&changes).expect("json rendering should succeed");
    let value: serde_json::Value =
        serde_json::from_str(&output).expect("json output should be valid");
    let entries = value
        .as_array()
        .expect("top-level JSON output should be an array");

    assert_eq!(entries.len(), 2, "expected two JSON manifest entries");
    assert_eq!(
        entries[0]["status"],
        serde_json::json!("A"),
        "first entry status code must match Added"
    );
    assert_eq!(
        entries[0]["path"],
        serde_json::json!("a.txt"),
        "first entry path must be serialized"
    );
    assert_eq!(
        entries[0]["old_path"],
        serde_json::Value::Null,
        "missing old path must serialize to null"
    );
    assert_eq!(
        entries[0]["new_oid"],
        serde_json::json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        "first entry new oid must be serialized as hex string"
    );

    assert_eq!(
        entries[1]["status"],
        serde_json::json!("R"),
        "second entry status code must match Renamed"
    );
    assert_eq!(
        entries[1]["path"],
        serde_json::json!("new.txt"),
        "second entry path must preserve original ordering"
    );
    assert_eq!(
        entries[1]["old_path"],
        serde_json::json!("old.txt"),
        "second entry old path must be serialized"
    );
    assert_eq!(
        entries[1]["old_oid"],
        serde_json::json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        "second entry old oid must be serialized as hex string"
    );
}
