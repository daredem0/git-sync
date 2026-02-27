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

// Verifies that create_bundle writes a v2 bundle with prerequisite and tip lines followed by PACK data.
#[test]
fn create_bundle_writes_valid_bundle_header_and_pack_data() {
    let repo_dir = temp_repo_dir("create-bundle-file");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed for a linear range");

    assert_eq!(
        result.from_commit_id, base_commit_id,
        "from commit in result should match resolved base ref"
    );
    assert_eq!(
        result.to_commit_id, tip_commit_id,
        "to commit in result should match resolved tip ref"
    );
    assert_eq!(
        result.tip_ref_name, "refs/heads/tip",
        "tip ref name should preserve the resolved tip reference when available"
    );

    let bytes = std::fs::read(&bundle_path).expect("must read created bundle");
    assert!(
        bytes.starts_with(b"# v2 git bundle\n"),
        "bundle should start with the v2 bundle signature line"
    );

    let header_preview = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
    assert!(
        header_preview.contains(&format!("-{base_commit_id}")),
        "bundle header should contain prerequisite commit line"
    );
    assert!(
        header_preview.contains(&format!("{tip_commit_id} refs/heads/tip")),
        "bundle header should contain tip commit to ref mapping"
    );
    assert!(
        bytes.windows(4).any(|w| w == b"PACK"),
        "bundle should contain a packfile payload"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that create_bundle rejects ranges where the end commit does not descend from the start commit.
#[test]
fn create_bundle_fails_when_to_commit_is_not_descendant_of_from_commit() {
    let repo_dir = temp_repo_dir("create-bundle-not-descendant");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let root_commit_id = commit_from_files(&repo, "root commit", &[("f.txt", "root")], &[]);
    let base_commit_id = commit_from_files(
        &repo,
        "base branch commit",
        &[("f.txt", "base branch")],
        &[root_commit_id],
    );
    let tip_commit_id = commit_from_files(
        &repo,
        "diverged tip commit",
        &[("f.txt", "tip branch")],
        &[root_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path);
    assert!(
        result.is_err(),
        "create_bundle must reject non-linear ranges for deterministic incremental export"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that a created bundle can be fetched into another repo when prerequisite commits are present.
#[test]
fn create_bundle_can_be_fetched_when_prerequisite_is_present() {
    use std::io::Write as _;

    let repo_dir = temp_repo_dir("create-bundle-fetch");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let receiver_dir = temp_repo_dir("create-bundle-fetch-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");

    // First, seed prerequisite history into receiver (simulates receiver already having base).
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite");

    // Then apply bundle pack payload into receiver object database via Indexer.
    let bundle_bytes = std::fs::read(&bundle_path).expect("must read created bundle");
    let pack_offset = bundle_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .expect("bundle must contain PACK payload");
    let pack_data = &bundle_bytes[pack_offset..];

    let odb = receiver_repo.odb().expect("must open receiver odb");
    let mut indexer = git2::Indexer::new(
        Some(&odb),
        receiver_repo.path().join("objects").join("pack").as_path(),
        0o644,
        true,
    )
    .expect("must create indexer");
    indexer
        .write_all(pack_data)
        .expect("must write pack payload into indexer");
    indexer.commit().expect("must finalize indexed pack");

    let imported_tip = receiver_repo
        .find_commit(tip_commit_id)
        .expect("tip commit from bundle pack should be present after indexing");
    assert_eq!(
        imported_tip.id(),
        tip_commit_id,
        "imported tip commit should match original tip commit id"
    );
    assert_eq!(
        result.tip_ref_name, "refs/heads/tip",
        "result metadata should preserve exported tip ref name"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive_bundle_input imports a zip-packaged bundle and updates exported head refs when prerequisites exist.
#[test]
fn receive_bundle_input_imports_zip_bundle_and_updates_heads_when_prerequisite_exists() {
    let repo_dir = temp_repo_dir("receive-bundle-success");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let bundle_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    remove_unarchived_bundle_artifacts(&bundle_result).expect("must remove loose bundle artifacts");

    let receiver_dir = temp_repo_dir("receive-bundle-success-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");

    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    let receive_result = receive_bundle_input(&bundle_result.archive_path, &receiver_dir)
        .expect("receive_bundle_input should succeed when prerequisites are present");
    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "receive should import exactly one exported head for this bundle"
    );
    assert_eq!(
        receive_result.imported_heads[0].reference, "refs/heads/tip",
        "receive should update the exported tip ref"
    );
    assert_eq!(
        receive_result.imported_heads[0].oid, tip_commit_id,
        "receive should point exported tip ref at the imported tip commit"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let tip_ref = receiver_repo
        .find_reference("refs/heads/tip")
        .expect("tip ref should exist after receive");
    assert_eq!(
        tip_ref.target(),
        Some(tip_commit_id),
        "receiver tip ref should match imported tip commit"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receiving the same bundle package twice is idempotent and does not create additional packfiles.
#[test]
fn receive_bundle_input_is_idempotent_when_same_package_is_applied_twice() {
    let repo_dir = temp_repo_dir("receive-bundle-idempotent");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let bundle_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    remove_unarchived_bundle_artifacts(&bundle_result).expect("must remove loose bundle artifacts");

    let receiver_dir = temp_repo_dir("receive-bundle-idempotent-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    let first_receive = receive_bundle_input(&bundle_result.archive_path, &receiver_dir)
        .expect("first receive should succeed");
    assert_eq!(
        first_receive.imported_heads.len(),
        1,
        "fixture range should import exactly one head"
    );

    let pack_dir = receiver_dir.join("objects").join("pack");
    let mut pack_entries_after_first = std::fs::read_dir(&pack_dir)
        .expect("receiver pack dir should exist")
        .map(|entry| {
            entry
                .expect("pack dir entry should be readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    pack_entries_after_first.sort();

    let second_receive = receive_bundle_input(&bundle_result.archive_path, &receiver_dir)
        .expect("second receive should also succeed");
    assert_eq!(
        second_receive.imported_heads, first_receive.imported_heads,
        "idempotent receive should report the same imported heads"
    );

    let mut pack_entries_after_second = std::fs::read_dir(&pack_dir)
        .expect("receiver pack dir should still exist")
        .map(|entry| {
            entry
                .expect("pack dir entry should be readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    pack_entries_after_second.sort();
    assert_eq!(
        pack_entries_after_second, pack_entries_after_first,
        "second receive should not add new pack files when package is already applied"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let tip_ref = receiver_repo
        .find_reference("refs/heads/tip")
        .expect("tip ref should exist");
    assert_eq!(
        tip_ref.target(),
        Some(tip_commit_id),
        "tip ref should remain pinned to the imported tip commit after repeated receive"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive_bundle_input fails when the receiver lacks prerequisite objects required by the bundle pack.
#[test]
fn receive_bundle_input_fails_when_prerequisite_is_missing() {
    let repo_dir = temp_repo_dir("receive-bundle-missing-prereq");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let bundle_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    remove_unarchived_bundle_artifacts(&bundle_result).expect("must remove loose bundle artifacts");

    let receiver_dir = temp_repo_dir("receive-bundle-missing-prereq-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");

    let receive_result = receive_bundle_input(&bundle_result.archive_path, &receiver_dir);
    assert!(
        receive_result.is_err(),
        "receive should fail when receiver does not have prerequisite commit history"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    assert!(
        receiver_repo.find_reference("refs/heads/tip").is_err(),
        "failed receive should not create tip ref"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive_bundle_input_with_options rejects imports when metadata verification is enabled and sidecar content is tampered.
#[test]
fn receive_bundle_input_with_options_rejects_tampered_metadata_when_verification_enabled() {
    let repo_dir = temp_repo_dir("receive-bundle-verify-metadata-fail");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    let metadata_bytes = std::fs::read(&caudit_path).expect("must read generated metadata");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");
    metadata["bundle_sha256"] =
        serde_json::Value::String("00000000000000000000000000000000".to_string());
    std::fs::write(
        &caudit_path,
        serde_json::to_vec_pretty(&metadata).expect("must serialize tampered metadata"),
    )
    .expect("must write tampered metadata");

    let receiver_dir = temp_repo_dir("receive-bundle-verify-metadata-fail-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    let receive_result = receive_bundle_input_with_options(
        &bundle_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: true,
        },
    );
    assert!(
        receive_result.is_err(),
        "receive with verify_metadata=true must reject tampered metadata"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    assert!(
        receiver_repo.find_reference("refs/heads/tip").is_err(),
        "failed receive should not update tip ref"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive_bundle_input_with_options can import when metadata verification is disabled even if metadata sidecar was tampered.
#[test]
fn receive_bundle_input_with_options_allows_tampered_metadata_when_verification_disabled() {
    let repo_dir = temp_repo_dir("receive-bundle-verify-metadata-disabled");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    let metadata_bytes = std::fs::read(&caudit_path).expect("must read generated metadata");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");
    metadata["bundle_sha256"] =
        serde_json::Value::String("ffffffffffffffffffffffffffffffff".to_string());
    std::fs::write(
        &caudit_path,
        serde_json::to_vec_pretty(&metadata).expect("must serialize tampered metadata"),
    )
    .expect("must write tampered metadata");

    let receiver_dir = temp_repo_dir("receive-bundle-verify-metadata-disabled-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    let receive_result = receive_bundle_input_with_options(
        &bundle_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: false,
        },
    )
    .expect("receive with verify_metadata=false should not block on tampered metadata");

    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "receive should import one head from this bundle range"
    );
    assert_eq!(
        receive_result.imported_heads[0].oid, tip_commit_id,
        "imported head must point to source tip commit"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let tip_ref = receiver_repo
        .find_reference("refs/heads/tip")
        .expect("tip ref should exist after receive");
    assert_eq!(
        tip_ref.target(),
        Some(tip_commit_id),
        "receiver tip ref should match source tip commit"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that create_bundle writes a .caudit.json sidecar with core audit identity fields.
#[test]
fn create_bundle_writes_caudit_metadata_file_with_core_identity_fields() {
    let repo_dir = temp_repo_dir("create-bundle-caudit-core");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let expected_caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    assert_eq!(
        result.audit_path, expected_caudit_path,
        "create_bundle should return the generated .caudit.json path"
    );
    assert!(
        result.audit_path.exists(),
        "create_bundle should write a .caudit.json metadata file"
    );

    let metadata_bytes =
        std::fs::read(&result.audit_path).expect("must read generated .caudit metadata file");
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");

    assert_eq!(
        metadata["schema_version"],
        serde_json::json!("1"),
        "schema version must match the initial metadata contract"
    );
    assert_eq!(
        metadata["range_from_oid"],
        serde_json::json!(base_commit_id.to_string()),
        "metadata must record the resolved from commit id"
    );
    assert_eq!(
        metadata["range_to_oid"],
        serde_json::json!(tip_commit_id.to_string()),
        "metadata must record the resolved to commit id"
    );
    assert_eq!(
        metadata["tip_ref"],
        serde_json::json!("refs/heads/tip"),
        "metadata must preserve the exported tip reference name"
    );
    assert_eq!(
        metadata["bundle_path"],
        serde_json::json!(bundle_path.display().to_string()),
        "metadata must include the bundle path used during creation"
    );
    assert_eq!(
        metadata["bundle_header_version"],
        serde_json::json!("v2"),
        "metadata should report the bundle format version"
    );
    let generated_by_username = metadata["generated_by_username"]
        .as_str()
        .expect("metadata should include generated_by_username as a string");
    assert!(
        !generated_by_username.is_empty(),
        "generated_by_username should not be empty"
    );
    let generated_by_hostname = metadata["generated_by_hostname"]
        .as_str()
        .expect("metadata should include generated_by_hostname as a string");
    assert!(
        !generated_by_hostname.is_empty(),
        "generated_by_hostname should not be empty"
    );
    let bundle_bytes = std::fs::read(&bundle_path).expect("must read generated bundle bytes");
    let expected_bundle_sha256 = sha256_hex(&bundle_bytes).expect("must hash bundle bytes");
    assert_eq!(
        metadata["bundle_size_bytes"],
        serde_json::json!(bundle_bytes.len() as u64),
        "metadata must report the exact bundle byte length"
    );
    let bundle_sha256 = metadata["bundle_sha256"]
        .as_str()
        .expect("bundle_sha256 should be present as a string");
    assert_eq!(
        bundle_sha256, expected_bundle_sha256,
        "metadata bundle_sha256 must match the actual bundle file content digest"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that create_bundle writes a .zip archive containing at least the bundle and metadata files.
#[test]
fn create_bundle_writes_archive_with_bundle_and_metadata_entries() {
    let repo_dir = temp_repo_dir("create-bundle-archive");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let expected_archive_path = PathBuf::from(format!("{}.zip", bundle_path.display()));
    assert_eq!(
        result.archive_path, expected_archive_path,
        "create_bundle should return a deterministic archive path next to the bundle"
    );
    assert!(
        result.archive_path.exists(),
        "create_bundle should write a .zip archive"
    );

    let archive_bytes =
        std::fs::read(&result.archive_path).expect("must read generated archive bytes");
    assert!(
        archive_bytes.starts_with(b"PK\x03\x04"),
        "archive should use ZIP local-header signature"
    );
    let archive_text = String::from_utf8_lossy(&archive_bytes);
    assert!(
        archive_text.contains("range.bundle"),
        "archive should contain the bundle file entry name"
    );
    assert!(
        archive_text.contains("range.bundle.caudit.json"),
        "archive should contain the metadata file entry name"
    );
    assert!(
        !archive_text.contains("range.bundle.caudit.patch"),
        "default archive should not include patch sidecar entry when patches are disabled"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that create_bundle metadata stays compact by omitting inline per-file patch text by default.
#[test]
fn create_bundle_caudit_omits_inline_patch_details_by_default() {
    let repo_dir = temp_repo_dir("create-bundle-caudit-patch");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base content")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "other")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    let metadata_bytes =
        std::fs::read(&result.audit_path).expect("must read generated .caudit metadata file");
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");

    let changed_files = metadata["changed_files"]
        .as_array()
        .expect("changed_files should be serialized as an array");
    let modified_f_txt = changed_files
        .iter()
        .find(|entry| {
            entry["status"] == serde_json::json!("M") && entry["path"] == serde_json::json!("f.txt")
        })
        .expect("changed_files should include f.txt as a modified entry");

    assert_eq!(
        modified_f_txt["is_binary"],
        serde_json::json!(false),
        "text file changes should be marked as non-binary"
    );
    assert!(
        modified_f_txt.get("patch").is_none(),
        "compact metadata should not embed full unified patch text per changed file"
    );
    assert!(
        metadata["patch_sidecar"].is_null(),
        "compact metadata should not include a patch sidecar descriptor unless explicitly requested"
    );
    assert!(
        result.patch_audit_path.is_none(),
        "create_bundle result should not expose a patch sidecar path by default"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that create_bundle can optionally write a patch sidecar and reference it from metadata.
#[test]
fn create_bundle_with_patch_sidecar_writes_and_references_sidecar() {
    let repo_dir = temp_repo_dir("create-bundle-caudit-sidecar");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base content")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "other")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle_with_options(
        &repo_dir,
        "refs/heads/base",
        "refs/heads/tip",
        &bundle_path,
        CreateBundleOptions {
            include_patch_sidecar: true,
        },
    )
    .expect("create_bundle_with_options should succeed with patch sidecar enabled");

    let patch_path = result
        .patch_audit_path
        .clone()
        .expect("patch sidecar path should be returned when enabled");
    assert!(
        patch_path.exists(),
        "patch sidecar should be written to disk"
    );

    let patch_bytes = std::fs::read(&patch_path).expect("must read patch sidecar bytes");
    let patch_text = String::from_utf8_lossy(&patch_bytes);
    assert!(
        patch_text.contains("base content"),
        "patch sidecar should include previous text content"
    );
    assert!(
        patch_text.contains("tip content"),
        "patch sidecar should include updated text content"
    );

    let metadata_bytes =
        std::fs::read(&result.audit_path).expect("must read generated .caudit metadata file");
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");
    let sidecar = metadata["patch_sidecar"]
        .as_object()
        .expect("metadata should include a patch_sidecar descriptor");
    let path_from_metadata = sidecar
        .get("path")
        .and_then(|value| value.as_str())
        .expect("patch_sidecar.path must be present");
    let sha_from_metadata = sidecar
        .get("sha256")
        .and_then(|value| value.as_str())
        .expect("patch_sidecar.sha256 must be present");
    assert_eq!(
        path_from_metadata,
        patch_path.display().to_string(),
        "metadata should reference the exact patch sidecar path"
    );
    assert_eq!(
        sha_from_metadata,
        sha256_hex(&patch_bytes).expect("must hash patch sidecar"),
        "metadata sidecar sha256 should match patch sidecar bytes"
    );
    assert!(
        result.archive_path.exists(),
        "archive path should be generated when patch sidecar is enabled"
    );
    let archive_bytes =
        std::fs::read(&result.archive_path).expect("must read generated archive bytes");
    let archive_text = String::from_utf8_lossy(&archive_bytes);
    assert!(
        archive_text.contains("range.bundle.caudit.patch"),
        "archive should include patch sidecar entry when patch generation is enabled"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that bundle metadata validation succeeds when the generated metadata matches the source repository state.
#[test]
fn verify_bundle_metadata_against_repo_accepts_matching_metadata() {
    let repo_dir = temp_repo_dir("verify-caudit-matching");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    verify_bundle_metadata_against_repo(&bundle_path, &repo_dir)
        .expect("metadata verification should succeed when metadata and repo state match");

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_changed_files_from_bundle_input reads changed-file metadata from a zip-only bundle package.
#[test]
fn collect_changed_files_from_bundle_input_accepts_zip_archive_without_loose_bundle_files() {
    let repo_dir = temp_repo_dir("collect-changes-input-zip");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base content")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip content"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    remove_unarchived_bundle_artifacts(&result).expect("must remove loose bundle artifacts");

    let changes = collect_changed_files_from_bundle_input(&result.archive_path)
        .expect("collect_changed_files_from_bundle_input should read metadata from zip package");
    assert!(
        !changes.is_empty(),
        "zip-contained metadata should provide changed file entries"
    );
    assert!(
        changes.iter().any(|entry| entry.path == "f.txt"),
        "changed file list from zip metadata should include modified file paths"
    );
    assert!(
        changes.iter().any(|entry| entry.path == "new.txt"),
        "changed file list from zip metadata should include added file paths"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata verification accepts zip-only bundle packages when no loose bundle files remain.
#[test]
fn verify_bundle_metadata_against_repo_input_accepts_zip_archive_without_loose_bundle_files() {
    let repo_dir = temp_repo_dir("verify-caudit-input-zip");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    remove_unarchived_bundle_artifacts(&result).expect("must remove loose bundle artifacts");

    verify_bundle_metadata_against_repo_input(&result.archive_path, &repo_dir).expect(
        "metadata verification should succeed when zip-contained bundle metadata matches repo truth",
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that bundle metadata validation rejects tampered metadata content.
#[test]
fn verify_bundle_metadata_against_repo_rejects_tampered_metadata() {
    let repo_dir = temp_repo_dir("verify-caudit-tampered");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    let metadata_bytes = std::fs::read(&caudit_path).expect("must read created caudit metadata");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");
    metadata["range_to_oid"] = serde_json::json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    std::fs::write(
        &caudit_path,
        serde_json::to_vec_pretty(&metadata).expect("must serialize tampered metadata"),
    )
    .expect("must write tampered metadata");

    let result = verify_bundle_metadata_against_repo(&bundle_path, &repo_dir);
    assert!(
        result.is_err(),
        "verification must reject metadata that does not match repository truth"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that inspect_bundle parses version, prerequisite, and head entries from a created bundle.
#[test]
fn inspect_bundle_parses_created_bundle_metadata() {
    let repo_dir = temp_repo_dir("inspect-bundle");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let inspection = inspect_bundle(&bundle_path).expect("bundle inspection should succeed");
    assert_eq!(
        inspection.version,
        BundleVersion::V2,
        "created bundle should use v2 bundle format"
    );
    assert_eq!(
        inspection.prerequisites,
        vec![base_commit_id],
        "inspection should parse prerequisite commit list"
    );
    assert_eq!(
        inspection.heads.len(),
        1,
        "inspection should parse one head"
    );
    assert_eq!(
        inspection.heads[0].oid, tip_commit_id,
        "inspection should parse head oid"
    );
    assert_eq!(
        inspection.heads[0].reference, "refs/heads/tip",
        "inspection should parse head reference name"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that inspect_bundle rejects files that do not begin with a valid bundle header signature.
#[test]
fn inspect_bundle_rejects_invalid_header_signature() {
    let bundle_path = std::env::temp_dir().join(format!(
        "git-sync-audit-invalid-bundle-header-{}-{}.bundle",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::write(&bundle_path, b"not-a-bundle\nPACK").expect("must write invalid bundle file");

    let result = inspect_bundle(&bundle_path);
    assert!(
        result.is_err(),
        "inspect_bundle must reject files with an invalid bundle signature line"
    );

    let _ = std::fs::remove_file(bundle_path);
}

// Verifies that resolve_repo_audit_range resolves commit ids from revspecs when the range is linear.
#[test]
fn resolve_repo_audit_range_accepts_linear_range() {
    let repo_dir = temp_repo_dir("repo-range-linear");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id =
        commit_from_files(&repo, "tip commit", &[("f.txt", "tip")], &[base_commit_id]);
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let range = resolve_repo_audit_range(&repo_dir, "refs/heads/base", "refs/heads/tip")
        .expect("linear repo range should resolve");
    assert_eq!(
        range.base_commit_id, base_commit_id,
        "base oid should resolve"
    );
    assert_eq!(range.tip_commit_id, tip_commit_id, "tip oid should resolve");

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that resolve_repo_audit_range rejects ranges where tip is not a descendant of base.
#[test]
fn resolve_repo_audit_range_rejects_non_descendant_tip() {
    let repo_dir = temp_repo_dir("repo-range-non-descendant");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let root_commit_id = commit_from_files(&repo, "root commit", &[("f.txt", "root")], &[]);
    let base_commit_id = commit_from_files(
        &repo,
        "base branch commit",
        &[("f.txt", "base branch")],
        &[root_commit_id],
    );
    let tip_commit_id = commit_from_files(
        &repo,
        "diverged tip commit",
        &[("f.txt", "tip branch")],
        &[root_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let result = resolve_repo_audit_range(&repo_dir, "refs/heads/base", "refs/heads/tip");
    assert!(
        result.is_err(),
        "repo audit range must reject non-descendant tip commits"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that inspect_bundle rejects non-existent bundle paths.
#[test]
fn inspect_bundle_rejects_missing_path() {
    let missing_path = std::env::temp_dir().join(format!(
        "git-sync-audit-missing-inspect-bundle-{}-{}.bundle",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));

    let result = inspect_bundle(&missing_path);
    assert!(
        result.is_err(),
        "inspect_bundle must reject non-existent bundle paths"
    );
}

// Verifies that inspect_bundle rejects paths that are directories instead of files.
#[test]
fn inspect_bundle_rejects_directory_path() {
    let bundle_dir = temp_repo_dir("inspect-bundle-dir");
    std::fs::create_dir_all(&bundle_dir).expect("must create bundle directory");

    let result = inspect_bundle(&bundle_dir);
    assert!(
        result.is_err(),
        "inspect_bundle must reject directory paths"
    );

    let _ = std::fs::remove_dir_all(bundle_dir);
}

// Verifies that extract_bundle_archive rejects missing archive paths.
#[test]
fn extract_bundle_archive_rejects_missing_path() {
    let missing_archive = std::env::temp_dir().join(format!(
        "git-sync-audit-missing-archive-{}-{}.zip",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));

    let result = extract_bundle_archive(&missing_archive);
    assert!(
        result.is_err(),
        "extract_bundle_archive must reject non-existent archive paths"
    );
}

// Verifies that extract_bundle_archive rejects archive paths that are directories.
#[test]
fn extract_bundle_archive_rejects_directory_path() {
    let archive_dir = temp_repo_dir("extract-archive-dir");
    std::fs::create_dir_all(&archive_dir).expect("must create archive directory");

    let result = extract_bundle_archive(&archive_dir);
    assert!(
        result.is_err(),
        "extract_bundle_archive must reject directory archive paths"
    );

    let _ = std::fs::remove_dir_all(archive_dir);
}

// Verifies that extract_bundle_archive rejects zip archives without any .bundle entry.
#[test]
fn extract_bundle_archive_rejects_zip_without_bundle_entry() {
    let work_dir = temp_repo_dir("extract-archive-no-bundle");
    std::fs::create_dir_all(&work_dir).expect("must create work dir");
    let archive_path = work_dir.join("input.zip");
    write_test_zip(&archive_path, &[("note.txt", b"not a bundle")]);

    let result = extract_bundle_archive(&archive_path);
    assert!(
        result.is_err(),
        "archive extraction must fail when no .bundle entry exists"
    );

    let _ = std::fs::remove_dir_all(work_dir);
}

// Verifies that extract_bundle_archive rejects zip archives containing multiple .bundle entries.
#[test]
fn extract_bundle_archive_rejects_zip_with_multiple_bundle_entries() {
    let work_dir = temp_repo_dir("extract-archive-multi-bundle");
    std::fs::create_dir_all(&work_dir).expect("must create work dir");
    let archive_path = work_dir.join("input.zip");
    write_test_zip(
        &archive_path,
        &[
            ("a.bundle", b"# v2 git bundle\n\nPACK"),
            ("b.bundle", b"# v2 git bundle\n\nPACK"),
        ],
    );

    let result = extract_bundle_archive(&archive_path);
    assert!(
        result.is_err(),
        "archive extraction must fail when multiple .bundle entries exist"
    );

    let _ = std::fs::remove_dir_all(work_dir);
}

// Verifies that metadata integrity validation rejects unsupported schema versions.
#[test]
fn verify_bundle_metadata_integrity_rejects_unsupported_schema_version() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-schema", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["schema_version"] = serde_json::json!("999");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject unsupported schema version values"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects bundle header version mismatches.
#[test]
fn verify_bundle_metadata_integrity_rejects_header_version_mismatch() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-header", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["bundle_header_version"] = serde_json::json!("v9");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject mismatched bundle_header_version values"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects prerequisite lists that do not match bundle header prerequisites.
#[test]
fn verify_bundle_metadata_integrity_rejects_prerequisite_mismatch() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-prereq", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["prerequisites"] = serde_json::json!([]);
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject prerequisite mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects head entries that do not match the bundle header.
#[test]
fn verify_bundle_metadata_integrity_rejects_heads_mismatch() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-heads", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["heads"][0]["reference"] = serde_json::json!("refs/heads/other");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject head mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects tip_ref/range_to_oid combinations that do not match any head entry.
#[test]
fn verify_bundle_metadata_integrity_rejects_tip_head_consistency_mismatch() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-tip", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["tip_ref"] = serde_json::json!("refs/heads/does-not-exist");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject tip_ref/range_to_oid mismatches against head list"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects unsupported patch sidecar format values.
#[test]
fn verify_bundle_metadata_integrity_rejects_patch_sidecar_unsupported_format() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("integrity-patch-format", true);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["patch_sidecar"]["format"] = serde_json::json!("unknown-format");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject unsupported patch sidecar formats"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects missing patch sidecar paths.
#[test]
fn verify_bundle_metadata_integrity_rejects_missing_patch_sidecar_path() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("integrity-patch-missing", true);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    let missing_patch_path = repo_dir.join("missing-sidecar.patch");
    metadata["patch_sidecar"]["path"] = serde_json::json!(missing_patch_path.display().to_string());
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject missing patch sidecar paths"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects patch sidecars with mismatched size values.
#[test]
fn verify_bundle_metadata_integrity_rejects_patch_sidecar_size_mismatch() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("integrity-patch-size", true);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    let size = metadata["patch_sidecar"]["size_bytes"]
        .as_u64()
        .expect("patch sidecar size should be present");
    metadata["patch_sidecar"]["size_bytes"] = serde_json::json!(size + 1);
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject patch sidecar size mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects patch sidecars with mismatched SHA-256 values.
#[test]
fn verify_bundle_metadata_integrity_rejects_patch_sidecar_sha_mismatch() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-patch-sha", true);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["patch_sidecar"]["sha256"] =
        serde_json::json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject patch sidecar sha mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_changed_files_from_bundle_input rejects unknown status codes in metadata.
#[test]
fn collect_changed_files_from_bundle_input_rejects_unknown_status_code() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("parse-status", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["changed_files"][0]["status"] = serde_json::json!("Z");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = collect_changed_files_from_bundle_input(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "changed-file parsing must reject unknown status codes"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_changed_files_from_bundle_input rejects invalid object IDs in metadata.
#[test]
fn collect_changed_files_from_bundle_input_rejects_invalid_oid() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("parse-oid", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["changed_files"][0]["new_oid"] = serde_json::json!("not-an-oid");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = collect_changed_files_from_bundle_input(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "changed-file parsing must reject invalid oid values"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that receive_bundle_input rejects bundles that do not declare any head entries.
#[test]
fn receive_bundle_input_rejects_bundle_without_heads() {
    let work_dir = temp_repo_dir("receive-no-heads");
    std::fs::create_dir_all(&work_dir).expect("must create work dir");
    let bundle_path = work_dir.join("empty-heads.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n\nPACK").expect("must write bundle file");

    let receiver_dir = temp_repo_dir("receive-no-heads-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");

    let result = receive_bundle_input(&bundle_path, &receiver_dir);
    assert!(
        result.is_err(),
        "receive must reject bundle payloads without any head entries"
    );

    let _ = std::fs::remove_dir_all(work_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that is_head_already_applied resolves symbolic refs (such as HEAD) before comparing target OIDs.
#[test]
fn is_head_already_applied_resolves_symbolic_reference_targets() {
    let repo_dir = temp_repo_dir("receive-symbolic-head");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let commit_id = commit_from_files(&repo, "commit", &[("f.txt", "content")], &[]);
    repo.reference("refs/heads/main", commit_id, true, "set main")
        .expect("must create main ref");
    repo.set_head("refs/heads/main")
        .expect("must set HEAD symbolic ref");

    let applied = is_head_already_applied(
        &repo,
        &BundleHead {
            oid: commit_id,
            reference: "HEAD".to_string(),
        },
    )
    .expect("symbolic reference resolution should not fail");
    assert!(
        applied,
        "symbolic HEAD should resolve to refs/heads/main and match commit id"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that is_head_already_applied returns false when symbolic refs cannot be resolved.
#[test]
fn is_head_already_applied_returns_false_for_broken_symbolic_reference() {
    let repo_dir = temp_repo_dir("receive-broken-symbolic");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let commit_id = commit_from_files(&repo, "commit", &[("f.txt", "content")], &[]);
    repo.reference_symbolic(
        "refs/heads/broken-symbolic",
        "refs/heads/does-not-exist",
        true,
        "create broken symbolic ref",
    )
    .expect("must create broken symbolic ref");

    let applied = is_head_already_applied(
        &repo,
        &BundleHead {
            oid: commit_id,
            reference: "refs/heads/broken-symbolic".to_string(),
        },
    )
    .expect("broken symbolic ref lookup should not return hard error");
    assert!(
        !applied,
        "broken symbolic refs should not be treated as already applied"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata verification rejects ranges whose from/to commits are present but not in ancestor-descendant order.
#[test]
fn verify_bundle_metadata_against_repo_rejects_non_linear_range_in_metadata() {
    let repo_dir = temp_repo_dir("verify-caudit-non-linear-range");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let root_commit_id = commit_from_files(&repo, "root", &[("f.txt", "root")], &[]);
    let base_commit_id = commit_from_files(&repo, "base", &[("f.txt", "base")], &[root_commit_id]);
    let tip_commit_id = commit_from_files(&repo, "tip", &[("f.txt", "tip")], &[base_commit_id]);
    let side_commit_id = commit_from_files(&repo, "side", &[("f.txt", "side")], &[root_commit_id]);
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    let mut metadata = read_json_value(&caudit_path);
    metadata["range_from_oid"] = serde_json::json!(side_commit_id.to_string());
    write_json_value(&caudit_path, &metadata);

    let result = verify_bundle_metadata_against_repo(&bundle_path, &repo_dir);
    assert!(
        result.is_err(),
        "verification must reject metadata where range_from_oid is not an ancestor of range_to_oid"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata verification rejects mismatched commit_chain values even when commit ids are valid and linear.
#[test]
fn verify_bundle_metadata_against_repo_rejects_commit_chain_mismatch() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("verify-caudit-chain", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["commit_chain"] = serde_json::json!([]);
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_against_repo(&bundle_result.bundle_path, &repo_dir);
    assert!(
        result.is_err(),
        "verification must reject metadata commit_chain mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata verification rejects mismatched changed_files values even when commit ids are valid and linear.
#[test]
fn verify_bundle_metadata_against_repo_rejects_changed_files_mismatch() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("verify-caudit-changes", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["changed_files"] = serde_json::json!([]);
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_against_repo(&bundle_result.bundle_path, &repo_dir);
    assert!(
        result.is_err(),
        "verification must reject metadata changed_files mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that verify_bundle_metadata_against_repo_input supports plain .bundle input paths in addition to zip archives.
#[test]
fn verify_bundle_metadata_against_repo_input_accepts_plain_bundle_input() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("verify-caudit-plain-input", false);

    verify_bundle_metadata_against_repo_input(&bundle_result.bundle_path, &repo_dir)
        .expect("verification should succeed for plain bundle inputs");

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that receive with verify_metadata=true supports plain .bundle input paths (non-zip) on the success path.
#[test]
fn receive_bundle_input_with_options_accepts_plain_bundle_with_verification() {
    let (repo_dir, bundle_result, base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-plain-verify", false);
    let receiver_dir = temp_repo_dir("receive-plain-verify-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite");

    let receive_result = receive_bundle_input_with_options(
        &bundle_result.bundle_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: true,
        },
    )
    .expect("receive should succeed with plain bundle input when metadata is valid");
    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "fixture bundle should import exactly one head"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let base_ref = receiver_repo
        .find_reference("refs/heads/base")
        .expect("base prerequisite ref should exist");
    assert_eq!(
        base_ref.target(),
        Some(base_commit_id),
        "base ref should match"
    );
    let tip_ref = receiver_repo
        .find_reference("refs/heads/tip")
        .expect("tip ref should exist");
    assert_eq!(
        tip_ref.target(),
        Some(tip_commit_id),
        "tip ref should match"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that verify_bundle_metadata_integrity_input accepts plain .bundle input paths on the success path.
#[test]
fn verify_bundle_metadata_integrity_input_accepts_plain_bundle_input() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("integrity-plain-input", false);
    verify_bundle_metadata_integrity_input(&bundle_result.bundle_path)
        .expect("integrity verification should succeed for valid plain bundle input");

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that removing unarchived artifacts also removes optional patch sidecar files when present.
#[test]
fn remove_unarchived_bundle_artifacts_removes_optional_patch_sidecar() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("remove-artifacts-patch", true);
    let patch_path = bundle_result
        .patch_audit_path
        .as_ref()
        .expect("patch sidecar path should be present when enabled")
        .clone();
    assert!(
        patch_path.exists(),
        "patch sidecar should exist before cleanup"
    );

    remove_unarchived_bundle_artifacts(&bundle_result)
        .expect("cleanup should succeed when patch sidecar exists");
    assert!(
        !bundle_result.bundle_path.exists(),
        "bundle file should be removed by cleanup"
    );
    assert!(
        !bundle_result.audit_path.exists(),
        "audit json should be removed by cleanup"
    );
    assert!(
        !patch_path.exists(),
        "patch sidecar should be removed by cleanup"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that remove_file_if_exists returns an error for paths that exist but are directories.
#[test]
fn remove_file_if_exists_rejects_directory_paths() {
    let dir_path = temp_repo_dir("remove-file-dir");
    std::fs::create_dir_all(&dir_path).expect("must create test directory");

    let result = remove_file_if_exists(&dir_path);
    assert!(
        result.is_err(),
        "remove_file_if_exists should error when given a directory path"
    );

    let _ = std::fs::remove_dir_all(dir_path);
}

// Verifies that write_zip_archive rejects missing archive input files.
#[test]
fn write_zip_archive_rejects_missing_input_file() {
    let work_dir = temp_repo_dir("zip-missing-input");
    std::fs::create_dir_all(&work_dir).expect("must create work dir");
    let archive_path = work_dir.join("out.zip");
    let missing_input = work_dir.join("missing.txt");

    let result = write_zip_archive(&archive_path, &[missing_input]);
    assert!(
        result.is_err(),
        "write_zip_archive should reject missing input paths"
    );

    let _ = std::fs::remove_dir_all(work_dir);
}

// Verifies that write_zip_archive rejects directory inputs in the file list.
#[test]
fn write_zip_archive_rejects_directory_input() {
    let work_dir = temp_repo_dir("zip-directory-input");
    std::fs::create_dir_all(&work_dir).expect("must create work dir");
    let archive_path = work_dir.join("out.zip");
    let input_dir = work_dir.join("dir-input");
    std::fs::create_dir_all(&input_dir).expect("must create directory input");

    let result = write_zip_archive(&archive_path, &[input_dir]);
    assert!(
        result.is_err(),
        "write_zip_archive should reject directory input entries"
    );

    let _ = std::fs::remove_dir_all(work_dir);
}

// Verifies that load_bundle_metadata_from_path rejects missing metadata sidecar files.
#[test]
fn load_bundle_metadata_from_path_rejects_missing_path() {
    let missing_path = std::env::temp_dir().join(format!(
        "git-sync-audit-missing-caudit-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));

    let result = load_bundle_metadata_from_path(&missing_path);
    assert!(
        result.is_err(),
        "metadata loader should reject missing paths"
    );
}

// Verifies that load_bundle_metadata_from_path rejects directory paths.
#[test]
fn load_bundle_metadata_from_path_rejects_directory_path() {
    let metadata_dir = temp_repo_dir("load-metadata-dir");
    std::fs::create_dir_all(&metadata_dir).expect("must create metadata dir");

    let result = load_bundle_metadata_from_path(&metadata_dir);
    assert!(
        result.is_err(),
        "metadata loader should reject directory paths"
    );

    let _ = std::fs::remove_dir_all(metadata_dir);
}

// Verifies that resolve_patch_sidecar_path falls back to metadata sibling directory when explicit path does not exist.
#[test]
fn resolve_patch_sidecar_path_uses_sibling_when_explicit_path_is_missing() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("resolve-sidecar-sibling", true);
    let patch_path = bundle_result
        .patch_audit_path
        .as_ref()
        .expect("patch sidecar path should exist")
        .clone();
    let patch_file_name = patch_path
        .file_name()
        .expect("patch sidecar should have file name")
        .to_string_lossy()
        .to_string();

    let patch_sidecar = CreateBundleAuditPatchSidecar {
        path: format!("missing-dir/{patch_file_name}"),
        format: "unified-diff".to_string(),
        size_bytes: 0,
        sha256: String::new(),
    };
    let resolved_path = resolve_patch_sidecar_path(&bundle_result.audit_path, &patch_sidecar)
        .expect("sibling resolution should succeed");
    assert_eq!(
        resolved_path, patch_path,
        "patch sidecar resolution should fallback to metadata sibling directory"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that receive rejects bundles whose header head OID is missing after pack import.
#[test]
fn receive_bundle_input_rejects_when_head_oid_is_missing_after_import() {
    let (repo_dir, bundle_result, base_commit_id, _) =
        create_linear_bundle_fixture("receive-missing-head-commit", false);
    let bundle_bytes =
        std::fs::read(&bundle_result.bundle_path).expect("must read generated bundle bytes");
    let pack_offset = bundle_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .expect("bundle must contain pack payload");
    let pack_data = &bundle_bytes[pack_offset..];

    let fake_head_oid = "ffffffffffffffffffffffffffffffffffffffff";
    let tampered_header =
        format!("# v2 git bundle\n-{base_commit_id}\n{fake_head_oid} refs/heads/tip\n\n");
    let mut tampered_bytes = tampered_header.into_bytes();
    tampered_bytes.extend_from_slice(pack_data);
    std::fs::write(&bundle_result.bundle_path, tampered_bytes)
        .expect("must write tampered bundle header");

    let receiver_dir = temp_repo_dir("receive-missing-head-commit-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let receive_result = receive_bundle_input(&bundle_result.bundle_path, &receiver_dir);
    assert!(
        receive_result.is_err(),
        "receive should fail when declared head oid is missing after pack import"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that is_head_already_applied returns false when the current ref target OID differs from requested head OID.
#[test]
fn is_head_already_applied_returns_false_when_ref_target_differs() {
    let repo_dir = temp_repo_dir("head-applied-target-differs");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init repo");

    let commit_a = commit_from_files(&repo, "A", &[("f.txt", "a")], &[]);
    let commit_b = commit_from_files(&repo, "B", &[("f.txt", "b")], &[commit_a]);
    repo.reference("refs/heads/main", commit_a, true, "set main")
        .expect("must create main ref");

    let applied = is_head_already_applied(
        &repo,
        &BundleHead {
            oid: commit_b,
            reference: "refs/heads/main".to_string(),
        },
    )
    .expect("ref comparison should not fail");
    assert!(
        !applied,
        "head should not be treated as applied when target oid differs"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

fn create_linear_bundle_fixture(
    suffix: &str,
    include_patch_sidecar: bool,
) -> (PathBuf, CreateBundleResult, git2::Oid, git2::Oid) {
    let repo_dir = temp_repo_dir(suffix);
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = if include_patch_sidecar {
        create_bundle_with_options(
            &repo_dir,
            "refs/heads/base",
            "refs/heads/tip",
            &bundle_path,
            CreateBundleOptions {
                include_patch_sidecar: true,
            },
        )
        .expect("create_bundle_with_options should succeed")
    } else {
        create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
            .expect("create_bundle should succeed")
    };

    (repo_dir, result, base_commit_id, tip_commit_id)
}

fn read_json_value(path: &std::path::Path) -> serde_json::Value {
    let bytes = std::fs::read(path).expect("must read json file");
    serde_json::from_slice(&bytes).expect("json content should be valid")
}

fn write_json_value(path: &std::path::Path, value: &serde_json::Value) {
    let serialized = serde_json::to_vec_pretty(value).expect("must serialize json value");
    std::fs::write(path, serialized).expect("must write json file");
}

fn write_test_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    use std::io::Write as _;
    use zip::write::FileOptions;

    let file = std::fs::File::create(path).expect("must create zip file");
    let mut writer = zip::ZipWriter::new(file);
    for (name, bytes) in entries {
        writer
            .start_file(
                *name,
                FileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .expect("must start zip entry");
        writer
            .write_all(bytes)
            .expect("must write zip entry content");
    }
    writer.finish().expect("must finish zip archive");
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
