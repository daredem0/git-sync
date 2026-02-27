use super::*;
use std::path::PathBuf;

// Verifies that open_context rejects a repository path that does not exist.
#[test]
fn open_context_fails_when_repo_path_does_not_exist() {
    let repo_path = std::env::temp_dir().join(format!(
        "git-sync-audit-missing-repo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));

    let cfg = AppConfig {
        repo_path,
        bundle_path: PathBuf::from("unused.bundle"),
        base_ref: "sync/last".to_string(),
        tip_ref: None,
    };

    let result = open_context(&cfg);
    assert!(
        result.is_err(),
        "open_context must reject a non-existent repository path"
    );
}

// Verifies that open_context rejects a path that exists but is not a Git repository.
#[test]
fn open_context_fails_when_path_exists_but_is_not_git_repo() {
    let dir = std::env::temp_dir().join(format!(
        "git-sync-audit-not-a-repo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));

    std::fs::create_dir_all(&dir).expect("must create temp dir");

    let cfg = AppConfig {
        repo_path: dir.clone(),
        bundle_path: std::path::PathBuf::from("unused.bundle"),
        base_ref: "sync/last".to_string(),
        tip_ref: None,
    };

    let result = open_context(&cfg);
    assert!(
        result.is_err(),
        "open_context must reject an existing directory that is not a git repository"
    );

    let _ = std::fs::remove_dir_all(dir);
}

// Verifies that open_context rejects a missing bundle path.
#[test]
fn open_context_fails_when_bundle_path_does_not_exist() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    git2::Repository::init(&repo_dir).expect("must init git repo");

    let missing_bundle = repo_dir.join("missing.bundle");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path: missing_bundle,
        base_ref: "sync/last".to_string(),
        tip_ref: None,
    };

    let result = open_context(&cfg);
    assert!(result.is_err(), "missing bundle path must be rejected");

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context rejects a base ref that cannot be resolved.
#[test]
fn open_context_fails_when_base_ref_is_missing() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-missing-base-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    git2::Repository::init(&repo_dir).expect("must init git repo");

    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: None,
    };

    let result = open_context(&cfg);
    assert!(
        result.is_err(),
        "open_context must reject configuration when base_ref is missing"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context succeeds when repository, bundle path, and base ref are valid.
#[test]
fn open_context_succeeds_when_base_ref_exists() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-base-exists-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    let tree_id = {
        let mut index = repo.index().expect("must open index");
        index.write_tree().expect("must write tree")
    };
    let tree = repo.find_tree(tree_id).expect("must find tree");
    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
        .expect("must create commit");
    repo.reference("refs/heads/sync/last", commit_id, true, "create base ref")
        .expect("must create base ref");

    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: None,
    };

    let result = open_context(&cfg);
    assert!(result.is_ok(), "valid base_ref should be accepted");

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context rejects a configured tip ref when it cannot be resolved.
#[test]
fn open_context_fails_when_tip_ref_is_missing() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-missing-tip-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    let tree_id = {
        let mut index = repo.index().expect("must open index");
        index.write_tree().expect("must write tree")
    };
    let tree = repo.find_tree(tree_id).expect("must find tree");
    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
        .expect("must create commit");
    repo.reference("refs/heads/sync/last", commit_id, true, "create base ref")
        .expect("must create base ref");

    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: Some("refs/heads/develop".to_string()),
    };

    let result = open_context(&cfg);
    assert!(
        result.is_err(),
        "open_context must reject a missing configured tip_ref"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context rejects a base ref that resolves to a non-commit object.
#[test]
fn open_context_fails_when_base_ref_is_not_a_commit() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-base-not-commit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");
    let blob_oid = repo.blob(b"blob-object").expect("must create blob object");

    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: blob_oid.to_string(),
        tip_ref: None,
    };

    let result = open_context(&cfg);
    assert!(
        result.is_err(),
        "open_context must reject base_ref values that are not commit objects"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context rejects a bundle path that exists but is not a valid Git bundle file.
#[test]
fn open_context_fails_when_bundle_file_is_not_a_git_bundle() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-invalid-bundle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    let tree_id = {
        let mut index = repo.index().expect("must open index");
        index.write_tree().expect("must write tree")
    };
    let tree = repo.find_tree(tree_id).expect("must find tree");
    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
        .expect("must create commit");
    repo.reference("refs/heads/sync/last", commit_id, true, "create base ref")
        .expect("must create base ref");

    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"not-a-bundle").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: None,
    };

    let result = open_context(&cfg);
    assert!(
        result.is_err(),
        "open_context must reject bundle files that are not valid git bundles"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context rejects a tip ref that resolves to a non-commit object.
#[test]
fn open_context_fails_when_tip_ref_is_not_a_commit() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-tip-not-commit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    let tree_id = {
        let mut index = repo.index().expect("must open index");
        index.write_tree().expect("must write tree")
    };
    let tree = repo.find_tree(tree_id).expect("must find tree");
    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
        .expect("must create commit");
    repo.reference("refs/heads/sync/last", commit_id, true, "create base ref")
        .expect("must create base ref");

    let blob_oid = repo.blob(b"tip-is-blob").expect("must create blob object");
    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: Some(blob_oid.to_string()),
    };

    let result = open_context(&cfg);
    assert!(
        result.is_err(),
        "open_context must reject tip_ref values that are not commit objects"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context succeeds when tip_ref resolves to a valid commit.
#[test]
fn open_context_succeeds_when_tip_ref_exists() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-tip-exists-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    let tree_id = {
        let mut index = repo.index().expect("must open index");
        index.write_tree().expect("must write tree")
    };
    let tree = repo.find_tree(tree_id).expect("must find tree");
    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
        .expect("must create commit");
    repo.reference("refs/heads/sync/last", commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/develop", commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: Some("refs/heads/develop".to_string()),
    };

    let result = open_context(&cfg);
    assert!(result.is_ok(), "valid tip_ref should be accepted");

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context returns resolved base and tip commit IDs for downstream audit operations.
#[test]
fn open_context_returns_resolved_commit_ids() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-returns-context-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    let base_tree_id = {
        let mut index = repo.index().expect("must open index");
        index.write_tree().expect("must write base tree")
    };
    let base_tree = repo.find_tree(base_tree_id).expect("must find base tree");
    let base_commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "base commit", &base_tree, &[])
        .expect("must create base commit");
    let base_commit = repo
        .find_commit(base_commit_id)
        .expect("must find base commit");

    let tip_tree_id = {
        let mut index = repo.index().expect("must open index");
        index
            .add_frombuffer(
                &git2::IndexEntry {
                    ctime: git2::IndexTime::new(0, 0),
                    mtime: git2::IndexTime::new(0, 0),
                    dev: 0,
                    ino: 0,
                    mode: 0o100644,
                    uid: 0,
                    gid: 0,
                    file_size: 0,
                    id: repo.blob(b"tip content").expect("must create blob"),
                    flags: 0,
                    flags_extended: 0,
                    path: b"tip.txt".to_vec(),
                },
                b"tip content",
            )
            .expect("must add tip file");
        index.write_tree().expect("must write tip tree")
    };
    let tip_tree = repo.find_tree(tip_tree_id).expect("must find tip tree");
    let tip_commit_id = repo
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            "tip commit",
            &tip_tree,
            &[&base_commit],
        )
        .expect("must create tip commit");

    repo.reference(
        "refs/heads/sync/last",
        base_commit_id,
        true,
        "create base ref",
    )
    .expect("must create base ref");
    repo.reference("refs/heads/develop", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: Some("refs/heads/develop".to_string()),
    };

    let context = open_context(&cfg).expect("open_context should resolve commit ids");
    assert_eq!(
        context.base_commit_id, base_commit_id,
        "base commit id should match the resolved base ref"
    );
    assert_eq!(
        context.tip_commit_id,
        Some(tip_commit_id),
        "tip commit id should match the resolved tip ref"
    );
    assert_eq!(
        context.bundle_version,
        BundleVersion::V2,
        "bundle version should reflect the parsed bundle header"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context returns no tip commit id when tip_ref is omitted.
#[test]
fn open_context_returns_none_tip_commit_id_when_tip_ref_is_not_provided() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-no-tip-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    let tree_id = {
        let mut index = repo.index().expect("must open index");
        index.write_tree().expect("must write tree")
    };
    let tree = repo.find_tree(tree_id).expect("must find tree");
    let base_commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "base commit", &tree, &[])
        .expect("must create base commit");
    repo.reference(
        "refs/heads/sync/last",
        base_commit_id,
        true,
        "create base ref",
    )
    .expect("must create base ref");

    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: None,
    };

    let context = open_context(&cfg).expect("open_context should resolve base commit");
    assert_eq!(
        context.tip_commit_id, None,
        "tip commit id must be None when no tip_ref is configured"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context accepts v3 bundle headers and reports the parsed version.
#[test]
fn open_context_succeeds_when_bundle_header_is_v3() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-v3-bundle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    let tree_id = {
        let mut index = repo.index().expect("must open index");
        index.write_tree().expect("must write tree")
    };
    let tree = repo.find_tree(tree_id).expect("must find tree");
    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "base commit", &tree, &[])
        .expect("must create base commit");
    repo.reference("refs/heads/sync/last", commit_id, true, "create base ref")
        .expect("must create base ref");

    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"# v3 git bundle\n").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: None,
    };

    let context = open_context(&cfg).expect("open_context should accept v3 bundle header");
    assert_eq!(
        context.bundle_version,
        BundleVersion::V3,
        "bundle version must be parsed as v3 when v3 header is provided"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context rejects a bundle path that points to a directory instead of a file.
#[test]
fn open_context_fails_when_bundle_path_is_directory() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-bundle-dir-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    let tree_id = {
        let mut index = repo.index().expect("must open index");
        index.write_tree().expect("must write tree")
    };
    let tree = repo.find_tree(tree_id).expect("must find tree");
    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "base commit", &tree, &[])
        .expect("must create base commit");
    repo.reference("refs/heads/sync/last", commit_id, true, "create base ref")
        .expect("must create base ref");

    let bundle_path = repo_dir.join("bundle-dir");
    std::fs::create_dir_all(&bundle_path).expect("must create bundle directory");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: None,
    };

    let result = open_context(&cfg);
    assert!(
        result.is_err(),
        "open_context must reject bundle paths that are directories"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that open_context rejects tip refs that are not descendants of the base ref.
#[test]
fn open_context_fails_when_tip_ref_is_not_descendant_of_base_ref() {
    let repo_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-repo-tip-not-descendant-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    let tree_id = {
        let mut index = repo.index().expect("must open index");
        index.write_tree().expect("must write tree")
    };
    let tree = repo.find_tree(tree_id).expect("must find tree");
    let root_commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "root commit", &tree, &[])
        .expect("must create root commit");
    let root_commit = repo
        .find_commit(root_commit_id)
        .expect("must find root commit");

    let base_commit_id = repo
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            "base branch commit",
            &tree,
            &[&root_commit],
        )
        .expect("must create base branch commit");

    let diverged_tip_commit_id = repo
        .commit(
            None,
            &sig,
            &sig,
            "diverged tip commit",
            &tree,
            &[&root_commit],
        )
        .expect("must create diverged tip commit");

    repo.reference(
        "refs/heads/sync/last",
        base_commit_id,
        true,
        "create base ref",
    )
    .expect("must create base ref");
    repo.reference(
        "refs/heads/develop",
        diverged_tip_commit_id,
        true,
        "create diverged tip ref",
    )
    .expect("must create tip ref");

    let bundle_path = repo_dir.join("input.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n").expect("must create bundle file");

    let cfg = AppConfig {
        repo_path: repo_dir.clone(),
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: Some("refs/heads/develop".to_string()),
    };

    let result = open_context(&cfg);
    assert!(
        result.is_err(),
        "open_context must reject tip refs that do not descend from base"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

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

fn temp_repo_dir(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "git-sync-audit-{}-{}-{}",
        suffix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ))
}

fn commit_from_files(
    repo: &git2::Repository,
    message: &str,
    files: &[(&str, &str)],
    parent_oids: &[git2::Oid],
) -> git2::Oid {
    let mut builder = repo.treebuilder(None).expect("must create tree builder");
    for (path, content) in files {
        let blob_id = repo
            .blob(content.as_bytes())
            .expect("must create blob object");
        builder
            .insert(*path, blob_id, 0o100644)
            .expect("must insert file entry in tree");
    }
    let tree_id = builder.write().expect("must write tree");
    let tree = repo.find_tree(tree_id).expect("must find tree");

    let parent_commits: Vec<git2::Commit<'_>> = parent_oids
        .iter()
        .map(|oid| repo.find_commit(*oid).expect("must find parent commit"))
        .collect();
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();
    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");

    repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
        .expect("must create commit")
}
