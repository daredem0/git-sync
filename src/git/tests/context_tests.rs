use super::*;
use std::path::PathBuf;

// Focus: validation behavior of open_context against repo/bundle/base/tip input combinations.
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
