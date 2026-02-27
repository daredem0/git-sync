use super::support::*;
use super::*;
use std::path::PathBuf;

// Focus: receive workflow correctness, prerequisite handling, idempotency, and ref-application checks.
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
            dry_run: false,
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
            dry_run: false,
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
            dry_run: false,
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

// Verifies that dry-run receive reports imported heads but does not write refs or packfiles.
#[test]
fn receive_bundle_input_with_options_dry_run_does_not_modify_receiver_repo() {
    let (repo_dir, bundle_result, base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-dry-run", false);
    let receiver_dir = temp_repo_dir("receive-dry-run-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite");

    let pack_dir = receiver_dir.join("objects").join("pack");
    let mut pack_entries_before = match std::fs::read_dir(&pack_dir) {
        Ok(entries) => entries
            .map(|entry| {
                entry
                    .expect("pack dir entry should be readable")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => panic!("must list receiver pack dir entries before dry-run: {err}"),
    };
    pack_entries_before.sort();

    let receive_result = receive_bundle_input_with_options(
        &bundle_result.archive_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: false,
            dry_run: true,
        },
    )
    .expect("dry-run receive should succeed");
    assert!(
        receive_result.can_apply_without_conflicts,
        "dry-run should confirm package can be applied without conflicts"
    );
    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "fixture bundle should still report one imported head during dry-run"
    );
    assert_eq!(
        receive_result.imported_heads[0].oid, tip_commit_id,
        "dry-run should report the same head oid that would be imported"
    );
    assert!(
        receive_result
            .line_stats
            .iter()
            .any(|stat| stat.path == "f.txt" && stat.additions > 0 && stat.deletions > 0),
        "dry-run should include modified file line stats"
    );
    assert!(
        receive_result
            .line_stats
            .iter()
            .any(|stat| stat.path == "new.txt" && stat.additions > 0),
        "dry-run should include added file line stats"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let base_ref = receiver_repo
        .find_reference("refs/heads/base")
        .expect("base prerequisite ref should exist");
    assert_eq!(
        base_ref.target(),
        Some(base_commit_id),
        "base prerequisite ref should remain unchanged"
    );
    assert!(
        receiver_repo.find_reference("refs/heads/tip").is_err(),
        "dry-run must not create or update tip refs"
    );

    let mut pack_entries_after = match std::fs::read_dir(&pack_dir) {
        Ok(entries) => entries
            .map(|entry| {
                entry
                    .expect("pack dir entry should be readable")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => panic!("must list receiver pack dir entries after dry-run: {err}"),
    };
    pack_entries_after.sort();
    assert_eq!(
        pack_entries_after, pack_entries_before,
        "dry-run must not add new packfiles to receiver object database"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that dry-run receive still enforces metadata verification when requested.
#[test]
fn receive_bundle_input_with_options_dry_run_rejects_tampered_metadata_when_verification_enabled() {
    let repo_dir = temp_repo_dir("receive-dry-run-verify-metadata-fail");
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

    let receiver_dir = temp_repo_dir("receive-dry-run-verify-metadata-fail-receiver");
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
            dry_run: true,
        },
    );
    assert!(
        receive_result.is_err(),
        "dry-run receive with verify_metadata=true must reject tampered metadata"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    assert!(
        receiver_repo.find_reference("refs/heads/tip").is_err(),
        "failed dry-run must not create tip ref"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
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

// Verifies that commit-audit entries expose author/committer identity fields needed by the commit-detail TUI page.
#[test]
fn collect_commit_audit_entries_includes_author_and_committer_identity() {
    let (repo_dir, bundle_result, _base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-commit-audit-identities", false);

    let receiver_dir = temp_repo_dir("receive-commit-audit-identities-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let entries =
        collect_commit_audit_entries_for_bundle_input(&bundle_result.archive_path, &receiver_dir)
            .expect("must collect commit-audit entries from package");
    assert_eq!(
        entries.len(),
        1,
        "fixture range should produce one commit entry (base..tip)"
    );
    let entry = &entries[0];
    assert_eq!(
        entry.commit_id, tip_commit_id,
        "entry commit id should match the tip commit in this fixture range"
    );
    assert_eq!(
        entry.author.name, "Test User",
        "author identity should expose the commit author name"
    );
    assert_eq!(
        entry.author.email, "test@example.com",
        "author identity should expose the commit author email"
    );
    assert_eq!(
        entry.committer.name, "Test User",
        "committer identity should expose the commit committer name"
    );
    assert_eq!(
        entry.committer.email, "test@example.com",
        "committer identity should expose the commit committer email"
    );
    assert!(
        entry.author.time_seconds > 0 && entry.committer.time_seconds > 0,
        "identity timestamps should contain valid unix timestamps"
    );
    assert_eq!(
        entry.author.offset_minutes, entry.committer.offset_minutes,
        "fixture uses the same timezone offset for author and committer"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that a per-file patch can be collected for a changed file in a commit inside a bundle package.
#[test]
fn collect_commit_file_patch_for_bundle_input_returns_patch_for_changed_file() {
    let (repo_dir, bundle_result, _base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-commit-file-patch", false);

    let receiver_dir = temp_repo_dir("receive-commit-file-patch-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let patch = collect_commit_file_patch_for_bundle_input(
        &bundle_result.archive_path,
        &receiver_dir,
        tip_commit_id,
        "f.txt",
    )
    .expect("must collect patch for changed file");
    assert!(
        patch.contains("diff --git a/f.txt b/f.txt"),
        "patch should include diff header for the selected path"
    );
    assert!(
        patch.contains("--- a/f.txt") && patch.contains("+++ b/f.txt"),
        "patch should include file headers for the selected path"
    );
    assert!(
        patch.contains("@@"),
        "patch should include at least one hunk for modified file content"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that per-file patch collection fails when the requested path is not changed by the selected commit.
#[test]
fn collect_commit_file_patch_for_bundle_input_rejects_missing_path() {
    let (repo_dir, bundle_result, _base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-commit-file-patch-missing", false);

    let receiver_dir = temp_repo_dir("receive-commit-file-patch-missing-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let result = collect_commit_file_patch_for_bundle_input(
        &bundle_result.archive_path,
        &receiver_dir,
        tip_commit_id,
        "does-not-exist.txt",
    );
    assert!(
        result.is_err(),
        "collecting a patch for an unchanged path must fail"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}
